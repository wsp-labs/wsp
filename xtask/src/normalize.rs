// SPDX-License-Identifier: AGPL-3.0-only
//! What two runs of one host never agree on, taken out before the diff: ids become `<kind-N>` in order of first sight
//! (`<ws-1>` for a `ws_` id, `<uuid-1>`, `<id-1>` for any other minted value), times `<time>`, a semver under
//! `version` `<version>`, and the facts of the one run (its temp folder, port, pid, token, the computer's name) their
//! placeholders. A key whose value is a reading of the computer the host runs on is blanked whole. The names are the
//! spec checker's (docs/rust-host-spec/checker), so a spec tape and a recorded one read alike.

use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The literal values one replay saw that the next will not: each is replaced wherever it appears in a string.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub literals: Vec<(String, String)>,
}

impl Facts {
    pub fn add(&mut self, value: impl Into<String>, placeholder: &str) {
        let value = value.into();
        // A value of a character or two would blank letters inside words.
        if value.len() >= 3 && !self.literals.iter().any(|(v, _)| *v == value) {
            self.literals.push((value, placeholder.to_string()));
            self.literals.sort_by_key(|l| std::cmp::Reverse(l.0.len()));
        }
    }

    /// A folder, and the same folder as an agent names its project store after it, every character not a letter or a
    /// digit a dash (Claude Code's `~/.claude/projects/-tmp-...`).
    pub fn add_folder(&mut self, path: &str, placeholder: &str) {
        self.add(path, placeholder);
        let dashed: String = path.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        self.add(dashed, &format!("{}-dashed>", placeholder.trim_end_matches('>')));
    }
}

/// Keys whose value reads the computer under the host, which a run on another computer answers differently.
pub const MACHINE_KEYS: &[&str] = &["diskFreeBytes", "shape", "cap", "capDefault", "os", "load1", "uptime", "addresses"];
/// Ports the system hands a listener that asked for none: Linux's 32768 to 60999 and the 49152 to 65535 of macOS.
const EPHEMERAL: std::ops::RangeInclusive<u64> = 32768..=65535;

/// Integers in this range are milliseconds since 1970: 2017 to 2049.
const EPOCH_MS: std::ops::RangeInclusive<f64> = 1.5e12..=2.5e12;

struct Patterns {
    time: Regex,
    uuid: Regex,
    prefixed: Regex,
    hex: Regex,
    opaque: Regex,
    semver: Regex,
    placeholder: Regex,
    fingerprint: Regex,
    join_code: Regex,
    loopback: Regex,
    temp_name: Regex,
    node_pid: Regex,
    reading: Regex,
    padding: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        time: Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?").expect("time"),
        uuid: Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b").expect("uuid"),
        prefixed: Regex::new(r"\b[a-z][a-z0-9]*_[0-9a-f]{6,}\b").expect("prefixed"),
        hex: Regex::new(r"^[0-9a-f]{8,}$").expect("hex"),
        opaque: Regex::new(r"^[A-Za-z0-9+/_-]{32,}={0,2}$").expect("opaque"),
        semver: Regex::new(r"^\d+\.\d+\.\d+").expect("semver"),
        placeholder: Regex::new(r"<([a-z][a-z0-9]*)-(\d+)>").expect("placeholder"),
        fingerprint: Regex::new(r"SHA256:[A-Za-z0-9+/]{43}").expect("fingerprint"),
        join_code: Regex::new(r"\b[0-9A-Z]{4}-[0-9A-Z]{4}\b").expect("join code"),
        loopback: Regex::new(r"\b(127\.0\.0\.1|localhost|\[::1\]):(?:3[2-9]|[4-6][0-9])\d{3}\b").expect("loopback"),
        temp_name: Regex::new(r"\b(wsp-[a-z0-9]+(?:-[a-z0-9]+)*-)([A-Za-z0-9]{6})\b").expect("temp name"),
        node_pid: Regex::new(r"\(node:\d+\)").expect("node pid"),
        reading: Regex::new(r"\b\d+(?:\.\d+)?[ \u{a0}]?(?:(?:[KMGT]i?)?B|ms)\b").expect("reading"),
        padding: Regex::new(r"  +").expect("padding"),
    })
}

