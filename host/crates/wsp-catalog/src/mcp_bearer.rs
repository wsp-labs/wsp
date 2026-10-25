// SPDX-License-Identifier: AGPL-3.0-only
//! The name Codex reads a bearer header by. Port of packages/catalog/src/mcp-bearer.ts at a3b870a6e; spec/jsonc.md.
use indexmap::IndexMap;

/// `<variable>_BEARER` (mcp-bearer.ts:8).
pub fn mcp_bearer_variable(variable: &str) -> String {
    format!("{variable}_BEARER")
}

/// The header token a bearer name is read off: a name of the form `WSP_MCP_<x>_AUTHORIZATION` with `<x>`
/// non-empty; nothing for any other name (mcp-bearer.ts:13).
pub fn bearer_of(name: &str) -> Option<String> {
    let token = name.strip_suffix("_BEARER")?;
    header_token(token).then(|| token.to_string())
}

/// Each header token's bearer name, `Bearer <value>`, ahead of the values themselves; a value named like a bearer
/// keeps the bearer's place and its own value (mcp-bearer.ts:19, an object spread).
pub fn with_bearer_variables(values: &IndexMap<String, String>) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    for (name, value) in values {
        if header_token(name) {
            out.insert(mcp_bearer_variable(name), format!("Bearer {value}"));
        }
    }
    for (name, value) in values {
        out.insert(name.clone(), value.clone());
    }
    out
}

/// `/^WSP_MCP_.+_AUTHORIZATION$/`: the middle is one or more characters, none of them a JavaScript line end.
fn header_token(name: &str) -> bool {
    name.strip_prefix("WSP_MCP_")
        .and_then(|rest| rest.strip_suffix("_AUTHORIZATION"))
        .is_some_and(|middle| !middle.is_empty() && !middle.contains(['\n', '\r', '\u{2028}', '\u{2029}']))
}
