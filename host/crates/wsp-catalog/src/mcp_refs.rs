// SPDX-License-Identifier: AGPL-3.0-only
//! A config's server values written by name for a copy on another machine, and the guards that refuse a copy still
//! holding one: the helpers both reference writers (mcp_json_refer, mcp_toml_refer) share, and `servers_by_name`.
//! Port of packages/catalog/src/mcp.ts:159-456 at a3b870a6e; the spec is spec/mcp_refs.md.
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use indexmap::{IndexMap, IndexSet};
use serde_json::Value;
use std::collections::HashSet;
use std::rc::Rc;

use crate::js_regex::compiled;
use crate::jsonc::is_js_space;
use crate::jsonc_tree::{JsonPath, PathSeg};
use crate::mcp::{KnownValue, McpFormat, McpReferred, ReferredServer, WholeRef};
use crate::mcp_bearer::{bearer_of, mcp_bearer_variable};
use wsp_words::exit::HostError;

const BEARER_HEADER: &str = r"^Bearer\s+(\S.*)$";
const OAUTH_SECRETS: [&str; 4] = ["clientsecret", "refreshtoken", "accesstoken", "idtoken"];

/// `WSP_MCP_<server>_<header>` upper-cased, every character outside A-Z 0-9 _ an underscore (mcp.ts:181).
pub fn mcp_header_variable(server: &str, header: &str) -> String {
    let name = format!("WSP_MCP_{server}_{header}").to_uppercase();
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' {
            out.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                out.push('_');
            }
        }
    }
    out
}

/// Whether the value holds a `\n` or a `\r` (mcp.ts:184).
pub fn crosses_lines(value: &str) -> bool {
    value.contains(['\n', '\r'])
}

/// The value whole, then without a leading `bearer`, `basic` or `token` word and its whitespace, any case
/// (mcp.ts:190).
pub fn value_forms(value: &str) -> Vec<String> {
    let mut out = vec![value.to_string()];
    if let Some(rest) = value_rest(&Part::whole(value)) {
        out.push(value[rest].to_string());
    }
    out
}

/// Where the value form's text stands in `part`, for `/^(?:bearer|basic|token)\s+(.+)$/i` read without a regex, whose
/// `.+$` would scan to the end of the string at every level of a chain of scheme words: the word, a run of white
/// space, then everything to the end where no line end stands. Where nothing is left after the run, `\s+` gives back
/// its last character to `.+`, which takes it only if it is not a line end and the run has another.
fn value_rest(part: &Part) -> Option<std::ops::Range<usize>> {
    let s = part.as_str();
    let word = ["bearer", "basic", "token"]
        .into_iter()
        .find(|w| s.len() > w.len() && s.as_bytes()[..w.len()].eq_ignore_ascii_case(w.as_bytes()))?;
    let (mut at, mut run, mut last) = (word.len(), 0, word.len());
    for c in s[word.len()..].chars().take_while(|c| is_js_space(*c)) {
        last = at;
        at += c.len_utf8();
        run += 1;
    }
    if run == 0 || part.has_line_end(at..s.len()) {
        return None;
    }
    if at < s.len() {
        return Some(at..s.len());
    }
    let alone = s[last..].chars().next().is_some_and(|c| !is_line_end(c));
    (run >= 2 && alone).then_some(last..s.len())
}

fn is_line_end(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// Why a value typed for a catalog row's own variable does not travel as a server's (mcp.ts:374).
pub fn row_variable_line(name: &str, row: &str) -> String {
    format!("{name} belongs to the {row} key, so set it there or give the variable another name")
}

/// Names every machine's environment defines (mcp.ts:203).
pub const EVERY_MACHINE: [&str; 8] = ["HOME", "USER", "LOGNAME", "PATH", "PWD", "SHELL", "TMPDIR", "LANG"];

/// A part of a string a value can stand as whole: where, and its offset where the part is the string's own
/// characters rather than a decoding (mcp.ts:253). Offsets are UTF-16 units (rule 8.3) only if they leave the unit;
/// here they only slice the same string, so bytes serve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub text: Part,
    pub at: String,
    pub start: Option<usize>,
}

/// A stretch of a string, shared with every other stretch cut from it, so the parts a chain of scheme words leaves
/// (each the one before it less a word) cost no copy of the text: they would be quadratic in its length.
#[derive(Clone)]
pub struct Part {
    base: Rc<Base>,
    from: usize,
    to: usize,
}

