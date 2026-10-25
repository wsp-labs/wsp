// SPDX-License-Identifier: AGPL-3.0-only
//! The three JSON formats' reads and edits: Claude Code's `mcpServers` (with `projects`), Gemini CLI's settings (with
//! `mcp.excluded`) and OpenCode's `mcp`. Port of packages/catalog/src/mcp.ts:470-1255 less the reference writer, at
//! a3b870a6e; the spec is spec/mcp_json.md. Every function takes one of the three JSON formats and panics on Codex's.
use crate::mcp::{McpEditScope, McpEdited, McpFormat, McpMergeScope, McpMerged, McpResolved, McpServer, McpTransport, RewriteString, WholeRef};
use crate::pending::HostError;

/// The key a format's servers sit under: `mcpServers` or `mcp` (mcp.ts:1184, 1229).
pub fn servers_key(format: McpFormat) -> &'static str {
    todo!()
}

/// Every server the text defines; text that does not parse or is not an object defines none. Gemini's names in
/// `mcp.excluded` read disabled; a folder's own servers under `projects[home]` follow the file's own, scope `home`
/// (mcp.ts:1118).
pub fn read(format: McpFormat, text: &str, home: &str) -> Vec<McpServer> {
    todo!()
}

/// A new file is `{"<key>": {"<name>": entry}}` pretty with a newline; else the entry put in place (mcp.ts:1131).
/// Errors "the file is not valid JSON; add the server by hand" and "the file is not a JSON object; add the server by
/// hand" (mcp.ts:479, 481).
pub fn place(format: McpFormat, text: Option<&str>, name: &str, server: &McpTransport) -> Result<String, HostError> {
    todo!()
}

/// The entry a format writes for a placed server (mcp.ts:1203, 1253).
pub fn entry(format: McpFormat, server: &McpTransport) -> serde_json::Value {
    todo!()
}

/// The edit the import runs (mcp.ts:850).
pub fn edit(format: McpFormat, lib: RewriteString<'_>, scope: &McpEditScope, text: &str) -> Result<McpEdited, HostError> {
    todo!()
}

/// A copy holding the named servers alone, pretty with a newline (mcp.ts:1138).
pub fn only(format: McpFormat, text: &str, names: &[String], from: Option<&str>, to: Option<&str>) -> Result<String, HostError> {
    todo!()
}

/// The canonical text of one server's entry (mcp.ts:1145).
pub fn entry_of(format: McpFormat, text: &str, name: &str, project: Option<&str>) -> Option<String> {
    todo!()
}

/// wsp's servers merged into the agent's own file (mcp.ts:773).
pub fn merge(format: McpFormat, lib: RewriteString<'_>, scope: &McpMergeScope, own: Option<&str>, travelled: &str) -> Result<McpMerged, HostError> {
    todo!()
}

/// The named servers taken out, the table left standing; a name Gemini switched off in `mcp.excluded` is switched
/// back on as it goes (mcp.ts:819).
pub fn remove(format: McpFormat, text: &str, names: &[String], project: Option<&str>) -> Result<String, HostError> {
    todo!()
}

/// The switch: OpenCode's `enabled`, Gemini's `mcp.excluded`; `None` for Claude Code's file (mcp.ts:1169).
pub fn enable(format: McpFormat, text: &str, name: &str, on: bool, project: Option<&str>) -> Option<Result<String, HostError>> {
    todo!()
}

/// Each reference in env, headers and the address filled from `value` or its default (mcp.ts:1093).
pub fn resolve(format: McpFormat, server: &McpServer, value: &dyn Fn(&str) -> Option<String>) -> McpResolved {
    todo!()
}

/// `${X}` or `{env:X}` (mcp.ts:1188, 1233).
pub fn ref_of(format: McpFormat, variable: &str) -> String {
    todo!()
}

/// The string as one whole reference, a bearer's scheme read only where `bearer` (mcp.ts:910).
pub fn whole_ref(format: McpFormat, value: &str, bearer: bool) -> Option<WholeRef> {
    todo!()
}

/// Each variable `values` name in the format's syntax, first seen first (mcp.ts:464).
pub fn refs_in(format: McpFormat, values: &[String]) -> Vec<String> {
    todo!()
}
