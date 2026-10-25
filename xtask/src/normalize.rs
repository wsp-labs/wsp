// SPDX-License-Identifier: AGPL-3.0-only
//! What two runs of one host never agree on, and only that, taken out before the diff: ids become `<kind-N>` in order
//! of first sight (`<ws-1>` for a `ws_` id, `<uuid-1>`, `<id-1>` for any other minted value, `<code-1>` for a pairing
//! code), a time its distance from the replay's start in whole minutes (`<time>` within the minute, `<time+10m>`, a
//! midnight `<day-1>`), and the facts of the one run (its temp folder, port, pid, token, version, the computer's name)
//! their placeholders. A key whose value is a reading that moves between two runs, as the disk's free bytes do, is
//! blanked whole. What a person set, a product rule fixes or the computer holds is left to the diff: a wrong port gets
//! those wrong. The names are the spec checker's (docs/rust-host-spec/checker), so a spec tape and a recorded one read
//! alike.

use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The literal values one replay saw that the next will not: each is replaced wherever it appears in a string. And the
/// moment the replay started, which every time is read against, and the times the tape itself sent, which stay as sent.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub literals: Vec<(String, String)>,
    /// Milliseconds since 1970 when the replay sent its first frame; none reads every time as `<time>`.
    pub started: Option<i64>,
    /// Times the tape's own frames carry, in milliseconds: a host that echoes one back answers it the same each run.
    pub sent: std::collections::BTreeSet<i64>,
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

/// Keys whose value is a reading that moves between two runs on one computer: what the disk has free, the load, how
/// long it has been up, the pid a turn's process was given and how long it was awake.
pub const MOVING_KEYS: &[&str] = &["diskFreeBytes", "load1", "uptime", "pid", "agentPid", "awakeMs"];
/// Keys whose value is a bearer the host mints at random each run, numbered as an id: one too short or too plain to
/// read as a key on its own (a ticket of 32 letters with no digit) is one all the same.
const BEARER_KEYS: &[&str] = &["ticket", "token", "deviceToken", "turnToken"];

/// Integers in this range are milliseconds since 1970: 2017 to 2049.
pub const EPOCH_MS: std::ops::RangeInclusive<f64> = 1.5e12..=2.5e12;

struct Patterns {
    time: Regex,
    uuid: Regex,
    prefixed: Regex,
    hex: Regex,
    opaque: Regex,
    placeholder: Regex,
    fingerprint: Regex,
    short_hex: Regex,
    join_code: Regex,
    pair_code: Regex,
    uptime: Regex,
    temp_name: Regex,
    node_pid: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        time: Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?").expect("time"),
        uuid: Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b").expect("uuid"),
        prefixed: Regex::new(r"\b[a-z][a-z0-9]*_[0-9a-f]{6,}\b").expect("prefixed"),
        hex: Regex::new(r"^[0-9a-f]{8,}$").expect("hex"),
        opaque: Regex::new(r"^[A-Za-z0-9+/_-]{32,}={0,2}$").expect("opaque"),
        placeholder: Regex::new(r"<([a-z][a-z0-9]*)-(\d+)>").expect("placeholder"),
        fingerprint: Regex::new(r"SHA256:[A-Za-z0-9+/]{43}").expect("fingerprint"),
        short_hex: Regex::new(r"\b[0-9a-f]{8,16}\b").expect("short hex"),
        join_code: Regex::new(r"\b[0-9A-Z]{4}-[0-9A-Z]{4}\b").expect("join code"),
        pair_code: Regex::new(r"^[0-9A-Z]{4}-?[0-9A-Z]{4}$").expect("pair code"),
        uptime: Regex::new(r"\bup \d+(?:\.\d+)?(?:ms|s|m|h)\b").expect("uptime"),
        temp_name: Regex::new(r"\b(wsp-[a-z0-9]+(?:-[a-z0-9]+)*-)([A-Za-z0-9]{6})\b").expect("temp name"),
        node_pid: Regex::new(r"\(node:\d+\)").expect("node pid"),
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

/// A string that is one JSON object or array, whole, and how it was laid out: compact or indented by two, as
/// `JSON.stringify` writes them, with or without a newline after. Text laid out any other way stays text.
fn whole_json(s: &str) -> Option<(Value, &'static str)> {
    let t = s.trim_start();
    if !(t.starts_with('{') || t.starts_with('[')) {
        return None;
    }
    let v = serde_json::from_str::<Value>(s).ok().filter(|v| v.is_object() || v.is_array())?;
    let compact = serde_json::to_string(&v).ok()?;
    let pretty = serde_json::to_string_pretty(&v).ok()?;
    let layout =
        [(compact.as_str(), "compact"), (pretty.as_str(), "indented")].into_iter().find_map(|(text, name)| match s.strip_prefix(text) {
            Some("") => Some(name),
            Some("\n") => Some(if name == "compact" { "compact, a newline after" } else { "indented, a newline after" }),
            _ => None,
        })?;
    Some((v, layout))
}

/// What a command printed, when every line of it is one compact JSON value, as `--json` prints: the lines as values,
/// so the rules for keys reach inside them.
fn json_lines(text: &str) -> Option<Vec<Value>> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let values: Vec<Value> = lines
        .iter()
        .map(|l| serde_json::from_str::<Value>(l).ok().filter(|v| serde_json::to_string(v).ok().as_deref() == Some(*l)))
        .collect::<Option<_>>()?;
    values.iter().any(|v| v.is_object() || v.is_array()).then_some(values)
}

