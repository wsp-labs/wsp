// SPDX-License-Identifier: AGPL-3.0-only
//! A URL as node's `new URL(text)` reads it. The `url` crate follows the same standard, but its IDNA tables are older than
//! node's in places and newer in others, so a host is checked against the code points the two answer differently first.
//! The tables are what node 22.23.4 answered for every code point at the start, middle and end of a host label.

use url::Url;

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

const SPECIAL: &[&str] = &["http", "https", "ws", "wss", "ftp"];

pub fn parse(text: &str) -> Option<NodeUrl> {
    let cleaned: String = text.chars().filter(|c| !matches!(c, '\t' | '\n' | '\r')).collect();
    let text = cleaned.trim_matches(|c: char| c <= ' ');
    match host_span(text) {
        Some((start, end)) if !text[start..end].is_ascii() || text[start..end].contains('%') => parse_host(text, start, end),
        _ => plain(text),
    }
}

fn plain(text: &str) -> Option<NodeUrl> {
    let url = Url::parse(text).ok()?;
    let host = url.host_str().unwrap_or_default().to_string();
    Some(NodeUrl { url, host })
}

fn host_span(text: &str) -> Option<(usize, usize)> {
    let (scheme, rest) = text.split_once(':')?;
    if !SPECIAL.contains(&scheme.to_ascii_lowercase().as_str()) {
        return None;
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

fn parse_host(text: &str, start: usize, end: usize) -> Option<NodeUrl> {
    let decoded: String = percent_decode(&text[start..end])
        .chars()
        .map(|c| if matches!(c, '\u{3002}' | '\u{ff0e}' | '\u{ff61}') { '.' } else { c })
        .collect();
    if decoded.chars().any(|c| forbidden(c) || in_table(REFUSED, c)) {
        return None;
    }
    let mapped = decoded.replace('\u{1e9e}', "ss");
    let mixed =
        |label: &str| label.chars().any(|c| in_table(LTR_ONLY, c)) && label.chars().any(|c| !c.is_ascii() && !in_table(LTR_ONLY, c));
    if mapped.split('.').any(mixed) {
        return None;
    }
    let spliced = format!("{}{mapped}{}", &text[..start], &text[end..]);
    if let Ok(url) = Url::parse(&spliced) {
        let host = url.host_str().unwrap_or_default().to_string();
        return Some(NodeUrl { url, host });
    }
    let host = if mapped.is_ascii() { None } else { fallback_host(&mapped) }?;
    let url = Url::parse(&format!("{}x.test{}", &text[..start], &text[end..])).ok()?;
    Some(NodeUrl { url, host })
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

/// A host the `url` crate refuses and node takes, for the labels whose odd characters the tables name: every other
/// label must be plain ASCII, so a host with a character the tables do not know stays refused. Node refuses a label that
/// does not start with a letter where another label of the host is of this kind, so every label here must.
fn fallback_host(host: &str) -> Option<String> {
    let labels: Vec<String> = host.split('.').map(fallback_label).collect::<Option<_>>()?;
    let letters = labels.iter().all(|label| label.starts_with(|c: char| c.is_ascii_alphabetic()));
    letters.then(|| labels.join("."))
}

fn fallback_label(label: &str) -> Option<String> {
    let lower = label.to_ascii_lowercase();
    if lower.is_ascii() {
        return (!lower.starts_with("xn--")).then_some(lower);
    }
    let chars: Vec<char> = lower.chars().collect();
    let plain = |c: &char| c.is_ascii() || in_table(LTR_ONLY, *c);
    let (last, head) = chars.split_last()?;
    let takes = if chars.iter().all(plain) {
        true
    } else if head.is_empty() {
        in_table(ALONE, *last)
    } else {
        head.iter().all(plain) && head[0].is_ascii_alphabetic() && in_table(LAST, *last)
    };
    takes.then(|| format!("xn--{}", punycode(&chars)))
}

/// RFC 3492's encoder.
fn punycode(input: &[char]) -> String {
    const BASE: u32 = 36;
    const T_MIN: u32 = 1;
    const T_MAX: u32 = 26;
    let digit = |d: u32| char::from(if d < 26 { b'a' + d as u8 } else { b'0' + (d - 26) as u8 });
    let adapt = |mut delta: u32, points: u32, first: bool| {
        delta = if first { delta / 700 } else { delta / 2 };
        delta += delta / points;
        let mut k = 0;
        while delta > (BASE - T_MIN) * T_MAX / 2 {
            delta /= BASE - T_MIN;
            k += BASE;
        }
        k + (BASE - T_MIN + 1) * delta / (delta + 38)
    };
    let mut out: String = input.iter().filter(|c| c.is_ascii()).collect();
    let basic = out.len() as u32;
    if basic > 0 {
        out.push('-');
    }
    let (mut n, mut delta, mut bias, mut handled) = (128u32, 0u32, 72u32, basic);
    while (handled as usize) < input.len() {
        let next = input.iter().map(|c| *c as u32).filter(|c| *c >= n).min().expect("an unhandled code point remains");
        delta += (next - n) * (handled + 1);
        n = next;
        for c in input.iter().map(|c| *c as u32) {
            if c < n {
                delta += 1;
            }
            if c == n {
                let (mut q, mut k) = (delta, BASE);
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
                bias = adapt(delta, handled + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    out
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

/// Code points node takes as plain letters and the url crate refuses (assigned after its tables).
const LTR_ONLY: &[(u32, u32)] = &[
    (0x61d, 0x61d),
    (0x870, 0x88e),
    (0x898, 0x89f),
    (0x8b5, 0x8b5),
    (0x8c8, 0x8d2),
    (0xc3c, 0xc3c),
    (0xcf3, 0xcf3),
    (0xece, 0xece),
    (0x1715, 0x1715),
    (0x1ac1, 0x1ace),
    (0x1dfa, 0x1dfa),
    (0xfbc2, 0xfbc2),
    (0x10efd, 0x10eff),
    (0x10f70, 0x10f89),
    (0x11070, 0x11070),
    (0x11073, 0x11074),
    (0x110c2, 0x110c2),
    (0x11241, 0x11241),
    (0x11f00, 0x11f01),
    (0x11f03, 0x11f03),
    (0x11f34, 0x11f3a),
    (0x11f3e, 0x11f42),
    (0x13440, 0x13440),
    (0x13447, 0x13455),
    (0x1cf00, 0x1cf2d),
    (0x1cf30, 0x1cf46),
    (0x1e08f, 0x1e08f),
    (0x1e2ae, 0x1e2ae),
    (0x1e4ec, 0x1e4ef),
];

/// Code points node takes as a whole label and the url crate refuses.
const ALONE: &[(u32, u32)] = &[(0x660, 0x669), (0x66b, 0x66c), (0x10d30, 0x10d39), (0x10e60, 0x10e7e)];

/// Code points node takes as the last character of a label that starts with a letter, which the url crate refuses.
const LAST: &[(u32, u32)] = &[
    (0x5be, 0x5be),
    (0x5c0, 0x5c0),
    (0x5c3, 0x5c3),
    (0x5c6, 0x5c6),
    (0x5d0, 0x5ea),
    (0x5ef, 0x5f4),
    (0x608, 0x608),
    (0x60b, 0x60b),
    (0x60d, 0x60d),
    (0x61b, 0x61b),
    (0x61e, 0x64a),
    (0x660, 0x669),
    (0x66b, 0x66f),
    (0x671, 0x674),
    (0x679, 0x6d5),
    (0x6e5, 0x6e6),
    (0x6ee, 0x6ef),
    (0x6fa, 0x70d),
    (0x710, 0x710),
    (0x712, 0x72f),
    (0x74d, 0x7a5),
    (0x7b1, 0x7b1),
    (0x7c0, 0x7ea),
    (0x7f4, 0x7f5),
    (0x7fa, 0x7fa),
    (0x7fe, 0x815),
    (0x81a, 0x81a),
    (0x824, 0x824),
    (0x828, 0x828),
    (0x830, 0x83e),
    (0x840, 0x858),
    (0x85e, 0x85e),
    (0x860, 0x86a),
    (0x8a0, 0x8b4),
    (0x8b6, 0x8c7),
    (0xfbb2, 0xfbc1),
    (0xfe73, 0xfe73),
    (0x10800, 0x10805),
    (0x10808, 0x10808),
    (0x1080a, 0x10835),
    (0x10837, 0x10838),
    (0x1083c, 0x1083c),
    (0x1083f, 0x10855),
    (0x10857, 0x1089e),
    (0x108a7, 0x108af),
    (0x108e0, 0x108f2),
    (0x108f4, 0x108f5),
    (0x108fb, 0x1091b),
    (0x10920, 0x10939),
    (0x1093f, 0x1093f),
    (0x10980, 0x109b7),
    (0x109bc, 0x109cf),
    (0x109d2, 0x10a00),
    (0x10a10, 0x10a13),
    (0x10a15, 0x10a17),
    (0x10a19, 0x10a35),
    (0x10a40, 0x10a48),
    (0x10a50, 0x10a58),
    (0x10a60, 0x10a9f),
    (0x10ac0, 0x10ae4),
    (0x10aeb, 0x10af6),
    (0x10b00, 0x10b35),
    (0x10b40, 0x10b55),
    (0x10b58, 0x10b72),
    (0x10b78, 0x10b91),
    (0x10b99, 0x10b9c),
    (0x10ba9, 0x10baf),
    (0x10c00, 0x10c48),
    (0x10cc0, 0x10cf2),
    (0x10cfa, 0x10d23),
    (0x10d30, 0x10d39),
    (0x10e60, 0x10e7e),
    (0x10e80, 0x10ea9),
    (0x10ead, 0x10ead),
    (0x10eb0, 0x10eb1),
    (0x10f00, 0x10f27),
    (0x10f30, 0x10f45),
    (0x10f51, 0x10f59),
    (0x10fb0, 0x10fcb),
    (0x10fe0, 0x10ff6),
    (0x1e800, 0x1e8c4),
    (0x1e8c7, 0x1e8cf),
    (0x1e922, 0x1e943),
    (0x1e94b, 0x1e94b),
    (0x1e950, 0x1e959),
    (0x1e95e, 0x1e95f),
    (0x1ec71, 0x1ecb4),
    (0x1ed01, 0x1ed3d),
];
