// SPDX-License-Identifier: AGPL-3.0-only
//! The JSON formats' reference writer: each server's header, command variable, OAuth secret and argument default
//! written by name in place. Port of packages/catalog/src/mcp.ts:921-1087 at a3b870a6e; spec/mcp_json_refer.md.
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};

use crate::json_edit::{edit_jsonc, JsonEdit};
use crate::jsonc_tree::{JsonPath, PathSeg};
use crate::mcp::{KnownValue, McpFormat, McpReferred, ReferredServer, WholeRef};
use crate::mcp_json::{dict, env_key, has_projects, json_root, oauth_secret_names, ref_of, ref_re, servers_key, whole_ref};
use crate::mcp_refs::{arg_strings, header_variables, known_hits, known_stands, mcp_header_variable, oauth_secret_keys, readable, refers, shape_refusal, spliced, still_stands, unread_line, bearer_token};
use wsp_words::exit::HostError;

/// `format.refer` for a JSON format (mcp.ts:957).
pub fn refer(format: McpFormat, text: &str, only: Option<&str>, known: &[KnownValue], outside: &IndexSet<String>) -> Result<McpReferred, HostError> {
    let root = json_root(Some(text))?;
    let beside = if only.is_none() { elsewhere(format, &root) } else { None };
    if let Some(at) = beside {
        return Err(HostError::new(format!(
            "the file keeps servers under {at}, whose values wsp does not write by name, so the file stays on this computer"
        )));
    }
    let tables = server_tables(format, &root, only.is_none())?;
    let whole = |v: &str| whole_ref(format, v, true);
    let bare = |v: &str| whole_ref(format, v, false);

    let mut asked: Vec<(usize, String, Map<String, Value>)> = Vec::new();
    for (ti, table) in tables.iter().enumerate() {
        for (name, raw) in &table.servers {
            if only.is_some_and(|o| o != name) {
                continue;
            }
            if let Some(def) = raw.as_object() {
                asked.push((ti, name.clone(), def.clone()));
            }
        }
    }
    let mut writes: IndexSet<String> = IndexSet::new();
    for (_, name, def) in &asked {
        writes.extend(literal_names(format, name, def, &whole, &bare));
    }
    let held = |n: &str, in_address: bool| readable(outside, &writes, n, in_address);

    let mut weighed: Vec<Weighed> = Vec::new();
    for (ti, name, def) in &asked {
        let table = &tables[*ti];
        let base = path_with(&table.at, name);
        let ctx = Ctx { format, name, held: &held, base: base.clone() };
        let mut w = Weighed { table: *ti, name: name.clone(), values: IndexMap::new(), mine: Vec::new(), shown: def.clone(), lost: Vec::new(), unread: None };

        put_entries(&ctx, &mut w, "headers", &dict(def.get("headers")), &whole, true, &|k| minted_header(name, k))?;
        put_entries(&ctx, &mut w, env_key(format), &dict(def.get(env_key(format))), &whole, false, &|k| k.to_string())?;
        oauth_entries(&ctx, &mut w, def, &bare)?;
        argument_entries(&ctx, &mut w, def)?;
        weighed.push(w);
    }

    let mut holds_writes = writes.clone();
    for w in &weighed {
        holds_writes.extend(w.values.keys().cloned());
    }
    let holds = |n: &str, in_address: bool| readable(outside, &holds_writes, n, in_address);
    let mut servers: Vec<ReferredServer> = Vec::new();
    let mut edits: Vec<JsonEdit> = Vec::new();
    let mut defs: Vec<Defined> = Vec::new();
    for w in weighed {
        let table = &tables[w.table];
        let gone = w.lost.iter().find(|l| !holds(&l.name, l.in_address));
        let unread = w.unread.clone().or_else(|| gone.map(|g| unread_line(&g.at)));
        let project = table.project.clone();
        if let Some(line) = unread {
            servers.push(ReferredServer { name: w.name.clone(), project, values: IndexMap::new(), unread: Some(line) });
            continue;
        }
        edits.extend(w.mine);
        let at = path_with(&table.at, &w.name);
        servers.push(ReferredServer { name: w.name.clone(), project, values: w.values, unread: None });
        defs.push(Defined { at, name: w.name, def: w.shown, index: servers.len() - 1 });
    }

    for d in &defs {
        let mut written: Vec<String> = Vec::new();
        for (path, v, where_) in arg_strings(&d.def) {
            let own = servers[d.index].values.clone();
            let hits = known_hits(&v, &where_, known, &servers, &d.name, &own)?;
            written.push(if hits.is_empty() { v.clone() } else { spliced(&v, &hits, &|n| ref_of(format, n)) });
            if hits.is_empty() {
                continue;
            }
            edits.push((path_with_path(&d.at, &path), Some(Value::String(written.last().cloned().unwrap_or_default()))));
            for h in hits {
                servers[d.index].values.insert(h.name, h.value);
            }
        }
        known_stands(&written, known, &d.name)?;
    }

    let out = if edits.is_empty() {
        text.to_string()
    } else {
        let mut root_value = Value::Object(root);
        edit_jsonc(text, &mut root_value, &edits)?
    };
    let entries: Vec<Value> = server_tables(format, &json_root(Some(&out))?, true)?
        .into_iter()
        .flat_map(|t| t.servers.into_values())
        .collect();
    still_stands(&entries, &servers)?;
    Ok(McpReferred { text: out, servers, entries })
}

