// SPDX-License-Identifier: AGPL-3.0-only
//! The three JSON formats' reads and edits: Claude Code's `mcpServers` (with `projects`), Gemini CLI's settings (with
//! `mcp.excluded`) and OpenCode's `mcp`. Port of packages/catalog/src/mcp.ts:470-1255 less the reference writer, at
//! a3b870a6e; the spec is spec/mcp_json.md. Every function takes one of the three JSON formats and panics on Codex's.
use std::collections::HashSet;
use std::sync::{LazyLock, OnceLock};

use indexmap::{IndexMap, IndexSet};
use regex::Regex;
use serde_json::{json, Map, Value};

use crate::json_edit::{edit_jsonc, json_canonical, set_at, JsonEdit};
use crate::jsonc::{js_stringify_pretty, json_object, map_strings, read_jsonc};
use crate::jsonc_tree::{JsonPath, PathSeg};
use crate::mcp::{
    EditOutcome, McpEditResult, McpEditScope, McpEdited, McpFormat, McpMergeResult, McpMergeScope, McpMerged, McpResolved, McpServer,
    McpServerScope, McpTransport, MergeOutcome, RewriteString, WholeRef,
};
use crate::mcp_travelled::travelled_servers;
use crate::plugins::js_entries;
use wsp_words::exit::HostError;
use wsp_words::js::{js_trim, JS_WS as WS};

const CLAUDE_REF: &str = r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-[^}]*)?\}";
const GEMINI_REF: &str = r"\$(?:\{([A-Za-z_][A-Za-z0-9_]*)(?::-[^}]*)?\}|([A-Za-z_][A-Za-z0-9_]*))";
const OPENCODE_REF: &str = r"\{env:([A-Za-z_][A-Za-z0-9_]*)\}";

/// The members every JavaScript object inherits, which `servers[name]` finds where the file holds no such key.
const INHERITED: [&str; 12] = [
    "constructor",
    "__defineGetter__",
    "__defineSetter__",
    "hasOwnProperty",
    "__lookupGetter__",
    "__lookupSetter__",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toString",
    "valueOf",
    "__proto__",
    "toLocaleString",
];
static INHERITED_FUNCTION: Value = Value::Null;
static INHERITED_PROTOTYPE: LazyLock<Value> = LazyLock::new(|| Value::Object(Map::new()));

/// `table[name]` on a plain JavaScript object: the file's own key, else a member `Object.prototype` gives every
/// object (`null` for a function, `{}` for `__proto__`), so a server named `constructor` is held as the TS holds it.
fn held_at<'a>(table: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    table.get(name).or_else(|| match INHERITED.iter().find(|&&member| member == name) {
        Some(&"__proto__") => Some(&INHERITED_PROTOTYPE),
        Some(_) => Some(&INHERITED_FUNCTION),
        None => None,
    })
}

const NOT_JSON: &str = "the file is not valid JSON; add the server by hand";
const NOT_OBJECT: &str = "the file is not a JSON object; add the server by hand";

/// The key a format's servers sit under: `mcpServers` or `mcp` (mcp.ts:1184, 1229).
pub fn servers_key(format: McpFormat) -> &'static str {
    match format {
        McpFormat::OpencodeJson => "mcp",
        _ => "mcpServers",
    }
}

/// The variable pattern a format's agent expands (mcp.ts:1208, 1212, 1223); the three are the JSON formats.
pub(crate) fn ref_re(format: McpFormat) -> &'static Regex {
    static CLAUDE: OnceLock<Regex> = OnceLock::new();
    static GEMINI: OnceLock<Regex> = OnceLock::new();
    static OPENCODE: OnceLock<Regex> = OnceLock::new();
    match format {
        McpFormat::McpServersJson => CLAUDE.get_or_init(|| Regex::new(CLAUDE_REF).expect("a fixed pattern")),
        McpFormat::GeminiSettingsJson => GEMINI.get_or_init(|| Regex::new(GEMINI_REF).expect("a fixed pattern")),
        _ => OPENCODE.get_or_init(|| Regex::new(OPENCODE_REF).expect("a fixed pattern")),
    }
}

