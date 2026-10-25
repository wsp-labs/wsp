// SPDX-License-Identifier: AGPL-3.0-only
//! JavaScript's text and number rules, the ones the TS host leans on and Rust computes differently: strings counted
//! in UTF-16 code units, JavaScript's whitespace set, `Math.round`, `toFixed`, `String(n)` and the fixed
//! `toLocaleString` forms and `localeCompare`. Spec chapter 08 (rules 8.1 to 8.14) names this module as their one home; see
//! spec/js.md. Every other unit of this crate calls these and never `str::len`, `str::trim` or `{:.n}`.

use std::cmp::Ordering;
use std::sync::OnceLock;

use icu_collator::CollatorBorrowed;

/// Text counted, cut and ordered in UTF-16 code units, as a JavaScript string is (rules 8.1 to 8.5, 8.8).
pub mod utf16 {
    use std::cmp::Ordering;

    fn units(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// Whole characters of these units; a lone surrogate half is dropped (rule 8.5).
    fn text(units: &[u16]) -> String {
        char::decode_utf16(units.iter().copied()).filter_map(Result::ok).collect()
    }

    /// A JavaScript index: negative counts from the end, and both ends clamp to `[0, len]`.
    fn index(i: i64, len: usize) -> usize {
        if i < 0 {
            (len as i64 + i).max(0) as usize
        } else {
            (i as usize).min(len)
        }
    }

    /// `s.length`: the UTF-16 code units of `s` (a char above U+FFFF counts 2).
    pub fn len(s: &str) -> usize {
        s.encode_utf16().count()
    }

    /// `s.slice(start, end)` with JavaScript's index rules: a negative index counts from the end, an index past the
    /// end is the end, and `start >= end` gives "". `end` None is the end. A cut that falls inside a surrogate pair
    /// drops that lone half (rule 8.5): the result holds only whole characters.
    pub fn slice(s: &str, start: i64, end: Option<i64>) -> String {
        let u = units(s);
        let from = index(start, u.len());
        let to = end.map_or(u.len(), |e| index(e, u.len()));
        if from >= to {
            return String::new();
        }
        text(&u[from..to])
    }

    /// The first `n` units of `s`: `s.slice(0, n)`, a split pair's lone half dropped (rule 8.5).
    pub fn cut(s: &str, n: usize) -> String {
        slice(s, 0, Some(n as i64))
    }

    /// The last `n` units of `s`: `s.slice(-n)` for n > 0, a split pair's lone half dropped; `n == 0` gives "" (where
    /// `s.slice(-0)` would be the whole string, so a caller porting `slice(-n)` with a possible 0 says which it means).
    pub fn tail(s: &str, n: usize) -> String {
        if n == 0 {
            return String::new();
        }
        slice(s, -(n as i64), None)
    }

    /// `s.indexOf(needle, from)` in units; None for -1. An empty needle is found at `min(from, len)`.
    pub fn index_of(s: &str, needle: &str, from: usize) -> Option<usize> {
        let (s, n) = (units(s), units(needle));
        let from = from.min(s.len());
        if n.is_empty() {
            return Some(from);
        }
        if n.len() > s.len() {
            return None;
        }
        (from..=s.len() - n.len()).find(|&i| s[i..i + n.len()] == n[..])
    }

    /// `s.lastIndexOf(needle)` in units; None for -1. An empty needle is found at `len(s)`.
    pub fn last_index_of(s: &str, needle: &str) -> Option<usize> {
        let (s, n) = (units(s), units(needle));
        if n.is_empty() {
            return Some(s.len());
        }
        if n.len() > s.len() {
            return None;
        }
        (0..=s.len() - n.len()).rev().find(|&i| s[i..i + n.len()] == n[..])
    }

    /// The fill that makes up `width` on `s`, repeated and cut to the missing units; None where `s` needs none or
    /// `fill` is empty.
    fn padding(s: &str, width: usize, fill: &str) -> Option<String> {
        let missing = width.saturating_sub(len(s));
        let fill = units(fill);
        if missing == 0 || fill.is_empty() {
            return None;
        }
        Some(text(&fill.iter().copied().cycle().take(missing).collect::<Vec<u16>>()))
    }

    /// `s.padStart(width, fill)` in units: `fill` repeated and cut to make up the width; `s` itself when already that
    /// long or when `fill` is empty.
    pub fn pad_start(s: &str, width: usize, fill: &str) -> String {
        match padding(s, width, fill) {
            Some(pad) => format!("{pad}{s}"),
            None => s.to_string(),
        }
    }

    /// `s.padEnd(width, fill)` in units, the same rules at the other end.
    pub fn pad_end(s: &str, width: usize, fill: &str) -> String {
        match padding(s, width, fill) {
            Some(pad) => format!("{s}{pad}"),
            None => s.to_string(),
        }
    }

    /// A bare `.sort()` on strings: by UTF-16 code units, which differs from byte order only between a char above
    /// U+FFFF and one from U+E000 to U+FFFF (rule 8.8).
    pub fn cmp(a: &str, b: &str) -> Ordering {
        a.encode_utf16().cmp(b.encode_utf16())
    }
}

/// `n.toString(16)` for a whole number: the exact hex digits of the double, `-` in front of a negative one, and no
/// padding. A value past 2^64 is its mantissa's digits and the zeros its exponent holds.
pub fn to_string_radix_16(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let sign = if n < 0.0 { "-" } else { "" };
    let magnitude = n.abs().trunc();
    let digits = if magnitude < 18_446_744_073_709_551_616.0 {
        format!("{:x}", magnitude as u64)
    } else {
        let bits = magnitude.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i64 - 1075;
        let mantissa = (bits & ((1 << 52) - 1)) | (1 << 52);
        format!("{:x}{}", mantissa << (exponent % 4), "0".repeat((exponent / 4) as usize))
    };
    format!("{sign}{digits}")
}

/// The twelve keys a plain JavaScript object holds without being given them, those of `Object.prototype`: a lookup in a
/// table written as an object literal finds a value for any of them where a Rust `match` finds none.
const OBJECT_PROTOTYPE_KEYS: [&str; 12] = [
    "constructor",
    "__defineGetter__",
    "__defineSetter__",
    "hasOwnProperty",
    "__lookupGetter__",
    "__lookupSetter__",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toString",
    "valueOf",
    "__proto__",
    "toLocaleString",
];

/// Whether `key` is one `Object.prototype` gives every plain object.
pub fn is_object_prototype_key(key: &str) -> bool {
    OBJECT_PROTOTYPE_KEYS.contains(&key)
}

/// What `String(table[key])` reads where `key` is one of `Object.prototype`'s and the table holds no row of its own: the
/// inherited function's source (`function Object() { [native code] }` for `constructor`, `function toString() ...` for
/// the rest) and `[object Object]` for `__proto__`. None for any other key.
pub fn inherited_text(key: &str) -> Option<String> {
    match key {
        "__proto__" => Some("[object Object]".to_string()),
        "constructor" => Some("function Object() { [native code] }".to_string()),
        _ if is_object_prototype_key(key) => Some(format!("function {key}() {{ [native code] }}")),
        _ => None,
    }
}

/// `Object.keys(object)` order: the keys that read as array indexes (`0`, `7`, `10`, never `01` or `-1`) first, in
/// ascending number, then every other key in the order it was added. A Rust map keeps insertion order alone.
pub fn js_key_order<'a, V>(entries: impl IntoIterator<Item = (&'a String, V)>) -> Vec<(&'a String, V)> {
    let mut indexes: Vec<(u32, (&'a String, V))> = Vec::new();
    let mut rest: Vec<(&'a String, V)> = Vec::new();
    for (key, value) in entries {
        match array_index(key) {
            Some(index) => indexes.push((index, (key, value))),
            None => rest.push((key, value)),
        }
    }
    indexes.sort_by_key(|(index, _)| *index);
    indexes.into_iter().map(|(_, entry)| entry).chain(rest).collect()
}

/// The array index a property key reads as: a canonical decimal under 2^32 - 1.
fn array_index(key: &str) -> Option<u32> {
    let canonical = key == "0" || (!key.starts_with('0') && !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit()));
    canonical.then(|| key.parse::<u32>().ok()).flatten().filter(|index| *index != u32::MAX)
}

/// The set `is_js_whitespace` names, as the inside of a regex class: what JavaScript's `\s` matches and Rust's does not.
pub const JS_WS: &str = r"\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}";

/// Whether `c` is whitespace to JavaScript's `trim()` and to `\s` in a regex: WhiteSpace plus LineTerminator of
/// ECMA-262, which is U+0009 to U+000D, U+0020, U+00A0, U+1680, U+2000 to U+200A, U+2028, U+2029, U+202F, U+205F,
/// U+3000 and U+FEFF. Not U+0085 and not U+180E, where Rust's `char::is_whitespace` says otherwise (rule 8.6).
pub fn is_js_whitespace(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

/// `` `${s.charAt(0).toUpperCase()}${s.slice(1)}` ``: the first UTF-16 unit upper-cased. A lone half of a surrogate pair
/// has no case, so a first character outside the BMP leaves `s` as it is.
pub fn upper_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) if first.len_utf16() == 1 => format!("{}{}", first.to_uppercase(), chars.as_str()),
        _ => s.to_string(),
    }
}

