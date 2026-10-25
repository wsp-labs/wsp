// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/mcp-refer.test.ts.
use std::fmt::Debug;

use base64::Engine;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::jsonc::parse_jsonc;
use wsp_catalog::mcp::{
    McpFormat, McpReferred, McpResolved, McpTransport, ReferredServer, CODEX_TOML, GEMINI_SETTINGS_JSON, MCP_SERVERS_JSON, OPENCODE_JSON,
};
use wsp_catalog::mcp_refs::{mcp_header_variable, servers_by_name, ByName, Dropped, HeldValue, KnownEntry};
use wsp_words::exit::HostError;

const HOME: &str = "/Users/dev";
const TOKEN: &str = "lin_api_TESTONLY";
const NOTION: &str = "ntn_TESTONLY";

type Held = IndexMap<String, HeldValue>;
type Known = IndexMap<String, KnownEntry>;

fn set(names: &[&str]) -> IndexSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

fn refer(format: McpFormat, text: &str) -> Result<McpReferred, HostError> {
    format.refer(text, None, &[], &IndexSet::new())
}

fn refer_only(format: McpFormat, text: &str, only: &str) -> Result<McpReferred, HostError> {
    format.refer(text, Some(only), &[], &IndexSet::new())
}

fn values(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn server(name: &str, pairs: &[(&str, &str)]) -> ReferredServer {
    ReferredServer { name: name.to_string(), project: None, values: values(pairs), unread: None }
}

fn server_in(name: &str, project: &str, pairs: &[(&str, &str)]) -> ReferredServer {
    ReferredServer { name: name.to_string(), project: Some(project.to_string()), values: values(pairs), unread: None }
}

fn said<T: Debug>(r: Result<T, HostError>) -> String {
    match r {
        Ok(_) => String::new(),
        Err(e) => e.message,
    }
}

fn throws<T: Debug>(r: Result<T, HostError>, said: &str) {
    match r {
        Ok(v) => panic!("expected an error containing {said:?}, got {v:?}"),
        Err(e) => assert!(e.message.contains(said), "expected {said:?} in {:?}", e.message),
    }
}

fn known(pairs: &[(&str, &str)], by: &[&str]) -> Known {
    pairs
        .iter()
        .map(|(n, v)| (n.to_string(), KnownEntry { value: v.to_string(), by: by.iter().map(|b| b.to_string()).collect() }))
        .collect()
}

fn no_rows(_: &str) -> Option<String> {
    None
}

fn rows(name: &str) -> Option<String> {
    (name == "GEMINI_API_KEY").then(|| "Gemini CLI".to_string())
}

fn by_name(format: McpFormat, text: &str) -> Result<ByName, HostError> {
    servers_by_name(format, text, &mut Held::new(), &no_rows, &Known::new())
}

fn by_name_known(format: McpFormat, text: &str, known: &Known) -> Result<ByName, HostError> {
    servers_by_name(format, text, &mut Held::new(), &no_rows, known)
}

fn by_name_held(format: McpFormat, text: &str, held: &mut Held, known: &Known) -> Result<ByName, HostError> {
    servers_by_name(format, text, held, &no_rows, known)
}

fn held_list(held: &Held) -> Vec<(String, HeldValue)> {
    held.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn held_value(value: &str, by: &str) -> HeldValue {
    HeldValue { value: value.to_string(), by: by.to_string() }
}

fn dropped(name: &str, reason: &str) -> Dropped {
    Dropped { name: name.to_string(), reason: reason.to_string() }
}

fn js(v: Value) -> String {
    serde_json::to_string(&v).unwrap()
}

fn js_pretty(v: Value) -> String {
    serde_json::to_string_pretty(&v).unwrap()
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn lines(l: &[&str]) -> String {
    l.join("\n")
}

fn btoa(v: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(v)
}

fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn dollar(name: &str) -> String {
    format!("${{{name}}}")
}

fn envref(name: &str) -> String {
    format!("{{env:{name}}}")
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v.clone());
    }
    Value::Object(m)
}

fn claude(servers: Value) -> String {
    js_pretty(json!({ "mcpServers": servers }))
}

fn unread_line(path: &str) -> String {
    format!("reads a variable servers.env does not hold in {path}, so it could not start there")
}

fn first_server(format: McpFormat, text: &str) -> wsp_catalog::mcp::McpServer {
    format.read(text, HOME).into_iter().next().unwrap()
}

/// What servers_by_name says of a file whose reference writer hands back these servers and entries: the TS tests'
/// stand-in format, `{ ...FORMAT, refer: async text => ({ text, servers, entries }) }`.
fn last_check(format: McpFormat, entries: Vec<Value>, servers: Vec<ReferredServer>, known: &Known) -> Result<(), HostError> {
    let refer = |text: &str, _: Option<&str>, _: &[(String, String)], _: &indexmap::IndexSet<String>| {
        Ok::<_, HostError>(wsp_catalog::mcp::McpReferred { text: text.to_string(), servers: servers.clone(), entries: entries.clone() })
    };
    let text = if format == McpFormat::CodexToml { "[mcp_servers.s]\n" } else { "{}" };
    let mut held = IndexMap::new();
    let out = wsp_catalog::mcp_refs::servers_by_name_with(format, &refer, text, &mut held, &|_| None, known)?;
    assert_eq!(out.dropped, vec![], "a stand-in writer that drops nothing leaves nothing dropped");
    Ok(())
}

fn lax(format: McpFormat, entries: Vec<Value>, known: &Known) -> Result<(), HostError> {
    last_check(format, entries, vec![server("s", &[])], known)
}

mod part_1;
mod part_2;