/// The variable pattern anchored to a whole string, as `^(?:<source>)$` (mcp.ts:910).
fn whole_re(format: McpFormat) -> &'static Regex {
    static CLAUDE: OnceLock<Regex> = OnceLock::new();
    static GEMINI: OnceLock<Regex> = OnceLock::new();
    static OPENCODE: OnceLock<Regex> = OnceLock::new();
    let (cell, source) = match format {
        McpFormat::McpServersJson => (&CLAUDE, CLAUDE_REF),
        McpFormat::GeminiSettingsJson => (&GEMINI, GEMINI_REF),
        _ => (&OPENCODE, OPENCODE_REF),
    };
    cell.get_or_init(|| Regex::new(&format!("^(?:{source})$")).expect("a fixed pattern"))
}

pub(crate) fn has_projects(format: McpFormat) -> bool {
    format != McpFormat::OpencodeJson
}

/// The key an entry's command variables sit under (mcp.ts:1229).
pub(crate) fn env_key(format: McpFormat) -> &'static str {
    match format {
        McpFormat::OpencodeJson => "environment",
        _ => "env",
    }
}

/// The keys of a server's `oauth` table whose secret the agent reads with the format's variables (mcp.ts:1212, 1229).
pub(crate) fn oauth_secret_names(format: McpFormat) -> &'static [&'static str] {
    match format {
        McpFormat::GeminiSettingsJson => &["clientSecret"],
        McpFormat::OpencodeJson => &["clientSecret", "client_secret"],
        _ => &[],
    }
}

/// The names Gemini CLI's `mcp.excluded` switches off (mcp.ts:1214); none for the other JSON formats.
pub(crate) fn switched_off(format: McpFormat, root: &Map<String, Value>) -> Vec<String> {
    if format != McpFormat::GeminiSettingsJson {
        return Vec::new();
    }
    let excluded = root.get("mcp").and_then(json_object).and_then(|m| m.get("excluded"));
    strs(excluded)
}

/// The switch edits that turn the entry at `entry` on or off by the agent's own switch, in the order they are made
/// (mcp.ts:1214, 1229); none where the format keeps no switch.
pub(crate) fn flip_edits(format: McpFormat, root: &Map<String, Value>, entry: &[PathSeg], name: &str, on: bool) -> Vec<JsonEdit> {
    match format {
        McpFormat::GeminiSettingsJson => {
            let excluded = root.get("mcp").and_then(json_object).and_then(|m| m.get("excluded"));
            let at = |i: i64| vec![key("mcp"), key("excluded"), PathSeg::Index(i)];
            let Some(Value::Array(held)) = excluded else {
                return if on { Vec::new() } else { vec![(vec![key("mcp"), key("excluded")], Some(json!([name])))] };
            };
            if !on {
                return vec![(at(-1), Some(Value::String(name.to_string())))];
            }
            let mut out: Vec<JsonEdit> =
                held.iter().enumerate().filter(|(_, n)| n.as_str() == Some(name)).map(|(i, _)| (at(i as i64), None)).collect();
            out.reverse();
            out
        }
        McpFormat::OpencodeJson => {
            let mut path = entry.to_vec();
            path.push(key("enabled"));
            vec![(path, Some(Value::Bool(on)))]
        }
        _ => Vec::new(),
    }
}

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_string())
}

fn as_text(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str)
}

/// A non-empty string (mcp.ts:459).
fn str_of(v: Option<&Value>) -> Option<String> {
    as_text(v).filter(|s| !s.is_empty()).map(str::to_string)
}