/// `s.replace(pattern, replacement)` with a string pattern: the first occurrence only, and the replacement's `$$`, `$&`,
/// `` $` `` and `$'` read as JavaScript reads them (a `$1` or `$<name>` has no group to name here and stays as written).
pub fn replace_first(s: &str, pattern: &str, replacement: &str) -> String {
    let Some(at) = s.find(pattern) else {
        return s.to_string();
    };
    let (before, after) = (&s[..at], &s[at + pattern.len()..]);
    let mut out = String::from(before);
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('$') => out.push('$'),
            Some('&') => out.push_str(pattern),
            Some('`') => out.push_str(before),
            Some('\'') => out.push_str(after),
            _ => {
                out.push('$');
                continue;
            }
        }
        chars.next();
    }
    out.push_str(after);
    out
}

/// `s.trim()` (rule 8.6).
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

/// `s.trimStart()` (rule 8.6).
pub fn js_trim_start(s: &str) -> &str {
    s.trim_start_matches(is_js_whitespace)
}

/// `s.trimEnd()` (rule 8.6).
pub fn js_trim_end(s: &str) -> &str {
    s.trim_end_matches(is_js_whitespace)
}

/// `s.replace(/\s+/g, " ")`: every run of JavaScript whitespace as one space, nothing trimmed.
pub fn js_collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_run = false;
    for c in s.chars() {
        if is_js_whitespace(c) {
            if !in_run {
                out.push(' ');
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }
    out
}

/// `Math.round(x)`: a half rounds toward positive infinity (`-2.5` is `-2`), where `f64::round` goes away from zero
/// (rule 8.12). NaN and the infinities come back as they went in; a result of negative zero is negative zero.
pub fn js_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    if x < 0.0 && x.fract() == -0.5 {
        return x.ceil();
    }
    x.round()
}

