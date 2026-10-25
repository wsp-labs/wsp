// SPDX-License-Identifier: AGPL-3.0-only
//! Codex's reference writer: the file read whole by a TOML parser, rewritten by name, the line editor's text kept where
//! it parses to the same tree. Port of packages/catalog/src/mcp.ts:1596-1735 at a3b870a6e; spec/mcp_toml_refer.md.
use std::collections::{HashMap, HashSet};

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

use crate::smol_toml::parse_toml;
use crate::json_edit::json_canonical;
use crate::jsonc_tree::PathSeg;
use crate::mcp::{KnownValue, McpReferred, McpTransport, ReferredServer};
use crate::mcp_refs::{
    arg_strings, codex_headers, known_hits, known_stands, oauth_secret_keys, readable, shape_refusal, still_stands, unread_line,
};
use crate::mcp_toml::{
    codex_owners, read_codex, toml_header, toml_inline, toml_open_array, toml_string, toml_strings, toml_value, uncomment_toml,
    unquote_key, TomlValue,
};
use wsp_words::exit::HostError;
use wsp_words::js::js_trim;

/// The key a table line sets, unquoted (mcp.ts:1597).
pub fn toml_key(line: &str) -> Option<String> {
    let code = uncomment_toml(line);
    let eq = code.find('=')?;
    Some(unquote_key(js_trim(&code[..eq])))
}

/// The text after a line's first `=`, comment taken out (mcp.ts:1606, the `slice(indexOf("=") + 1)` it reads).
fn rhs(line: &str) -> String {
    let code = uncomment_toml(line);
    match code.find('=') {
        Some(i) => code[i + 1..].to_string(),
        None => code,
    }
}

/// The text edited line by line toward the tree referCodex wants (mcp.ts:1606).
pub fn refer_codex_lines(text: &str, only: Option<&str>) -> String {
    refer_lines(text, only).unwrap_or_else(|_| text.to_string())
}

fn refer_lines(text: &str, only: Option<&str>) -> Result<String, HostError> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    for s in read_codex(text) {
        if only.is_some_and(|o| s.name != o) {
            continue;
        }
        let (headers, env) = match &s.transport {
            McpTransport::Http { headers, .. } => (headers.clone(), IndexMap::new()),
            McpTransport::Stdio { env, .. } => (IndexMap::new(), env.clone()),
        };
        if headers.len() + env.len() == 0 {
            continue;
        }
        let next = {
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            edit_server(&refs, &s.name, &headers, &env)?
        };
        lines = next;
    }
    Ok(lines.join("\n"))
}