/// Milliseconds since 1970 of an ISO 8601 time as `toISOString` and its kin write one.
pub fn iso_ms(s: &str) -> Option<i64> {
    let n = |a: usize, b: usize| s.get(a..b).and_then(|t| t.parse::<i64>().ok());
    let (y, mo, d, h, mi, sec) = (n(0, 4)?, n(5, 7)?, n(8, 10)?, n(11, 13)?, n(14, 16)?, n(17, 19)?);
    let mut rest = &s[19..];
    let mut ms = 0;
    if let Some(frac) = rest.strip_prefix('.') {
        let digits: String = frac.chars().take_while(char::is_ascii_digit).collect();
        ms = format!("{:0<3}", &digits[..digits.len().min(3)]).parse().ok()?;
        rest = &frac[digits.len()..];
    }
    let offset = match rest {
        "" | "Z" => 0,
        _ => {
            let sign = if rest.starts_with('-') { -1 } else { 1 };
            sign * (rest.get(1..3)?.parse::<i64>().ok()? * 60 + rest.get(4..6)?.parse::<i64>().ok()?)
        }
    };
    // Days from 1970-01-01 to the civil date (Howard Hinnant's days_from_civil).
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    Some(((days * 24 + h) * 60 + mi - offset) * 60_000 + sec * 1000 + ms)
}

