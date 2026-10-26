// SPDX-License-Identifier: AGPL-3.0-only
//! What two runs of one host never agree on, and only that, taken out before the diff: ids become `<kind-N>` in order
//! of first sight (`<ws-1>` for a `ws_` id, `<uuid-1>`, `<id-1>` for any other minted value, `<code-1>` for a pairing
//! code), each with its form where the kind does not fix it (`<uuid-1, in capitals>`), a time its distance from the replay's start in whole minutes (`<time>` within the minute, `<time+10m>`, a
//! midnight `<day-1>`), and the facts of the one run (its temp folder, port, pid, token, version, the computer's name)
//! their placeholders. A key whose value is a reading that moves between two runs, as the disk's free bytes do, is
//! blanked whole when it has the kind the TS host gives it. Only the value goes: a time not written as JavaScript
//! writes one, or given as a number, says its form beside its placeholder, json lines keep how they are laid out, and an
//! id the tape sent, or one a refusal says there is none of, stays as written. What a person set, a product rule fixes or
//! the computer holds is left to the diff: a wrong port gets those wrong. The names are the spec checker's (docs/rust-host-spec/checker), so a spec tape and a recorded one read
//! alike.

use crate::json;
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
    /// Ids the tape's own frames carry, which a host that names one back names the same each run.
    pub sent_ids: std::collections::BTreeSet<String>,
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

/// A time as JavaScript's `toISOString` writes it, which a time's placeholder stands for alone.
const JS_TIME: &str = r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$";

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
    js_time: Regex,
    none_of: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        time: Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?").expect("time"),
        uuid: Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b").expect("uuid"),
        prefixed: Regex::new(r"\b[a-z][a-z0-9]*_[0-9a-f]{6,}\b").expect("prefixed"),
        hex: Regex::new(r"^[0-9a-f]{8,}$").expect("hex"),
        opaque: Regex::new(r"^[A-Za-z0-9+/_-]{32,}={0,2}$").expect("opaque"),
        placeholder: Regex::new(r"<([a-z][a-z0-9]*)-(\d+)((?:, [a-z0-9 ]+)?)>").expect("placeholder"),
        fingerprint: Regex::new(r"SHA256:[A-Za-z0-9+/]{43}").expect("fingerprint"),
        short_hex: Regex::new(r"\b[0-9a-f]{8,16}\b").expect("short hex"),
        join_code: Regex::new(r"\b[0-9A-Z]{4}-[0-9A-Z]{4}\b").expect("join code"),
        pair_code: Regex::new(r"^[0-9A-Z]{4}-?[0-9A-Z]{4}$").expect("pair code"),
        uptime: Regex::new(r"\bup \d+(?:\.\d+)?(?:ms|s|m|h)\b").expect("uptime"),
        temp_name: Regex::new(r"\b(wsp-[a-z0-9]+(?:-[a-z0-9]+)*-)([A-Za-z0-9]{6})\b").expect("temp name"),
        node_pid: Regex::new(r"\(node:\d+\)").expect("node pid"),
        js_time: Regex::new(JS_TIME).expect("js time"),
        none_of: Regex::new(r#"\bno (?:such )?[a-z]+(?: named)?:? "?$"#).expect("none of"),
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
    let v = json::read(s).ok().filter(|v| v.is_object() || v.is_array())?;
    let compact = json::write(&v);
    let pretty = json::write_pretty(&v);
    let layout =
        [(compact.as_str(), "compact"), (pretty.as_str(), "indented")].into_iter().find_map(|(text, name)| match s.strip_prefix(text) {
            Some("") => Some(name),
            Some("\n") => Some(if name == "compact" { "compact, a newline after" } else { "indented, a newline after" }),
            _ => None,
        })?;
    Some((v, layout))
}