/// One server's lines rewritten: its headers and env lines go, its names move to `env_http_headers` and `env_vars`.
fn edit_server(
    refs: &[&str],
    name: &str,
    headers: &IndexMap<String, String>,
    env: &IndexMap<String, String>,
) -> Result<Vec<String>, HostError> {
    let n = refs.len();
    let owner = codex_owners(refs);
    let subs: Vec<Option<String>> = refs
        .iter()
        .map(|l| {
            let t = js_trim(&uncomment_toml(l)).to_string();
            if t.starts_with('[') {
                toml_header(&t).and_then(|h| h.1)
            } else {
                None
            }
        })
        .collect();
    let mut sub: Option<String> = None;
    let sub_of: Vec<Option<String>> = (0..n)
        .map(|i| {
            if owner[i].as_deref() != Some(name) {
                return None;
            }
            if js_trim(&uncomment_toml(refs[i])).starts_with('[') {
                sub = subs[i].clone();
            }
            sub.clone()
        })
        .collect();
    let mine = |i: usize| owner[i].as_deref() == Some(name);
    let head: i64 =
        (0..n).find(|&i| mine(i) && js_trim(&uncomment_toml(refs[i])).starts_with('[') && subs[i].is_none()).map_or(-1, |i| i as i64);
    let main_key = |key_name: &str| -> Option<usize> {
        (0..n).find(|&i| mine(i) && (i as i64) > head && sub_of[i].is_none() && toml_key(refs[i]).as_deref() == Some(key_name))
    };
    let env_headers: Vec<(String, String)> = codex_headers(name, headers)?.into_iter().map(|c| (c.header, c.reads)).collect();
    let env_names: Vec<String> = env.keys().cloned().collect();
    let mut drop: HashSet<usize> = (0..n)
        .filter(|&i| {
            mine(i)
                && (sub_of[i].as_deref() == Some("http_headers")
                    || sub_of[i].as_deref() == Some("env")
                    || (sub_of[i].is_none()
                        && (i as i64) > head
                        && matches!(toml_key(refs[i]).as_deref(), Some("http_headers") | Some("env"))))
        })
        .collect();
    let mut replace: HashMap<usize, Vec<String>> = HashMap::new();
    let mut added: Vec<String> = Vec::new();
    if !env_headers.is_empty() {
        let inline = main_key("env_http_headers");
        let table_at = (0..n)
            .find(|&i| mine(i) && subs[i].as_deref() == Some("env_http_headers") && js_trim(&uncomment_toml(refs[i])).starts_with('['));
        if let Some(inline) = inline {
            let mut merged: IndexMap<String, String> = match toml_value(&rhs(refs[inline])) {
                Some(TomlValue::Table(t)) => t,
                _ => IndexMap::new(),
            };
            for (h, v) in &env_headers {
                merged.insert(h.clone(), v.clone());
            }
            replace.insert(inline, vec![format!("env_http_headers = {}", toml_inline(&merged))]);
        } else if let Some(table_at) = table_at {
            let mut last = table_at;
            let mut i = table_at + 1;
            while i < n && sub_of[i].as_deref() == Some("env_http_headers") && mine(i) {
                if !js_trim(refs[i]).is_empty() {
                    last = i;
                }
                i += 1;
            }
            let mut block = vec![refs[last].to_string()];
            block.extend(env_headers.iter().map(|(h, v)| format!("{} = {}", toml_string(h), toml_string(v))));
            replace.insert(last, block);
        } else {
            let map: IndexMap<String, String> = env_headers.iter().cloned().collect();
            added.push(format!("env_http_headers = {}", toml_inline(&map)));
        }
    }
    if !env_names.is_empty() {
        if let Some(at) = main_key("env_vars") {
            let mut end = at;
            let mut value = js_trim(&rhs(refs[at])).to_string();
            while toml_open_array(&value) && end + 1 < n {
                end += 1;
                value.push_str(js_trim(&uncomment_toml(refs[end])));
            }
            let had = toml_strings(&value);
            let mut all = had.clone();
            all.extend(env_names.iter().filter(|k| !had.contains(k)).cloned());
            let listed: Vec<String> = all.iter().map(|k| toml_string(k)).collect();
            replace.insert(at, vec![format!("env_vars = [{}]", listed.join(", "))]);
            drop.extend(at + 1..=end);
        } else {
            let listed: Vec<String> = env_names.iter().map(|k| toml_string(k)).collect();
            added.push(format!("env_vars = [{}]", listed.join(", ")));
        }
    }
    let mut out = Vec::new();
    for (i, line) in refs.iter().enumerate() {
        if head >= 0 && i as i64 == head {
            out.push(line.to_string());
            out.extend(added.iter().cloned());
        } else if !drop.contains(&i) {
            match replace.get(&i) {
                Some(block) => out.extend(block.iter().cloned()),
                None => out.push(line.to_string()),
            }
        }
    }
    Ok(out)
}