/// The string items of an array (mcp.ts:460).
fn strs(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

/// An object's string values in its own order (mcp.ts:466).
pub(crate) fn dict(v: Option<&Value>) -> IndexMap<String, String> {
    match v.and_then(json_object) {
        Some(map) => map.iter().filter_map(|(k, x)| x.as_str().map(|s| (k.clone(), s.to_string()))).collect(),
        None => IndexMap::new(),
    }
}

/// Each variable the values name, first seen first (mcp.ts:464).
fn refs_of(re: &Regex, values: &[&str]) -> Vec<String> {
    let mut seen = IndexSet::new();
    for v in values {
        for caps in re.captures_iter(v) {
            if let Some(m) = caps.get(1).or(caps.get(2)) {
                seen.insert(m.as_str().to_string());
            }
        }
    }
    seen.into_iter().collect()
}

/// Each variable `values` name in the format's syntax, first seen first (mcp.ts:464).
pub fn refs_in(format: McpFormat, values: &[String]) -> Vec<String> {
    let borrowed: Vec<&str> = values.iter().map(String::as_str).collect();
    refs_of(ref_re(format), &borrowed)
}

/// `${X}` or `{env:X}` (mcp.ts:1188, 1233).
pub fn ref_of(format: McpFormat, variable: &str) -> String {
    match format {
        McpFormat::OpencodeJson => format!("{{env:{variable}}}"),
        _ => format!("${{{variable}}}"),
    }
}

/// What `${X:-d}` reads as where X has no value: `d`; nothing for a reference with no default (mcp.ts:907). The default
/// may not cross a line break, as JavaScript's `.` will not.
fn fallback_of(reference: &str) -> Option<String> {
    let inner = reference.strip_prefix("${")?.strip_suffix('}')?;
    let at = inner.find(":-")?;
    let rest = &inner[at + 2..];
    if rest.contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    Some(rest.to_string())
}

/// The string as one whole reference in the agent's syntax and nothing else; a bearer's scheme read only where `bearer`
/// (mcp.ts:910). `None` for Codex and for a string that is not one.
pub fn whole_ref(format: McpFormat, value: &str, bearer: bool) -> Option<WholeRef> {
    static BEARER: OnceLock<Regex> = OnceLock::new();
    let bearer_re = BEARER.get_or_init(|| Regex::new(&format!(r"(?i)^Bearer[{WS}]+")).expect("a fixed pattern"));
    let scheme = if bearer { bearer_re.find(value).map_or("", |m| m.as_str()) } else { "" };
    let rest = &value[scheme.len()..];
    let caps = whole_re(format).captures(rest)?;
    let name = caps.get(1).or(caps.get(2))?.as_str().to_string();
    let fallback = fallback_of(rest);
    Some(WholeRef { name, scheme: scheme.to_string(), fallback })
}

/// The JSON value a format's JSON file parses to as an object: blank text is an empty object (mcp.ts:473).
pub(crate) fn json_root(text: Option<&str>) -> Result<Map<String, Value>, HostError> {
    let Some(t) = text.filter(|t| !js_trim(t).is_empty()) else {
        return Ok(Map::new());
    };
    let value = read_jsonc(t).map_err(|_| HostError::new(NOT_JSON))?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(HostError::new(NOT_OBJECT)),
    }
}

/// The servers table an agent keeps: the file's own `key` or a folder's under `projects` (mcp.ts:1129, 1147).
fn table_of<'a>(root: &'a Map<String, Value>, folder: Option<&str>, key_name: &str) -> Option<&'a Map<String, Value>> {
    scope_of(root, folder)?.get(key_name).and_then(json_object)
}

/// The object a scope holds: the file itself, or the folder's object under `projects`.
fn scope_of<'a>(root: &'a Map<String, Value>, folder: Option<&str>) -> Option<&'a Map<String, Value>> {
    match folder {
        None => Some(root),
        Some(f) => root.get("projects").and_then(json_object).and_then(|p| p.get(f)).and_then(json_object),
    }
}

/// The path of a servers table: `[key]`, or `["projects", folder, key]` (mcp.ts:1119).
fn table_path(folder: Option<&str>, key_name: &str) -> JsonPath {
    match folder {
        None => vec![key(key_name)],
        Some(f) => vec![key("projects"), key(f), key(key_name)],
    }
}

fn path_with(base: &JsonPath, name: &str) -> JsonPath {
    let mut path = base.clone();
    path.push(key(name));
    path
}

/// A definition's program: its `command` string, or the first word of its command array (mcp.ts:757).
fn json_command_of(def: &Value) -> Option<String> {
    let command = def.as_object()?.get("command")?;
    match command {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => items.first().and_then(Value::as_str).map(str::to_string),
        _ => None,
    }
}