/// Adds one to the decimal digits in `digits` (ASCII bytes), carrying into a new leading digit where it overflows.
fn increment(digits: &mut Vec<u8>) {
    for d in digits.iter_mut().rev() {
        if *d == b'9' {
            *d = b'0';
        } else {
            *d += 1;
            return;
        }
    }
    digits.insert(0, b'1');
}

/// Keeps `places` fraction digits of `int` and `frac` (ASCII digits), rounding half away from zero on the digit after
/// them, and answers the integer and fraction digits kept. A fraction shorter than `places` keeps all of itself.
fn round_half_up(int: &str, frac: &str, places: usize) -> (String, String) {
    let kept = frac.len().min(places);
    let mut all: Vec<u8> = int.bytes().chain(frac.bytes().take(kept)).collect();
    if frac.as_bytes().get(places).is_some_and(|d| *d >= b'5') {
        increment(&mut all);
    }
    let int_len = all.len() - kept;
    let text = |b: &[u8]| String::from_utf8(b.to_vec()).expect("digits are ASCII");
    (text(&all[..int_len]), text(&all[int_len..]))
}

/// `x.toFixed(digits)` for digits 0 to 100: the nearer of the two candidates on the exact value of the double, the
/// larger in magnitude on an exact tie (`(2.5).toFixed(0)` is "3", `(-2.5).toFixed(0)` is "-3", `(0.125).toFixed(2)`
/// is "0.13", `(1.005).toFixed(2)` is "1.00" because the double is below 1.005), where Rust's `{:.n}` ties to even
/// (rule 8.13). `|x| >= 1e21` gives `String(x)`; NaN gives "NaN"; negative zero gives "0" (JavaScript drops the sign
/// of zero, and of a negative value that rounds to zero it keeps it: `(-0.0001).toFixed(2)` is "-0.00").
pub fn to_fixed(x: f64, digits: u32) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.abs() >= 1e21 {
        return number_string(x);
    }
    // Every double below 1e21 has at most 1074 fraction digits, so 1100 places print it exactly.
    let exact = format!("{:.1100}", x.abs());
    let (int, frac) = exact.split_once('.').expect("a fixed-point format has a point");
    let (int, frac) = round_half_up(int, frac, digits as usize);
    let sign = if x < 0.0 { "-" } else { "" };
    if digits == 0 {
        format!("{sign}{int}")
    } else {
        format!("{sign}{int}.{frac}")
    }
}

