// SPDX-License-Identifier: AGPL-3.0-only
//! The wsp-wire and wsp-words items the catalog reads, declared here until those crates land on this branch: the merge
//! deletes this file and points each `use crate::pending::X` at the real item. Constants carry their TS values; every
//! function is the other crate's to implement, at the TS line named on it, all at a3b870a6e.
use serde::{Deserialize, Serialize};

/// The host's one error (spec chapter 05, proposal 4): the sentence, the fix where it is a refusal, and its kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct HostError {
    pub message: String,
    pub fix: Option<String>,
    pub kind: Option<Kind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Usage,
    Invalid,
    NotFound,
}

impl HostError {
    /// A thrown `new Error(message)`: no kind, no fix.
    pub fn plain(message: impl Into<String>) -> HostError {
        HostError { message: message.into(), fix: None, kind: None }
    }
}

/// protocol/src/exit.ts:106 with refusalLine of protocol/src/words/thread.ts:393.
pub fn usage_refusal(happened: &str, fix: &str) -> HostError {
    let said = happened.trim_end_matches(js_space);
    let stop = said.ends_with(['.', '!', '?', ':']);
    HostError { message: format!("{said}{} {fix}", if stop { "" } else { "." }), fix: Some(fix.to_string()), kind: Some(Kind::Usage) }
}

/// protocol/src/shell-quote.ts:4.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// protocol/src/agents-report.ts:211.
pub fn config_changed_refusal(file: &str) -> String {
    format!("{file} changed while wsp was writing it, so nothing was written; try again.")
}

/// protocol/src/agents-report.ts:207.
pub fn config_hard_link_refusal(file: &str) -> String {
    format!("{file} has another hard link, which a write by rename would split from it, so nothing was written. See the link count with ls -li {file} and find the other name with find / -xdev -samefile {file}, since a hard link stays on its own volume; remove that name or give {file} a copy of its own, then run the command again.")
}

/// protocol/src/agents-report.ts:191.
pub fn config_link_refusal(file: &str, to: &str) -> String {
    format!("{file} is a link to {to}, outside the folder it belongs to, so wsp does not write through it.")
}

/// protocol/src/project-path.ts:16.
pub fn under_project(path: &str, root: &str) -> bool {
    path == root || path.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
}

/// protocol/src/wire/place-link.ts:357.
pub fn host_under(host: &str, domain: &str) -> bool {
    let without_port = match host.rfind(':') {
        Some(at) if host[at + 1..].bytes().all(|b| b.is_ascii_digit()) => &host[..at],
        _ => host,
    };
    let lower = without_port.to_lowercase();
    let name = lower.strip_suffix('.').unwrap_or(&lower);
    name == domain || name.ends_with(&format!(".{domain}"))
}

/// protocol/src/views/golden-image.ts:445, the row read by its id.
pub fn agent_of_row(id: &str) -> Option<String> {
    if id.starts_with("agents/mcp/") {
        return None;
    }
    id.strip_prefix("agents/").map(String::from)
}

/// protocol/src/views/golden-image.ts:456, the row read by its id.
pub fn package_of(id: &str) -> String {
    id.split('/').skip(2).collect::<Vec<_>>().join("/")
}

/// protocol/src/views/golden-image.ts:448.
pub fn tool_row_prefix(manager: &str) -> String {
    format!("tools/{manager}/")
}

/// protocol/src/semver.mjs:32: negative, zero or positive.
pub fn compare_versions(a: &str, b: &str) -> i64 {
    fn parts(version: &str) -> (Vec<&str>, Option<Vec<&str>>) {
        let rest = version.split('+').next().unwrap_or("");
        match rest.find('-') {
            None => (rest.split('.').collect(), None),
            Some(cut) => (rest[..cut].split('.').collect(), Some(rest[cut + 1..].split('.').collect())),
        }
    }
    fn numeric(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
    }
    fn identifiers(a: &str, b: &str) -> i64 {
        match (numeric(a), numeric(b)) {
            (true, true) => {
                let (x, y) = (a.parse::<f64>().unwrap_or(f64::INFINITY), b.parse::<f64>().unwrap_or(f64::INFINITY));
                if x < y {
                    -1
                } else if x > y {
                    1
                } else {
                    0
                }
            }
            (true, false) => -1,
            (false, true) => 1,
            (false, false) => utf16_cmp(a, b) as i64,
        }
    }
    let ((left_core, left_pre), (right_core, right_pre)) = (parts(a), parts(b));
    let (left_has_pre, right_has_pre) = (left_pre.is_some(), right_pre.is_some());
    for i in 0..left_core.len().max(right_core.len()) {
        let order = identifiers(left_core.get(i).copied().unwrap_or("0"), right_core.get(i).copied().unwrap_or("0"));
        if order != 0 {
            return order;
        }
    }
    let (Some(left_pre), Some(right_pre)) = (left_pre, right_pre) else {
        return match (left_has_pre, right_has_pre) {
            (false, false) => 0,
            (false, true) => 1,
            _ => -1,
        };
    };
    for i in 0..left_pre.len().max(right_pre.len()) {
        let (Some(l), Some(r)) = (left_pre.get(i), right_pre.get(i)) else {
            return if left_pre.get(i).is_none() { -1 } else { 1 };
        };
        let order = identifiers(l, r);
        if order != 0 {
            return order;
        }
    }
    0
}