/// A key, a nonce or a token in base64 has capitals and small letters, and a digit, a `+` or a `/`, or else the length of
/// a 32-byte key; a long word or path of one case has neither.
fn keyish(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_uppercase())
        && s.chars().any(|c| c.is_ascii_lowercase())
        && (s.chars().any(|c| c.is_ascii_digit() || c == '+' || c == '/') || s.len() >= 40)
}

/// Whether a whole string is a value a host mints: a uuid, a `ws_` style id, a run of hex, or a key in base64. A
/// replay follows only these from a tape's recorded answers to the host's own.
pub fn minted(s: &str) -> bool {
    let p = patterns();
    let whole = |re: &Regex| re.find(s).is_some_and(|m| m.start() == 0 && m.end() == s.len());
    whole(&p.uuid) || whole(&p.prefixed) || p.hex.is_match(s) || (p.opaque.is_match(s) && keyish(s))
}

/// A string that is one JSON object or array, whole.
fn whole_json(s: &str) -> Option<Value> {
    let t = s.trim_start();
    if !(t.starts_with('{') || t.starts_with('[')) {
        return None;
    }
    serde_json::from_str::<Value>(s).ok().filter(|v| v.is_object() || v.is_array())
}

/// What a command printed, when every line of it is JSON, as `--json` prints: the lines as values, so the rules for
/// keys reach inside them.
fn json_lines(text: &str) -> Option<Vec<Value>> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let values: Vec<Value> = lines.iter().map(|l| serde_json::from_str(l).ok()).collect::<Option<_>>()?;
    values.iter().any(|v| v.is_object() || v.is_array()).then_some(values)
}

/// One tape's numbering of ids, so the same id reads the same placeholder in every answer of that tape.
#[derive(Default)]
pub struct Normalizer {
    facts: Facts,
    ids: HashMap<String, String>,
    counts: HashMap<String, usize>,
    /// Off for events, whose order is not defined: an id first seen there reads `<id>` with no number.
    numbering: bool,
}

impl Normalizer {
    pub fn new(facts: Facts) -> Normalizer {
        Normalizer { facts, ids: HashMap::new(), counts: HashMap::new(), numbering: true }
    }

