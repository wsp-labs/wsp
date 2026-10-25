// SPDX-License-Identifier: AGPL-3.0-only
//! Codex's TOML, read and edited line by line in the table form Codex documents, so comments and order survive. Port of
//! packages/catalog/src/mcp.ts:1257-1757 less the reference writer, at a3b870a6e; the spec is spec/mcp_toml.md.
use std::collections::HashSet;
use std::sync::OnceLock;

use indexmap::IndexMap;
use regex::Regex;

use crate::jsonc::js_string;
use crate::mcp::{
    EditOutcome, McpEditResult, McpEditScope, McpEdited, McpMergeResult, McpMergeScope, McpMerged, McpReads, McpServer,
    McpServerScope, McpTransport, MergeOutcome, RewriteString,
};
use wsp_words::exit::HostError;
use wsp_words::js::js_trim;

/// A line without its trailing comment: `#` outside quotes ends it (mcp.ts:1263).
pub fn uncomment_toml(line: &str) -> String {
    let mut quote: Option<char> = None;
    let mut chars = line.char_indices();
    while let Some((i, c)) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' && q == '"' {
                    chars.next();
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' => return line[..i].to_string(),
            None => {}
        }
    }
    line.to_string()
}

fn header_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"^\[\s*mcp_servers\s*\.\s*(?:"([^"]+)"|'([^']+)'|([A-Za-z0-9_-]+))\s*(?:\.\s*([A-Za-z0-9_-]+)\s*)?\]$"#)
            .expect("a fixed pattern")
    })
}

/// `[mcp_servers.<name>]` or `[mcp_servers.<name>.<sub>]`, the name bare or quoted; nothing for another header
/// (mcp.ts:1277).
pub fn toml_header(line: &str) -> Option<(String, Option<String>)> {
    let caps = header_re().captures(line)?;
    let name = caps.get(1).or(caps.get(2)).or(caps.get(3))?.as_str().to_string();
    Some((name, caps.get(4).map(|m| m.as_str().to_string())))
}

fn string_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#""((?:[^"\\]|\\.)*)"|'([^']*)'"#).expect("a fixed pattern"))
}

fn decode_toml(s: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\\(u[0-9A-Fa-f]{4}|U[0-9A-Fa-f]{8}|.)").expect("a fixed pattern"));
    re.replace_all(s, |caps: &regex::Captures<'_>| {
        let whole = caps.get(0).map_or("", |m| m.as_str());
        let e = caps.get(1).map_or("", |m| m.as_str());
        let decoded = match e.chars().next() {
            Some('u') | Some('U') => u32::from_str_radix(&e[1..], 16).ok().and_then(char::from_u32).map(String::from),
            _ => toml_escape(e).map(String::from),
        };
        decoded.unwrap_or_else(|| whole.to_string())
    })
    .into_owned()
}

/// The character a backslash and one escape letter stand for (mcp.ts:1288).
fn toml_escape(e: &str) -> Option<char> {
    match e {
        "b" => Some('\u{8}'),
        "t" => Some('\t'),
        "n" => Some('\n'),
        "f" => Some('\u{c}'),
        "r" => Some('\r'),
        "\"" => Some('"'),
        "\\" => Some('\\'),
        _ => None,
    }
}