/// A string and where its line ends stand, which is what tells whether a stretch of it holds one without a scan.
struct Base {
    text: String,
    line_ends: Vec<usize>,
}

impl Part {
    fn whole(text: &str) -> Part {
        let line_ends = text.char_indices().filter(|(_, c)| is_line_end(*c)).map(|(i, _)| i).collect();
        Part { base: Rc::new(Base { text: text.to_string(), line_ends }), from: 0, to: text.len() }
    }

    pub fn as_str(&self) -> &str {
        &self.base.text[self.from..self.to]
    }

    /// The part for `sub`, a slice of this part's own text.
    fn of(&self, sub: &str) -> Part {
        if sub.is_empty() {
            return Part { base: self.base.clone(), from: self.from, to: self.from };
        }
        let from = sub.as_ptr() as usize - self.as_str().as_ptr() as usize + self.from;
        Part { base: self.base.clone(), from, to: from + sub.len() }
    }

    /// Whether a line end stands in the stretch of this part at `range`.
    fn has_line_end(&self, range: std::ops::Range<usize>) -> bool {
        let ends = &self.base.line_ends;
        ends.partition_point(|&i| i < self.from + range.start) < ends.partition_point(|&i| i < self.from + range.end)
    }
}

impl std::ops::Deref for Part {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Part {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for Part {
    fn eq(&self, other: &Part) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Part {}

impl std::hash::Hash for Part {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl std::fmt::Debug for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

/// Every part of `s` a value can stand as whole, in the TS generator's order (mcp.ts:262): the string; a `--flag=v`'s
/// value; a header line's value; what follows a scheme word; each query value raw, then decoded where decoding changes
/// it. The empty string has none. Depth-first with a stack of the parts still to visit, so a string that nests by
/// its own text (`bearer bearer ...`) costs the stack nothing.
pub fn places(s: &str, at: &str, start: Option<usize>) -> Vec<Place> {
    let mut out = Vec::new();
    let mut work = vec![(Part::whole(s), at.to_string(), start)];
    while let Some((part, at, start)) = work.pop() {
        let s = part.as_str();
        if s.is_empty() {
            continue;
        }
        out.push(Place { text: part.clone(), at: at.clone(), start });
        let tail = |sub: &str| start.map(|st| st + s.len() - sub.len());
        let mut inner: Vec<(Part, String, Option<usize>)> = Vec::new();
        if let Some((flag, sub)) = flag_arg(s) {
            inner.push((part.of(sub), format!("the {flag} argument"), tail(sub)));
        }
        if let Some((name, sub)) = header_line(s) {
            inner.push((part.of(sub), format!("the {name} header"), tail(sub)));
        }
        if let Some(rest) = value_rest(&part) {
            let sub = &s[rest];
            inner.push((part.of(sub), at.clone(), tail(sub)));
        }
        if let Some((mut offset, query)) = address_query(s) {
            for pair in query.split('&') {
                let (key, raw) = match pair.find('=') {
                    Some(i) => (&pair[..i], &pair[i + 1..]),
                    None => (pair, ""),
                };
                let whose = format!("the {key} parameter of an address");
                inner.push((part.of(raw), whose.clone(), start.map(|st| st + offset + key.len() + 1)));
                let decoded = query_value(raw);
                if decoded != raw {
                    inner.push((Part::whole(&decoded), whose, None));
                }
                offset += pair.len() + 1;
            }
        }
        work.extend(inner.into_iter().rev());
    }
    out
}

/// The flag and its value, for `/^(-{1,2}[A-Za-z0-9][\w.-]*)=([^]+)$/` read without a regex: the regexes of the three
/// readers here would each cost a scan of the whole string at every level of a chain of `--a=`.
fn flag_arg(s: &str) -> Option<(&str, &str)> {
    let dashes = s.bytes().take_while(|&c| c == b'-').count();
    if !(1..=2).contains(&dashes) || !s.as_bytes().get(dashes)?.is_ascii_alphanumeric() {
        return None;
    }
    let name = dashes + s.as_bytes()[dashes..].iter().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-')).count();
    let value = s[name..].strip_prefix('=')?;
    (!value.is_empty()).then_some((&s[..name], value))
}

/// The header name and its value, for `/^([A-Za-z0-9-]+):[ \t]*([^/][^]*)$/`: where nothing but a `/` or the end follows
/// the spaces, the last space is given back to the value.
fn header_line(s: &str) -> Option<(&str, &str)> {
    let name = s.bytes().take_while(|c| c.is_ascii_alphanumeric() || *c == b'-').count();
    if name == 0 {
        return None;
    }
    let rest = s[name..].strip_prefix(':')?;
    let spaces = rest.bytes().take_while(|&c| c == b' ' || c == b'\t').count();
    let value = &rest[spaces..];
    if !value.is_empty() && !value.starts_with('/') {
        return Some((&s[..name], value));
    }
    (spaces > 0).then(|| (&s[..name], &rest[spaces - 1..]))
}

/// Where the query of an address starts and the query, for `/^[a-z][a-z0-9+.-]*:\/\/[^?#]*\?([^#]*)/i` read without a regex: fancy-regex runs a
/// pattern with a case-folded class by trying every start, which is a scan of the whole string at each level of a chain.
fn address_query(s: &str) -> Option<(usize, &str)> {
    let b = s.as_bytes();
    if !b.first()?.is_ascii_alphabetic() {
        return None;
    }
    let scheme = b.iter().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'.' | b'-')).count();
    let after = s[scheme..].strip_prefix("://")?;
    let from = scheme + 3 + after.find(['?', '#']).filter(|&i| after.as_bytes()[i] == b'?')? + 1;
    let query = &s[from..];
    Some((from, &query[..query.find('#').unwrap_or(query.len())]))
}

/// `decodeURIComponent` of a query value with `+` as a space; the raw text where it is not valid percent-encoding
/// (mcp.ts:243).
pub fn query_value(raw: &str) -> String {
    decode_uri_component(&raw.replace('+', " ")).unwrap_or_else(|| raw.to_string())
}

/// Refuses a copy whose definitions still hold a value taken out of them, whole in any place, or as its base64 with
/// padding, without, or url-safe (mcp.ts:288). The message names the server and where.
pub fn still_stands(entries: &[Value], servers: &[ReferredServer]) -> Result<(), HostError> {
    let mut strings = Vec::new();
    for entry in entries {
        strings_in(entry, &mut strings);
    }
    let spellings = |form: &str| -> [String; 3] {
        let encoded = STANDARD.encode(form.as_bytes());
        let b = encoded.trim_end_matches('=');
        [format!("{b}{}", "=".repeat((4 - b.len() % 4) % 4)), b.to_string(), b.replace('+', "-").replace('/', "_")]
    };
    let lengths: HashSet<usize> = servers
        .iter()
        .flat_map(|s| s.values.values())
        .flat_map(|v| value_forms(v))
        .flat_map(|form| spellings(&form).into_iter().map(|e| e.len()).chain([form.len()]).collect::<Vec<_>>())
        .collect();
    let mut held: IndexMap<Part, String> = IndexMap::new();
    for s in &strings {
        for p in places(s, "the file", None) {
            if lengths.contains(&p.text.len()) {
                held.entry(p.text).or_insert(p.at);
            }
        }
    }
    let where_ = |form: &str| -> Option<String> {
        let found = spellings(form).iter().find_map(|e| held.get(e.as_str()).cloned());
        held.get(form).cloned().or_else(|| found.map(|at| format!("{at}, as base64")))
    };
    for s in servers {
        for v in s.values.values() {
            if let Some(found) = value_forms(v).iter().find_map(|f| where_(f.as_str())) {
                return Err(HostError::new(format!(
                    "{}'s value still stands in {found} after it was written by name, so the file stays on this computer",
                    s.name
                )));
            }
        }
    }
    Ok(())
}

/// Where a known value stands whole in one of the string's places as its own characters, the outer place winning
/// where two overlap (mcp.ts:317); `(name, value, at, start, end)`.
pub fn known_in(s: &str, at: &str, known: &[KnownValue]) -> Vec<KnownHit> {
    let mut hits: Vec<KnownHit> = places(s, at, Some(0))
        .into_iter()
        .filter_map(|p| {
            let (name, value) = known.iter().find(|(_, v)| v.as_str() == p.text.as_str())?;
            let start = p.start?;
            Some(KnownHit { name: name.clone(), value: value.clone(), at: p.at, start, end: start + p.text.len() })
        })
        .collect();
    hits.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    let mut apart: Vec<KnownHit> = Vec::new();
    for h in hits {
        if h.start >= apart.last().map_or(0, |x| x.end) {
            apart.push(h);
        }
    }
    apart
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownHit {
    pub name: String,
    pub value: String,
    pub at: String,
    pub start: usize,
    pub end: usize,
}

/// The string with each hit replaced by `reference(name)` (mcp.ts:329).
pub fn spliced(s: &str, hits: &[KnownHit], reference: &dyn Fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut last = 0;
    for h in hits {
        out.push_str(&s[last..h.start]);
        out.push_str(&reference(&h.name));
        last = h.end;
    }
    out.push_str(&s[last..]);
    out
}

/// Known values in one of `server`'s strings, its own names taken first, refused where another server of the file
/// sets that name to another value (mcp.ts:341).
pub fn known_hits(
    s: &str,
    at: &str,
    known: &[KnownValue],
    servers: &[ReferredServer],
    server: &str,
    own: &IndexMap<String, String>,
) -> Result<Vec<KnownHit>, HostError> {
    let given: Vec<KnownValue> = known.iter().filter(|(_, v)| !v.is_empty()).cloned().collect();
    let ordered: Vec<KnownValue> =
        given.iter().filter(|(n, _)| own.contains_key(n)).chain(given.iter().filter(|(n, _)| !own.contains_key(n))).cloned().collect();
    let hits = known_in(s, at, &ordered);
    for h in &hits {
        let other = servers.iter().find(|x| x.values.get(&h.name).is_some_and(|v| *v != h.value));
        if let Some(other) = other {
            return Err(HostError::new(format!(
                "{server} passes the value servers.env holds as {} in {}, which {} sets to another value, so the file stays on this computer",
                h.name, h.at, other.name
            )));
        }
    }
    Ok(hits)
}

/// Refuses a server whose written strings still hold a known non-empty value (mcp.ts:353).
pub fn known_stands(strings: &[String], known: &[KnownValue], server: &str) -> Result<(), HostError> {
    let entries: Vec<Value> = strings.iter().map(|s| Value::String(s.clone())).collect();
    let values: IndexMap<String, String> = known.iter().filter(|(_, v)| !v.is_empty()).cloned().collect();
    still_stands(&entries, &[ReferredServer { name: server.to_string(), values, ..Default::default() }])
}

/// Each argument, command and address of a definition: `args[i]` and `command[i]` of a list, `command` as a string,
/// `url`, `httpUrl`; with where each stands ("an argument", "the command", "the address"), mcp.ts:359.
pub fn arg_strings(def: &serde_json::Map<String, Value>) -> Vec<(JsonPath, String, String)> {
    let mut out = Vec::new();
    for key in ["args", "command"] {
        match def.get(key) {
            Some(Value::Array(list)) => {
                for (i, v) in list.iter().enumerate() {
                    if let Value::String(s) = v {
                        let at = if key == "command" && i == 0 { "the command" } else { "an argument" };
                        out.push((vec![PathSeg::Key(key.to_string()), PathSeg::Index(i as i64)], s.clone(), at.to_string()));
                    }
                }
            }
            Some(Value::String(s)) if key == "command" => {
                out.push((vec![PathSeg::Key(key.to_string())], s.clone(), "the command".to_string()))
            }
            _ => {}
        }
    }
    for key in ["url", "httpUrl"] {
        if let Some(Value::String(s)) = def.get(key) {
            out.push((vec![PathSeg::Key(key.to_string())], s.clone(), "the address".to_string()));
        }
    }
    out
}

/// The minted variable of each header, refused where two would travel under one (mcp.ts:377).
pub fn header_variables(
    server: &str,
    headers: &[String],
    variable: &dyn Fn(&str) -> String,
) -> Result<IndexMap<String, String>, HostError> {
    let mut out = IndexMap::new();
    let mut seen: IndexMap<String, String> = IndexMap::new();
    for h in headers {
        let n = variable(h);
        if let Some(other) = seen.get(&n) {
            return Err(HostError::new(format!("{server} sends {other} and {h}, which would both travel as {n}; rename one")));
        }
        seen.insert(n.clone(), h.clone());
        out.insert(h.clone(), n);
    }
    Ok(out)
}

/// `Authorization: Bearer <token>` gives the token alone; nothing for any other header (mcp.ts:391).
pub fn bearer_token(header: &str, value: &str) -> Option<String> {
    if !header.eq_ignore_ascii_case("authorization") {
        return None;
    }
    let g = groups(BEARER_HEADER, "i", value)?;
    g.get(1).copied().flatten().map(str::to_string)
}

/// Each Codex header: the variable env_http_headers reads it by, the one the vault keeps it under, and that value
/// (mcp.ts:396).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexHeader {
    pub header: String,
    pub reads: String,
    pub keeps: String,
    pub value: String,
}

pub fn codex_headers(server: &str, headers: &IndexMap<String, String>) -> Result<Vec<CodexHeader>, HostError> {
    let names: Vec<String> = headers.keys().cloned().collect();
    let reads = header_variables(server, &names, &|h: &str| match headers.get(h).and_then(|v| bearer_token(h, v)) {
        None => mcp_header_variable(server, h),
        Some(_) => mcp_bearer_variable(&mcp_header_variable(server, h)),
    })?;
    Ok(headers
        .iter()
        .zip(reads.values())
        .map(|((header, value), reads)| CodexHeader {
            header: header.clone(),
            reads: reads.clone(),
            keeps: mcp_header_variable(server, header),
            value: bearer_token(header, value).unwrap_or_else(|| value.clone()),
        })
        .collect())
}

/// The keys of an `oauth` table that hold a secret, spelled any way (mcp.ts:222).
pub fn oauth_secret_keys(oauth: &serde_json::Map<String, Value>) -> Vec<String> {
    oauth.keys().filter(|k| OAUTH_SECRETS.contains(&squashed(k).as_str())).cloned().collect()
}

/// Whether a reference may read `name`: held outside, written by this file, its bearer's token held either way, or
/// every machine defines it outside an address (mcp.ts:207).
pub fn readable(outside: &IndexSet<String>, writes: &IndexSet<String>, name: &str, in_address: bool) -> bool {
    let held = |n: &str| outside.contains(n) || writes.contains(n);
    held(name) || bearer_of(name).is_some_and(|token| held(&token)) || (!in_address && EVERY_MACHINE.contains(&name))
}

/// One whole reference with no default to a readable name (mcp.ts:197).
pub fn refers(v: &str, named: &dyn Fn(&str) -> Option<WholeRef>, held: &dyn Fn(&str) -> bool) -> bool {
    named(v).is_some_and(|r| r.fallback.is_none() && held(&r.name))
}

/// Why a server reading a variable nobody holds travels nowhere (mcp.ts:214).
pub fn unread_line(path: &JsonPath) -> String {
    format!("reads a variable servers.env does not hold in {}, so it could not start there", path_text(path))
}

/// The refusal for a definition no reader maps (mcp.ts:217).
pub fn shape_refusal(path: &JsonPath) -> HostError {
    HostError::new(format!("{} is written in a shape wsp does not read, so its values cannot be written by name", path_text(path)))
}

/// A value servers.env holds and the server it was recorded for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldValue {
    pub value: String,
    pub by: String,
}

/// A value servers.env holds and every server recorded for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownEntry {
    pub value: String,
    pub by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByName {
    pub text: String,
    pub dropped: Vec<Dropped>,
}

/// A file's servers written by name for a copy (mcp.ts:414). `held` takes every value this file kept, on success
/// only; a server that clashes, crosses lines, sets a row's variable or reads what nobody holds is dropped with why.
pub fn servers_by_name(
    format: McpFormat,
    text: &str,
    held: &mut IndexMap<String, HeldValue>,
    row_of: &dyn Fn(&str) -> Option<String>,
    known: &IndexMap<String, KnownEntry>,
) -> Result<ByName, HostError> {
    let refer = |t: &str, only: Option<&str>, k: &[KnownValue], h: &IndexSet<String>| format.refer(t, only, k, h);
    servers_by_name_with(format, &refer, text, held, row_of, known)
}

/// A reference writer as `servers_by_name` calls it: `(text, only, known, held)`.
pub type Refer<'a> = &'a dyn Fn(&str, Option<&str>, &[KnownValue], &IndexSet<String>) -> Result<McpReferred, HostError>;

/// `servers_by_name` with the format's `refer` replaced, every other method the format's own: the TS tests' stand-in
/// format `{ ...FORMAT, refer }` (mcp-refer.test.ts:277, 405, 647, 688). `servers_by_name` is this with
/// `format.refer`.
pub fn servers_by_name_with(
    format: McpFormat,
    refer: Refer<'_>,
    text: &str,
    held: &mut IndexMap<String, HeldValue>,
    row_of: &dyn Fn(&str) -> Option<String>,
    known: &IndexMap<String, KnownEntry>,
) -> Result<ByName, HostError> {
    let known_values: Vec<KnownValue> = known.iter().map(|(name, entry)| (name.clone(), entry.value.clone())).collect();
    let outside: IndexSet<String> = known.keys().chain(held.keys()).cloned().collect();
    let found = refer(text, None, &known_values, &outside)?.servers;
    let mut dropped: Vec<(Dropped, Option<String>)> = Vec::new();
    let mut kept: IndexMap<String, (String, String)> = IndexMap::new();
    for s in &found {
        let owned = s.values.keys().find_map(|name| row_of(name).map(|row| (name.clone(), row)));
        let clash = s.values.iter().find(|(name, value)| match recorded(held, &kept, name) {
            Some((at_value, _)) => at_value != value.as_str(),
            None => known
                .get(name.as_str())
                .is_some_and(|vault| vault.value != value.as_str() && !(vault.by.len() == 1 && vault.by[0] == s.name)),
        });
        let tall = s.values.iter().find(|(_, value)| crosses_lines(value));
        let project = s.project.clone();
        if let Some(reason) = &s.unread {
            dropped.push((Dropped { name: s.name.clone(), reason: reason.clone() }, project));
        } else if let Some((name, row)) = owned {
            dropped.push((Dropped { name: s.name.clone(), reason: row_variable_line(&name, &row) }, project));
        } else if let Some((name, _)) = clash {
            let by: Vec<String> = match recorded(held, &kept, name) {
                Some((_, by)) => vec![by.to_string()],
                None => {
                    known.get(name.as_str()).map(|vault| vault.by.iter().filter(|b| **b != s.name).cloned().collect()).unwrap_or_default()
                }
            };
            dropped.push((Dropped { name: s.name.clone(), reason: clash_line(name, &by) }, project));
        } else if let Some((name, _)) = tall {
            let reason = format!("sets {name} to a value on more than one line, which cannot travel by name");
            dropped.push((Dropped { name: s.name.clone(), reason }, project));
        } else {
            for (name, value) in &s.values {
                if !held.contains_key(name) && !kept.contains_key(name) {
                    kept.insert(name.clone(), (value.clone(), s.name.clone()));
                }
            }
        }
    }
    let mut out = text.to_string();
    for (d, project) in &dropped {
        out = format.remove(&out, std::slice::from_ref(&d.name), project.as_deref())?;
    }
    if found.is_empty() {
        return Ok(ByName { text: text.to_string(), dropped: Vec::new() });
    }
    let mut current = refer(&out, None, &known_values, &outside)?;
    loop {
        let lost: Vec<(String, String, Option<String>)> =
            current.servers.iter().filter_map(|s| s.unread.clone().map(|reason| (s.name.clone(), reason, s.project.clone()))).collect();
        if lost.is_empty() {
            break;
        }
        for (name, reason, project) in lost {
            dropped.push((Dropped { name: name.clone(), reason }, project.clone()));
            out = format.remove(&out, std::slice::from_ref(&name), project.as_deref())?;
            kept.retain(|_, (_, by)| by.as_str() != name.as_str());
        }
        current = refer(&out, None, &known_values, &outside)?;
    }
    still_stands(&current.entries, &found)?;
    let written = |n: &str| outside.contains(n) || current.servers.iter().any(|s| s.values.contains_key(n));
    let secret = oauth_secrets_in(&Value::Array(current.entries.clone()), &|v: &str| refers(v, &|x: &str| format.named(x), &written))
        .into_iter()
        .next()
        .map(|path| path.split_once('.').map_or_else(|| path.clone(), |(_, rest)| rest.to_string()));
    if let Some(secret) = secret {
        return Err(HostError::new(format!(
            "a server still carries an OAuth secret in {secret} after it was written by name, so the file stays on this computer"
        )));
    }
    for (name, (value, by)) in kept {
        held.insert(name, HeldValue { value, by });
    }
    Ok(ByName { text: current.text, dropped: dropped.into_iter().map(|(d, _)| d).collect() })
}

/// Why a server is left out of a copy for setting a name `by` already set to another value; servers.env's own where
/// it recorded no server for the name.
fn clash_line(name: &str, by: &[String]) -> String {
    if by.is_empty() {
        return format!("sets {name}, which servers.env already holds with another value and no server recorded for it");
    }
    let verb = if by.len() > 1 { "set" } else { "sets" };
    format!("sets {name}, which {} already {verb} to another value", by.join(" and "))
}

/// The value and the server a name was recorded for, in the file's holdings first and the copy's kept ones after.
fn recorded<'a>(
    held: &'a IndexMap<String, HeldValue>,
    kept: &'a IndexMap<String, (String, String)>,
    name: &str,
) -> Option<(&'a str, &'a str)> {
    match held.get(name) {
        Some(h) => Some((h.value.as_str(), h.by.as_str())),
        None => kept.get(name).map(|(value, by)| (value.as_str(), by.as_str())),
    }
}

