// SPDX-License-Identifier: AGPL-3.0-only
//! JSON as `JSON.parse` takes it and `JSON.stringify` writes it, for the one thing serde_json refuses: a surrogate
//! escape with no partner, which a JavaScript string holds and a Rust one cannot. Each such half is held as one
//! character of a private-use range while a frame is a `Value`, so two frames compare unit for unit, and written
//! back as the same escape on the wire, on disk and in a printed difference.

use serde_json::Value;

/// The character that stands for the lone surrogate U+D800, the next for U+D801, up to U+10FFFF for U+DFFF.
const STAND_IN: u32 = 0x10F800;
const SURROGATES: std::ops::Range<u32> = 0xD800..0xE000;

fn stand_in(unit: u32) -> char {
    char::from_u32(STAND_IN + unit - SURROGATES.start).expect("the range is inside Unicode")
}

fn unit_of(c: char) -> Option<u32> {
    (c as u32 >= STAND_IN).then(|| c as u32 - STAND_IN + SURROGATES.start)
}

fn refusal(c: char) -> String {
    format!("U+{:X} stands in for a lone surrogate here, so a line holding it, or a pair that makes it, cannot be read", c as u32)
}

/// The unit `\uXXXX` writes at byte `at`, if that is what is there.
fn escape_at(text: &str, at: usize) -> Option<u32> {
    let hex = text.get(at..at + 6)?.strip_prefix("\\u")?;
    hex.bytes().all(|b| b.is_ascii_hexdigit()).then(|| u32::from_str_radix(hex, 16).ok()).flatten()
}

fn holding_halves(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(c) = text[i..].chars().next() {
        if unit_of(c).is_some() {
            return Err(refusal(c));
        }
        if c != '\\' {
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        match escape_at(text, i) {
            Some(high) if (0xD800..0xDC00).contains(&high) => match escape_at(text, i + 6).filter(|low| (0xDC00..0xE000).contains(low)) {
                Some(low) => {
                    let pair = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                    if let Some(over) = char::from_u32(pair).filter(|c| unit_of(*c).is_some()) {
                        return Err(refusal(over));
                    }
                    out.push_str(&text[i..i + 12]);
                    i += 12;
                }
                None => {
                    out.push(stand_in(high));
                    i += 6;
                }
            },
            Some(low) if SURROGATES.contains(&low) => {
                out.push(stand_in(low));
                i += 6;
            }
            // Any other escape: the backslash and the character after it go through whole, so `\\ud83d` stays text.
            _ => {
                out.push('\\');
                i += 1;
                if let Some(next) = text[i..].chars().next() {
                    if unit_of(next).is_some() {
                        return Err(refusal(next));
                    }
                    out.push(next);
                    i += next.len_utf8();
                }
            }
        }
    }
    Ok(out)
}

fn escaping_halves(text: String) -> String {
    if !text.chars().any(|c| unit_of(c).is_some()) {
        return text;
    }
    text.chars().fold(String::with_capacity(text.len()), |mut out, c| {
        match unit_of(c) {
            Some(unit) => out.push_str(&format!("\\u{unit:04x}")),
            None => out.push(c),
        }
        out
    })
}

/// A string a client sends as it is: a text frame or a command-line word cannot carry a lone half, and a real client
/// sends U+FFFD for each, so a stand-in never reaches a host as its own character.
pub fn as_client_sends(text: &str) -> String {
    text.chars().map(|c| if unit_of(c).is_some() { '\u{FFFD}' } else { c }).collect()
}

/// One JSON text as `JSON.parse` reads it, lone surrogates in a key or a value kept.
pub fn read(text: &str) -> Result<Value, String> {
    serde_json::from_str(&holding_halves(text)?).map_err(|e| e.to_string())
}

/// A value as `JSON.stringify` writes it, compact, each lone surrogate as its escape.
pub fn write(value: &Value) -> String {
    escaping_halves(value.to_string())
}

/// A value on several lines, as a golden file keeps it.
pub fn write_pretty(value: &Value) -> String {
    escaping_halves(format!("{value:#}"))
}
