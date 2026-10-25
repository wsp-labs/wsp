// SPDX-License-Identifier: AGPL-3.0-only
//! Codex's TOML, read and edited line by line in the table form Codex documents, so comments and order survive. Port of
//! packages/catalog/src/mcp.ts:1257-1757 less the reference writer, at a3b870a6e; the spec is spec/mcp_toml.md.
use indexmap::IndexMap;

use crate::mcp::{McpEditScope, McpEdited, McpMergeScope, McpMerged, McpServer, McpTransport, RewriteString};
use wsp_words::exit::HostError;

/// A line without its trailing comment: `#` outside quotes ends it (mcp.ts:1263).
pub fn uncomment_toml(line: &str) -> String {
    todo!()
}

/// `[mcp_servers.<name>]` or `[mcp_servers.<name>.<sub>]`, the name bare or quoted; nothing for another header
/// (mcp.ts:1277).
pub fn toml_header(line: &str) -> Option<(String, Option<String>)> {
    todo!()
}

/// Every string in the text, basic strings decoded (mcp.ts:1306).
pub fn toml_strings(s: &str) -> Vec<String> {
    todo!()
}

/// Whether a value read so far is an array closing on a later line (mcp.ts:1311).
pub fn toml_open_array(value: &str) -> bool {
    todo!()
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
    todo!()
}

/// A TOML basic string as `JSON.stringify` writes it (mcp.ts:1388).
pub fn toml_string(s: &str) -> String {
    todo!()
}

/// `{ "k" = "v", ... }` (mcp.ts:1391).
pub fn toml_inline(d: &IndexMap<String, String>) -> String {
    todo!()
}

/// A line with every string on it put through `to`, re-encoded only where changed (mcp.ts:1423).
pub fn rewrite_toml_line(line: &str, to: &dyn Fn(&str) -> String) -> String {
    todo!()
}

/// The server each line belongs to (mcp.ts:1440).
pub fn codex_owners(lines: &[&str]) -> Vec<Option<String>> {
    todo!()
}

/// The `[mcp_servers.*]` tables and their sub-tables, in file order (mcp.ts:1339).
pub fn read_codex(text: &str) -> Vec<McpServer> {
    todo!()
}

/// The table replaced in place or appended after a blank line (mcp.ts:1407).
pub fn place_codex(text: Option<&str>, name: &str, server: &McpTransport) -> String {
    todo!()
}

/// The editor (mcp.ts:1484).
pub fn codex_editor(lib: RewriteString<'_>, scope: &McpEditScope, before: &str) -> McpEdited {
    todo!()
}

/// The merge (mcp.ts:1512).
pub fn codex_merge(lib: RewriteString<'_>, scope: &McpMergeScope, own: Option<&str>, travelled: &str) -> McpMerged {
    todo!()
}

/// The remove (mcp.ts:1564).
pub fn remove_codex(text: &str, names: &[String]) -> String {
    todo!()
}

/// The switch; errors "the file names no server called <name>" (mcp.ts:1578).
pub fn enable_codex(text: &str, name: &str, on: bool) -> Result<String, HostError> {
    todo!()
}

/// `only` (mcp.ts:1741).
pub fn only_codex(text: &str, names: &[String]) -> String {
    todo!()
}

/// `entryOf` (mcp.ts:1746).
pub fn entry_of_codex(text: &str, name: &str) -> Option<String> {
    todo!()
}
