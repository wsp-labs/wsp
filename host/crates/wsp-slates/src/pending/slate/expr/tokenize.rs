// SPDX-License-Identifier: AGPL-3.0-only
//! The tokenizer of expr.ts.
// ---- the tokenizer ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Num,
    Str,
    Tpl,
    Own,
    Name,
    Op,
    End,
}

#[derive(Clone, Debug)]
pub(super) enum RawPart {
    Text(String),
    Hole { src: String, at: usize },
}

#[derive(Clone, Debug)]
pub(super) struct Token {
    pub(super) kind: Kind,
    pub(super) v: String,
    pub(super) parts: Vec<RawPart>,
    pub(super) at: usize,
}

#[derive(Debug)]
pub(super) struct Fault {
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) at: usize,
    pub(super) fix: Option<String>,
    pub(super) absolute: bool,
}

pub(super) type PResult<T> = Result<T, Fault>;

pub(super) fn fault(code: &'static str, message: String, at: usize, fix: Option<String>) -> Fault {
    Fault { code, message, at, fix, absolute: false }
}

/// JavaScript method names a model writes by reflex, with this language's spelling.
pub(super) fn method_fix(name: &str) -> Option<&'static str> {
    Some(match name {
        "length" => "len(x)",
        "map" => "x | map(name: item.field) or pluck(x, 'field')",
        "filter" => "x | where(item.cond)",
        "toUpperCase" => "upper(x)",
        "toLowerCase" => "lower(x)",
        "includes" => "contains(x, v)",
        "join" => "join(x, sep)",
        "slice" => "x | take(n)",
        "find" => "x | where(...) | first",
        "some" => "len(x | where(...)) > 0",
        "trim" => "trim(x)",
        "split" => "split(x, sep)",
        "replace" => "replace(x, a, b)",
        "startsWith" => "startsWith(x, p)",
        "endsWith" => "endsWith(x, p)",
        "reduce" => "x | sum(item.field)",
        "sort" => "x | sortBy(item.field)",
        "toString" => "str(x)",
        "toFixed" => "number(x, places)",
        "test" => "contains, startsWith or endsWith; there are no regular expressions",
        "forEach" => "a piece that takes items",
        _ => return None,
    })
}

fn escape_of(c: u16) -> Option<&'static str> {
    Some(match c as u8 {
        b'n' if c < 128 => "\n",
        b't' if c < 128 => "\t",
        b'r' if c < 128 => "\r",
        b'b' if c < 128 => "\u{8}",
        b'f' if c < 128 => "\u{c}",
        _ => return None,
    })
}

pub(super) fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

