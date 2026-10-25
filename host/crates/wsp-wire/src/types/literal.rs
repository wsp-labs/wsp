// SPDX-License-Identifier: AGPL-3.0-only
//! A `z.literal("...")` key a struct keeps: a unit type that writes its one value and refuses any other.
//! `z.literal(true)` and `z.literal(false)` are wsp-frames' `True` and `False`. See spec/check.md.

use serde::{Deserializer, Serializer};

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

        impl $crate::types::check::Check for $name {}
    };
}

/// A literal's value as the macro hands it over: a string or a number.
pub trait LiteralValue: Copy + std::fmt::Debug {}
impl LiteralValue for &'static str {}
impl LiteralValue for i64 {}
impl LiteralValue for f64 {}

/// Writes the literal as JSON writes it: a string as a string, a number as JavaScript writes it.
pub fn write_literal<S: Serializer, V: LiteralValue>(s: S, value: V) -> Result<S::Ok, S::Error> {
    todo!()
}

/// Reads one value and refuses any other, with zod's `invalid_literal` words: `Invalid literal value, expected "x"`.
pub fn read_literal<'de, D: Deserializer<'de>, V: LiteralValue>(d: D, value: V) -> Result<(), D::Error> {
    todo!()
}