/// Each string of a definition the machine reads another way, as an edit at its own path (mcp.ts:838). The path is
/// one stack the walk cuts back as it climbs.
fn string_edits(lib: RewriteString<'_>, v: &Value, path: JsonPath, command: bool, out: &mut Vec<JsonEdit>) {
    let mut path = path;
    let mut work: Vec<(&Value, usize, Option<PathSeg>, bool)> = vec![(v, path.len(), None, command)];
    while let Some((x, depth, step, command)) = work.pop() {
        path.truncate(depth);
        path.extend(step);
        match x {
            Value::String(s) => {
                let next = lib(s, command);
                if next != *s {
                    out.push((path.clone(), Some(Value::String(next))));
                }
            }
            Value::Array(items) => {
                work.extend(items.iter().enumerate().rev().map(|(i, y)| (y, path.len(), Some(PathSeg::Index(i as i64)), command && i == 0)))
            }
            Value::Object(map) => work.extend(map.iter().rev().map(|(k, y)| (y, path.len(), Some(key(k)), k == "command"))),
            _ => {}
        }
    }
}

/// A server's entry as one format reads it, or nothing when it names neither a command nor an address (mcp.ts:1196,
/// 1244).
fn server_of(format: McpFormat, name: &str, raw: &Value, scope: McpServerScope) -> Option<McpServer> {
    let def = raw.as_object()?;
    let re = ref_re(format);
    if format == McpFormat::OpencodeJson {
        let disabled = (def.get("enabled") == Some(&Value::Bool(false))).then_some(true);
        if def.get("type").and_then(Value::as_str) == Some("remote") {
            let url = str_of(def.get("url"))?;
            let headers = dict(def.get("headers"));
            let header_values: Vec<&str> = headers.values().map(String::as_str).collect();
            let env_refs = refs_of(re, &header_values);
            return Some(McpServer {
                name: name.to_string(),
                scope,
                transport: McpTransport::Http { url, headers },
                env_refs,
                reads: None,
                disabled,
            });
        }
        let words = strs(def.get("command"));
        let mut words = words.into_iter();
        let command = words.next()?;
        return Some(McpServer {
            name: name.to_string(),
            scope,
            transport: McpTransport::Stdio {
                command,
                args: words.collect(),
                env: dict(def.get("environment")),
                cwd: None,
                tool_timeout_sec: None,
            },
            env_refs: Vec::new(),
            reads: None,
            disabled,
        });
    }
    if let Some(command) = str_of(def.get("command")) {
        let cwd = str_of(def.get("cwd"));
        return Some(McpServer {
            name: name.to_string(),
            scope,
            transport: McpTransport::Stdio { command, args: strs(def.get("args")), env: dict(def.get("env")), cwd, tool_timeout_sec: None },
            env_refs: Vec::new(),
            reads: None,
            disabled: None,
        });
    }
    let url = str_of(def.get("httpUrl")).or_else(|| str_of(def.get("url")))?;
    let headers = dict(def.get("headers"));
    let mut values: Vec<&str> = vec![url.as_str()];
    values.extend(headers.values().map(String::as_str));
    let env_refs = refs_of(re, &values);
    Some(McpServer { name: name.to_string(), scope, transport: McpTransport::Http { url, headers }, env_refs, reads: None, disabled: None })
}

/// The servers under a format's key of one object, each read by the format's own entry shape with the variables its
/// OAuth secrets read (mcp.ts:732).
fn servers_in(format: McpFormat, root: &Map<String, Value>, scope: McpServerScope) -> Vec<McpServer> {
    let Some(table) = root.get(servers_key(format)).and_then(json_object) else {
        return Vec::new();
    };
    let re = ref_re(format);
    js_entries(table)
        .into_iter()
        .filter_map(|(name, raw)| {
            let s = server_of(format, name, raw, scope)?;
            let oauth = raw.as_object().and_then(|d| d.get("oauth")).and_then(json_object);
            let secrets: Vec<&str> = match oauth {
                Some(o) => oauth_secret_names(format).iter().filter_map(|k| as_text(o.get(*k))).collect(),
                None => Vec::new(),
            };
            if secrets.is_empty() {
                return Some(s);
            }
            let mut env_refs = s.env_refs.clone();
            env_refs.extend(refs_of(re, &secrets));
            let mut seen = IndexSet::new();
            env_refs.retain(|r| seen.insert(r.clone()));
            Some(McpServer { env_refs, ..s })
        })
        .collect()
}