pub(super) fn text(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

fn at_is(src: &[u16], i: usize, c: u8) -> bool {
    src.get(i) == Some(&u16::from(c))
}

pub(super) fn starts(src: &[u16], i: usize, pat: &str) -> bool {
    pat.bytes().enumerate().all(|(k, b)| src.get(i + k) == Some(&u16::from(b)))
}

fn is_digit(src: &[u16], i: usize) -> bool {
    src.get(i).is_some_and(|c| (u16::from(b'0')..=u16::from(b'9')).contains(c))
}

fn is_name_start(c: u16) -> bool {
    c < 128 && ((c as u8).is_ascii_alphabetic() || c == u16::from(b'_'))
}

fn is_name_char(c: u16) -> bool {
    c < 128 && ((c as u8).is_ascii_alphanumeric() || c == u16::from(b'_'))
}

pub(super) fn tokenize(src: &[u16]) -> PResult<Vec<Token>> {
    let mut out = Vec::new();
    let n = src.len();
    let mut i = 0;
    while i < n {
        let c = src[i];
        if matches!(c, 0x20 | 0x09 | 0x0a | 0x0d) {
            i += 1;
            continue;
        }
        let at = i;
        if is_digit(src, i) {
            let mut j = i;
            while is_digit(src, j) {
                j += 1;
            }
            if at_is(src, j, b'.') && is_digit(src, j + 1) {
                j += 1;
                while is_digit(src, j) {
                    j += 1;
                }
            }
            if at_is(src, j, b'e') || at_is(src, j, b'E') {
                let mut k = j + 1;
                if at_is(src, k, b'-') {
                    k += 1;
                }
                if is_digit(src, k) {
                    while is_digit(src, k) {
                        k += 1;
                    }
                    j = k;
                }
            }
            out.push(Token { kind: Kind::Num, v: text(&src[i..j]), parts: vec![], at });
            i = j;
            continue;
        }
        if c == u16::from(b'\'') || c == u16::from(b'"') {
            let mut s: Vec<u16> = Vec::new();
            i += 1;
            while i < n && src[i] != c {
                if src[i] == u16::from(b'\\') && i + 1 < n {
                    let nc = src[i + 1];
                    if nc == u16::from(b'u') {
                        let hex: Option<u16> = (i + 2..(i + 6).min(n))
                            .map(|k| char::from_u32(u32::from(src[k])).and_then(|ch| ch.to_digit(16)))
                            .collect::<Option<Vec<u32>>>()
                            .filter(|digits| digits.len() == 4)
                            .map(|digits| digits.iter().fold(0u32, |acc, d| acc * 16 + d) as u16);
                        if let Some(unit) = hex {
                            s.push(unit);
                            i += 6;
                            continue;
                        }
                    }
                    match escape_of(nc) {
                        Some(rep) => s.extend(rep.encode_utf16()),
                        None => s.push(nc),
                    }
                    i += 2;
                    continue;
                }
                s.push(src[i]);
                i += 1;
            }
            if i >= n {
                return Err(fault("X400", format!("a string opened at column {} never closes", at + 1), at, None));
            }
            i += 1;
            out.push(Token { kind: Kind::Str, v: text(&s), parts: vec![], at });
            continue;
        }
        if c == u16::from(b'`') {
            let mut parts: Vec<RawPart> = Vec::new();
            let mut lit: Vec<u16> = Vec::new();
            i += 1;
            while i < n && src[i] != u16::from(b'`') {
                if starts(src, i, "$${") {
                    lit.extend("${".encode_utf16());
                    i += 3;
                    continue;
                }
                if starts(src, i, "\\`") {
                    lit.push(u16::from(b'`'));
                    i += 2;
                    continue;
                }
                if starts(src, i, "${") {
                    let start = i + 2;
                    let Some(end) = closing_brace_units(src, start) else {
                        return Err(fault("X400", format!("a ${{ at column {} never closes", i + 1), i, None));
                    };
                    if !lit.is_empty() {
                        parts.push(RawPart::Text(text(&lit)));
                        lit.clear();
                    }
                    parts.push(RawPart::Hole { src: text(&src[start..end]), at: start });
                    i = end + 1;
                    continue;
                }
                lit.push(src[i]);
                i += 1;
            }
            if i >= n {
                return Err(fault("X400", format!("a template opened at column {} never closes", at + 1), at, None));
            }
            if !lit.is_empty() {
                parts.push(RawPart::Text(text(&lit)));
            }
            i += 1;
            out.push(Token { kind: Kind::Tpl, v: "`".into(), parts, at });
            continue;
        }
        if c == u16::from(b'$') {
            if !src.get(i + 1).is_some_and(|&d| is_name_start(d)) {
                return Err(fault("X400", format!("$ at column {} must be followed by a name", at + 1), at, None));
            }
            let mut j = i + 1;
            while j < n && is_name_char(src[j]) {
                j += 1;
            }
            out.push(Token { kind: Kind::Own, v: text(&src[i + 1..j]), parts: vec![], at });
            i = j;
            continue;
        }
        if is_name_start(c) {
            let mut j = i;
            while j < n && is_name_char(src[j]) {
                j += 1;
            }
            out.push(Token { kind: Kind::Name, v: text(&src[i..j]), parts: vec![], at });
            i = j;
            continue;
        }
        if starts(src, i, "...") {
            return Err(fault("X420", "no spreads here; a list is reshaped with steps".into(), at, None));
        }
        if starts(src, i, "===") || starts(src, i, "!==") {
            out.push(Token { kind: Kind::Op, v: text(&src[i..i + 2]), parts: vec![], at });
            i += 3;
            continue;
        }
        if starts(src, i, "=>") {
            return Err(fault(
                "X420",
                "no functions here; a list is reshaped with steps".into(),
                at,
                Some("items | map(name: item.name) or pluck(items, 'name')".into()),
            ));
        }
        if starts(src, i, "??") {
            return Err(fault("X420", "?? is not an operator here".into(), at, Some("orElse(a, b)".into())));
        }
        if starts(src, i, "?.") {
            return Err(fault("X420", "?. is not needed: a missing field reads as null".into(), at, Some("a.b".into())));
        }
        let two: Option<&str> = ["==", "!=", "<=", ">="].into_iter().chain(["&&", "||"]).find(|p| starts(src, i, p));
        if let Some(p) = two {
            let v = match p {
                "&&" => "and",
                "||" => "or",
                other => other,
            };
            out.push(Token { kind: Kind::Op, v: v.into(), parts: vec![], at });
            i += 2;
            continue;
        }
        if c == u16::from(b'!') {
            out.push(Token { kind: Kind::Op, v: "not".into(), parts: vec![], at });
            i += 1;
            continue;
        }
        if c < 128 && b"()[],.?:+-*/%<>|{}".contains(&(c as u8)) {
            out.push(Token { kind: Kind::Op, v: text(&[c]), parts: vec![], at });
            i += 1;
            continue;
        }
        if c == u16::from(b'=') {
            return Err(fault("X420", "compare with ==; to write a value use set($x, 1) in a handler".into(), at, Some("==".into())));
        }
        return Err(fault("X400", format!("\"{}\" is not part of an expression (column {})", text(&[c]), at + 1), at, None));
    }
    out.push(Token { kind: Kind::End, v: String::new(), parts: vec![], at: src.len() });
    Ok(out)
}

/// The index of the } closing a ${ whose body starts at start, quotes and nested braces skipped; None when none.
pub(super) fn closing_brace_units(src: &[u16], start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut quote: Option<u16> = None;
    let mut j = start;
    while j < src.len() {
        let ch = src[j];
        if let Some(q) = quote {
            if ch == u16::from(b'\\') {
                j += 1;
            } else if ch == q {
                quote = None;
            }
        } else if ch == u16::from(b'\'') || ch == u16::from(b'"') || ch == u16::from(b'`') {
            quote = Some(ch);
        } else if ch == u16::from(b'{') {
            depth += 1;
        } else if ch == u16::from(b'}') {
            depth -= 1;
            if depth == 0 {
                return Some(j);
            }
        }
        j += 1;
    }
    None
}

/// The index of the } closing a ${ whose body starts at start (a UTF-16 offset), or None when none closes.
pub fn closing_brace(src: &str, start: usize) -> Option<usize> {
    closing_brace_units(&units(src), start)
}