/// `CODEX_TOML.refer` (mcp.ts:1661). Errors "the file is not valid TOML (<first line of the parser's message>)".
pub fn refer_codex(text: &str, only: Option<&str>, known: &[KnownValue], outside: &IndexSet<String>) -> Result<McpReferred, HostError> {
    let tree = parse_toml(text).map_err(|e| HostError::new(format!("the file is not valid TOML ({})", e.lines().next().unwrap_or_default())))?;
    let Some(table_value) = tree.get("mcp_servers") else {
        return Ok(McpReferred { text: text.to_string(), servers: Vec::new(), entries: Vec::new() });
    };
    let Some(table) = table_value.as_table() else {
        return Err(shape_refusal(&vec![key("mcp_servers")]));
    };
    let mut writes: IndexSet<String> = IndexSet::new();
    for (name, raw) in table.iter() {
        let Some(raw) = raw.as_table() else {
            continue;
        };
        if only.is_some_and(|o| name != o) {
            continue;
        }
        for c in codex_headers(name, &strings_of(raw.get("http_headers")))? {
            writes.insert(c.keeps);
        }
        writes.extend(strings_of(raw.get("env")).into_keys());
    }
    let held = |n: &str| readable(outside, &writes, n, false);
    let mut servers: Vec<ReferredServer> = Vec::new();
    let mut next = table.clone();
    let mut changed = false;
    for (name, raw_value) in table.iter() {
        let at_server = vec![key("mcp_servers"), key(name)];
        let Some(raw) = raw_value.as_table() else {
            return Err(shape_refusal(&at_server));
        };
        for key_name in ["http_headers", "env"] {
            if let Some(v) = raw.get(key_name) {
                let shaped = v.as_table().is_some_and(|t| t.values().all(|x| x.is_str()));
                if !shaped {
                    return Err(shape_refusal(&at_server));
                }
            }
        }
        let oauth = raw.get("oauth").map(|o| o.as_table().ok_or_else(|| shape_refusal(&at_server))).transpose()?;
        if only.is_some_and(|o| name != o) {
            continue;
        }
        if let Some(oauth) = oauth {
            if let Some(secret) = oauth_secret_keys(&json_map(oauth)?).first() {
                return Err(HostError::new(format!(
                    "{name} keeps an OAuth secret in mcp_servers.{name}.oauth.{secret}, and Codex reads no variable there, so the file stays on this computer"
                )));
            }
        }
        let mut reads: Vec<(Vec<String>, Option<String>)> =
            vec![(vec!["bearer_token_env_var".to_string()], raw.get("bearer_token_env_var").and_then(|v| v.as_str()).map(str::to_string))];
        reads.extend(strings_of(raw.get("env_http_headers")).into_iter().map(|(h, v)| (vec!["env_http_headers".to_string(), h], Some(v))));
        if let Some((path, _)) = reads.iter().find(|(_, n)| n.as_ref().is_some_and(|n| !held(n))) {
            let mut at = at_server.clone();
            at.extend(path.iter().map(|s| key(s)));
            servers.push(ReferredServer {
                name: name.clone(),
                values: IndexMap::new(),
                unread: Some(unread_line(&at)),
                ..Default::default()
            });
            continue;
        }
        let headers = strings_of(raw.get("http_headers"));
        let env = strings_of(raw.get("env"));
        servers.push(ReferredServer { name: name.clone(), values: IndexMap::new(), ..Default::default() });
        let index = servers.len() - 1;
        if headers.len() + env.len() == 0 {
            continue;
        }
        let sent = codex_headers(name, &headers)?;
        let env_header_map: IndexMap<String, String> = sent.iter().map(|c| (c.header.clone(), c.reads.clone())).collect();
        let mut values: IndexMap<String, String> = IndexMap::new();
        for c in &sent {
            values.insert(c.keeps.clone(), c.value.clone());
        }
        for (k, v) in &env {
            if !v.is_empty() {
                values.insert(k.clone(), v.clone());
            }
        }
        servers[index].values = values;
        let mut def = raw.clone();
        def.remove("http_headers");
        def.remove("env");
        if !env_header_map.is_empty() {
            let mut merged = strings_of(raw.get("env_http_headers"));
            for (h, v) in env_header_map {
                merged.insert(h, v);
            }
            def.insert("env_http_headers".to_string(), toml::Value::Table(to_toml_table(&merged)));
        }
        if !env.is_empty() {
            let had: Vec<toml::Value> = raw.get("env_vars").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let mut all = had.clone();
            for k in env.keys() {
                if !had.iter().any(|x| x.as_str() == Some(k.as_str())) {
                    all.push(toml::Value::String(k.clone()));
                }
            }
            def.insert("env_vars".to_string(), toml::Value::Array(all));
        }
        next.insert(name.clone(), toml::Value::Table(def));
        changed = true;
    }
    for (name, raw_value) in table.iter() {
        if only.is_some_and(|o| name != o) || servers.iter().any(|s| &s.name == name && s.unread.is_some()) {
            continue;
        }
        let Some(raw) = raw_value.as_table() else {
            continue;
        };
        let strings = arg_strings(&json_map(raw)?);
        let own = servers.iter().find(|s| &s.name == name).map(|s| s.values.clone()).unwrap_or_default();
        for (_, v, where_) in &strings {
            if let Some(hit) = known_hits(v, where_, known, &servers, name, &own)?.into_iter().next() {
                return Err(HostError::new(format!(
                    "{name} passes the value of {} in {}, and Codex reads no variable there, so the file stays on this computer",
                    hit.name, hit.at
                )));
            }
        }
        let values: Vec<String> = strings.iter().map(|(_, v, _)| v.clone()).collect();
        known_stands(&values, known, name)?;
    }
    if !changed {
        let entries = table.values().map(to_json).collect::<Result<Vec<Value>, HostError>>()?;
        return Ok(McpReferred { text: text.to_string(), servers, entries });
    }
    let mut want = tree.clone();
    want.insert("mcp_servers".to_string(), toml::Value::Table(next));
    let out = match refer_lines(text, only) {
        Ok(lined) if same_tree(&lined, &want) => lined,
        _ => toml::to_string(&want).map_err(|e| HostError::new(e.to_string()))?,
    };
    let written = parse_toml(&out).map_err(HostError::new)?;
    let entries = match written.get("mcp_servers").and_then(|v| v.as_table()) {
        Some(t) => t.values().map(to_json).collect::<Result<Vec<Value>, HostError>>()?,
        None => Vec::new(),
    };
    still_stands(&entries, &servers)?;
    Ok(McpReferred { text: out, servers, entries })
}

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_string())
}