/// How json lines sit in what a command printed: each line's JSON as `<json>`, the newlines, carriage returns and blank
/// lines between them as written.
fn json_layout(text: &str) -> String {
    text.split('\n')
        .map(|seg| {
            let core = seg.strip_suffix('\r').unwrap_or(seg);
            if core.trim().is_empty() {
                seg.to_string()
            } else {
                seg.replacen(core, "<json>", 1)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What a command printed, when every line of it is one compact JSON value, as `--json` prints: the lines as values,
/// so the rules for keys reach inside them.
fn json_lines(text: &str) -> Option<Vec<Value>> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let values: Vec<Value> = lines.iter().map(|l| json::read(l).ok().filter(|v| json::write(v) == *l)).collect::<Option<_>>()?;
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
    /// Off for events, which the TS host cannot order across streams: an id first seen there reads `<id>` with no number.
    numbering: bool,
    /// On while the answer being read is a refusal, whose "no such workspace: ws_..." names an id the host never minted.
    refusal: bool,
}

impl Normalizer {
    pub fn new(facts: Facts) -> Normalizer {
        Normalizer { facts, ids: HashMap::new(), counts: HashMap::new(), numbering: true, refusal: false }
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
                let read = self.time(n.as_i64().expect("an integer"), &n.to_string());
                Value::String(if read.starts_with('<') { format!("{} in ms>", read.trim_end_matches('>')) } else { read })
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
                    let normalized = if MOVING_KEYS.contains(&k.as_str()) && val.is_number() {
                        Value::String("<moving>".into())
                    } else if let Some(bearer) =
                        val.as_str().filter(|b| BEARER_KEYS.contains(&k.as_str()) && !b.is_empty() && !b.starts_with('<'))
                    {
                        Value::String(self.id(bearer, "id"))
                    } else if let Some(code) = val.as_str().filter(|c| k == "code" && patterns().pair_code.is_match(c)) {
                        // A pairing code, minted at random each run; a refusal's code is a word and is compared.
                        Value::String(self.id(code, "code"))
                    } else if let Some(lines) = (k == "stdout").then(|| val.as_str().and_then(json_lines)).flatten() {
                        let mut out = Map::new();
                        out.insert("jsonLines".into(), Value::Array(lines.iter().map(|l| self.value(l)).collect()));
                        out.insert("layout".into(), Value::String(json_layout(val.as_str().expect("text"))));
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
            let mut read = iso_ms(&t).map(|ms| self.time(ms, &t)).unwrap_or_else(|| "<time>".into());
            if read.starts_with('<') && !p.js_time.is_match(&t) {
                let form: String = t.chars().map(|c| if c.is_ascii_digit() { 'd' } else { c }).collect();
                read = format!("{} written {form}>", read.trim_end_matches('>'));
            }
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
            let asked = |m: &regex::Match| {
                kind.is_none() && self.refusal && !self.ids.contains_key(m.as_str()) && p.none_of.is_match(&out[..m.start()])
            };
            let found: Vec<String> =
                re.find_iter(&out).filter(|m| id_like(m.as_str()) && !asked(m)).map(|m| m.as_str().to_string()).collect();
            for id in found {
                let kind = kind.map(str::to_string).unwrap_or_else(|| id.split('_').next().unwrap_or("id").to_string());
                let placeholder = self.id(&id, &kind);
                out = out.replace(&id, &placeholder);
            }
        }
        out
    }

    fn id(&mut self, raw: &str, kind: &str) -> String {
        if self.facts.sent_ids.contains(raw) {
            return raw.to_string();
        }
        if let Some(known) = self.ids.get(raw) {
            return known.clone();
        }
        let form = form(raw, kind);
        if !self.numbering {
            return format!("<{kind}{form}>");
        }
        let n = self.counts.entry(kind.to_string()).or_default();
        *n += 1;
        let placeholder = format!("<{kind}-{n}{form}>");
        self.ids.insert(raw.to_string(), placeholder.clone());
        placeholder
    }
}

/// What an id's placeholder keeps of how it was written, said after its number where the kind does not fix it: a uuid
/// in capitals, a pairing code with a dash, a prefixed id of other than the eight hex digits `randomBytes(4)` gives, and
/// any other id's length.
fn form(raw: &str, kind: &str) -> String {
    let p = patterns();
    match kind {
        "uuid" if raw.bytes().any(|b| b.is_ascii_uppercase()) => {
            if raw.bytes().any(|b| b.is_ascii_lowercase()) { ", in mixed case" } else { ", in capitals" }.into()
        }
        "uuid" => String::new(),
        "code" if raw.contains('-') => ", with a dash".into(),
        "code" => String::new(),
        "id" if p.fingerprint.is_match(raw) || p.join_code.is_match(raw) => String::new(),
        "id" => format!(", {} characters", raw.chars().count()),
        _ => match raw.split_once('_').map(|(_, digits)| digits.len()) {
            Some(8) | None => String::new(),
            Some(n) => format!(", {n} digits"),
        },
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
                // A day before the replay's start is `<day-1>`, a distance and never an id's number.
                if &c[1] == "day" {
                    return c[0].to_string();
                }
                let key = (c[1].to_string(), c[2].to_string());
                let form = &c[3];
                if let Some(known) = self.map.get(&key) {
                    return format!("{known}{form}>");
                }
                if self.frozen {
                    return format!("<{}{form}>", &c[1]);
                }
                let n = self.counts.entry(c[1].to_string()).or_default();
                *n += 1;
                let renamed = format!("<{}-{n}", &c[1]);
                self.map.insert(key, renamed.clone());
                format!("{renamed}{form}>")
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

/// The port of a host a tape's own command started, in its status table: the system hands it on each run, and the
/// replay never dialled it. The replay's own host's ports, its door's among them, are facts of the run.
fn started_ports(v: &Value, facts: &mut Facts) {
    static SAID: OnceLock<Regex> = OnceLock::new();
    let said = SAID.get_or_init(|| Regex::new(r"(?m)^(?:app {2,}https?|runtime ws {2,}ws)://127\.0\.0\.1:(\d+)").expect("said"));
    match v {
        Value::String(text) => {
            for c in said.captures_iter(text) {
                facts.add(&c[1], "<port>");
            }
        }
        Value::Object(obj) => obj.values().for_each(|x| started_ports(x, facts)),
        Value::Array(items) => items.iter().for_each(|x| started_ports(x, facts)),
        _ => {}
    }
}

/// Whether an answer says no: `ok: false` or an error, a tool result marked one, a command line's exit other than 0.
fn refused(r: &Value) -> bool {
    r.get("ok") == Some(&Value::Bool(false))
        || r.get("error").is_some()
        || r.pointer("/result/isError") == Some(&Value::Bool(true))
        || r.get("exit").and_then(Value::as_i64).is_some_and(|e| e != 0)
}

/// Every id a value carries, as the normalizer reads ids: whole keys and runs of hex, uuids, `ws_` style ids, host key
/// fingerprints and join codes.
pub fn ids_in(v: &Value, out: &mut std::collections::BTreeSet<String>) {
    match v {
        Value::String(text) => {
            let p = patterns();
            if p.hex.is_match(text) || (p.opaque.is_match(text) && keyish(text)) {
                out.insert(text.clone());
            }
            for re in [&p.uuid, &p.prefixed, &p.fingerprint, &p.join_code, &p.short_hex] {
                out.extend(re.find_iter(text).map(|m| m.as_str().to_string()));
            }
        }
        Value::Object(obj) => obj.values().for_each(|x| ids_in(x, out)),
        Value::Array(items) => items.iter().for_each(|x| ids_in(x, out)),
        _ => {}
    }
}

/// The stream an event arrives in order on, by the ids it carries as sent (parity/README.md, "How events are
/// compared"): a thread's rows pushed, a thread's other events, a turn's, a workspace's cost ticks, and else the
/// event's family about the one thing it names (a pty, a workspace) or about the host; the frames the socket itself
/// sends are one stream.
fn stream(e: &Value) -> String {
    let at = |k: &str| e.get(k).and_then(Value::as_str);
    let Some(kind) = at("type") else { return "socket".into() };
    if let Some(thread) = at("threadId") {
        return if kind == "session.row" { format!("rows {thread}") } else { format!("thread {thread}") };
    }
    if kind == "session.row" {
        return format!("rows {}", at("id").unwrap_or_default());
    }
    if let Some(turn) = at("turnId").or(at("sessionId")) {
        return format!("turn {turn}");
    }
    let family = kind.split('.').next().unwrap_or(kind);
    let workspace = at("workspaceId").or_else(|| e.pointer("/workspace/id").and_then(Value::as_str));
    if let (true, Some(w)) = (kind == "workspace.cost", workspace) {
        return format!("cost {w}");
    }
    match at(&format!("{family}Id")).or(at("id")).or(workspace) {
        Some(id) => format!("{family} {id}"),
        None => family.into(),
    }
}

/// A replay's answers in the form goldens are kept in: the replies in the order their requests were sent, numbering
/// ids as they first appear, then the events, each stream's in the order they arrived and the streams, which the TS
/// host cannot order against each other, one after another sorted by their text.
pub fn canonical(replies: &[Value], events: &[Value], facts: &Facts) -> Value {
    let mut facts = facts.clone();
    for r in replies {
        started_ports(r, &mut facts);
    }
    let mut n = Normalizer::new(facts);
    let replies: Vec<Value> = replies
        .iter()
        .map(|r| {
            n.refusal = refused(r);
            n.value(r)
        })
        .collect();
    n.refusal = false;
    n.numbering = false;
    let raw = events;
    let events: Vec<Value> = events.iter().map(|e| n.value(e)).collect();
    let mut again = Renumber::default();
    let replies: Vec<Value> = replies.iter().map(|r| again.value(r)).collect();
    again.frozen = true;
    let mut streams: Vec<(String, Vec<Value>)> = Vec::new();
    for (sent, e) in raw.iter().zip(events.iter().map(|e| again.value(e))) {
        let key = stream(sent);
        match streams.iter_mut().find(|(k, _)| *k == key) {
            Some((_, s)) => s.push(e),
            None => streams.push((key, vec![e])),
        }
    }
    let mut streams: Vec<Vec<Value>> = streams.into_iter().map(|(_, s)| s).collect();
    streams.sort_by_cached_key(|s| Value::Array(s.clone()).to_string());
    let events: Vec<Value> = streams.into_iter().flatten().collect();
    let mut out = Map::new();
    out.insert("replies".into(), Value::Array(replies));
    out.insert("events".into(), Value::Array(events));
    Value::Object(out)
}
