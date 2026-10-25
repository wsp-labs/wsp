// SPDX-License-Identifier: AGPL-3.0-only
//! How an agent's harness signs one MCP server in, and whether a server needs a credential a cloud machine never holds.
//! Port of packages/catalog/src/mcp-login.ts at a3b870a6e; the spec is spec/mcp_modules.md.
use serde::{Deserialize, Serialize};

use crate::js_regex::JsRegex;
use crate::mcp::McpServer;
use wsp_words::exit::HostError;
use wsp_wire::types::agents_report::PageReach;

/// One module per harness (mcp-login.ts:229-243): a command, or a line typed inside the harness's own session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpLogin {
    Claude,
    Codex,
    Opencode,
    Gemini,
}

impl McpLogin {
    pub fn measured(self) -> &'static str {
        todo!()
    }

    /// The command run in a pty; `None` for Gemini, which has only `inside`.
    pub fn command(self, name: &str) -> Option<String> {
        todo!()
    }

    /// The no-browser variant whose page hands back an address; Claude Code's alone.
    pub fn pasted(self, name: &str) -> Option<String> {
        todo!()
    }

    /// The command's words for a name its config does not hold; Claude Code's and Codex's.
    pub fn not_set_up(self) -> Option<JsRegex> {
        todo!()
    }

    /// The line typed inside the harness: Gemini's `/mcp auth <name>`, unquoted.
    pub fn inside(self, name: &str) -> Option<String> {
        todo!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PtyFinish {
    Code,
    Callback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CopyWhy {
    Inside,
    Callback,
}

/// Where one server's sign-in runs (mcp-login.ts:247).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ServerSignInRoad {
    Pty { command: String, finish: PtyFinish },
    Copy { line: String, why: CopyWhy },
}

/// mcp-login.ts:251.
pub fn brokered_server_sign_in_line(name: &str) -> String {
    todo!()
}

pub const BROKERED_SERVER_SIGN_IN_FIX: &str = "Cloud threads run without its tools; sign it in on your own computer for the threads there.";

/// Whether a server needs a sign-in or a key of the person's (mcp-login.ts:277); `signs_in` is the unsigned
/// initialize's answer for an address, nothing where nobody asked.
pub fn needs_own_credential(server: &McpServer, signs_in: &dyn Fn(&str) -> Option<bool>) -> bool {
    todo!()
}

/// The addresses only an unsigned initialize can tell, once each in first-seen order (mcp-login.ts:286).
pub fn addresses_to_ask(servers: &[McpServer]) -> Vec<String> {
    todo!()
}

/// The road for one server; a usage refusal on a brokered machine (mcp-login.ts:290).
pub fn login_road(login: McpLogin, name: &str, reach: PageReach, brokered: bool) -> Result<ServerSignInRoad, HostError> {
    todo!()
}
