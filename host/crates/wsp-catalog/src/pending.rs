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
    todo!()
}

/// protocol/src/shell-quote.ts:4.
pub fn shell_quote(value: &str) -> String {
    todo!()
}

/// protocol/src/agents-report.ts:211.
pub fn config_changed_refusal(file: &str) -> String {
    todo!()
}

/// protocol/src/agents-report.ts:207.
pub fn config_hard_link_refusal(file: &str) -> String {
    todo!()
}

/// protocol/src/agents-report.ts:191.
pub fn config_link_refusal(file: &str, to: &str) -> String {
    todo!()
}

/// protocol/src/project-path.ts:16.
pub fn under_project(path: &str, root: &str) -> bool {
    todo!()
}

/// protocol/src/wire/place-link.ts:357.
pub fn host_under(host: &str, domain: &str) -> bool {
    todo!()
}

/// protocol/src/views/golden-image.ts:445, the row read by its id.
pub fn agent_of_row(id: &str) -> Option<String> {
    todo!()
}

/// protocol/src/views/golden-image.ts:456, the row read by its id.
pub fn package_of(id: &str) -> String {
    todo!()
}

/// protocol/src/views/golden-image.ts:448.
pub fn tool_row_prefix(manager: &str) -> String {
    todo!()
}

/// protocol/src/semver.mjs:32: negative, zero or positive.
pub fn compare_versions(a: &str, b: &str) -> i64 {
    todo!()
}

/// protocol/src/words/computer.ts:27.
pub fn this_computer(platform: Platform) -> String {
    todo!()
}

/// protocol/src/words/image.ts:188.
pub fn pin_mismatch_line(what: &str, tag: &str, recorded: &str, served: &str) -> String {
    todo!()
}

/// protocol/src/words/image.ts:184.
pub fn short_sum(sha256: &str) -> String {
    todo!()
}

/// protocol/src/agents-report.ts:272: why a missing plugin loads in no turn there.
pub fn plugin_missing_line(id: &str, marketplace: &str, missing: Option<PluginMissing>, on: bool, agent: &str, computer: &str) -> String {
    todo!()
}

/// protocol/src/projects.ts:132, `OWNER_REPO.test(s)`.
pub fn is_owner_repo(s: &str) -> bool {
    todo!()
}

/// JavaScript's `trim()`: spec rule 8.6, `wsp_words::js_trim`.
pub fn js_trim(s: &str) -> &str {
    todo!()
}

/// UTF-16 order of a bare `.sort()`: spec rule 8.8, `wsp_words::utf16::cmp`.
pub fn utf16_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    todo!()
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