/// protocol/src/words/computer.ts:27.
pub fn this_computer(platform: Platform) -> String {
    match platform {
        Platform::Darwin => "this Mac".to_string(),
        Platform::Linux => "this computer".to_string(),
    }
}

/// protocol/src/words/image.ts:188.
pub fn pin_mismatch_line(what: &str, tag: &str, recorded: &str, served: &str) -> String {
    format!("{what} at {tag} does not match the checksum recorded on its first install: recorded {recorded}, served {served}")
}

/// protocol/src/words/image.ts:184.
pub fn short_sum(sha256: &str) -> String {
    let mut units = 0;
    let mut out = String::new();
    for c in sha256.chars() {
        units += c.len_utf16();
        if units > SUM_SHOWN {
            break;
        }
        out.push(c);
    }
    out
}

/// protocol/src/agents-report.ts:272: why a missing plugin loads in no turn there.
pub fn plugin_missing_line(id: &str, marketplace: &str, missing: Option<PluginMissing>, on: bool, agent: &str, computer: &str) -> String {
    if missing == Some(PluginMissing::Folder) {
        return format!("{agent} has {id} {}, but its folder is not on {computer}, so no turn loads it.", if on { "on" } else { "installed" });
    }
    if missing == Some(PluginMissing::Uninstalled) {
        return format!("The config.toml on {computer} names {id}, but {agent} has it in its marketplace {marketplace} and not installed, so no turn loads it.");
    }
    let unlisted = format!("The config.toml on {computer} names {id}, but {agent} lists no such plugin there.");
    if missing == Some(PluginMissing::Marketplace) {
        format!("{unlisted} Its marketplace, {marketplace}, is a folder that is not on {computer}.")
    } else {
        unlisted
    }
}

/// protocol/src/projects.ts:132, `OWNER_REPO.test(s)`.
pub fn is_owner_repo(s: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let part = |p: &str| {
        let mut chars = p.chars();
        chars.next().is_some_and(|c| word(c) || c == '.') && chars.all(|c| word(c) || c == '.' || c == '-')
    };
    s.split_once('/').is_some_and(|(owner, name)| part(owner) && part(name))
}

/// JavaScript's `trim()`: spec rule 8.6, `wsp_words::js_trim`.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(js_space)
}

fn js_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

/// UTF-16 order of a bare `.sort()`: spec rule 8.8, `wsp_words::utf16::cmp`.
pub fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

pub const SUM_SHOWN: usize = 12;
pub const TOOL_LINKS_DIR: &str = "/usr/local/bin";
pub const HOMEBREW_HOME: &str = "/home/linuxbrew";
pub const HOMEBREW_PREFIX: &str = "/home/linuxbrew/.linuxbrew";
pub const PNPM_HOME: &str = "/root/.local/share/pnpm";
pub const TOOL_PREFIX: &str = "/opt/wsp";
pub const PLUGIN_SEED_ENV: &str = "CLAUDE_CODE_PLUGIN_SEED_DIR";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Darwin,
    Linux,
}

/// protocol/src/views/golden-image.ts:126.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ToolPin {
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<Arch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    #[serde(rename = "x86_64")]
    X86_64,
    #[serde(rename = "aarch64")]
    Aarch64,
}

/// protocol/src/agents-report.ts:229.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PluginBrings {
    pub skills: Vec<String>,
    pub commands: Vec<String>,
    pub subagents: Vec<String>,
    pub hooks: Vec<String>,
    pub servers: Vec<String>,
    pub lsp: Vec<String>,
    pub apps: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginScope {
    User,
    Project,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginMissing {
    Folder,
    Marketplace,
    Unlisted,
    Uninstalled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSignInKind {
    ApiKey,
    Subscription,
    Oauth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignInFinish {
    Callback,
    Code,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignInRoad {
    Device,
    Code,
    Key,
    Token,
    Terminal,
    None,
}

/// protocol/src/views/place-setup-words.ts:173.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignInWay {
    Token,
    Key,
    Machine,
}

/// protocol/src/views/workspace.ts:364.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarryModule {
    pub id: String,
    pub lockfiles: Vec<String>,
    pub carry: Vec<String>,
    pub never: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<String>,
}

/// protocol/src/wire/daemon.ts:508.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpBearer {
    pub file: String,
    pub at: Vec<String>,
    pub field: String,
}

/// protocol/src/views/preferences.ts:57.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EditorId {
    Vscode,
    Cursor,
    VscodeInsiders,
    Zed,
    Idea,
    Webstorm,
    Pycharm,
    Goland,
    Rustrover,
    Clion,
    Phpstorm,
    Rubymine,
    Rider,
    Finder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageReach {
    Here,
    Relay,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpAuth {
    Open,
    EnvKey,
    Connected,
    SignedIn,
    NeedsSignIn,
    Failed,
    Unknown,
}

/// protocol/src/agents-report.ts:308.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum McpRowTransport {
    Stdio { line: String },
    Http { host: String },
}
