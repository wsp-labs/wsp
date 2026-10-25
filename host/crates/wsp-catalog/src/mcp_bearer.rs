// SPDX-License-Identifier: AGPL-3.0-only
//! The name Codex reads a bearer header by. Port of packages/catalog/src/mcp-bearer.ts at a3b870a6e; spec/jsonc.md.
use indexmap::IndexMap;

/// `<variable>_BEARER` (mcp-bearer.ts:8).
pub fn mcp_bearer_variable(variable: &str) -> String {
    todo!()
}

/// The header token a bearer name is read off: a name of the form `WSP_MCP_<x>_AUTHORIZATION_BEARER` with `<x>`
/// non-empty; nothing for any other name (mcp-bearer.ts:13).
pub fn bearer_of(name: &str) -> Option<String> {
    todo!()
}

/// Each header token's bearer name, `Bearer <value>`, ahead of the values themselves; a value named like a bearer
/// keeps the bearer's place and its own value (mcp-bearer.ts:19, an object spread).
pub fn with_bearer_variables(values: &IndexMap<String, String>) -> IndexMap<String, String> {
    todo!()
}
