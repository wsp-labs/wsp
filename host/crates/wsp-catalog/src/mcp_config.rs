// SPDX-License-Identifier: AGPL-3.0-only
//! What an agent entry registers about its MCP servers. Port of packages/catalog/src/mcp-config.ts at a3b870a6e; the
//! spec is spec/mcp_modules.md.
use serde::Serialize;

use crate::mcp::McpFormat;
use crate::mcp_check::McpCheck;
use crate::mcp_codex::codex_trusts;
use crate::mcp_login::McpLogin;
use crate::mcp_oauth::McpOAuthStore;

/// A top-level key a config needs on a Linux computer with no keyring (mcp-config.ts:112).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeadlessKey {
    pub key: String,
    pub value: String,
}

/// Whether the agent reads a project's own file in a folder, off its own config's text: Codex's trust table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrustRead {
    Codex,
}

/// mcp-config.ts:93. Serialized as the oracle fixture writes it, each module by its id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    pub format: McpFormat,
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_files: Option<Vec<String>>,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_auth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<McpCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<McpLogin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<McpOAuthStore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headless: Option<Vec<HeadlessKey>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trusts: Option<TrustRead>,
}

impl McpConfig {
    /// Whether the agent reads a project's own file in `folder`: `None` where it always does; for Codex false with no
    /// user file, else `codex_trusts` (agents/codex.ts:147).
    pub fn trusts(&self, user: Option<&str>, folder: &str) -> Option<bool> {
        match self.trusts? {
            TrustRead::Codex => Some(match user {
                None => false,
                Some(text) => codex_trusts(text, folder),
            }),
        }
    }
}
