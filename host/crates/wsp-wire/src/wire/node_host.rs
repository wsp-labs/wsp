// SPDX-License-Identifier: AGPL-3.0-only
//! A URL as node's `new URL(text)` reads it: the `url` crate's answer, corrected where its IDNA tables or its rules differ from node's.

use std::collections::HashMap;
use url::Url;

mod classes;

/// A URL that node would parse, with the host as node answers it.
pub struct NodeUrl {
    url: Url,
    host: String,
}

impl NodeUrl {
    pub fn hostname(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> Option<u16> {
        self.url.port()
    }

    /// `URL.host`: the hostname, and the port where it is not the scheme's own.
    pub fn host(&self) -> String {
        match self.url.port() {
            Some(port) => format!("{}:{port}", self.host),
            None => self.host.clone(),
        }
    }

    pub fn query_param(&self, name: &str) -> Option<String> {
        self.url.query_pairs().find(|(key, _)| key == name).map(|(_, value)| value.into_owned())
    }
}

const SPECIAL: &[&str] = &["http", "https", "ws", "wss", "ftp", "file"];

pub fn parse(text: &str) -> Option<NodeUrl> {
    let cleaned: String = text.chars().filter(|c| !matches!(c, '\t' | '\n' | '\r')).collect();
    let text = cleaned.trim_matches(|c: char| c <= ' ');
    let span = host_span(text);
    match span {
        Some((start, end)) if needs_tables(&text[start..end]) => parse_host(text, start, end),
        _ => plain(text, span),
    }
}

fn plain(text: &str, span: Option<(usize, usize)>) -> Option<NodeUrl> {
    if opaque_refused(text) {
        return None;
    }
    let url = Url::parse(text).ok()?;
    let host = span.map_or_else(|| url_host(&url, ""), |(_, end)| url_host(&url, &text[..end]));
    Some(NodeUrl { url, host })
}

/// The `url` crate blanks the host of a `file:` URL whose path starts with a drive letter and node does not; `head` is the URL up to its host.
fn url_host(url: &Url, head: &str) -> String {
    let host = url.host_str().unwrap_or_default();
    if host.is_empty() && url.scheme() == "file" && !head.is_empty() {
        if let Ok(bare) = Url::parse(&format!("{head}/")) {
            return bare.host_str().unwrap_or_default().to_string();
        }
    }
    host.to_string()
}

/// A scheme that is not special ends its authority only at `/ ? #` and refuses credentials with no host; the `url` crate does neither.
fn opaque_refused(text: &str) -> bool {
    let Some((scheme, rest)) = text.split_once(':') else { return false };
    if SPECIAL.contains(&scheme.to_ascii_lowercase().as_str()) {
        return false;
    }
    let Some(after) = rest.strip_prefix("//") else { return false };
    let authority = &after[..after.find(['/', '?', '#']).unwrap_or(after.len())];
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    host_port.contains('\\') || (host_port.is_empty() && authority.contains('@'))
}

fn host_span(text: &str) -> Option<(usize, usize)> {
    let (scheme, rest) = text.split_once(':')?;
    let scheme = scheme.to_ascii_lowercase();
    if !SPECIAL.contains(&scheme.as_str()) {
        return None;
    }
    if scheme == "file" {
        let after = rest.strip_prefix(['/', '\\'])?.strip_prefix(['/', '\\'])?;
        let start = text.len() - after.len();
        return Some((start, start + after.find(['/', '\\', '?', '#']).unwrap_or(after.len())));
    }
    let after = rest.trim_start_matches(['/', '\\']);
    let authority = &after[..after.find(['/', '\\', '?', '#']).unwrap_or(after.len())];
    let at = authority.rfind('@').map_or(0, |i| i + 1);
    let host = &authority[at..];
    if host.starts_with('[') {
        return None;
    }
    let start = text.len() - after.len() + at;
    Some((start, start + host.find(':').unwrap_or(host.len())))
}

fn needs_tables(host: &str) -> bool {
    !host.is_ascii() || host.contains('%') || host.split('.').any(is_punycode_label)
}

fn is_punycode_label(label: &str) -> bool {
    label.get(..4).is_some_and(|head| head.eq_ignore_ascii_case("xn--"))
}

fn parse_host(text: &str, start: usize, end: usize) -> Option<NodeUrl> {
    let decoded: String = percent_decode(&text[start..end])
        .chars()
        .map(|c| if matches!(c, '\u{3002}' | '\u{ff0e}' | '\u{ff61}') { '.' } else { c })
        .collect();
    let mapped = decoded.replace('\u{1e9e}', "ss");
    let labels: Vec<Label> = mapped.split('.').map(Label::read).collect::<Option<_>>()?;
    if decoded.chars().any(|c| forbidden(c) || in_table(REFUSED, c)) || labels.iter().flat_map(|l| &l.chars).any(|c| in_table(REFUSED, *c))
    {
        return None;
    }
    let mut probes = Probes::default();
    if labels.iter().any(|label| label.punycode.is_some() && !stable(&label.chars, &mut probes)) {
        return None;
    }
    let head = format!("{}{mapped}", &text[..start]);
    let spliced = format!("{head}{}", &text[end..]);
    if let Ok(url) = Url::parse(&spliced) {
        if labels.iter().any(|label| label.chars.iter().any(|c| class_of(*c) == Class::Unknown) && !bidi_ok(&label.chars)) {
            return None;
        }
        let host = url_host(&url, &head);
        return Some(NodeUrl { url, host });
    }
    let host = fallback_host(&mapped)?;
    let url = Url::parse(&format!("{}x.test{}", &text[..start], &text[end..])).ok()?;
    Some(NodeUrl { url, host })
}

/// A label's characters, a punycode label's decoded, with the punycode label in lower case where it decodes past ASCII.
struct Label {
    chars: Vec<char>,
    punycode: Option<String>,
}

impl Label {
    /// None for a punycode label node refuses: not punycode, or decoding to nothing.
    fn read(label: &str) -> Option<Label> {
        let lower = label.to_ascii_lowercase();
        let Some(code) = lower.strip_prefix("xn--") else { return Some(Label { chars: label.chars().collect(), punycode: None }) };
        match punycode_decode(code) {
            Some(chars) if chars.is_empty() => None,
            Some(chars) if chars.iter().all(char::is_ascii) => Some(Label { chars: label.chars().collect(), punycode: None }),
            Some(chars) => Some(Label { chars, punycode: Some(lower) }),
            None => None,
        }
    }
}

/// Node refuses a punycode label that mapping or normalizing would change; the `url` crate does not, so its answer for the decoded label is compared.
fn stable(chars: &[char], probes: &mut Probes) -> bool {
    let text: String = chars.iter().collect();
    let Ok(url) = Url::parse(&format!("http://{text}.com/")) else {
        return chars.iter().all(|c| c.is_ascii() || (*c != '\u{1e9e}' && probes.mapped(*c) == [*c]))
            && probes.normalized(chars.to_vec()) == chars;
    };
    let label = url.host_str().and_then(|host| host.strip_suffix(".com"));
    label.and_then(|label| label.strip_prefix("xn--")).and_then(punycode_decode).as_deref() == Some(chars)
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (bytes[i], bytes.get(i + 1).and_then(|b| hex(*b)), bytes.get(i + 2).and_then(|b| hex(*b))) {
            (b'%', Some(hi), Some(lo)) => {
                out.push((hi * 16 + lo) as u8);
                i += 3;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// WHATWG's forbidden domain code points.
fn forbidden(c: char) -> bool {
    c <= ' ' || matches!(c, '#' | '/' | ':' | '<' | '>' | '?' | '@' | '[' | '\\' | ']' | '^' | '|' | '%' | '\u{7f}')
}

fn in_table(table: &[(u32, u32)], c: char) -> bool {
    let c = c as u32;
    table
        .binary_search_by(|&(low, high)| {
            if high < c {
                std::cmp::Ordering::Less
            } else if low > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// What node's bidi check sees a character as. `Unknown` is valid and has no class in node's tables.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Invalid,
    Left,
    Neutral,
    Unknown,
    Right,
    Mark,
    Digit,
    LeftMark,
    Arabic,
}

fn class_of(c: char) -> Class {
    if c.is_ascii() {
        return match c {
            'a'..='z' | 'A'..='Z' => Class::Left,
            '0'..='9' => Class::Digit,
            _ => Class::Neutral,
        };
    }
    let at = classes::RUNS.partition_point(|run| run >> 4 <= c as u32);
    match classes::RUNS[at - 1] & 0xf {
        1 => Class::Left,
        2 => Class::Neutral,
        3 => Class::Unknown,
        4 => Class::Right,
        5 => Class::Mark,
        6 => Class::Digit,
        7 => Class::LeftMark,
        8 => Class::Arabic,
        _ => Class::Invalid,
    }
}

/// Node's bidi check of one label, which is not RFC 5893's: a label that starts with a letter may end in a right-to-left one, and any other is held to the right-to-left rules only when it holds one.
fn bidi_ok(label: &[char]) -> bool {
    let classes: Vec<Class> = label.iter().map(|c| class_of(*c)).collect();
    let Some(first) = classes.first() else { return true };
    if matches!(first, Class::Mark | Class::LeftMark) {
        return false;
    }
    let right = |c: &Class| matches!(c, Class::Right | Class::Arabic);
    let end = classes.iter().rposition(|c| *c != Class::Mark).unwrap_or(0);
    if matches!(first, Class::Left) {
        let body = &classes[..end];
        return !body.iter().any(right) && !(right(&classes[end]) && body.contains(&Class::Unknown));
    }
    if !classes.iter().any(right) {
        return true;
    }
    let digits = classes.contains(&Class::Digit) && classes.contains(&Class::Arabic);
    !digits
        && !classes.iter().any(|c| matches!(c, Class::Left | Class::LeftMark | Class::Unknown))
        && matches!(classes[end], Class::Right | Class::Arabic | Class::Digit)
}

/// A host the `url` crate refuses and node takes, each label rebuilt in punycode; a host of plain ASCII labels stays refused.
fn fallback_host(host: &str) -> Option<String> {
    if !host.split('.').any(|label| !label.is_ascii() || is_punycode_label(label)) {
        return None;
    }
    let mut probes = Probes::default();
    let labels: Vec<String> = host.split('.').map(|label| fallback_label(label, &mut probes)).collect::<Option<_>>()?;
    (!ends_in_number(&labels)).then(|| labels.join("."))
}

fn fallback_label(label: &str, probes: &mut Probes) -> Option<String> {
    let mut mapped: Vec<char> = Vec::with_capacity(label.len());
    for c in label.chars() {
        if c.is_ascii() {
            mapped.push(c.to_ascii_lowercase());
        } else {
            mapped.extend_from_slice(probes.mapped(c));
        }
    }
    let mapped = probes.normalized(mapped);
    if mapped.iter().all(char::is_ascii) {
        let ascii: String = mapped.iter().collect();
        let Some(code) = ascii.strip_prefix("xn--") else { return Some(ascii) };
        let chars = punycode_decode(code).filter(|chars| !chars.is_empty())?;
        let refused = chars.iter().any(|c| in_table(REFUSED, *c));
        return (chars.iter().all(char::is_ascii) || (!refused && usable(&chars) && stable(&chars, probes))).then_some(ascii);
    }
    if label.get(..4).is_some_and(|head| head.eq_ignore_ascii_case("xn--")) || !usable(&mapped) {
        return None;
    }
    punycode(&mapped).map(|code| format!("xn--{code}"))
}

fn usable(label: &[char]) -> bool {
    label.iter().all(|c| class_of(*c) != Class::Invalid) && bidi_ok(label)
}

/// What the `url` crate answers for single characters and runs of marks, kept for one host.
#[derive(Default)]
struct Probes {
    mapped: HashMap<char, Vec<char>>,
    marks: HashMap<char, bool>,
}

impl Probes {
    /// What the `url` crate maps a character to; one it cannot place stays as it is.
    fn mapped(&mut self, c: char) -> &[char] {
        self.mapped.entry(c).or_insert_with(|| match class_of(c) {
            Class::Invalid | Class::Unknown => vec![c],
            class => through_url(&[c], class).unwrap_or_else(|| vec![c]),
        })
    }

    /// Node's tables lack the newer marks, which the `url` crate refuses at the start of a label.
    fn is_mark(&mut self, c: char) -> bool {
        match class_of(c) {
            Class::Mark | Class::LeftMark => true,
            Class::Unknown => *self.marks.entry(c).or_insert_with(|| Url::parse(&format!("http://{c}/")).is_err()),
            _ => false,
        }
    }

    /// Each run of marks put in order and composed with the character before it, as the `url` crate does.
    fn normalized(&mut self, label: Vec<char>) -> Vec<char> {
        let mut out: Vec<char> = Vec::with_capacity(label.len());
        let mut at = 0;
        while at < label.len() {
            if !self.is_mark(label[at]) {
                out.push(label[at]);
                at += 1;
                continue;
            }
            let mut end = at;
            while end < label.len() && self.is_mark(label[end]) {
                end += 1;
            }
            if out.is_empty() && end - at == 1 {
                out.push(label[at]);
            } else {
                let unit: Vec<char> = out.pop().into_iter().chain(label[at..end].iter().copied()).collect();
                let class =
                    if unit.iter().any(|c| matches!(class_of(*c), Class::Right | Class::Arabic)) { Class::Right } else { Class::Left };
                out.extend(through_url(&unit, class).unwrap_or(unit));
            }
            at = end;
        }
        out
    }
}

/// What the `url` crate turns `body` into, read off a label it takes with `body` between two letters of a script that keep its bidi rules quiet.
fn through_url(body: &[char], class: Class) -> Option<Vec<char>> {
    let (right, left) = (('\u{5d0}', '\u{5d1}'), ('1', 'a'));
    let wraps = if matches!(class, Class::Right | Class::Arabic) { [right, left] } else { [left, right] };
    let text: String = body.iter().collect();
    wraps.iter().find_map(|(first, last)| {
        let url = Url::parse(&format!("http://{first}{text}{last}/")).ok()?;
        let host = url.host_str()?;
        let chars = match host.strip_prefix("xn--") {
            Some(code) => punycode_decode(code)?,
            None => host.chars().collect(),
        };
        (chars.len() >= 2 && !chars.contains(&'.')).then(|| chars[1..chars.len() - 1].to_vec())
    })
}

/// WHATWG's "ends in a number": such a host is read as IPv4 and fails where another label is not a number.
fn ends_in_number(labels: &[String]) -> bool {
    let mut labels = labels;
    if labels.len() > 1 && labels[labels.len() - 1].is_empty() {
        labels = &labels[..labels.len() - 1];
    }
    let Some(last) = labels.last() else { return false };
    let hex = last.strip_prefix("0x").or_else(|| last.strip_prefix("0X")).is_some_and(|rest| rest.bytes().all(|b| b.is_ascii_hexdigit()));
    !last.is_empty() && (last.bytes().all(|b| b.is_ascii_digit()) || hex)
}

const BASE: u32 = 36;
const T_MIN: u32 = 1;
const T_MAX: u32 = 26;

fn adapt(delta: u32, points: u32, first: bool) -> u32 {
    let mut delta = if first { delta / 700 } else { delta / 2 };
    delta += delta / points;
    let mut k = 0;
    while delta > (BASE - T_MIN) * T_MAX / 2 {
        delta /= BASE - T_MIN;
        k += BASE;
    }
    k + (BASE - T_MIN + 1) * delta / (delta + 38)
}

/// RFC 3492's encoder; node refuses a label whose running delta passes an `i32`.
fn punycode(input: &[char]) -> Option<String> {
    const LIMIT: u64 = i32::MAX as u64;
    let digit = |d: u32| char::from(if d < 26 { b'a' + d as u8 } else { b'0' + (d - 26) as u8 });
    let mut out: String = input.iter().filter(|c| c.is_ascii()).collect();
    let basic = out.len() as u32;
    if basic > 0 {
        out.push('-');
    }
    let (mut n, mut delta, mut bias, mut handled) = (128u32, 0u64, 72u32, basic);
    while (handled as usize) < input.len() {
        let next = input.iter().map(|c| *c as u32).filter(|c| *c >= n).min()?;
        delta += u64::from(next - n) * (u64::from(handled) + 1);
        n = next;
        for c in input.iter().map(|c| *c as u32) {
            if c < n {
                delta += 1;
            }
            if delta > LIMIT {
                return None;
            }
            if c == n {
                let (mut q, mut k) = (delta as u32, BASE);
                loop {
                    let t = if k <= bias { T_MIN } else { (k - bias).min(T_MAX) };
                    if q < t {
                        break;
                    }
                    out.push(digit(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                out.push(digit(q));
                bias = adapt(delta as u32, handled + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    Some(out)
}

/// RFC 3492's decoder; None where the text is not punycode.
fn punycode_decode(code: &str) -> Option<Vec<char>> {
    let (basic, rest) = code.rfind('-').map_or(("", code), |at| (&code[..at], &code[at + 1..]));
    if !basic.is_ascii() {
        return None;
    }
    let mut out: Vec<u32> = basic.bytes().map(u32::from).collect();
    let (mut n, mut i, mut bias) = (128u32, 0u32, 72u32);
    let mut digits = rest.bytes().peekable();
    while digits.peek().is_some() {
        let (old, mut w, mut k) = (i, 1u32, BASE);
        loop {
            let d = match digits.next()? {
                b @ b'a'..=b'z' => u32::from(b - b'a'),
                b @ b'A'..=b'Z' => u32::from(b - b'A'),
                b @ b'0'..=b'9' => u32::from(b - b'0') + 26,
                _ => return None,
            };
            i = i.checked_add(d.checked_mul(w)?)?;
            let t = if k <= bias { T_MIN } else { (k - bias).min(T_MAX) };
            if d < t {
                break;
            }
            w = w.checked_mul(BASE - t)?;
            k = k.checked_add(BASE)?;
        }
        let points = out.len() as u32 + 1;
        bias = adapt(i - old, points, old == 0);
        n = n.checked_add(i / points)?;
        char::from_u32(n)?;
        out.insert((i % points) as usize, n);
        i = i % points + 1;
    }
    out.into_iter().map(char::from_u32).collect()
}

/// Code points node's IDNA refuses in every host, which the url crate maps away or keeps.
const REFUSED: &[(u32, u32)] = &[
    (0x4c0, 0x4c0),
    (0x558, 0x558),
    (0x58b, 0x58c),
    (0x5c8, 0x5c9),
    (0x88f, 0x88f),
    (0x897, 0x897),
    (0xb53, 0xb54),
    (0xc5c, 0xc5c),
    (0xcdc, 0xcdc),
    (0x10a0, 0x10c5),
    (0x115f, 0x1160),
    (0x17b4, 0x17b5),
    (0x1806, 0x1806),
    (0x180e, 0x180e),
    (0x1acf, 0x1af0),
    (0x1b4e, 0x1b4f),
    (0x1b7f, 0x1b7f),
    (0x1c89, 0x1c8a),
    (0x2061, 0x2063),
    (0x206a, 0x206f),
    (0x208f, 0x208f),
    (0x209d, 0x209f),
    (0x20c1, 0x20c4),
    (0x2132, 0x2132),
    (0x2183, 0x2183),
    (0x2427, 0x2429),
    (0x2b96, 0x2b96),
    (0x2e60, 0x2e63),
    (0x3164, 0x3164),
    (0x31e4, 0x31e5),
    (0xa7cb, 0xa7cf),
    (0xa7d2, 0xa7d2),
    (0xa7d4, 0xa7d4),
    (0xa7da, 0xa7dd),
    (0xa7e2, 0xa7e2),
    (0xa7f1, 0xa7f1),
    (0xab6c, 0xab6d),
    (0xfbc3, 0xfbd2),
    (0xfd90, 0xfd91),
    (0xfdc8, 0xfdce),
    (0xffa0, 0xffa0),
    (0x105c0, 0x105f3),
    (0x107bb, 0x107bf),
    (0x10940, 0x10959),
    (0x10d40, 0x10d65),
    (0x10d69, 0x10d85),
    (0x10d8e, 0x10d8f),
    (0x10ec2, 0x10ec7),
    (0x10ec9, 0x10eee),
    (0x10ef0, 0x10efc),
    (0x11380, 0x11389),
    (0x1138b, 0x1138b),
    (0x1138e, 0x1138e),
    (0x11390, 0x113b5),
    (0x113b7, 0x113c0),
    (0x113c2, 0x113c2),
    (0x113c5, 0x113c5),
    (0x113c7, 0x113ca),
    (0x113cc, 0x113d5),
    (0x113d7, 0x113d8),
    (0x113e1, 0x113e2),
    (0x116d0, 0x116e3),
    (0x11b0a, 0x11b0a),
    (0x11b60, 0x11b67),
    (0x11bc0, 0x11be1),
    (0x11bf0, 0x11bf9),
    (0x11db0, 0x11ddb),
    (0x11de0, 0x11de9),
    (0x11df0, 0x11df1),
    (0x11f5a, 0x11f5a),
    (0x1246f, 0x1246f),
    (0x12475, 0x1247f),
    (0x12550, 0x12686),
    (0x13460, 0x143fa),
    (0x16100, 0x16139),
    (0x16d40, 0x16d79),
    (0x16ea0, 0x16eb8),
    (0x16ebb, 0x16ed3),
    (0x16ff2, 0x16ff6),
    (0x187f8, 0x187ff),
    (0x18cd6, 0x18cda),
    (0x18cff, 0x18cff),
    (0x18d09, 0x18d20),
    (0x18d80, 0x18df2),
    (0x18e00, 0x19191),
    (0x191a0, 0x191d2),
    (0x1b123, 0x1b128),
    (0x1b168, 0x1b168),
    (0x1cc00, 0x1ccfc),
    (0x1cd00, 0x1ceb3),
    (0x1ceba, 0x1ced0),
    (0x1ced2, 0x1ced4),
    (0x1cedd, 0x1cefd),
    (0x1d127, 0x1d128),
    (0x1d173, 0x1d17a),
    (0x1d1eb, 0x1d1ff),
    (0x1d250, 0x1d281),
    (0x1d6a6, 0x1d6a6),
    (0x1db00, 0x1db1c),
    (0x1df1f, 0x1df24),
    (0x1df2b, 0x1df81),
    (0x1df90, 0x1df96),
    (0x1dfcd, 0x1dfff),
    (0x1e5d0, 0x1e5fa),
    (0x1e5ff, 0x1e5ff),
    (0x1e6c0, 0x1e6de),
    (0x1e6e0, 0x1e6f5),
    (0x1e6fe, 0x1e6ff),
    (0x1f1ae, 0x1f1ae),
    (0x1f6d8, 0x1f6d9),
    (0x1f777, 0x1f77a),
    (0x1f7da, 0x1f7db),
    (0x1f7f1, 0x1f7ff),
    (0x1f8b2, 0x1f8bb),
    (0x1f8c0, 0x1f8c1),
    (0x1f8d0, 0x1f8d8),
    (0x1fa54, 0x1fa57),
    (0x1fa89, 0x1fa8f),
    (0x1fabe, 0x1fabe),
    (0x1fac6, 0x1fac6),
    (0x1fac8, 0x1fac8),
    (0x1facc, 0x1facd),
    (0x1fadc, 0x1fadd),
    (0x1fadf, 0x1fadf),
    (0x1fae9, 0x1faeb),
    (0x1faef, 0x1faef),
    (0x1faf9, 0x1fafa),
    (0x1fbcb, 0x1fbef),
    (0x1fbfa, 0x1fbfa),
    (0x2b73a, 0x2b73f),
    (0x2b81e, 0x2b81e),
    (0x2cea2, 0x2cead),
    (0x2ebf0, 0x2ee5d),
    (0x2f868, 0x2f868),
    (0x2f874, 0x2f874),
    (0x2f91f, 0x2f91f),
    (0x2f95f, 0x2f95f),
    (0x2f9bf, 0x2f9bf),
    (0x323b0, 0x33479),
    (0x3d000, 0x3fc3f),
];
