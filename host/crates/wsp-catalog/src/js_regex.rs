// SPDX-License-Identifier: AGPL-3.0-only
//! A JavaScript regular expression kept as data on a catalog row (a token's shape, a sign-in's question, a code's
//! shape): its source as the TS writes it, matched with JavaScript's meaning. Lookbehind is supported, so the
//! implementation sits on fancy-regex (rule 8.7). The spec is spec/jsonc.md.
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use fancy_regex::Regex;
use serde::{Serialize, Serializer};

use wsp_words::js::JS_WS as JS_SPACE;
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
        compiled(self.source, self.flags).find(text).is_some()
    }

    /// `re.exec(text)?.[0]`.
    pub fn find(&self, text: &str) -> Option<String> {
        compiled(self.source, self.flags).find(text).map(|range| text[range].to_string())
    }

    /// `new RegExp(`^(?:${source})$`).test(text)`: the whole text is one match.
    pub fn is_whole_match(&self, text: &str) -> bool {
        let (behind, rest) = split_lookbehind(self.source).map_or((None, self.source), |(x, rest)| (Some(x), rest));
        let behind_holds = behind.is_none_or(|x| build(&format!(r"(?:{x})\z"), "").is_match("").unwrap_or(false));
        behind_holds && build(&format!("^(?:{rest})$"), "").is_match(text).unwrap_or(false)
    }
}

impl Serialize for JsRegex {
    /// The source alone, as the oracle fixtures write a regex.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.source)
    }
}

/// A compiled JavaScript pattern. A pattern that opens with a lookbehind of unbounded width, which fancy-regex
/// refuses and JavaScript takes, is held as the lookbehind's body (`behind`, anchored at its end) and the rest.
pub(crate) struct Matcher {
    pub(crate) re: Regex,
    behind: Option<Regex>,
}

impl Matcher {
    /// `re.exec(text)?.index` and its end: the leftmost match whose lookbehind, if the pattern opens with one, holds.
    pub(crate) fn find(&self, text: &str) -> Option<Range<usize>> {
        let Some(behind) = &self.behind else {
            return self.re.find(text).ok().flatten().map(|m| m.range());
        };
        let mut from = 0;
        loop {
            let m = self.re.find_from_pos(text, from).ok().flatten()?;
            if behind.is_match(&text[..m.start()]).unwrap_or(false) {
                return Some(m.range());
            }
            from = m.start() + text[m.start()..].chars().next()?.len_utf8();
        }
    }
}

/// The compiled pattern for a literal source and its flags, built once per thread. A pattern fancy-regex refuses
/// panics: every catalog pattern is compiled by a test, and none ever reads as matching nothing.
pub(crate) fn compiled(source: &'static str, flags: &'static str) -> Rc<Matcher> {
    type Cache = HashMap<(&'static str, &'static str), Rc<Matcher>>;
    thread_local! {
        static CACHE: RefCell<Cache> = RefCell::new(HashMap::new());
    }
    CACHE.with(|cache| cache.borrow_mut().entry((source, flags)).or_insert_with(|| Rc::new(matcher(source, flags))).clone())
}

fn matcher(source: &str, flags: &str) -> Matcher {
    match split_lookbehind(source) {
        Some((body, rest)) => Matcher { re: build(rest, flags), behind: Some(build(&format!(r"(?:{body})\z"), flags)) },
        None => Matcher { re: build(source, flags), behind: None },
    }
}

/// The body of a leading `(?<=...)` and the pattern after it.
fn split_lookbehind(source: &str) -> Option<(&str, &str)> {
    let inner = source.strip_prefix("(?<=")?;
    let (mut depth, mut in_class, mut escaped) = (1, false, false);
    for (i, c) in inner.char_indices() {
        match (c, escaped) {
            (_, true) => escaped = false,
            ('\\', false) => escaped = true,
            ('[', false) => in_class = true,
            (']', false) => in_class = false,
            ('(', false) if !in_class => depth += 1,
            (')', false) if !in_class => {
                depth -= 1;
                if depth == 0 {
                    return Some((&inner[..i], &inner[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

fn build(source: &str, flags: &str) -> Regex {
    let ignore_case = flags.contains('i');
    let mut pattern = String::new();
    if ignore_case {
        pattern.push_str("(?i)");
    }
    pattern.push_str(&translate(source, flags.contains('s'), flags.contains('m'), ignore_case));
    Regex::new(&pattern).unwrap_or_else(|e| panic!("the pattern /{source}/{flags} is not one fancy-regex compiles: {e}"))
}

const LINE_ENDS: &str = r"\n\r\x{2028}\x{2029}";
const FOLDS_TO_ASCII: &str = r"\x{17F}\x{212A}";

/// The JavaScript source as a fancy-regex pattern: `\s`, `\d`, `\w` and `\b` as JavaScript's ASCII and white-space
/// sets, `.` without the line ends unless `dot_all`, `[^]` as any character, `^` and `$` at every line end under
/// `multiline`, and under `ignore_case` no fold of U+017F to s or U+212A to k.
fn translate(source: &str, dot_all: bool, multiline: bool, ignore_case: bool) -> String {
    let cs: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len() + 16);
    let mut in_class = false;
    let mut class_start = 0;
    let mut negated = false;
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
                    class_start = out.len();
                    out.push('[');
                    negated = cs.get(i) == Some(&'^');
                    if negated {
                        out.push('^');
                        i += 1;
                    }
                }
            }
            ']' if in_class => {
                in_class = false;
                out.push(']');
                if ignore_case {
                    let class = out.split_off(class_start);
                    if negated {
                        out.push_str(&format!("(?:{class}|(?-i:[{FOLDS_TO_ASCII}]))"));
                    } else {
                        out.push_str(&format!("(?:(?-i:(?![{FOLDS_TO_ASCII}])){class})"));
                    }
                }
            }
            '[' | '&' | '~' if in_class => {
                out.push('\\');
                out.push(c);
            }
            '(' if !in_class && cs[i..].starts_with(&['?', '<']) && !matches!(cs.get(i + 2), Some('=' | '!')) => {
                let name_end = cs[i..].iter().position(|&n| n == '>').map_or(cs.len(), |n| i + n + 1);
                out.push('(');
                out.extend(&cs[i..name_end]);
                i = name_end;
            }
            '.' if !in_class => out.push_str(if dot_all { "(?s:.)" } else { r"[^\n\r\x{2028}\x{2029}]" }),
            '^' if !in_class && multiline => out.push_str(&format!("(?:^|(?<=[{LINE_ENDS}]))")),
            '$' if !in_class && multiline => out.push_str(&format!("(?:$|(?=[{LINE_ENDS}]))")),
            's' | 'S' | 'k' | 'K' if ignore_case && !in_class => {
                out.push_str(&format!("(?-i:[{}{}])", c.to_ascii_lowercase(), c.to_ascii_uppercase()));
            }
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
