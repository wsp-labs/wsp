// SPDX-License-Identifier: AGPL-3.0-only
//! JavaScript's text and number rules, the ones the TS host leans on and Rust computes differently: strings counted
//! in UTF-16 code units, JavaScript's whitespace set, `Math.round`, `toFixed`, `String(n)` and the fixed
//! `toLocaleString` forms and `localeCompare`. Spec chapter 08 (rules 8.1 to 8.14) names this module as their one home; see
//! spec/js.md. Every other unit of this crate calls these and never `str::len`, `str::trim` or `{:.n}`.

/// Text counted, cut and ordered in UTF-16 code units, as a JavaScript string is (rules 8.1 to 8.5, 8.8).
pub mod utf16 {
    use std::cmp::Ordering;

    /// `s.length`: the UTF-16 code units of `s` (a char above U+FFFF counts 2).
    pub fn len(s: &str) -> usize {
        todo!()
    }

    /// `s.slice(start, end)` with JavaScript's index rules: a negative index counts from the end, an index past the
    /// end is the end, and `start >= end` gives "". `end` None is the end. A cut that falls inside a surrogate pair
    /// drops that lone half (rule 8.5): the result holds only whole characters.
    pub fn slice(s: &str, start: i64, end: Option<i64>) -> String {
        todo!()
    }

    /// The first `n` units of `s`: `s.slice(0, n)`, a split pair's lone half dropped (rule 8.5).
    pub fn cut(s: &str, n: usize) -> String {
        todo!()
    }

    /// The last `n` units of `s`: `s.slice(-n)` for n > 0, a split pair's lone half dropped; `n == 0` gives "" (where
    /// `s.slice(-0)` would be the whole string, so a caller porting `slice(-n)` with a possible 0 says which it means).
    pub fn tail(s: &str, n: usize) -> String {
        todo!()
    }

    /// `s.indexOf(needle, from)` in units; None for -1. An empty needle is found at `min(from, len)`.
    pub fn index_of(s: &str, needle: &str, from: usize) -> Option<usize> {
        todo!()
    }

    /// `s.lastIndexOf(needle)` in units; None for -1. An empty needle is found at `len(s)`.
    pub fn last_index_of(s: &str, needle: &str) -> Option<usize> {
        todo!()
    }

    /// `s.padStart(width, fill)` in units: `fill` repeated and cut to make up the width; `s` itself when already that
    /// long or when `fill` is empty.
    pub fn pad_start(s: &str, width: usize, fill: &str) -> String {
        todo!()
    }

    /// `s.padEnd(width, fill)` in units, the same rules at the other end.
    pub fn pad_end(s: &str, width: usize, fill: &str) -> String {
        todo!()
    }

    /// A bare `.sort()` on strings: by UTF-16 code units, which differs from byte order only between a char above
    /// U+FFFF and one from U+E000 to U+FFFF (rule 8.8).
    pub fn cmp(a: &str, b: &str) -> Ordering {
        todo!()
    }
}

/// Whether `c` is whitespace to JavaScript's `trim()` and to `\s` in a regex: WhiteSpace plus LineTerminator of
/// ECMA-262, which is U+0009 to U+000D, U+0020, U+00A0, U+1680, U+2000 to U+200A, U+2028, U+2029, U+202F, U+205F,
/// U+3000 and U+FEFF. Not U+0085 and not U+180E, where Rust's `char::is_whitespace` says otherwise (rule 8.6).
pub fn is_js_whitespace(c: char) -> bool {
    todo!()
}

/// `s.trim()` (rule 8.6).
pub fn js_trim(s: &str) -> &str {
    todo!()
}

/// `s.trimStart()` (rule 8.6).
pub fn js_trim_start(s: &str) -> &str {
    todo!()
}

/// `s.trimEnd()` (rule 8.6).
pub fn js_trim_end(s: &str) -> &str {
    todo!()
}

/// `s.replace(/\s+/g, " ")`: every run of JavaScript whitespace as one space, nothing trimmed.
pub fn js_collapse_whitespace(s: &str) -> String {
    todo!()
}

/// `Math.round(x)`: a half rounds toward positive infinity (`-2.5` is `-2`), where `f64::round` goes away from zero
/// (rule 8.12). NaN and the infinities come back as they went in; a result of negative zero is negative zero.
pub fn js_round(x: f64) -> f64 {
    todo!()
}

/// `x.toFixed(digits)` for digits 0 to 100: the nearer of the two candidates on the exact value of the double, the
/// larger in magnitude on an exact tie (`(2.5).toFixed(0)` is "3", `(-2.5).toFixed(0)` is "-3", `(0.125).toFixed(2)`
/// is "0.13", `(1.005).toFixed(2)` is "1.00" because the double is below 1.005), where Rust's `{:.n}` ties to even
/// (rule 8.13). `|x| >= 1e21` gives `String(x)`; NaN gives "NaN"; negative zero gives "0" (JavaScript drops the sign
/// of zero, and of a negative value that rounds to zero it keeps it: `(-0.0001).toFixed(2)` is "-0.00").
pub fn to_fixed(x: f64, digits: u32) -> String {
    todo!()
}

/// `String(x)`, a template literal's `${x}` and `JSON.stringify`'s number: the shortest digits that read back as
/// `x`, an integral value with no ".0", exponent form from 1e21 up and below 1e-6 (`1e+21`, `1e-7`), negative zero
/// as "0", "NaN", "Infinity", "-Infinity" (rule 8.11). Rule 4.25 names one such formatter in `wsp-wire`; until that
/// crate's lands this is it, and at the merge one of the two re-exports the other.
pub fn number_string(x: f64) -> String {
    todo!()
}

/// `a.localeCompare(b)` as node 22's ICU answers it with the root locale and default options: case and accent are
/// tie-breakers below the letters, punctuation and spaces sort before digits, digits before letters, and "ß" after
/// "ss" (rule 8.8). Only the sign of the TS result is read, so this answers an Ordering.
pub fn collate(a: &str, b: &str) -> std::cmp::Ordering {
    todo!()
}

/// The digit grouping a fixed `toLocaleString` form uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grouping {
    /// `en-US`: commas every three digits (1,234,567).
    EnUs,
    /// `en-IN`: the last three digits, then commas every two (12,34,567), as India writes lakh and crore.
    EnIn,
}

/// `x.toLocaleString(locale, { minimumFractionDigits: min, maximumFractionDigits: max })` for the two locales the
/// host uses (rule 8.14), without a currency sign. ICU rounds the shortest decimal that reads back as `x`, half away
/// from zero, so `1.005` with 2 digits is "1.01" where `toFixed` gives "1.00", and `999.995` is "1,000.00". Trailing
/// zeros past `min` are dropped. A minus sign leads ("-1,234"). `toLocaleString("en-US")` with no options is
/// `min` 0, `max` 3.
pub fn locale_number(x: f64, grouping: Grouping, min_fraction: u32, max_fraction: u32) -> String {
    todo!()
}

/// A serde `serialize_with` for an `f64` field written to JSON as JavaScript writes it (rule 4.25): a finite value
/// that is integral and within 2^53 is written as an integer (`1`, not `1.0`; -0 as `0`), so `serde_json::Value`
/// equality and the text both match the TS host's; every other value as a float.
pub fn serialize_f64<S: serde::Serializer>(x: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    todo!()
}

/// `serialize_f64` for an `Option<f64>` field: None is written as null (a field the TS leaves undefined also carries
/// `skip_serializing_if`).
pub fn serialize_opt_f64<S: serde::Serializer>(x: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error> {
    todo!()
}
