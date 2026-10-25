// SPDX-License-Identifier: AGPL-3.0-only
//! A JavaScript regular expression kept as data on a catalog row (a token's shape, a sign-in's question, a code's
//! shape): its source as the TS writes it, matched with JavaScript's meaning. Lookbehind is supported, so the
//! implementation sits on fancy-regex (rule 8.7). The spec is spec/jsonc.md.
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fancy_regex::Regex;
use serde::{Serialize, Serializer};

const JS_SPACE: &str = r"\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}";
const WORD: &str = "A-Za-z0-9_";

/// The source between the slashes and the flags after them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsRegex {
    pub source: &'static str,
    pub flags: &'static str,
}

impl JsRegex {
    pub const fn new(source: &'static str) -> JsRegex {
        JsRegex { source, flags: "" }
    }

    pub const fn with_flags(source: &'static str, flags: &'static str) -> JsRegex {
        JsRegex { source, flags }
    }

    /// `re.test(text)`.
    pub fn is_match(&self, text: &str) -> bool {
        compiled(self.source, self.flags).is_some_and(|re| re.is_match(text).unwrap_or(false))
    }

    /// `re.exec(text)?.[0]`.
    pub fn find(&self, text: &str) -> Option<String> {
        let re = compiled(self.source, self.flags)?;
        re.find(text).ok().flatten().map(|m| m.as_str().to_string())
    }

    /// `new RegExp(`^(?:${source})$`).test(text)`: the whole text is one match.
    pub fn is_whole_match(&self, text: &str) -> bool {
        build(&format!("^(?:{})$", self.source), "").is_some_and(|re| re.is_match(text).unwrap_or(false))
    }
}

impl Serialize for JsRegex {
    /// The source alone, as the oracle fixtures write a regex.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.source)
    }
}

/// The compiled regex for a literal source and its flags, built once per thread.
pub(crate) fn compiled(source: &'static str, flags: &'static str) -> Option<Rc<Regex>> {
    thread_local! {
        static CACHE: RefCell<HashMap<(&'static str, &'static str), Option<Rc<Regex>>>> = RefCell::new(HashMap::new());
    }
    CACHE.with(|cache| cache.borrow_mut().entry((source, flags)).or_insert_with(|| build(source, flags).map(Rc::new)).clone())
}

fn build(source: &str, flags: &str) -> Option<Regex> {
    let mut pattern = String::new();
    if flags.contains('i') {
        pattern.push_str("(?i)");
    }
    if flags.contains('m') {
        pattern.push_str("(?m)");
    }
    pattern.push_str(&translate(source, flags.contains('s')));
    Regex::new(&pattern).ok()
}

/// The JavaScript source as a fancy-regex pattern: `\s`, `\d`, `\w` and `\b` as JavaScript's ASCII and white-space
/// sets, `.` without the line ends unless `dot_all`, `[^]` as any character.
fn translate(source: &str, dot_all: bool) -> String {
    let cs: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len() + 16);
    let mut in_class = false;
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        i += 1;
        match c {
            '\\' => {
                let Some(&n) = cs.get(i) else {
                    out.push_str(r"\\");
                    break;
                };
                i += 1;
                out.push_str(&escape(n, in_class));
            }
            '[' if !in_class => {
                if cs[i..].starts_with(&['^', ']']) {
                    out.push_str("(?s:.)");
                    i += 2;
                } else if cs[i..].starts_with(&[']']) {
                    out.push_str("(?!)");
                    i += 1;
                } else {
                    in_class = true;
                    out.push('[');
                    if cs.get(i) == Some(&'^') {
                        out.push('^');
                        i += 1;
                    }
                }
            }
            ']' if in_class => {
                in_class = false;
                out.push(']');
            }
            '[' | '&' | '~' if in_class => {
                out.push('\\');
                out.push(c);
            }
            '.' if !in_class => out.push_str(if dot_all { "(?s:.)" } else { r"[^\n\r\x{2028}\x{2029}]" }),
            _ => out.push(c),
        }
    }
    out
}

fn escape(n: char, in_class: bool) -> String {
    match (n, in_class) {
        ('s', false) => format!("[{JS_SPACE}]"),
        ('S', false) => format!("[^{JS_SPACE}]"),
        ('s', true) => JS_SPACE.to_string(),
        ('d', false) => "[0-9]".to_string(),
        ('d', true) => "0-9".to_string(),
        ('D', false) => "[^0-9]".to_string(),
        ('w', false) => format!("[{WORD}]"),
        ('w', true) => WORD.to_string(),
        ('W', false) => format!("[^{WORD}]"),
        ('b', false) => format!("(?:(?<=[{WORD}])(?![{WORD}])|(?<![{WORD}])(?=[{WORD}]))"),
        ('B', false) => format!("(?:(?<=[{WORD}])(?=[{WORD}])|(?<![{WORD}])(?![{WORD}]))"),
        ('/', _) => "/".to_string(),
        (c, _) => format!("\\{c}"),
    }
}