/// Every server the text defines; text that does not parse or is not an object defines none. Gemini's names in
/// `mcp.excluded` read disabled; a folder's own servers under `projects[home]` follow the file's own, scope `home`
/// (mcp.ts:1118).
pub fn read(format: McpFormat, text: &str, home: &str) -> Vec<McpServer> {
    let Ok(Value::Object(root)) = read_jsonc(text) else {
        return Vec::new();
    };
    let off = switched_off(format, &root);
    let own: Vec<McpServer> = servers_in(format, &root, McpServerScope::User)
        .into_iter()
        .map(|s| if off.contains(&s.name) { McpServer { disabled: Some(true), ..s } } else { s })
        .collect();
    let folder = if has_projects(format) { table_of_folder(&root, home) } else { None };
    match folder {
        Some(project) => {
            let mut all = own;
            all.extend(servers_in(format, project, McpServerScope::Home));
            all
        }
        None => own,
    }
}

/// The object under `projects[home]`, where the file holds one.
fn table_of_folder<'a>(root: &'a Map<String, Value>, home: &str) -> Option<&'a Map<String, Value>> {
    root.get("projects").and_then(json_object).and_then(|p| p.get(home)).and_then(json_object)
}

/// The entry a format writes for a placed server (mcp.ts:1203, 1253).
pub fn entry(format: McpFormat, server: &McpTransport) -> Value {
    match (format, server) {
        (McpFormat::OpencodeJson, McpTransport::Stdio { command, args, env, .. }) => {
            let mut command_list = vec![Value::String(command.clone())];
            command_list.extend(args.iter().map(|a| Value::String(a.clone())));
            let mut out = Map::new();
            out.insert("type".into(), json!("local"));
            out.insert("command".into(), Value::Array(command_list));
            out.insert("enabled".into(), json!(true));
            insert_some(&mut out, "environment", env);
            Value::Object(out)
        }
        (McpFormat::OpencodeJson, McpTransport::Http { url, headers }) => {
            let mut out = Map::new();
            out.insert("type".into(), json!("remote"));
            out.insert("url".into(), json!(url));
            out.insert("enabled".into(), json!(true));
            insert_some(&mut out, "headers", headers);
            Value::Object(out)
        }
        (_, McpTransport::Stdio { command, args, env, .. }) => {
            let mut out = Map::new();
            out.insert("command".into(), json!(command));
            out.insert("args".into(), json!(args));
            insert_some(&mut out, "env", env);
            Value::Object(out)
        }
        (McpFormat::GeminiSettingsJson, McpTransport::Http { url, headers }) => {
            let mut out = Map::new();
            out.insert("httpUrl".into(), json!(url));
            insert_some(&mut out, "headers", headers);
            Value::Object(out)
        }
        (_, McpTransport::Http { url, headers }) => {
            let mut out = Map::new();
            out.insert("type".into(), json!("http"));
            out.insert("url".into(), json!(url));
            insert_some(&mut out, "headers", headers);
            Value::Object(out)
        }
    }
}

/// A record written only where it holds something (mcp.ts:462).
fn insert_some(out: &mut Map<String, Value>, key_name: &str, d: &IndexMap<String, String>) {
    if !d.is_empty() {
        out.insert(key_name.into(), Value::Object(d.iter().map(|(k, v)| (k.clone(), json!(v))).collect()));
    }
}