fn encode_toml(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let back = match c {
            '\u{8}' => Some("b"),
            '\t' => Some("t"),
            '\n' => Some("n"),
            '\u{c}' => Some("f"),
            '\r' => Some("r"),
            '"' => Some("\""),
            '\\' => Some("\\"),
            _ => None,
        };
        if let Some(b) = back {
            out.push('\\');
            out.push_str(b);
        } else if (c as u32) <= 0x1f || c as u32 == 0x7f {
            out.push_str(&format!("\\u{:04x}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// Every string in the text, basic strings decoded (mcp.ts:1306).
pub fn toml_strings(s: &str) -> Vec<String> {
    string_re()
        .captures_iter(s)
        .map(|m| match m.get(1) {
            Some(basic) => decode_toml(basic.as_str()),
            None => m.get(2).map_or_else(String::new, |g| g.as_str().to_string()),
        })
        .collect()
}

/// Whether a value read so far is an array closing on a later line (mcp.ts:1311).
pub fn toml_open_array(value: &str) -> bool {
    value.starts_with('[') && !value.ends_with(']')
}

/// A value the line reader keeps: a string, a list of strings, or an inline table of strings (mcp.ts:1316).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TomlValue {
    Str(String),
    List(Vec<String>),
    Table(IndexMap<String, String>),
}

/// Nothing for any other value (mcp.ts:1316).
pub fn toml_value(raw: &str) -> Option<TomlValue> {
    static TABLE: OnceLock<Regex> = OnceLock::new();
    let s = js_trim(raw);
    if s.starts_with('[') {
        return Some(TomlValue::List(toml_strings(s)));
    }
    if s.starts_with('{') {
        let re = TABLE.get_or_init(|| {
            Regex::new(r#"(?:"([^"]+)"|'([^']+)'|([A-Za-z0-9_-]+))\s*=\s*("(?:[^"\\]|\\.)*"|'[^']*')"#).expect("a fixed pattern")
        });
        let mut out = IndexMap::new();
        for m in re.captures_iter(js_slice_inner(s)) {
            let name = m.get(1).or(m.get(2)).or(m.get(3)).map_or("", |g| g.as_str());
            let value = m.get(4).and_then(|v| toml_strings(v.as_str()).into_iter().next()).unwrap_or_default();
            out.insert(name.to_string(), value);
        }
        return Some(TomlValue::Table(out));
    }
    if s.starts_with('"') || s.starts_with('\'') {
        return toml_strings(s).into_iter().next().map(TomlValue::Str);
    }
    None
}

/// `s.slice(1, -1)`: the text without its first and last character.
fn js_slice_inner(s: &str) -> &str {
    let start = s.char_indices().nth(1).map_or(s.len(), |(i, _)| i);
    let end = s.char_indices().last().map_or(0, |(i, _)| i);
    if end <= start {
        ""
    } else {
        &s[start..end]
    }
}

/// A TOML basic string as `JSON.stringify` writes it (mcp.ts:1388).
pub fn toml_string(s: &str) -> String {
    js_string(s)
}

/// `{ "k" = "v", ... }` (mcp.ts:1391).
pub fn toml_inline(d: &IndexMap<String, String>) -> String {
    let body: Vec<String> = d.iter().map(|(k, v)| format!("{} = {}", toml_string(k), toml_string(v))).collect();
    format!("{{ {} }}", body.join(", "))
}

/// A line with every string on it put through `to`, re-encoded only where changed (mcp.ts:1423).
pub fn rewrite_toml_line(line: &str, to: &dyn Fn(&str) -> String) -> String {
    let code = uncomment_toml(line);
    let mut out = String::new();
    let mut last = 0;
    for caps in string_re().captures_iter(&code) {
        let whole = caps.get(0).expect("group 0 always matches");
        out.push_str(&code[last..whole.start()]);
        last = whole.end();
        match (caps.get(1), caps.get(2)) {
            (_, Some(literal)) => {
                let value = to(literal.as_str());
                if value == literal.as_str() {
                    out.push_str(whole.as_str());
                } else {
                    out.push_str(&format!("'{value}'"));
                }
            }
            (basic, None) => {
                let value = decode_toml(basic.map_or("", |b| b.as_str()));
                let rewritten = to(&value);
                if rewritten == value {
                    out.push_str(whole.as_str());
                } else {
                    out.push_str(&format!("\"{}\"", encode_toml(&rewritten)));
                }
            }
        }
    }
    out.push_str(&code[last..]);
    out.push_str(&line[code.len()..]);
    out
}

/// The server each line belongs to (mcp.ts:1440).
pub fn codex_owners(lines: &[&str]) -> Vec<Option<String>> {
    let mut owner = Vec::with_capacity(lines.len());
    let mut current: Option<String> = None;
    for raw in lines {
        let t = js_trim(&uncomment_toml(raw)).to_string();
        if t.starts_with('[') {
            current = toml_header(&t).map(|h| h.0);
        }
        owner.push(current.clone());
    }
    owner
}

/// Where one server's tables sit in the text, in file order (mcp.ts:1453).
fn codex_block(lines: &[&str], name: &str, trim: bool) -> Vec<usize> {
    let owner = codex_owners(lines);
    let mut at: Vec<usize> = (0..lines.len()).filter(|&i| owner[i].as_deref() == Some(name)).collect();
    if trim {
        while at.last().is_some_and(|&i| js_trim(lines[i]).is_empty()) {
            at.pop();
        }
    }
    at
}

/// One server's tables in the one shape they keep when Codex writes the file again (mcp.ts:1462).
fn codex_entry(lines: &[&str]) -> String {
    lines.iter().map(|l| js_trim(&uncomment_toml(l)).to_string()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// The command one server's tables name, as the machine will run it (mcp.ts:1469).
fn codex_command(lines: &[&str]) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^\s*command\s*=\s*").expect("a fixed pattern"));
    for line in lines {
        let code = uncomment_toml(line);
        if let Some(m) = re.find(&code) {
            return toml_strings(&code[m.end()..]).into_iter().next();
        }
    }
    None
}

/// A kept server's line with every string on it as the machine reads it (mcp.ts:1479).
fn codex_rewrite(lib: RewriteString<'_>, line: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^\s*command\s*=").expect("a fixed pattern"));
    let command = re.is_match(&uncomment_toml(line));
    rewrite_toml_line(line, &|value| lib(value, command))
}

/// The table replaced in place or appended after a blank line (mcp.ts:1407).
pub fn place_codex(text: Option<&str>, name: &str, server: &McpTransport) -> String {
    static BARE: OnceLock<Regex> = OnceLock::new();
    let bare = BARE.get_or_init(|| Regex::new(r"^[A-Za-z0-9_-]+$").expect("a fixed pattern"));
    let shown = if bare.is_match(name) { name.to_string() } else { toml_string(name) };
    let header = format!("[mcp_servers.{shown}]");
    let mut table_lines = vec![header.clone()];
    table_lines.extend(codex_lines(server));
    let table = format!("{}\n", table_lines.join("\n"));
    let Some(text) = text.filter(|t| !js_trim(t).is_empty()) else {
        return table;
    };
    let lines: Vec<&str> = text.split('\n').collect();
    let Some(start) = lines.iter().position(|l| js_trim(l) == header) else {
        let sep = if text.ends_with('\n') { "" } else { "\n" };
        return format!("{text}{sep}\n{table}");
    };
    let mut end = start + 1;
    while end < lines.len() && !lines[end].trim_start().starts_with('[') {
        end += 1;
    }
    let gap: Vec<&str> = if end < lines.len() { vec![""] } else { Vec::new() };
    let body: Vec<&str> = table[..table.len() - 1].split('\n').collect();
    let mut out: Vec<&str> = Vec::new();
    out.extend(&lines[..start]);
    out.extend(body);
    out.extend(gap);
    out.extend(&lines[end..]);
    out.join("\n")
}

/// The lines of a server's own table under its header (mcp.ts:1395).
fn codex_lines(server: &McpTransport) -> Vec<String> {
    match server {
        McpTransport::Http { url, headers } => {
            let mut out = vec![format!("url = {}", toml_string(url))];
            if !headers.is_empty() {
                out.push(format!("http_headers = {}", toml_inline(headers)));
            }
            out
        }
        McpTransport::Stdio { command, args, env, tool_timeout_sec, .. } => {
            let mut out = vec![
                format!("command = {}", toml_string(command)),
                format!("args = [{}]", args.iter().map(|a| toml_string(a)).collect::<Vec<_>>().join(", ")),
            ];
            if !env.is_empty() {
                out.push(format!("env = {}", toml_inline(env)));
            }
            if let Some(t) = tool_timeout_sec {
                out.push(format!("tool_timeout_sec = {t}"));
            }
            out
        }
    }
}

#[derive(Default)]
struct CodexTable {
    values: IndexMap<String, TomlValue>,
    disabled: bool,
    env: IndexMap<String, String>,
    headers: IndexMap<String, String>,
    env_headers: IndexMap<String, String>,
}

/// The `[mcp_servers.*]` tables and their sub-tables, in file order (mcp.ts:1339).
pub fn read_codex(text: &str) -> Vec<McpServer> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut tables: IndexMap<String, CodexTable> = IndexMap::new();
    let mut current: Option<(String, Option<String>)> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = js_trim(&uncomment_toml(lines[i])).to_string();
        i += 1;
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            current = toml_header(&line);
            if let Some((name, _)) = &current {
                tables.entry(name.clone()).or_default();
            }
            continue;
        }
        let Some((name, sub)) = current.clone() else {
            continue;
        };
        let Some(eq) = line.find('=') else {
            continue;
        };
        let key = unquote_key(js_trim(&line[..eq]));
        let mut value = js_trim(&line[eq + 1..]).to_string();
        while toml_open_array(&value) && i < lines.len() {
            value.push_str(js_trim(&uncomment_toml(lines[i])));
            i += 1;
        }
        let table = tables.entry(name).or_default();
        if sub.is_none() && key == "enabled" && value == "false" {
            table.disabled = true;
        }
        let Some(parsed) = toml_value(&value) else {
            continue;
        };
        match sub.as_deref() {
            None => {
                table.values.insert(key, parsed);
            }
            Some(sub) => {
                if let TomlValue::Str(s) = parsed {
                    match sub {
                        "env" => {
                            table.env.insert(key, s);
                        }
                        "http_headers" => {
                            table.headers.insert(key, s);
                        }
                        "env_http_headers" => {
                            table.env_headers.insert(key, s);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for (name, t) in tables {
        let command = str_of(t.values.get("command"));
        let url = str_of(t.values.get("url"));
        let bearer = str_of(t.values.get("bearer_token_env_var"));
        let mut env_headers = table_of(t.values.get("env_http_headers"));
        for (k, v) in &t.env_headers {
            env_headers.insert(k.clone(), v.clone());
        }
        let mut env_refs: Vec<String> = bearer.iter().cloned().collect();
        env_refs.extend(env_headers.values().cloned());
        let disabled = t.disabled.then_some(true);
        let passed = list_of(t.values.get("env_vars"));
        let reads = (!passed.is_empty() || !env_headers.is_empty() || bearer.is_some())
            .then(|| McpReads { env: passed, headers: env_headers.clone(), bearer: bearer.clone() });
        if let Some(command) = command {
            let mut env = table_of(t.values.get("env"));
            for (k, v) in &t.env {
                env.insert(k.clone(), v.clone());
            }
            out.push(McpServer {
                name,
                scope: McpServerScope::User,
                transport: McpTransport::Stdio { command, args: list_of(t.values.get("args")), env, cwd: None, tool_timeout_sec: None },
                env_refs,
                reads,
                disabled,
            });
        } else if let Some(url) = url {
            let mut headers = table_of(t.values.get("http_headers"));
            for (k, v) in &t.headers {
                headers.insert(k.clone(), v.clone());
            }
            out.push(McpServer { name, scope: McpServerScope::User, transport: McpTransport::Http { url, headers }, env_refs, reads, disabled });
        }
    }
    out
}

/// A key with its quotes taken off where the whole key is one quoted string (mcp.ts:1297).
pub(crate) fn unquote_key(key: &str) -> String {
    if key.len() >= 2 && key.starts_with('"') && key.ends_with('"') {
        key[1..key.len() - 1].to_string()
    } else {
        key.to_string()
    }
}

fn str_of(v: Option<&TomlValue>) -> Option<String> {
    match v {
        Some(TomlValue::Str(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn list_of(v: Option<&TomlValue>) -> Vec<String> {
    match v {
        Some(TomlValue::List(items)) => items.clone(),
        _ => Vec::new(),
    }
}

fn table_of(v: Option<&TomlValue>) -> IndexMap<String, String> {
    match v {
        Some(TomlValue::Table(t)) => t.clone(),
        _ => IndexMap::new(),
    }
}

/// The editor (mcp.ts:1484).
pub fn codex_editor(lib: RewriteString<'_>, scope: &McpEditScope, before: &str) -> McpEdited {
    let lines: Vec<&str> = before.split('\n').collect();
    let owner = codex_owners(&lines);
    let keep: HashSet<&str> = scope.keep.iter().map(String::as_str).collect();
    let drop: HashSet<&str> = scope.drop.iter().map(String::as_str).collect();
    let mut out: Vec<String> = Vec::new();
    let mut out_owner: Vec<Option<String>> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let o = owner[i].clone();
        if o.as_deref().is_some_and(|n| drop.contains(n)) {
            continue;
        }
        let kept = o.as_deref().is_some_and(|n| keep.contains(n));
        out.push(if kept { codex_rewrite(lib, line) } else { line.to_string() });
        out_owner.push(o);
    }
    let mut results = Vec::new();
    for name in &scope.keep {
        if !owner.iter().any(|o| o.as_deref() == Some(name.as_str())) {
            results.push(McpEditResult { name: name.clone(), outcome: EditOutcome::Missing, command: None });
            continue;
        }
        let mine: Vec<&str> = out
            .iter()
            .zip(&out_owner)
            .filter(|(_, o)| o.as_deref() == Some(name.as_str()))
            .map(|(l, _)| l.as_str())
            .collect();
        results.push(McpEditResult { name: name.clone(), outcome: EditOutcome::Written, command: codex_command(&mine) });
    }
    for name in &scope.drop {
        results.push(McpEditResult { name: name.clone(), outcome: EditOutcome::Dropped, command: None });
    }
    McpEdited { text: out.join("\n"), results }
}

/// The block in place of the lines it stands on, which for an empty block is those lines taken out (mcp.ts:1521).
fn over(lines: &mut Vec<String>, at: &[usize], block: &[String]) {
    let first = at.first().copied();
    let mut next = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if Some(i) == first {
            next.extend(block.iter().cloned());
        } else if !at.contains(&i) {
            next.push(line.clone());
        }
    }
    *lines = next;
}

/// The merge (mcp.ts:1512).
pub fn codex_merge(lib: RewriteString<'_>, scope: &McpMergeScope, own: Option<&str>, travelled: &str) -> McpMerged {
    let from: Vec<&str> = travelled.split('\n').collect();
    let replace: HashSet<&str> = scope.replace.iter().map(String::as_str).collect();
    let mut lines: Vec<String> = own.map_or_else(Vec::new, |t| t.split('\n').map(str::to_string).collect());
    let mut results = Vec::new();
    let mut wrote = false;
    for name in &scope.keep {
        let at = codex_block(&from, name, true);
        if at.is_empty() {
            results.push(McpMergeResult { name: name.clone(), outcome: MergeOutcome::Missing, command: None });
            continue;
        }
        let block: Vec<String> = at.iter().map(|&i| codex_rewrite(lib, from[i])).collect();
        let block_refs: Vec<&str> = block.iter().map(String::as_str).collect();
        let command = codex_command(&block_refs);
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let here = codex_block(&refs, name, true);
        let outcome = if here.is_empty() {
            let mut kept = lines.clone();
            while kept.last().is_some_and(|l| js_trim(l).is_empty()) {
                kept.pop();
            }
            lines = if kept.is_empty() {
                let mut v = block.clone();
                v.push(String::new());
                v
            } else {
                let mut v = kept;
                v.push(String::new());
                v.extend(block.iter().cloned());
                v.push(String::new());
                v
            };
            wrote = true;
            MergeOutcome::Added
        } else {
            let here_lines: Vec<&str> = here.iter().map(|&i| refs[i]).collect();
            if codex_entry(&here_lines) == codex_entry(&block_refs) {
                MergeOutcome::Same
            } else if !replace.contains(name.as_str()) {
                MergeOutcome::Theirs
            } else {
                over(&mut lines, &here, &block);
                wrote = true;
                MergeOutcome::Replaced
            }
        };
        results.push(McpMergeResult { name: name.clone(), outcome, command });
    }
    for name in &scope.drop {
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let here = codex_block(&refs, name, false);
        if here.is_empty() || !replace.contains(name.as_str()) {
            results.push(McpMergeResult { name: name.clone(), outcome: MergeOutcome::Left, command: None });
            continue;
        }
        over(&mut lines, &here, &[]);
        wrote = true;
        results.push(McpMergeResult { name: name.clone(), outcome: MergeOutcome::Dropped, command: None });
    }
    let text = if wrote { lines.join("\n") } else { own.unwrap_or("").to_string() };
    McpMerged { text, results }
}

/// The remove (mcp.ts:1564).
pub fn remove_codex(text: &str, names: &[String]) -> String {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let mut took = false;
    for name in names {
        let at: HashSet<usize> = codex_block(&lines, name, false).into_iter().collect();
        if at.is_empty() {
            continue;
        }
        lines = lines.iter().enumerate().filter(|(i, _)| !at.contains(i)).map(|(_, l)| *l).collect();
        took = true;
    }
    if took {
        lines.join("\n")
    } else {
        text.to_string()
    }
}

/// The switch; errors "the file names no server called <name>" (mcp.ts:1578).
pub fn enable_codex(text: &str, name: &str, on: bool) -> Result<String, HostError> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let header_of = |l: &str| -> Option<(String, Option<String>)> {
        let t = js_trim(&uncomment_toml(l)).to_string();
        if t.starts_with('[') {
            toml_header(&t)
        } else {
            None
        }
    };
    let start = lines
        .iter()
        .position(|l| matches!(header_of(l), Some((ref n, None)) if n == name))
        .ok_or_else(|| HostError::new(format!("the file names no server called {name}")))?;
    let mut end = start + 1;
    while end < lines.len() && !js_trim(&uncomment_toml(&lines[end])).starts_with('[') {
        end += 1;
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"^\s*enabled\s*=").expect("a fixed pattern"));
    let at = (start + 1..end).find(|&i| re.is_match(&uncomment_toml(&lines[i])));
    let next: Vec<String> = match (on, at) {
        (true, Some(at)) => lines.into_iter().enumerate().filter(|(i, _)| *i != at).map(|(_, l)| l).collect(),
        (true, None) => lines,
        (false, Some(at)) => {
            lines[at] = "enabled = false".to_string();
            lines
        }
        (false, None) => {
            lines.insert(start + 1, "enabled = false".to_string());
            lines
        }
    };
    Ok(next.join("\n"))
}

/// `only` (mcp.ts:1741).
pub fn only_codex(text: &str, names: &[String]) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let blocks: Vec<String> = names
        .iter()
        .map(|name| codex_block(&lines, name, true).iter().map(|&i| lines[i]).collect::<Vec<_>>().join("\n"))
        .filter(|b| !b.is_empty())
        .collect();
    blocks.iter().map(|b| format!("{b}\n")).collect::<Vec<_>>().join("\n")
}

/// `entryOf` (mcp.ts:1746).
pub fn entry_of_codex(text: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let at = codex_block(&lines, name, true);
    if at.is_empty() {
        None
    } else {
        Some(codex_entry(&at.iter().map(|&i| lines[i]).collect::<Vec<_>>()))
    }
}