/// The capture groups of a JavaScript pattern against `s`, group 0 first; nothing where it does not match.
fn groups<'a>(source: &'static str, flags: &'static str, s: &'a str) -> Option<Vec<Option<&'a str>>> {
    let caps = compiled(source, flags).re.captures(s).ok()??;
    Some((0..caps.len()).map(|i| caps.get(i).map(|m| m.as_str())).collect())
}

/// `decodeURIComponent`: each `%XX` a byte, the bytes read as UTF-8; nothing where a percent sign is not followed by
/// two hex digits or the bytes are not UTF-8.
fn decode_uri_component(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let digits = bytes.get(i + 1..i + 3)?;
            if !digits.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            out.push(u8::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn strings_in(v: &Value, out: &mut Vec<String>) {
    let mut work = vec![v];
    while let Some(x) = work.pop() {
        match x {
            Value::String(s) => out.push(s.clone()),
            Value::Array(items) => work.extend(items.iter().rev()),
            Value::Object(map) => work.extend(map.values().rev()),
            _ => {}
        }
    }
}

fn squashed(key: &str) -> String {
    key.to_lowercase().chars().filter(|c| *c != '-' && *c != '_').collect()
}

fn path_text(path: &JsonPath) -> String {
    path.iter()
        .map(|seg| match seg {
            PathSeg::Key(key) => key.clone(),
            PathSeg::Index(i) => i.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// Where a definition still holds an OAuth secret, as dotted paths (mcp.ts:226), in the order a walk down the tree
/// meets them. The path is one stack the walk cuts back as it climbs, so no level copies it.
pub fn oauth_secrets_in(v: &Value, reference: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut found = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut work: Vec<(&Value, usize, Option<(String, bool)>)> = vec![(v, 0, None)];
    while let Some((x, depth, step)) = work.pop() {
        path.truncate(depth);
        if let Some((segment, oauth)) = step {
            path.push(segment);
            if oauth {
                found.extend(held_in(x, reference, &path));
            }
        }
        match x {
            Value::Array(items) => work.extend(items.iter().enumerate().rev().map(|(i, y)| (y, path.len(), Some((i.to_string(), false))))),
            Value::Object(map) => work.extend(map.iter().rev().map(|(k, y)| (y, path.len(), Some((k.clone(), squashed(k) == "oauth"))))),
            _ => {}
        }
    }
    found
}

/// The secrets an `oauth` value holds, written as a table or as a list of tables.
fn held_in(x: &Value, reference: &dyn Fn(&str) -> bool, at: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    let mut path = at.to_vec();
    let mut work: Vec<(&Value, usize, Option<usize>)> = vec![(x, path.len(), None)];
    while let Some((y, depth, index)) = work.pop() {
        path.truncate(depth);
        path.extend(index.map(|i| i.to_string()));
        match y {
            Value::Array(items) => work.extend(items.iter().enumerate().rev().map(|(i, z)| (z, path.len(), Some(i)))),
            Value::Object(map) => {
                for secret in oauth_secret_keys(map)
                    .into_iter()
                    .filter(|s| !matches!(map.get(s), Some(Value::String(v)) if v.is_empty() || reference(v)))
                {
                    path.push(secret);
                    found.push(path.join("."));
                    path.pop();
                }
            }
            _ => {}
        }
    }
    found
}