/// A new file is `{"<key>": {"<name>": entry}}` pretty with a newline; else the entry put in place (mcp.ts:1131).
/// Errors "the file is not valid JSON; add the server by hand" and "the file is not a JSON object; add the server by
/// hand" (mcp.ts:479, 481).
pub fn place(format: McpFormat, text: Option<&str>, name: &str, server: &McpTransport) -> Result<String, HostError> {
    let root = json_root(text)?;
    let key_name = servers_key(format);
    let entry = entry(format, server);
    let Some(t) = text.filter(|t| !js_trim(t).is_empty()) else {
        let mut inner = Map::new();
        inner.insert(name.to_string(), entry);
        let mut outer = Map::new();
        outer.insert(key_name.to_string(), Value::Object(inner));
        return Ok(format!("{}\n", js_stringify_pretty(&Value::Object(outer))));
    };
    let edit: JsonEdit = if root.get(key_name).is_some_and(|v| v.is_object()) {
        (vec![key(key_name), key(name)], Some(entry))
    } else {
        (vec![key(key_name)], Some(Value::Object(Map::from_iter([(name.to_string(), entry)]))))
    };
    let mut root_value = Value::Object(root);
    edit_jsonc(t, &mut root_value, &[edit])
}

/// The text parsed as a JSON object, for the editors that change a copy of it.
fn parse_object(text: &str) -> Result<Value, HostError> {
    let value = read_jsonc(text)?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(HostError::new("the file is not a JSON object"))
    }
}

/// The edit the import runs (mcp.ts:850).
pub fn edit(format: McpFormat, lib: RewriteString<'_>, scope: &McpEditScope, text: &str) -> Result<McpEdited, HostError> {
    let key_name = servers_key(format);
    let parsed = parse_object(text)?;
    let root = parsed.as_object().ok_or_else(|| HostError::new("the file is not a JSON object"))?;
    let folder_from = scope.project.as_ref().map(|p| p.from.as_str());
    let folder_to = scope.project.as_ref().map(|p| p.to.as_str());
    let src = table_path(folder_from, key_name);
    let dst = table_path(folder_to, key_name);
    let servers: Map<String, Value> = table_of(root, folder_from, key_name).cloned().unwrap_or_default();
    let moved: Map<String, Value> = match folder_to {
        Some(_) => table_of(root, folder_to, key_name).cloned().unwrap_or_default(),
        None => Map::new(),
    };
    let mut results = Vec::new();
    let mut edits: Vec<JsonEdit> = Vec::new();
    for name in &scope.keep {
        let Some(def) = held_at(&servers, name) else {
            match held_at(&moved, name) {
                Some(there) => {
                    results.push(McpEditResult { name: name.clone(), outcome: EditOutcome::Written, command: json_command_of(there) })
                }
                None => results.push(McpEditResult { name: name.clone(), outcome: EditOutcome::Missing, command: None }),
            }
            continue;
        };
        let next = map_strings(def, false, &mut |s, command| lib(s, command));
        if folder_to.is_none() {
            string_edits(lib, def, path_with(&src, name), false, &mut edits);
        } else {
            edits.push((path_with(&src, name), None));
            edits.push((path_with(&dst, name), Some(next.clone())));
        }
        results.push(McpEditResult { name: name.clone(), outcome: EditOutcome::Written, command: json_command_of(&next) });
    }
    for name in &scope.drop {
        if servers.contains_key(name) {
            edits.push((path_with(&src, name), None));
        }
        results.push(McpEditResult { name: name.clone(), outcome: EditOutcome::Dropped, command: None });
    }
    let text_out = if edits.is_empty() {
        text.to_string()
    } else {
        let mut working = parsed.clone();
        edit_jsonc(text, &mut working, &edits)?
    };
    Ok(McpEdited { text: text_out, results })
}

/// A copy holding the named servers alone, pretty with a newline (mcp.ts:1138).
pub fn only(format: McpFormat, text: &str, names: &[String], from: Option<&str>, to: Option<&str>) -> Result<String, HostError> {
    let key_name = servers_key(format);
    let parsed = parse_object(text)?;
    let root = parsed.as_object().ok_or_else(|| HostError::new("the file is not a JSON object"))?;
    let table = table_of(root, from, key_name);
    let mut kept = Map::new();
    for n in names {
        if let Some(v) = table.and_then(|t| t.get(n)) {
            kept.insert(n.clone(), v.clone());
        }
    }
    let out = match to {
        Some(folder) if has_projects(format) => json!({ "projects": { folder: { key_name: Value::Object(kept) } } }),
        _ => json!({ key_name: Value::Object(kept) }),
    };
    Ok(format!("{}\n", js_stringify_pretty(&out)))
}

