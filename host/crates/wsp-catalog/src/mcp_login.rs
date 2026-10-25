// SPDX-License-Identifier: AGPL-3.0-only
//! How an agent's harness signs one MCP server in, and whether a server needs a credential a cloud machine never holds.
//! Port of packages/catalog/src/mcp-login.ts at a3b870a6e; the spec is spec/mcp_modules.md.
use std::sync::OnceLock;

use indexmap::IndexSet;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::js_regex::JsRegex;
use crate::mcp::{McpServer, McpTransport};
use crate::mcp_check::CLAUDE_NOT_SET_UP;
use wsp_wire::types::agents_report::PageReach;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::js::js_trim;

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
        match self {
            McpLogin::Claude => "claude 2.1.282",
            McpLogin::Codex => "codex-cli 0.155.1",
            McpLogin::Opencode => "opencode 1.18.18",
            McpLogin::Gemini => "gemini docs",
        }
    }

    /// The command run in a pty; `None` for Gemini, which has only `inside`.
    pub fn command(self, name: &str) -> Option<String> {
        match self {
            McpLogin::Claude => Some(format!("claude mcp login {}", shell_quote(name))),
            McpLogin::Codex => Some(format!("codex mcp login {}", shell_quote(name))),
            McpLogin::Opencode => Some(format!("opencode mcp auth {}", shell_quote(name))),
            McpLogin::Gemini => None,
        }
    }

    /// The no-browser variant whose page hands back an address; Claude Code's alone.
    pub fn pasted(self, name: &str) -> Option<String> {
        match self {
            McpLogin::Claude => Some(format!("claude mcp login {} --no-browser", shell_quote(name))),
            _ => None,
        }
    }

    /// The command's words for a name its config does not hold; Claude Code's and Codex's.
    pub fn not_set_up(self) -> Option<JsRegex> {
        match self {
            McpLogin::Claude => Some(CLAUDE_NOT_SET_UP),
            McpLogin::Codex => Some(JsRegex::with_flags("^Error: No MCP server named '", "m")),
            _ => None,
        }
    }

    /// The line typed inside the harness: Gemini's `/mcp auth <name>`, unquoted.
    pub fn inside(self, name: &str) -> Option<String> {
        match self {
            McpLogin::Gemini => Some(format!("/mcp auth {name}")),
            _ => None,
        }
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
    format!("A cloud machine holds no sign-in, so {name} cannot be signed in on it")
}

pub const BROKERED_SERVER_SIGN_IN_FIX: &str = "Cloud threads run without its tools; sign it in on your own computer for the threads there.";

/// Names every login sets, which a turn on a cloud machine reads there (mcp-login.ts:272).
const LOGIN_NAMES: [&str; 8] = ["HOME", "PATH", "USER", "LOGNAME", "SHELL", "PWD", "TMPDIR", "LANG"];

fn is_key(name: &str) -> bool {
    !LOGIN_NAMES.contains(&name)
}

/// The variable names a command server's env value reads, in `${NAME}` or `{env:NAME}` (mcp-login.ts:268).
fn env_reads(value: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}|\{env:([A-Za-z_][A-Za-z0-9_]*)\}").expect("a fixed pattern"));
    re.captures_iter(value).filter_map(|c| c.get(1).or(c.get(2)).map(|m| m.as_str().to_string())).collect()
}

/// A word as the program it names, without its folder or its `@version`, `==version` or `:tag` pin (mcp-login.ts:276).
fn program_word(word: &str) -> String {
    let base = match word.rfind('/') {
        Some(i) => &word[i + 1..],
        None => word,
    };
    let cut = base.char_indices().find(|&(i, c)| c == '@' || c == ':' || base[i..].starts_with("==")).map(|(i, _)| i);
    cut.map_or(base, |i| &base[..i]).to_string()
}

/// Programs that sign every request with a CLI's login unless the flag sends them unsigned (mcp-login.ts:272).
fn signs_with_login(command: &str, args: &[String]) -> bool {
    const SIGNS_WITH_LOGIN: [(&str, &str); 2] = [("mcp-proxy-for-aws", "--skip-auth"), ("mcp-proxy-for-aws-cli", "--skip-auth")];
    let words: Vec<String> =
        std::iter::once(command).chain(args.iter().map(String::as_str)).flat_map(str::split_whitespace).map(program_word).collect();
    SIGNS_WITH_LOGIN.iter().any(|(program, unsigned)| words.iter().any(|w| w == program) && !words.iter().any(|w| w == unsigned))
}

/// Whether a header's value is empty once a leading scheme word is taken off (mcp-login.ts:281).
fn header_is_empty(value: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?i)^\s*(?:Bearer|Basic|Token)(?-u:\b)").expect("a fixed pattern"));
    js_trim(&re.replace(value, "")).is_empty()
}

/// Whether a server needs a sign-in or a key of the person's, which a cloud machine never holds (mcp-login.ts:277);
/// `signs_in` is the unsigned initialize's answer for an address, nothing where nobody asked.
pub fn needs_own_credential(server: &McpServer, signs_in: &dyn Fn(&str) -> Option<bool>) -> bool {
    if server.env_refs.iter().any(|n| is_key(n)) {
        return true;
    }
    match &server.transport {
        McpTransport::Stdio { command, args, env, .. } => {
            signs_with_login(command, args) || env.values().any(|v| env_reads(v).iter().any(|n| is_key(n)))
        }
        McpTransport::Http { url, headers } => {
            if headers.is_empty() {
                signs_in(url) == Some(true)
            } else {
                headers.values().any(|v| header_is_empty(v))
            }
        }
    }
}

/// The addresses only an unsigned initialize can tell, once each in first-seen order (mcp-login.ts:286).
pub fn addresses_to_ask(servers: &[McpServer]) -> Vec<String> {
    let mut seen = IndexSet::new();
    for s in servers {
        if let McpTransport::Http { url, headers } = &s.transport {
            if headers.is_empty() && !s.env_refs.iter().any(|n| is_key(n)) {
                seen.insert(url.clone());
            }
        }
    }
    seen.into_iter().collect()
}

/// The road for one server; a usage refusal on a brokered machine (mcp-login.ts:290).
pub fn login_road(login: McpLogin, name: &str, reach: PageReach, brokered: bool) -> Result<ServerSignInRoad, HostError> {
    if brokered {
        return Err(usage_refusal(&brokered_server_sign_in_line(name), BROKERED_SERVER_SIGN_IN_FIX));
    }
    if let Some(line) = login.inside(name) {
        return Ok(ServerSignInRoad::Copy { line, why: CopyWhy::Inside });
    }
    let Some(command) = login.command(name) else {
        return Err(HostError::new(format!("the sign-in of {name} has no command or line in this harness")));
    };
    if reach != PageReach::None {
        return Ok(ServerSignInRoad::Pty { command, finish: PtyFinish::Callback });
    }
    Ok(match login.pasted(name) {
        Some(command) => ServerSignInRoad::Pty { command, finish: PtyFinish::Code },
        None => ServerSignInRoad::Copy { line: command, why: CopyWhy::Callback },
    })
}
