// SPDX-License-Identifier: AGPL-3.0-only
//! Codex's own reads of its TOML: what a server's start hands it, and the trust table. Port of
//! packages/catalog/src/mcp-codex.ts at a3b870a6e; the spec is spec/mcp_toml_refer.md.
use std::sync::OnceLock;

use indexmap::IndexMap;
use regex::Regex;

use crate::mcp::{McpResolved, McpServer, McpTransport};
use crate::mcp_toml::toml_string;
use crate::smol_toml::parse_toml;
use wsp_words::exit::HostError;
use wsp_words::js::js_trim;

/// env_vars pass through where they have a value; each env_http_headers and bearer_token_env_var header must have one
/// (mcp-codex.ts:12).
pub fn resolve_codex(server: &McpServer, value: &dyn Fn(&str) -> Option<String>) -> McpResolved {
    let Some(reads) = &server.reads else {
        return McpResolved::Resolved { transport: server.transport.clone(), values: Vec::new() };
    };
    let mut values: Vec<String> = Vec::new();
    let mut read = |name: &str| -> Option<String> {
        let got = value(name);
        if let Some(v) = &got {
            values.push(v.clone());
        }
        got
    };
    match &server.transport {
        McpTransport::Stdio { command, args, env, cwd, tool_timeout_sec } => {
            let mut env = env.clone();
            for name in &reads.env {
                if let Some(got) = read(name) {
                    env.insert(name.clone(), got);
                }
            }
            let transport = McpTransport::Stdio {
                command: command.clone(),
                args: args.clone(),
                env,
                cwd: cwd.clone(),
                tool_timeout_sec: *tool_timeout_sec,
            };
            McpResolved::Resolved { transport, values }
        }
        McpTransport::Http { url, headers } => {
            let mut headers: IndexMap<String, String> = headers.clone();
            let mut sent: Vec<(String, String, bool)> = reads.headers.iter().map(|(h, n)| (h.clone(), n.clone(), false)).collect();
            if let Some(bearer) = &reads.bearer {
                sent.push(("Authorization".to_string(), bearer.clone(), true));
            }
            for (header, name, bearer) in sent {
                let Some(got) = read(&name) else {
                    return McpResolved::Missing(name);
                };
                headers.insert(header, if bearer { format!("Bearer {got}") } else { got });
            }
            McpResolved::Resolved { transport: McpTransport::Http { url: url.clone(), headers }, values }
        }
    }
}

/// Whether `[projects."<folder>"].trust_level` is "trusted", by a TOML parser; false where the text does not parse
/// (mcp-codex.ts:43).
pub fn codex_trusts(text: &str, folder: &str) -> bool {
    let Ok(tree) = parse_toml(text) else {
        return false;
    };
    let entry = tree.get("projects").and_then(toml::Value::as_table).and_then(|p| p.get(folder)).and_then(toml::Value::as_table);
    entry.and_then(|e| e.get("trust_level")).and_then(toml::Value::as_str) == Some("trusted")
}

/// The folder's trust written as Codex writes it (mcp-codex.ts:57).
pub fn trust_codex(text: Option<&str>, folder: &str) -> Result<String, HostError> {
    static SECTION: OnceLock<Regex> = OnceLock::new();
    static LEVEL: OnceLock<Regex> = OnceLock::new();
    let section = SECTION.get_or_init(|| Regex::new(r"^\s*\[").expect("a fixed pattern"));
    let level_re = LEVEL.get_or_init(|| Regex::new(r"^\s*trust_level\s*=").expect("a fixed pattern"));
    let was = text.unwrap_or("");
    if codex_trusts(was, folder) {
        return Ok(was.to_string());
    }
    let parsed = parse_toml(was).map_err(HostError::new)?;
    let projects = parsed.get("projects").and_then(toml::Value::as_table);
    let header = format!("[projects.{}]", toml_string(folder));
    let mut lines: Vec<String> = was.split('\n').map(str::to_string).collect();
    let at = lines.iter().position(|l| js_trim(l) == header);
    let out = match at {
        None => {
            if projects.is_some_and(|p| p.contains_key(folder)) {
                return Err(HostError::new(format!("the trust of {folder} is written in a shape wsp does not edit")));
            }
            let lead = if was.is_empty() || was.ends_with('\n') { "" } else { "\n" };
            let gap = if was.is_empty() { "" } else { "\n" };
            format!("{was}{lead}{gap}{header}\ntrust_level = \"trusted\"\n")
        }
        Some(at) => {
            let end = (at + 1..lines.len()).find(|&i| section.is_match(&lines[i])).unwrap_or(lines.len());
            match (at + 1..end).find(|&i| level_re.is_match(&lines[i])) {
                Some(level) => lines[level] = "trust_level = \"trusted\"".to_string(),
                None => lines.insert(at + 1, "trust_level = \"trusted\"".to_string()),
            }
            lines.join("\n")
        }
    };
    if parse_toml(&out).is_err() {
        return Err(HostError::new(format!(
            "the trust of {folder} cannot be added to a file that writes its projects in a shape wsp does not edit"
        )));
    }
    Ok(out)
}