/// The canonical text of one server's entry (mcp.ts:1145).
pub fn entry_of(format: McpFormat, text: &str, name: &str, project: Option<&str>) -> Option<String> {
    let Ok(Value::Object(root)) = read_jsonc(text) else {
        return None;
    };
    let def = scope_of(&root, project)?.get(servers_key(format)).and_then(json_object)?;
    Some(json_canonical(held_at(def, name)?))
}

/// wsp's servers merged into the agent's own file (mcp.ts:773).
pub fn merge(
    format: McpFormat,
    lib: RewriteString<'_>,
    scope: &McpMergeScope,
    own: Option<&str>,
    travelled: &str,
) -> Result<McpMerged, HostError> {
    let key_name = servers_key(format);
    let held = own.filter(|t| !js_trim(t).is_empty());
    let mut root = match held {
        None => Value::Object(Map::new()),
        Some(t) => read_jsonc(t)?,
    };
    if !root.is_object() {
        return Err(HostError::new("the file is not a JSON object"));
    }
    let folder = scope.project.as_ref().map(|p| p.to.as_str());
    let arrived = travelled_servers(travelled, key_name, scope.project.as_ref())?;
    let at = table_path(folder, key_name);
    let standing: Map<String, Value> = root.as_object().and_then(|r| table_of(r, folder, key_name)).cloned().unwrap_or_default();
    let replace: HashSet<&str> = scope.replace.iter().map(String::as_str).collect();
    let mut results = Vec::new();
    let mut edits: Vec<JsonEdit> = Vec::new();
    for name in &scope.keep {
        let Some(def) = held_at(&arrived, name) else {
            results.push(McpMergeResult { name: name.clone(), outcome: MergeOutcome::Missing, command: None });
            continue;
        };
        let next = map_strings(def, false, &mut |s, command| lib(s, command));
        let command = json_command_of(&next);
        let here = held_at(&standing, name);
        let outcome = if here.is_some_and(|h| json_canonical(h) == json_canonical(&next)) {
            MergeOutcome::Same
        } else if here.is_some() && !replace.contains(name.as_str()) {
            MergeOutcome::Theirs
        } else {
            edits.push((path_with(&at, name), Some(next)));
            if here.is_none() {
                MergeOutcome::Added
            } else {
                MergeOutcome::Replaced
            }
        };
        results.push(McpMergeResult { name: name.clone(), outcome, command });
    }
    for name in &scope.drop {
        if held_at(&standing, name).is_none() || !replace.contains(name.as_str()) {
            results.push(McpMergeResult { name: name.clone(), outcome: MergeOutcome::Left, command: None });
            continue;
        }
        edits.push((path_with(&at, name), None));
        results.push(McpMergeResult { name: name.clone(), outcome: MergeOutcome::Dropped, command: None });
    }
    if edits.is_empty() {
        return Ok(McpMerged { text: own.unwrap_or("").to_string(), results });
    }
    match held {
        Some(t) => Ok(McpMerged { text: edit_jsonc(t, &mut root, &edits)?, results }),
        None => {
            for edit in &edits {
                set_at(&mut root, edit);
            }
            Ok(McpMerged { text: format!("{}\n", js_stringify_pretty(&root)), results })
        }
    }
}

