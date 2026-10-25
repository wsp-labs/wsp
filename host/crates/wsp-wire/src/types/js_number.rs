// SPDX-License-Identifier: AGPL-3.0-only
//! A plain `z.number()`: any JavaScript number, written as JavaScript writes it (rule 4.25). See spec/check.md.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::check::Check;

/// A JSON number the TS reads as a JavaScript number. It serializes as `Number.prototype.toString` does: an integral
/// value with no `.0` (`3`, not `3.0`), `1e+21` from 10^21 up, `1e-7` from 10^-7 down, `-0` as `0`. serde_json's own
/// f64 writer differs on all three. It reads any JSON number; zod refuses NaN and the infinities, which JSON cannot
/// carry anyway.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Default)]
pub struct JsNumber(pub f64);

impl JsNumber {
    /// The number as JavaScript's `String(n)` writes it.
    pub fn js_string(&self) -> String {
        todo!()
    }
}

impl From<f64> for JsNumber {
    fn from(n: f64) -> Self {
        JsNumber(n)
    }
}

impl From<i64> for JsNumber {
    fn from(n: i64) -> Self {
        JsNumber(n as f64)
    }
}

impl Serialize for JsNumber {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        todo!()
    }
}

impl<'de> Deserialize<'de> for JsNumber {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        todo!()
    }
}

impl Check for JsNumber {}
