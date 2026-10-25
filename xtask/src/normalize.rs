// SPDX-License-Identifier: AGPL-3.0-only
//! What two runs of one host never agree on, taken out before the diff: ids become `<id:N>` in order of first sight,
//! times `<time>`, and the facts of the one run (its temp folder, port, pid, the computer's name) their placeholders.
//! A key whose value is a reading of the computer the host runs on is blanked whole.

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
}

/// Keys whose value reads the computer under the host, which a run on another computer answers differently.
pub const MACHINE_KEYS: &[&str] = &["diskFreeBytes", "shape", "cap", "capDefault", "os", "load1", "uptime"];

/// Integers in this range are milliseconds since 1970: 2017 to 2049.
const EPOCH_MS: std::ops::RangeInclusive<f64> = 1.5e12..=2.5e12;

struct Patterns {
    time: Regex,
    uuid: Regex,
    prefixed: Regex,
    hex: Regex,
    opaque: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        time: Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?").expect("time"),
        uuid: Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b").expect("uuid"),
        prefixed: Regex::new(r"\b[a-z][a-z0-9]*_[0-9a-f]{6,}\b").expect("prefixed"),
        hex: Regex::new(r"^[0-9a-f]{8,}$").expect("hex"),
        opaque: Regex::new(r"^[A-Za-z0-9_-]{32,}$").expect("opaque"),
    })
}

/// One tape's numbering of ids, so the same id reads the same placeholder in every answer of that tape.
#[derive(Default)]
pub struct Normalizer {
    facts: Facts,
    ids: HashMap<String, String>,
    /// Off for events, whose order is not defined: an id first seen there reads `<id>` with no number.
    numbering: bool,
}

impl Normalizer {
    pub fn new(facts: Facts) -> Normalizer {
        Normalizer { facts, ids: HashMap::new(), numbering: true }
    }

    pub fn value(&mut self, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(self.string(s)),
            Value::Number(n) if n.as_f64().is_some_and(|f| f.fract() == 0.0 && EPOCH_MS.contains(&f)) => Value::String("<time>".into()),
            Value::Array(items) => Value::Array(items.iter().map(|i| self.value(i)).collect()),
            Value::Object(obj) => {
                let mut out = Map::new();
                for (k, val) in obj {
                    let normalized = if MACHINE_KEYS.contains(&k.as_str()) { Value::String("<machine>".into()) } else { self.value(val) };
                    out.insert(k.clone(), normalized);
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }

    fn string(&mut self, s: &str) -> String {
        let mut out = s.to_string();
        for (value, placeholder) in &self.facts.literals {
            if out.contains(value.as_str()) {
                out = out.replace(value.as_str(), placeholder);
            }
        }
        let p = patterns();
        out = p.time.replace_all(&out, "<time>").into_owned();
        if p.hex.is_match(&out) || p.opaque.is_match(&out) {
            return self.id(&out);
        }
        for re in [&p.uuid, &p.prefixed] {
            let found: Vec<String> = re.find_iter(&out).map(|m| m.as_str().to_string()).collect();
            for id in found {
                let placeholder = self.id(&id);
                out = out.replace(&id, &placeholder);
            }
        }
        out
    }

    fn id(&mut self, raw: &str) -> String {
        if let Some(known) = self.ids.get(raw) {
            return known.clone();
        }
        if !self.numbering {
            return "<id>".into();
        }
        let placeholder = format!("<id:{}>", self.ids.len() + 1);
        self.ids.insert(raw.to_string(), placeholder.clone());
        placeholder
    }
}

/// A replay's answers in the form goldens are kept in: the replies in the order their requests were sent, numbering
/// ids as they first appear, then the events, which have no defined order, normalized and sorted.
pub fn canonical(replies: &[Value], events: &[Value], facts: &Facts) -> Value {
    let mut n = Normalizer::new(facts.clone());
    let replies: Vec<Value> = replies.iter().map(|r| n.value(r)).collect();
    n.numbering = false;
    let mut events: Vec<Value> = events.iter().map(|e| n.value(e)).collect();
    events.sort_by_cached_key(|e| e.to_string());
    let mut out = Map::new();
    out.insert("replies".into(), Value::Array(replies));
    out.insert("events".into(), Value::Array(events));
    Value::Object(out)
}