/// The named servers taken out, the table left standing; a name Gemini switched off in `mcp.excluded` is switched
/// back on as it goes (mcp.ts:819).
pub fn remove(format: McpFormat, text: &str, names: &[String], project: Option<&str>) -> Result<String, HostError> {
    let key_name = servers_key(format);
    let parsed = parse_object(text)?;
    let mut root = parsed;
    let root_map = root.as_object().ok_or_else(|| HostError::new("the file is not a JSON object"))?;
    let at = table_path(project, key_name);
    let Some(table) = table_of(root_map, project, key_name) else {
        return Ok(text.to_string());
    };
    let took: Vec<String> = names.iter().filter(|n| table.contains_key(n.as_str())).cloned().collect();
    if took.is_empty() {
        return Ok(text.to_string());
    }
    let edits: Vec<JsonEdit> = took.iter().map(|n| (path_with(&at, n), None)).collect();
    let mut out = edit_jsonc(text, &mut root, &edits)?;
    for name in &took {
        let root_map = root.as_object().ok_or_else(|| HostError::new("the file is not a JSON object"))?;
        if !switched_off(format, root_map).contains(name) {
            continue;
        }
        let flips = flip_edits(format, root_map, &path_with(&at, name), name, true);
        if !flips.is_empty() {
            out = edit_jsonc(&out, &mut root, &flips)?;
        }
    }
    Ok(out)
}

/// The switch: OpenCode's `enabled`, Gemini's `mcp.excluded`; `None` for Claude Code's file (mcp.ts:1169).
pub fn enable(format: McpFormat, text: &str, name: &str, on: bool, project: Option<&str>) -> Option<Result<String, HostError>> {
    if !matches!(format, McpFormat::GeminiSettingsJson | McpFormat::OpencodeJson) {
        return None;
    }
    Some(enable_switch(format, text, name, on, project))
}

fn enable_switch(format: McpFormat, text: &str, name: &str, on: bool, project: Option<&str>) -> Result<String, HostError> {
    let key_name = servers_key(format);
    let mut root = Value::Object(json_root(Some(text))?);
    let root_map = root.as_object().ok_or_else(|| HostError::new(NOT_OBJECT))?;
    let held = scope_of(root_map, project).and_then(|s| s.get(key_name)).and_then(json_object);
    if held.and_then(|t| held_at(t, name)).and_then(json_object).is_none() {
        return Err(HostError::new(format!("the file names no server called {name}")));
    }
    let at = match project {
        None => vec![key(key_name), key(name)],
        Some(f) => vec![key("projects"), key(f), key(key_name), key(name)],
    };
    let edits = flip_edits(format, root_map, &at, name, on);
    edit_jsonc(text, &mut root, &edits)
}

/// Each reference in env, headers and the address filled from `value` or its default (mcp.ts:1093).
pub fn resolve(format: McpFormat, server: &McpServer, value: &dyn Fn(&str) -> Option<String>) -> McpResolved {
    let re = ref_re(format);
    let mut values: Vec<String> = Vec::new();
    let mut missing: Option<String> = None;
    let mut fill = |v: &str| -> String {
        let mut out = String::new();
        let mut last = 0;
        for caps in re.captures_iter(v) {
            let whole = caps.get(0).expect("group 0 always matches");
            out.push_str(&v[last..whole.start()]);
            last = whole.end();
            let name = caps.get(1).or(caps.get(2)).map_or("", |m| m.as_str());
            match value(name) {
                Some(got) => {
                    out.push_str(&got);
                    values.push(got);
                }
                None => match fallback_of(whole.as_str()) {
                    Some(d) => out.push_str(&d),
                    None => {
                        missing.get_or_insert_with(|| name.to_string());
                        out.push_str(whole.as_str());
                    }
                },
            }
        }
        out.push_str(&v[last..]);
        out
    };
    let transport = match &server.transport {
        McpTransport::Stdio { command, args, env, cwd, tool_timeout_sec } => McpTransport::Stdio {
            command: command.clone(),
            args: args.clone(),
            env: env.iter().map(|(k, v)| (k.clone(), fill(v))).collect(),
            cwd: cwd.clone(),
            tool_timeout_sec: *tool_timeout_sec,
        },
        McpTransport::Http { url, headers } => {
            let url = fill(url);
            let headers = headers.iter().map(|(k, v)| (k.clone(), fill(v))).collect();
            McpTransport::Http { url, headers }
        }
    };
    match missing {
        Some(name) => McpResolved::Missing(name),
        None => McpResolved::Resolved { transport, values },
    }
}