const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;

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

    /// A time read against the replay's start: within the minute either way it is `<time>`, a time on a whole hour is
    /// the day it falls on and the hour (`<day>`, `<day-1 03:00>`), as the usage buckets are, and any other its
    /// distance in whole minutes (`<time+10m>`), so a code that stands ten minutes reads otherwise than one that stands
    /// an hour. A time the tape sent stays as sent.
    fn time(&self, ms: i64, written: &str) -> String {
        if self.facts.sent.contains(&ms) {
            return written.to_string();
        }
        let Some(start) = self.facts.started else { return "<time>".into() };
        if ms.rem_euclid(HOUR_MS) == 0 {
            let days = ms.div_euclid(DAY_MS) - start.div_euclid(DAY_MS);
            let day = if days == 0 { "<day".to_string() } else { format!("<day{days:+}") };
            let hour = ms.rem_euclid(DAY_MS) / HOUR_MS;
            return if hour == 0 { format!("{day}>") } else { format!("{day} {hour:02}:00>") };
        }
        let minutes = (ms - start) / 60_000;
        if minutes == 0 {
            "<time>".into()
        } else {
            format!("<time{minutes:+}m>")
        }
    }

    pub fn value(&mut self, v: &Value) -> Value {
        match v {
            Value::String(s) => match whole_json(s) {
                // JSON carried as text, as a tool's answer is: the rules for keys reach inside it, and its layout stays.
                Some((inner, layout)) => {
                    let mut out = Map::new();
                    out.insert("json".into(), self.value(&inner));
                    out.insert("layout".into(), Value::String(layout.into()));
                    Value::Object(out)
                }
                None => Value::String(self.string(s)),
            },
            Value::Number(n) if n.as_i64().is_some_and(|i| EPOCH_MS.contains(&(i as f64)) && !self.facts.sent.contains(&i)) => {
                Value::String(self.time(n.as_i64().expect("an integer"), &n.to_string()))
            }
            // The run's own port or pid given as a number.
            Value::Number(n) if self.facts.literals.iter().any(|(v, _)| *v == n.to_string()) => {
                let text = n.to_string();
                Value::String(self.facts.literals.iter().find(|(v, _)| *v == text).expect("found").1.clone())
            }
            Value::Array(items) => Value::Array(items.iter().map(|i| self.value(i)).collect()),
            Value::Object(obj) => {
                let mut out = Map::new();
                for (k, val) in obj {
                    let normalized = if MOVING_KEYS.contains(&k.as_str()) {
                        Value::String("<moving>".into())
                    } else if let Some(bearer) = val.as_str().filter(|b| BEARER_KEYS.contains(&k.as_str()) && !b.starts_with('<')) {
                        Value::String(self.id(bearer, "id"))
                    } else if let Some(code) = val.as_str().filter(|c| k == "code" && patterns().pair_code.is_match(c)) {
                        // A pairing code, minted at random each run; a refusal's code is a word and is compared.
                        Value::String(self.id(code, "code"))
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
                // A port or a pid is a run of digits, which an id can hold by chance (a pid of 4000 in a uuid's third
                // group): only the number standing alone, with no letter, digit, dash or underscore beside it.
                let alone = Regex::new(&format!(r"(^|[^0-9A-Za-z_-]){value}($|[^0-9A-Za-z_-])")).expect("digits");
                while alone.is_match(&out) {
                    out = alone.replace_all(&out, |c: &regex::Captures| format!("{}{placeholder}{}", &c[1], &c[2])).into_owned();
                }
            } else {
                out = out.replace(value.as_str(), placeholder);
            }
        }
        let p = patterns();
        // A key is decided whole before any rule for text cuts into it.
        if p.hex.is_match(&out) || (p.opaque.is_match(&out) && keyish(&out)) {
            return self.id(&out, "id");
        }
        let times: Vec<String> = p.time.find_iter(&out).map(|m| m.as_str().to_string()).collect();
        for t in times {
            let read = iso_ms(&t).map(|ms| self.time(ms, &t)).unwrap_or_else(|| "<time>".into());
            out = out.replacen(&t, &read, 1);
        }
        out = p.node_pid.replace_all(&out, "(node:<pid>)").into_owned();
        // How long the host has been up, as wsp status prints it.
        out = p.uptime.replace_all(&out, "up <moving>").into_owned();
        if MOVING_COLUMNS.iter().any(|c| out.contains(c)) {
            out = moving_columns(&out);
        }
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
        for (re, kind) in [
            (&p.uuid, Some("uuid")),
            (&p.prefixed, None),
            (&p.fingerprint, Some("id")),
            (&p.join_code, Some("id")),
            (&p.short_hex, Some("id")),
        ] {
            // A short run of hex in a sentence is an id with both a digit and a letter, or eight digits alone, as a
            // short id drawn from hex is about one time in 43: "deadbeef" and "1234567890" stay a word and a number.
            let id_like = |m: &str| {
                !std::ptr::eq(re, &p.short_hex)
                    || (m.bytes().any(|b| b.is_ascii_digit()) && m.bytes().any(|b| b.is_ascii_alphabetic()))
                    || (m.len() == 8 && m.bytes().all(|b| b.is_ascii_digit()))
            };
            let found: Vec<String> = re.find_iter(&out).map(|m| m.as_str().to_string()).filter(|m| id_like(m)).collect();
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

/// Columns of a printed table whose cells read this moment: what wsp computers says the disk has free, how long each of
/// wsp doctor's steps took.
const MOVING_COLUMNS: &[&str] = &["DISK FREE", "time"];

/// A printed table's moving columns, each cell `<moving>`, the rows split on runs of two spaces or more, as the table
/// pads them: the padding follows the widest cell, so it collapses to two spaces in every row of the table. A table
/// starts at a header line that names such a column and ends at a blank line.
fn moving_columns(text: &str) -> String {
    let gaps = Regex::new(r"  +").expect("gaps");
    let mut out = Vec::new();
    let mut moving: Vec<usize> = Vec::new();
    for line in text.split('\n') {
        let cells: Vec<&str> = gaps.split(line.trim_end()).collect();
        let header: Vec<usize> = cells.iter().enumerate().filter(|(_, c)| MOVING_COLUMNS.contains(c)).map(|(i, _)| i).collect();
        if !header.is_empty() {
            moving = header;
            out.push(cells.join("  "));
        } else if line.trim().is_empty() {
            moving.clear();
            out.push(line.to_string());
        } else if !moving.is_empty() && cells.len() > 1 {
            let row: Vec<&str> = cells.iter().enumerate().map(|(i, c)| if moving.contains(&i) { "<moving>" } else { c }).collect();
            out.push(row.join("  "));
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// The port the host's door listens on, which the system hands it on each run: as `door.port` says it, and as the
/// address a join line names.
fn door_ports(v: &Value, facts: &mut Facts) {
    static SAID: OnceLock<Regex> = OnceLock::new();
    // The port a host a tape's own command started says it serves on, in its status table, and the door's in a join
    // line printed as text: the system hands both on each run.
    let said = SAID.get_or_init(|| {
        Regex::new(r"(?m)^(?:app {2,}https?|runtime ws {2,}ws)://127\.0\.0\.1:(\d+)|wsp join https?://127\.0\.0\.1:(\d+)").expect("said")
    });
    match v {
        Value::String(text) => {
            for c in said.captures_iter(text) {
                if let Some(port) = c.get(1) {
                    facts.add(port.as_str(), "<port>");
                }
                if let Some(port) = c.get(2) {
                    facts.add(port.as_str(), "<door-port>");
                }
            }
        }
        Value::Object(obj) => {
            if let Some(port) = obj.get("door").and_then(|d| d.get("port")).and_then(Value::as_u64) {
                facts.add(port.to_string(), "<door-port>");
            }
            for join in obj.get("joins").and_then(Value::as_array).into_iter().flatten() {
                let port = join
                    .get("url")
                    .and_then(Value::as_str)
                    .and_then(|u| u.rsplit(':').next())
                    .and_then(|p| p.trim_end_matches('/').parse::<u16>().ok());
                if let Some(port) = port {
                    facts.add(port.to_string(), "<door-port>");
                }
            }
            obj.values().for_each(|x| door_ports(x, facts));
        }
        Value::Array(items) => items.iter().for_each(|x| door_ports(x, facts)),
        _ => {}
    }
}

/// A replay's answers in the form goldens are kept in: the replies in the order their requests were sent, numbering
/// ids as they first appear, then the events, which have no defined order, normalized and sorted.
pub fn canonical(replies: &[Value], events: &[Value], facts: &Facts) -> Value {
    let mut facts = facts.clone();
    for r in replies {
        door_ports(r, &mut facts);
    }
    let mut n = Normalizer::new(facts);
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