    pub fn value(&mut self, v: &Value) -> Value {
        match v {
            Value::String(s) => match whole_json(s) {
                // JSON carried as text, as a tool's answer is: the rules for keys reach inside it.
                Some(inner) => {
                    let mut out = Map::new();
                    out.insert("json".into(), self.value(&inner));
                    Value::Object(out)
                }
                None => Value::String(self.string(s)),
            },
            Value::Number(n) if n.as_f64().is_some_and(|f| f.fract() == 0.0 && EPOCH_MS.contains(&f)) => Value::String("<time>".into()),
            Value::Array(items) => Value::Array(items.iter().map(|i| self.value(i)).collect()),
            Value::Object(obj) => {
                let mut out = Map::new();
                for (k, val) in obj {
                    let normalized = if MACHINE_KEYS.contains(&k.as_str()) {
                        Value::String("<machine>".into())
                    } else if k == "code" && val.is_string() {
                        // A pairing code, minted at random each run; an exit code is a number and is compared.
                        Value::String("<code>".into())
                    } else if k == "port" && val.as_u64().is_some_and(|p| EPHEMERAL.contains(&p)) {
                        Value::String("<port>".into())
                    } else if k == "version" && val.as_str().is_some_and(|v| patterns().semver.is_match(v)) {
                        Value::String("<version>".into())
                    } else if let Some(lines) = (k == "stdout").then(|| val.as_str().and_then(json_lines)).flatten() {
                        let mut out = Map::new();
                        out.insert("jsonLines".into(), Value::Array(lines.iter().map(|l| self.value(l)).collect()));
                        Value::Object(out)
                    } else {
                        self.value(val)
                    };
                    // An id can be a key, as a record kept per project is.
                    out.insert(self.string(k), normalized);
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }

    fn string(&mut self, s: &str) -> String {
        let mut out = s.to_string();
        for (value, placeholder) in &self.facts.literals {
            if !out.contains(value.as_str()) {
                continue;
            }
            if value.bytes().all(|b| b.is_ascii_digit()) {
                // A port or a pid is a run of digits, which a hex id can hold by chance: only the number standing alone.
                out = Regex::new(&format!(r"\b{value}\b")).expect("digits").replace_all(&out, placeholder.as_str()).into_owned();
            } else {
                out = out.replace(value.as_str(), placeholder);
            }
        }
        let p = patterns();
        out = p.time.replace_all(&out, "<time>").into_owned();
        out = p.loopback.replace_all(&out, "$1:<port>").into_owned();
        out = p.node_pid.replace_all(&out, "(node:<pid>)").into_owned();
        // A folder mkdtemp named, its six characters drawn at random: one of them a capital or a digit, so a word
        // that happens to be six letters long stays.
        out = p
            .temp_name
            .replace_all(&out, |c: &regex::Captures| {
                if c[2].chars().any(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit()) {
                    format!("{}<temp>", &c[1])
                } else {
                    c[0].to_string()
                }
            })
            .into_owned();
        // A size or a time in prose or a table reads this computer's disk or how long a step took. A table pads its
        // columns to the widest reading, so where one was blanked the padding is too.
        if p.reading.is_match(&out) {
            out = p.reading.replace_all(&out, "<reading>").into_owned();
            out = p.padding.replace_all(&out, "  ").into_owned();
        }
        if p.hex.is_match(&out) || (p.opaque.is_match(&out) && keyish(&out)) {
            return self.id(&out, "id");
        }
        for (re, kind) in [(&p.uuid, Some("uuid")), (&p.prefixed, None), (&p.fingerprint, Some("id")), (&p.join_code, Some("id"))] {
            let found: Vec<String> = re.find_iter(&out).map(|m| m.as_str().to_string()).collect();
            for id in found {
                let kind = kind.map(str::to_string).unwrap_or_else(|| id.split('_').next().unwrap_or("id").to_string());
                let placeholder = self.id(&id, &kind);
                out = out.replace(&id, &placeholder);
            }
        }
        out
    }

    fn id(&mut self, raw: &str, kind: &str) -> String {
        if let Some(known) = self.ids.get(raw) {
            return known.clone();
        }
        if !self.numbering {
            return format!("<{kind}>");
        }
        let n = self.counts.entry(kind.to_string()).or_default();
        *n += 1;
        let placeholder = format!("<{kind}-{n}>");
        self.ids.insert(raw.to_string(), placeholder.clone());
        placeholder
    }
}

/// Placeholders numbered again by first sight, each kind on its own count: a spec's checker numbers the frames it
/// sent as well as the answers, so the same answers can carry other numbers there. Once `frozen`, a placeholder not
/// seen yet loses its number, as an id first seen in an event does.
#[derive(Default)]
struct Renumber {
    map: HashMap<(String, String), String>,
    counts: HashMap<String, usize>,
    frozen: bool,
}

impl Renumber {
    fn value(&mut self, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(self.string(s)),
            Value::Array(items) => Value::Array(items.iter().map(|i| self.value(i)).collect()),
            Value::Object(obj) => Value::Object(obj.iter().map(|(k, v)| (self.string(k), self.value(v))).collect()),
            other => other.clone(),
        }
    }

    fn string(&mut self, s: &str) -> String {
        patterns()
            .placeholder
            .replace_all(s, |c: &regex::Captures| {
                let key = (c[1].to_string(), c[2].to_string());
                if let Some(known) = self.map.get(&key) {
                    return known.clone();
                }
                if self.frozen {
                    return format!("<{}>", &c[1]);
                }
                let n = self.counts.entry(c[1].to_string()).or_default();
                *n += 1;
                let renamed = format!("<{}-{n}>", &c[1]);
                self.map.insert(key, renamed.clone());
                renamed
            })
            .into_owned()
    }
}

/// A replay's answers in the form goldens are kept in: the replies in the order their requests were sent, numbering
/// ids as they first appear, then the events, which have no defined order, normalized and sorted.
pub fn canonical(replies: &[Value], events: &[Value], facts: &Facts) -> Value {
    let mut n = Normalizer::new(facts.clone());
    let replies: Vec<Value> = replies.iter().map(|r| n.value(r)).collect();
    n.numbering = false;
    let events: Vec<Value> = events.iter().map(|e| n.value(e)).collect();
    let mut again = Renumber::default();
    let replies: Vec<Value> = replies.iter().map(|r| again.value(r)).collect();
    again.frozen = true;
    let mut events: Vec<Value> = events.iter().map(|e| again.value(e)).collect();
    events.sort_by_cached_key(|e| e.to_string());
    let mut out = Map::new();
    out.insert("replies".into(), Value::Array(replies));
    out.insert("events".into(), Value::Array(events));
    Value::Object(out)
}