/// The shortest digits that read back as the positive finite `x`, and where the decimal point sits: `x` is `0.DIGITS`
/// times ten to the point. Where two candidates of that length are equally near `x`, the even last digit is the one
/// JavaScript writes, and Rust's `{:e}` takes the upper.
fn shortest(x: f64) -> (String, i32) {
    let sci = format!("{x:e}");
    let (mantissa, exp) = sci.split_once('e').expect("{:e} writes an exponent");
    let mut digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let mut point = exp.parse::<i32>().expect("an exponent") + 1;
    let k = digits.len();
    // Only a spelling of 16 or 17 digits has two candidates inside the span of numbers that read back as `x`.
    if k >= 16 {
        let exact: String =
            format!("{:.*e}", k + 30, x).split_once('e').map(|(m, _)| m.chars().filter(|c| *c != '.').collect()).unwrap_or_default();
        if exact.as_bytes().get(k) == Some(&b'5') && exact[k + 1..].bytes().all(|b| b == b'0') {
            let lower = &exact[..k];
            let (upper, carried) = increment_digits(lower);
            let take_lower = (lower.as_bytes()[k - 1] - b'0').is_multiple_of(2);
            let (candidate, candidate_point) = if take_lower { (lower.to_string(), point) } else { (upper, point + i32::from(carried)) };
            let candidate = candidate.trim_end_matches('0').to_string();
            let text = format!("0.{candidate}e{candidate_point}");
            if text.parse::<f64>() == Ok(x) {
                digits = candidate;
                point = candidate_point;
            }
        }
    }
    (digits, point)
}

/// The decimal digits one unit higher, and whether that added a digit in front.
fn increment_digits(digits: &str) -> (String, bool) {
    let mut bytes = digits.as_bytes().to_vec();
    for byte in bytes.iter_mut().rev() {
        if *byte == b'9' {
            *byte = b'0';
        } else {
            *byte += 1;
            return (String::from_utf8(bytes).expect("digits"), false);
        }
    }
    bytes.insert(0, b'1');
    (String::from_utf8(bytes).expect("digits"), true)
}

