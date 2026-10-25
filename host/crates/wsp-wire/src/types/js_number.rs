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
        let digits = even_on_a_tie(n.abs(), mantissa.chars().filter(|c| *c != '.').collect(), exp);
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

/// Where two digit strings of the shortest length sit exactly as far from the double, JavaScript takes the one whose last
/// digit is even and Rust's `{:e}` the upper one.
fn even_on_a_tie(n: f64, shortest: String, exp: &str) -> String {
    let k = shortest.len();
    let exact = format!("{:.*e}", k + 40, n);
    let exact_digits: String = exact.split_once('e').map_or("", |(m, _)| m).chars().filter(|c| *c != '.').collect();
    let is_tie = exact_digits.as_bytes().get(k) == Some(&b'5') && exact_digits[k + 1..].bytes().all(|b| b == b'0');
    let last_even = |d: &str| d.bytes().last().is_some_and(|b| (b - b'0').is_multiple_of(2));
    if !is_tie {
        return shortest;
    }
    let lower = exact_digits[..k].to_string();
    let candidate = if last_even(&lower) {
        lower
    } else {
        let mut up: Vec<u8> = lower.into_bytes();
        let mut i = up.len();
        while i > 0 && up[i - 1] == b'9' {
            up[i - 1] = b'0';
            i -= 1;
        }
        if i == 0 {
            return shortest;
        }
        up[i - 1] += 1;
        String::from_utf8(up).unwrap_or(shortest.clone())
    };
    let back = format!("{}e{}", candidate, exp.parse::<i32>().unwrap_or(0) - (k as i32 - 1));
    if back.parse::<f64>().is_ok_and(|v| v == n) {
        candidate
    } else {
        shortest
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
    /// A whole number below 2^53 is an integer; any other is the text `String(n)` gives, held as a raw value so that
    /// serde_json writes it as it is and `to_value` reads it back as the number it names.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if !self.0.is_finite() {
            return s.serialize_unit();
        }
        if self.0.fract() == 0.0 && self.0.abs() < 9007199254740992.0 {
            return s.serialize_i64(self.0 as i64);
        }
        match serde_json::value::RawValue::from_string(self.js_string()) {
            Ok(text) => text.serialize(s),
            Err(_) => s.serialize_f64(self.0),
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

/// A `z.number().int()`: a JavaScript number that zod's integer check takes (`Number.isInteger`), so `5.0`, `-0`,
/// `1e21` and any integral value past 2^63 are integers, and `2.5` is not. It is held as the JsNumber it reads as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Default, Serialize)]
#[serde(transparent)]
pub struct JsInt(pub JsNumber);

impl<'de> Deserialize<'de> for JsInt {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let n = f64::deserialize(d)?;
        if n.is_finite() && n.fract() == 0.0 {
            Ok(JsInt(JsNumber(n)))
        } else {
            Err(D::Error::invalid_value(serde::de::Unexpected::Float(n), &"integer"))
        }
    }
}

impl JsInt {
    pub fn as_f64(self) -> f64 {
        self.0 .0
    }

    /// The integer as JavaScript holds it, a float; the integer's exact value where it fits an i64.
    pub fn get(self) -> i64 {
        self.0 .0 as i64
    }
}

impl From<i64> for JsInt {
    fn from(n: i64) -> Self {
        JsInt(JsNumber(n as f64))
    }
}

impl Check for JsInt {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Int)
    }

    fn dirty(value: &Value) -> Option<Self> {
        let n = value.as_f64().filter(|n| n.is_finite() && n.fract() != 0.0)?;
        Some(JsInt(JsNumber(n)))
    }
}

// zod never reads a NaN from JSON, so a JsNumber is equal to itself.
impl Eq for JsNumber {}

impl std::hash::Hash for JsNumber {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state)
    }
}

/// A number in a sentence as JavaScript writes it (`String(n)`).
impl std::fmt::Display for JsNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.js_string())
    }
}

impl std::fmt::Display for JsInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