/// A table of a file's servers: the file's own (`project` none) or a folder's under `projects`.
struct Table {
    at: JsonPath,
    project: Option<String>,
    servers: Map<String, Value>,
}

/// The state of one server while its references are weighed.
struct Weighed {
    table: usize,
    name: String,
    values: IndexMap<String, String>,
    mine: Vec<JsonEdit>,
    shown: Map<String, Value>,
    lost: Vec<Lost>,
    unread: Option<String>,
}

struct Lost {
    at: JsonPath,
    name: String,
    in_address: bool,
}

struct Defined {
    at: JsonPath,
    name: String,
    def: Map<String, Value>,
    index: usize,
}

/// The server being weighed, the names the file reads, and its own path.
struct Ctx<'a> {
    format: McpFormat,
    name: &'a str,
    held: &'a dyn Fn(&str, bool) -> bool,
    base: JsonPath,
}

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_string())
}

fn path_with(base: &JsonPath, name: &str) -> JsonPath {
    let mut path = base.clone();
    path.push(key(name));
    path
}

fn path_with_path(base: &JsonPath, rest: &[PathSeg]) -> JsonPath {
    let mut path = base.clone();
    path.extend(rest.iter().cloned());
    path
}

fn path_text(path: &JsonPath) -> String {
    path.iter()
        .map(|seg| match seg {
            PathSeg::Key(k) => k.clone(),
            PathSeg::Index(i) => i.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// The variable a header or an OAuth secret travels under (mcp.ts:1048 with `headerVariables`).
fn minted_header(name: &str, k: &str) -> String {
    mcp_header_variable(name, k)
}

/// A server table with every entry an object; any other shape refused (mcp.ts:925).
fn servers_at(v: &Value, at: &JsonPath) -> Result<Map<String, Value>, HostError> {
    let servers = v.as_object().ok_or_else(|| shape_refusal(at))?;
    if let Some((odd, _)) = servers.iter().find(|(_, x)| !x.is_object()) {
        return Err(shape_refusal(&path_with(at, odd)));
    }
    Ok(servers.clone())
}

/// The file's own server table, and with `folders` each folder's under `projects` (mcp.ts:925).
fn server_tables(format: McpFormat, root: &Map<String, Value>, folders: bool) -> Result<Vec<Table>, HostError> {
    let own_key = servers_key(format);
    let own_at = vec![key(own_key)];
    let own = match root.get(own_key) {
        None | Some(Value::Null) => Map::new(),
        Some(v) => servers_at(v, &own_at)?,
    };
    let mut out = vec![Table { at: own_at, project: None, servers: own }];
    if has_projects(format) && folders {
        if let Some(projects) = root.get("projects").and_then(|p| p.as_object()) {
            for (folder, held) in projects {
                let servers = held.as_object().and_then(|h| h.get(own_key));
                let Some(v) = servers.filter(|v| !v.is_null()) else {
                    continue;
                };
                let at = vec![key("projects"), key(folder), key(own_key)];
                let servers = servers_at(v, &at)?;
                out.push(Table { at, project: Some(folder.clone()), servers });
            }
        }
    }
    Ok(out)
}

/// The key of the object `mcp.servers` where the file keeps servers its agent starts outside `mcp` (mcp.ts:1236);
/// nothing where that object is empty or reads as one server called `servers`.
fn elsewhere(format: McpFormat, root: &Map<String, Value>) -> Option<String> {
    if format != McpFormat::OpencodeJson {
        return None;
    }
    let nested = root.get("mcp").and_then(|m| m.as_object()).and_then(|m| m.get("servers")).and_then(|s| s.as_object())?;
    if nested.is_empty() {
        return None;
    }
    let reads_as_one = ["type", "enabled"].iter().any(|k| nested.get(*k).is_some_and(|v| !v.is_object()));
    if reads_as_one {
        return None;
    }
    Some("mcp.servers".to_string())
}

/// The names a server's literal headers, env keys and OAuth secrets travel under (mcp.ts:939).
fn literal_names(format: McpFormat, name: &str, def: &Map<String, Value>, whole: &dyn Fn(&str) -> Option<WholeRef>, bare: &dyn Fn(&str) -> Option<WholeRef>) -> Vec<String> {
    let secrets: Vec<String> = match def.get("oauth").and_then(|o| o.as_object()) {
        None => Vec::new(),
        Some(oauth) => oauth_secret_keys(oauth)
            .into_iter()
            .filter(|k| matches!(oauth.get(k), Some(Value::String(s)) if !s.is_empty() && bare(s).is_none()))
            .collect(),
    };
    let headers = dict(def.get("headers"));
    let env = dict(def.get(env_key(format)));
    let mut out: Vec<String> = headers
        .iter()
        .filter(|(_, v)| !v.is_empty() && whole(v).is_none())
        .map(|(h, _)| mcp_header_variable(name, h))
        .collect();
    out.extend(env.iter().filter(|(_, v)| !v.is_empty() && whole(v).is_none()).map(|(k, _)| k.clone()));
    out.extend(secrets.iter().map(|k| mcp_header_variable(name, &format!("oauth.{k}"))));
    out
}

/// What a literal value becomes: `None` where it stands as written or is handled, else the text that travels
/// (mcp.ts:979).
fn literal_of(ctx: &Ctx<'_>, w: &mut Weighed, v: &str, path: &JsonPath, form: &dyn Fn(&str) -> Option<WholeRef>, where_: &str) -> Result<Option<String>, HostError> {
    let format = ctx.format;
    let reads = |n: &str| (ctx.held)(n, false);
    let named = form(v);
    if refers(v, form, &reads) {
        return Ok(None);
    }
    if let Some(r) = &named {
        if reads(&r.name) {
            w.mine.push((path_with_path(&ctx.base, path), Some(Value::String(format!("{}{}", r.scheme, ref_of(format, &r.name))))));
            return Ok(None);
        }
        if r.fallback.is_none() {
            w.lost.push(Lost { at: path_with_path(&ctx.base, path), name: r.name.clone(), in_address: false });
            return Ok(None);
        }
    }
    if named.is_none() && ref_re(format).is_match(v) {
        return Err(HostError::new(format!(
            "{}'s {where_} mixes a value with a variable, so it cannot travel by name; make it one or the other",
            ctx.name
        )));
    }
    Ok(Some(match named {
        None => v.to_string(),
        Some(r) => format!("{}{}", r.scheme, r.fallback.unwrap_or_default()),
    }))
}

/// Headers or env keys: each literal value put in place by its variable (mcp.ts:994).
fn put_entries(
    ctx: &Ctx<'_>,
    w: &mut Weighed,
    key_name: &str,
    entries: &IndexMap<String, String>,
    whole: &dyn Fn(&str) -> Option<WholeRef>,
    bearer: bool,
    variable: &dyn Fn(&str) -> String,
) -> Result<(), HostError> {
    for (k, v) in entries {
        let path = vec![key(key_name), key(k)];
        let literal = if v.is_empty() { None } else { literal_of(ctx, w, v, &path, whole, k)? };
        let Some(literal) = literal else {
            continue;
        };
        if literal.is_empty() {
            w.mine.push((path_with_path(&ctx.base, &path), Some(Value::String(String::new()))));
            continue;
        }
        let n = variable(k);
        let token = if bearer { bearer_token(k, &literal) } else { None };
        w.values.insert(n.clone(), token.clone().unwrap_or(literal));
        let shown = match token {
            Some(_) => format!("Bearer {}", ref_of(ctx.format, &n)),
            None => ref_of(ctx.format, &n),
        };
        w.mine.push((path_with_path(&ctx.base, &path), Some(Value::String(shown))));
    }
    Ok(())
}

/// OAuth: the table must be absent, false or an object; each secret the agent reads is written by its variable
/// (mcp.ts:1008).
fn oauth_entries(ctx: &Ctx<'_>, w: &mut Weighed, def: &Map<String, Value>, bare: &dyn Fn(&str) -> Option<WholeRef>) -> Result<(), HostError> {
    let format = ctx.format;
    let oauth = def.get("oauth");
    let shape_ok = match oauth {
        None | Some(Value::Bool(false)) => true,
        Some(v) => v.is_object(),
    };
    if !shape_ok {
        return Err(shape_refusal(&ctx.base));
    }
    let oauth_map = oauth.and_then(|o| o.as_object());
    let secrets: Vec<String> = oauth_map.map(oauth_secret_keys).unwrap_or_default();
    let headers = dict(def.get("headers"));
    let mut minted_names: Vec<String> = headers.keys().cloned().collect();
    minted_names.extend(secrets.iter().map(|k| format!("oauth.{k}")));
    let minted: IndexMap<String, String> = header_variables(ctx.name, &minted_names, &|h| mcp_header_variable(ctx.name, h))?;
    for k in &secrets {
        let Some(Value::String(v)) = oauth_map.and_then(|o| o.get(k)) else {
            return Err(secret_refusal(ctx, &ctx.base, k));
        };
        let path = vec![key("oauth"), key(k)];
        if v.is_empty() {
            continue;
        }
        let literal = literal_of(ctx, w, v, &path, bare, &format!("oauth.{k}"))?;
        let Some(literal) = literal else {
            continue;
        };
        if literal.is_empty() {
            w.mine.push((path_with_path(&ctx.base, &path), Some(Value::String(String::new()))));
            continue;
        }
        if !oauth_secret_names(format).contains(&k.as_str()) {
            return Err(secret_refusal(ctx, &ctx.base, k));
        }
        let n = minted.get(&format!("oauth.{k}")).cloned().unwrap_or_default();
        w.values.insert(n.clone(), literal);
        w.mine.push((path_with_path(&ctx.base, &path), Some(Value::String(ref_of(format, &n)))));
    }
    Ok(())
}

fn secret_refusal(ctx: &Ctx<'_>, base: &JsonPath, k: &str) -> HostError {
    let path = path_with_path(base, &[key("oauth"), key(k)]);
    HostError::new(format!(
        "{} keeps an OAuth secret in {}, and its agent reads no variable there, so the file stays on this computer",
        ctx.name,
        path_text(&path)
    ))
}

/// The command, arguments and address: each reference by name, a default moved under a variable of its own
/// (mcp.ts:1026).
fn argument_entries(ctx: &Ctx<'_>, w: &mut Weighed, def: &Map<String, Value>) -> Result<(), HostError> {
    let format = ctx.format;
    let re = ref_re(format);
    let bare = |v: &str| whole_ref(format, v, false);
    let env_names = dict(def.get(env_key(format)));
    for (path, v, _where) in arg_strings(def) {
        let key_name = match &path[0] {
            PathSeg::Key(k) => k.clone(),
            PathSeg::Index(_) => continue,
        };
        let index = match path.get(1) {
            Some(PathSeg::Index(i)) => Some(*i as usize),
            _ => None,
        };
        let address = key_name == "url" || key_name == "httpUrl";
        let at = path_with_path(&ctx.base, &path);
        let mut moved = 0usize;
        let mut bared = String::new();
        let mut last = 0;
        for caps in re.captures_iter(&v) {
            let whole = caps.get(0).expect("group 0 always matches");
            bared.push_str(&v[last..whole.start()]);
            last = whole.end();
            let m = whole.as_str();
            let Some(r) = bare(m) else {
                bared.push_str(m);
                continue;
            };
            let in_address = address && m != v;
            let reads = |n: &str| (ctx.held)(n, in_address);
            if refers(m, &bare, &reads) {
                bared.push_str(m);
            } else if reads(&r.name) {
                bared.push_str(&ref_of(format, &r.name));
            } else if r.fallback.is_none() {
                w.lost.push(Lost { at: at.clone(), name: r.name.clone(), in_address });
                bared.push_str(m);
            } else if address {
                if w.unread.is_none() {
                    w.unread = Some(unread_line(&at));
                }
                bared.push_str(m);
            } else if r.fallback.as_deref() == Some("") {
                // An empty default is written as nothing at all (mcp.ts:1048).
            } else {
                let mut suffix = path.clone();
                if moved > 0 {
                    suffix.push(PathSeg::Index(moved as i64));
                }
                let n = mcp_header_variable(ctx.name, &path_text(&suffix));
                moved += 1;
                if w.values.contains_key(&n) || env_names.contains_key(&n) {
                    return Err(HostError::new(format!(
                        "{} sets {n}, the name the default in {} would travel under; rename it",
                        ctx.name,
                        path_text(&at)
                    )));
                }
                w.values.insert(n.clone(), r.fallback.clone().unwrap_or_default());
                bared.push_str(&ref_of(format, &n));
            }
        }
        bared.push_str(&v[last..]);
        if bared == v {
            continue;
        }
        match index {
            None => {
                w.shown.insert(key_name.clone(), Value::String(bared.clone()));
            }
            Some(i) => {
                if let Some(Value::Array(items)) = w.shown.get_mut(&key_name) {
                    if let Some(slot) = items.get_mut(i) {
                        *slot = Value::String(bared.clone());
                    }
                }
            }
        }
        w.mine.push((at, Some(Value::String(bared))));
    }
    Ok(())
}