/// `String(x)`, a template literal's `${x}` and `JSON.stringify`'s number: the shortest digits that read back as
/// `x`, an integral value with no ".0", exponent form from 1e21 up and below 1e-6 (`1e+21`, `1e-7`), negative zero
/// as "0", "NaN", "Infinity", "-Infinity" (rule 8.11). Rule 4.25 names one such formatter in `wsp-wire`; this one
/// writes the same text and takes the even digit where two spellings are equally near.
pub fn number_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if x == 0.0 {
        return "0".into();
    }
    let (digits, point) = shortest(x.abs());
    let k = digits.len() as i32;
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
    if x < 0.0 {
        format!("-{body}")
    } else {
        body
    }
}

/// `Number(text)`: JavaScript's string to number, where the text is trimmed of JavaScript whitespace, "" is 0, a
/// decimal or exponent literal reads as itself, `Infinity` with a sign, a `0x`, `0o` or `0b` integer reads in its
/// base, and anything else is NaN.
pub fn to_number(text: &str) -> f64 {
    let t = js_trim(text);
    if t.is_empty() {
        return 0.0;
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = t.strip_prefix(prefix) {
            if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
                return f64::NAN;
            }
            return power_of_two_radix_value(digits, radix);
        }
    }
    if is_decimal_literal(t) {
        t.parse().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// The integer `digits` writes in a radix of 2, 8 or 16, rounded once to the nearest double (ties to even), as JavaScript
/// reads a `0x`, `0o` or `0b` literal: folding digit by digit in an `f64` would round at every step past 2^53.
fn power_of_two_radix_value(digits: &str, radix: u32) -> f64 {
    let width = radix.trailing_zeros() as usize;
    let mut bits: Vec<u8> = Vec::with_capacity(digits.len() * width);
    for c in digits.chars() {
        let d = c.to_digit(radix).expect("checked above");
        bits.extend((0..width).rev().map(|i| ((d >> i) & 1) as u8));
    }
    let start = bits.iter().position(|b| *b == 1).unwrap_or(bits.len());
    let bits = &bits[start..];
    if bits.len() <= 53 {
        return bits.iter().fold(0.0, |n, b| n * 2.0 + f64::from(*b));
    }
    let mut mantissa = bits[..53].iter().fold(0u64, |n, b| n * 2 + u64::from(*b));
    let round = bits[53] == 1;
    let sticky = bits[54..].contains(&1);
    if round && (sticky || mantissa % 2 == 1) {
        mantissa += 1;
    }
    mantissa as f64 * 2f64.powi((bits.len() - 53) as i32)
}

/// A signed decimal with an optional fraction and exponent, as JavaScript's `Number` reads one.
fn is_decimal_literal(t: &str) -> bool {
    let unsigned = t.strip_prefix(['+', '-']).unwrap_or(t);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(i) => (&unsigned[..i], Some(&unsigned[i + 1..])),
        None => (unsigned, None),
    };
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some((int, frac)) => (int.is_empty() || digits(int)) && (frac.is_empty() || digits(frac)) && !(int.is_empty() && frac.is_empty()),
        None => !mantissa.is_empty() && digits(mantissa),
    };
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    mantissa_ok && exponent_ok
}

/// `a.localeCompare(b)` as node 22's ICU answers it with the root locale and default options: case and accent are
/// tie-breakers below the letters, punctuation and spaces sort before digits, digits before letters, and "ß" after
/// "ss" (rule 8.8). Only the sign of the TS result is read, so this answers an Ordering.
pub fn collate(a: &str, b: &str) -> Ordering {
    static ROOT: OnceLock<CollatorBorrowed<'static>> = OnceLock::new();
    ROOT.get_or_init(|| wsp_wire::text::collator("und")).compare(a, b)
}