/// The string values of a table, every other entry left out (mcp.ts:1632, `dict`).
fn strings_of(v: Option<&toml::Value>) -> IndexMap<String, String> {
    match v.and_then(|v| v.as_table()) {
        Some(t) => t.iter().filter_map(|(k, x)| x.as_str().map(|s| (k.clone(), s.to_string()))).collect(),
        None => IndexMap::new(),
    }
}

fn to_toml_table(d: &IndexMap<String, String>) -> toml::Table {
    d.iter().map(|(k, v)| (k.clone(), toml::Value::String(v.clone()))).collect()
}

fn to_json(v: &toml::Value) -> Result<Value, HostError> {
    serde_json::to_value(v).map_err(|e| HostError::new(e.to_string()))
}

fn json_map(t: &toml::Table) -> Result<serde_json::Map<String, Value>, HostError> {
    match to_json(&toml::Value::Table(t.clone()))? {
        Value::Object(map) => Ok(map),
        _ => Err(HostError::new("the file is not a TOML table")),
    }
}

/// Whether the text parses to the tree wanted, compared as `jsonCanonical` compares (mcp.ts:1685).
fn same_tree(lined: &str, want: &toml::Table) -> bool {
    let Ok(parsed) = parse_toml(lined) else {
        return false;
    };
    match (to_json(&toml::Value::Table(parsed)), to_json(&toml::Value::Table(want.clone()))) {
        (Ok(a), Ok(b)) => json_canonical(&a) == json_canonical(&b),
        _ => false,
    }
}
