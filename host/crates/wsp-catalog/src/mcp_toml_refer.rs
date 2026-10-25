// SPDX-License-Identifier: AGPL-3.0-only
//! Codex's reference writer: the file read whole by a TOML parser, rewritten by name, the line editor's text kept where
//! it parses to the same tree. Port of packages/catalog/src/mcp.ts:1596-1735 at a3b870a6e; spec/mcp_toml_refer.md.
use indexmap::IndexSet;

use crate::mcp::{KnownValue, McpReferred};
use wsp_words::exit::HostError;

/// The key a table line sets, unquoted (mcp.ts:1597).
pub fn toml_key(line: &str) -> Option<String> {
    todo!()
}

/// The text edited line by line toward the referred tree (mcp.ts:1606).
pub fn refer_codex_lines(text: &str, only: Option<&str>) -> String {
    todo!()
}

/// `CODEX_TOML.refer` (mcp.ts:1661). Errors "the file is not valid TOML (<first line of the parser's message>)".
pub fn refer_codex(text: &str, only: Option<&str>, known: &[KnownValue], outside: &IndexSet<String>) -> Result<McpReferred, HostError> {
    todo!()
}