/// The digit grouping a fixed `toLocaleString` form uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grouping {
    /// `en-US`: commas every three digits (1,234,567).
    EnUs,
    /// `en-IN`: the last three digits, then commas every two (12,34,567), as India writes lakh and crore.
    EnIn,
}

fn group(int: &str, grouping: Grouping) -> String {
    let mut groups = Vec::new();
    let mut rest = int;
    let mut width = 3;
    while rest.len() > width {
        let (head, tail) = rest.split_at(rest.len() - width);
        groups.push(tail);
        rest = head;
        if grouping == Grouping::EnIn {
            width = 2;
        }
    }
    groups.push(rest);
    groups.reverse();
    groups.join(",")
}

/// The integer and fraction digits of `x`'s shortest decimal, the digits `String(x)` would print.
fn shortest_digits(x: f64) -> (String, String) {
    if x == 0.0 {
        return ("0".into(), String::new());
    }
    let (digits, point) = shortest(x);
    if point <= 0 {
        ("0".into(), format!("{}{digits}", "0".repeat((-point) as usize)))
    } else if point as usize >= digits.len() {
        (format!("{digits}{}", "0".repeat(point as usize - digits.len())), String::new())
    } else {
        (digits[..point as usize].to_string(), digits[point as usize..].to_string())
    }
}

/// `x.toLocaleString(locale, { minimumFractionDigits: min, maximumFractionDigits: max })` for the two locales the
/// host uses (rule 8.14), without a currency sign. ICU rounds the shortest decimal that reads back as `x`, half away
/// from zero, so `1.005` with 2 digits is "1.01" where `toFixed` gives "1.00", and `999.995` is "1,000.00". Trailing
/// zeros past `min` are dropped. A minus sign leads ("-1,234"). `toLocaleString("en-US")` with no options is
/// `min` 0, `max` 3.
pub fn locale_number(x: f64, grouping: Grouping, min_fraction: u32, max_fraction: u32) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    let sign = if x.is_sign_negative() { "-" } else { "" };
    if x.is_infinite() {
        return format!("{sign}∞");
    }
    let (int, frac) = shortest_digits(x.abs());
    let (int, mut frac) = round_half_up(&int, &frac, max_fraction as usize);
    let min = min_fraction as usize;
    while frac.len() > min && frac.ends_with('0') {
        frac.pop();
    }
    while frac.len() < min {
        frac.push('0');
    }
    let int = group(&int, grouping);
    if frac.is_empty() {
        format!("{sign}{int}")
    } else {
        format!("{sign}{int}.{frac}")
    }
}

/// A serde `serialize_with` for an `f64` field written to JSON as JavaScript writes it (rule 4.25): a finite value
/// that is integral and within 2^53 is written as an integer (`1`, not `1.0`; -0 as `0`), so `serde_json::Value`
/// equality and the text both match the TS host's; every other value as a float.
pub fn serialize_f64<S: serde::Serializer>(x: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    use serde::Serialize;
    if x.is_finite() && x.fract() == 0.0 && x.abs() <= 9007199254740992.0 {
        serializer.serialize_i64(*x as i64)
    } else if x.is_finite() {
        // The text itself, since serde_json writes 3e-6 and 1e21 where JavaScript writes 0.000003 and 1e+21.
        serde_json::value::RawValue::from_string(number_string(*x)).map_err(serde::ser::Error::custom)?.serialize(serializer)
    } else {
        serializer.serialize_f64(*x)
    }
}

/// `serialize_f64` for an `Option<f64>` field: None is written as null (a field the TS leaves undefined also carries
/// `skip_serializing_if`).
pub fn serialize_opt_f64<S: serde::Serializer>(x: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error> {
    match x {
        Some(v) => serialize_f64(v, serializer),
        None => serializer.serialize_none(),
    }
}
