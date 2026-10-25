// SPDX-License-Identifier: AGPL-3.0-only
//! A plain `z.number()`: any JavaScript number, written as JavaScript writes it (rule 4.25). See spec/check.md.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use serde_json::Value;

use crate::types::check::{leaf, Check, Zod};

/// A JSON number the TS reads as a JavaScript number. It serializes as `Number.prototype.toString` does: an integral
/// value with no `.0` (`3`, not `3.0`), `1e+21` from 10^21 up, `1e-7` from 10^-7 down, `-0` as `0`. serde_json's own
/// f64 writer differs on all three. It reads any JSON number; zod refuses NaN and the infinities, which JSON cannot
/// carry anyway.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Default)]
pub struct JsNumber(pub f64);

impl JsNumber {
    /// The number as JavaScript's `String(n)` writes it.
    pub fn js_string(&self) -> String {
        let n = self.0;
        if n == 0.0 {
            return "0".into();
        }
        let sci = format!("{:e}", n.abs());
        let (mantissa, exp) = sci.split_once('e').expect("{:e} writes an exponent");
        let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
        let k = digits.len() as i32;
        let point = exp.parse::<i32>().expect("an exponent") + 1;
        let body = if k <= point && point <= 21 {
            format!("{digits}{}", "0".repeat((point - k) as usize))
        } else if 0 < point && point <= 21 {
            format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
        } else if -6 < point && point <= 0 {
            format!("0.{}{digits}", "0".repeat((-point) as usize))
        } else {
            let e = point - 1;
            let sign = if e < 0 { "-" } else { "+" };
            match digits.len() {
                1 => format!("{digits}e{sign}{}", e.abs()),
                _ => format!("{}.{}e{sign}{}", &digits[..1], &digits[1..], e.abs()),
            }
        };
        if n < 0.0 {
            format!("-{body}")
        } else {
            body
        }
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
        if self.0.fract() == 0.0 && self.0.abs() < 9007199254740992.0 {
            s.serialize_i64(self.0 as i64)
        } else {
            s.serialize_f64(self.0)
        }
    }
}

impl<'de> Deserialize<'de> for JsNumber {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        f64::deserialize(d).map(JsNumber)
    }
}

impl Check for JsNumber {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Word("number"))
    }
}
