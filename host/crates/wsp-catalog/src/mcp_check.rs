// SPDX-License-Identifier: AGPL-3.0-only
//! How an agent's own harness answers for one MCP server's sign-in. Port of packages/catalog/src/mcp-check.ts at
//! a3b870a6e; the spec is spec/mcp_modules.md.
use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use crate::js_regex::JsRegex;
use wsp_wire::types::agents_report::McpAuth;
use wsp_wire::types::shell_quote::shell_quote;

/// The harness's word on one server: its sign-in, or that the config it read has no server of that name (mcp-check.ts:127).
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
        match self {
            McpCheck::Claude => "claude 2.1.296",
            McpCheck::Codex => "codex-cli 0.162.1",
        }
    }

    /// `claude mcp get '<name>'`, or `codex mcp list --json` whatever the name.
    pub fn line(self, name: &str) -> String {
        match self {
            McpCheck::Claude => format!("claude mcp get {}", shell_quote(name)),
            McpCheck::Codex => "codex mcp list --json".to_string(),
        }
    }

    /// The server's word off what the line printed; nothing where the words are not the measured ones.
    pub fn auth(self, output: &str, name: &str) -> Option<McpCheckWord> {
        match self {
            McpCheck::Claude => claude_auth(output),
            McpCheck::Codex => codex_auth(output, name),
        }
    }
}

/// The text of the first `Status:` line, without its label (mcp-check.ts:148).
fn status_of(output: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?m)^\s*Status:\s*(.*)$").expect("a fixed pattern"));
    re.captures(output).and_then(|c| c.get(1)).map_or_else(String::new, |m| m.as_str().to_string())
}

fn claude_auth(output: &str) -> Option<McpCheckWord> {
    if CLAUDE_NOT_SET_UP.is_match(output) {
        return Some(McpCheckWord::NotSetUp);
    }
    let status = status_of(output);
    if status.contains("Needs authentication") {
        Some(McpCheckWord::Auth(McpAuth::NeedsSignIn))
    } else if status.contains("Failed to connect") {
        Some(McpCheckWord::Auth(McpAuth::Failed))
    } else if status.contains("Connected") {
        Some(McpCheckWord::Auth(McpAuth::SignedIn))
    } else {
        None
    }
}

fn codex_auth(output: &str, name: &str) -> Option<McpCheckWord> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?m)^\[").expect("a fixed pattern"));
    let at = re.find(output)?.start();
    let listed: Value = serde_json::from_str(&output[at..]).ok()?;
    let entries = listed.as_array()?;
    let entry = entries.iter().find(|s| s.get("name").and_then(Value::as_str) == Some(name));
    let Some(entry) = entry else {
        return Some(McpCheckWord::NotSetUp);
    };
    match entry.get("auth_status").and_then(Value::as_str) {
        Some("o_auth") => Some(McpCheckWord::Auth(McpAuth::SignedIn)),
        Some("not_logged_in") => Some(McpCheckWord::Auth(McpAuth::NeedsSignIn)),
        _ => None,
    }
}
