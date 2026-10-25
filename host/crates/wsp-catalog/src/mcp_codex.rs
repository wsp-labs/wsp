// SPDX-License-Identifier: AGPL-3.0-only
//! Codex's own reads of its TOML: what a server's start hands it, and the trust table. Port of
//! packages/catalog/src/mcp-codex.ts at a3b870a6e; the spec is spec/mcp_toml_refer.md.
use crate::mcp::{McpResolved, McpServer};
use wsp_words::exit::HostError;

/// env_vars pass through where they have a value; each env_http_headers and bearer_token_env_var header must have one
/// (mcp-codex.ts:12).
pub fn resolve_codex(server: &McpServer, value: &dyn Fn(&str) -> Option<String>) -> McpResolved {
    todo!()
}

/// Whether `[projects."<folder>"].trust_level` is "trusted", by a TOML parser; false where the text does not parse
/// (mcp-codex.ts:43).
pub fn codex_trusts(text: &str, folder: &str) -> bool {
    todo!()
}

/// The folder's trust written as Codex writes it (mcp-codex.ts:57).
pub fn trust_codex(text: Option<&str>, folder: &str) -> Result<String, HostError> {
    todo!()
}
