// SPDX-License-Identifier: AGPL-3.0-only
//! How an agent's own harness answers for one MCP server's sign-in. Port of packages/catalog/src/mcp-check.ts at
//! a3b870a6e; the spec is spec/mcp_modules.md.
use serde::Serialize;

use crate::js_regex::JsRegex;
use crate::pending::McpAuth;

/// The harness's word on one server (mcp-check.ts:127).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpCheckWord {
    Auth(McpAuth),
    NotSetUp,
}

/// Claude Code's words for a name its config does not hold, 2.1.296's then 2.1.281's (mcp-check.ts:140).
pub const CLAUDE_NOT_SET_UP: JsRegex = JsRegex::with_flags("^No MCP server (named \"|found with name: )", "m");

/// One module per harness (mcp-check.ts:143, 159).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpCheck {
    Claude,
    Codex,
}

impl McpCheck {
    /// "claude 2.1.296" or "codex-cli 0.162.1".
    pub fn measured(self) -> &'static str {
        todo!()
    }

    /// `claude mcp get '<name>'`, or `codex mcp list --json` whatever the name.
    pub fn line(self, name: &str) -> String {
        todo!()
    }

    /// The server's word off what the line printed; nothing where the words are not the measured ones.
    pub fn auth(self, output: &str, name: &str) -> Option<McpCheckWord> {
        todo!()
    }
}
