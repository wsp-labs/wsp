// SPDX-License-Identifier: AGPL-3.0-only
//! Which MCP servers a turn of an agent gets in a folder, by scope, and the values the vault hands one turn's launch.
//! Port of packages/catalog/src/mcp-launch.ts at a3b870a6e; the spec is spec/mcp_launch.md. The TS functions that
//! return a Promise do so only to load a TOML parser; the port is synchronous.
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp::{McpFormat, McpServer};
use crate::mcp_config::HeadlessKey;

/// One config file as that computer holds it (mcp-launch.ts:16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnFile {
    pub path: String,
    pub text: String,
}

/// mcp-launch.ts:23.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchConfigs {
    pub user: Option<TurnFile>,
    pub projects: Option<Vec<TurnFile>>,
    pub folder: String,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnScope {
    User,
    Local,
    Project,
}

/// mcp-launch.ts:39.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnServer {
    pub server: McpServer,
    pub scope: TurnScope,
    pub file: String,
}

/// mcp-launch.ts:99.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout {
    pub top: Option<String>,
    pub key: String,
}

/// `-C <folder> rev-parse --path-format=absolute --show-toplevel --git-common-dir` (mcp-launch.ts:105).
pub fn checkout_args(folder: &str) -> Vec<String> {
    todo!()
}

/// mcp-launch.ts:108.
pub fn checkout_of(said: Option<&str>, folder: &str) -> Checkout {
    todo!()
}

/// The first definition of each name in reading order, where it is on (mcp-launch.ts:148).
pub fn started_servers(servers: &[TurnServer]) -> Vec<TurnServer> {
    todo!()
}

/// Claude Code: each started server reading a name `values` holds, its whole entry filled (mcp-launch.ts:161).
pub fn claude_launch_servers(configs: &LaunchConfigs, values: &IndexMap<String, String>) -> IndexMap<String, Value> {
    todo!()
}

/// The keys a TOML config sets no top-level value of; none where the text does not parse (mcp-launch.ts:180).
pub fn unset_toml_keys(text: &str, keys: &[HeadlessKey]) -> Vec<HeadlessKey> {
    todo!()
}

/// Codex: `mcp_servers.<s>.env.<n>` and `mcp_servers.<s>.http_headers.<h>` for each plain-keyed server reading a held
/// name (mcp-launch.ts:194).
pub fn codex_launch_config(configs: &LaunchConfigs, values: &IndexMap<String, String>) -> IndexMap<String, String> {
    todo!()
}

/// mcp-launch.ts:206.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LaunchServerValues {
    pub entries: Option<IndexMap<String, Value>>,
    pub config: Option<IndexMap<String, String>>,
}

/// A folder's own switches by list key (mcp-launch.ts:212).
pub type FolderSwitches = IndexMap<String, Vec<String>>;

/// mcp-launch.ts:233.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedServers {
    pub files: Vec<String>,
    pub base: String,
    pub folder: Option<String>,
    pub trust: bool,
}

/// The agents whose CLI takes servers at launch (mcp-launch.ts:258).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchServerRoad {
    Claude,
    Codex,
}

/// The road of an agent id, if it has one.
pub fn launch_server_road(agent: &str) -> Option<LaunchServerRoad> {
    todo!()
}

impl LaunchServerRoad {
    /// `<store or home>/.claude.json`, or `<store or home/.codex>/config.toml`.
    pub fn user(self, store: Option<&str>, home: &str) -> String {
        todo!()
    }

    /// Claude: `.mcp.json` in each folder from `folder` up to `/`; Codex: `.codex/config.toml` up to `top` (or the
    /// folder alone where `top` does not hold it). Paths normalised as mcp-launch.ts:84 does.
    pub fn project_files(self, folder: &str, top: Option<&str>) -> Vec<String> {
        todo!()
    }

    pub fn servers(self, configs: &LaunchConfigs) -> Vec<TurnServer> {
        todo!()
    }

    pub fn carry(self, folder: &str, own_files: &[String], own_base: &str) -> CarriedServers {
        todo!()
    }

    /// Claude Code's `disabledMcpServers` and `disabledMcpjsonServers` under the folder's entry; `None` for Codex,
    /// `Some(None)` where no list names a server.
    pub fn switches_of(self, text: &str, key: &str) -> Option<Option<FolderSwitches>> {
        todo!()
    }

    /// The user file with each list joined into the folder's entry; `None` for Codex (mcp-launch.ts:226).
    pub fn switches_put(self, text: Option<&str>, folder: &str, switches: &FolderSwitches) -> Option<Result<String, crate::pending::HostError>> {
        todo!()
    }

    pub fn fill(self, configs: &LaunchConfigs, values: &IndexMap<String, String>) -> LaunchServerValues {
        todo!()
    }

    pub fn hands(self, server: &McpServer, name: &str) -> bool {
        todo!()
    }
}

/// An agent as the resolver reads it (mcp-launch.ts:279).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnAgent {
    pub id: String,
    pub format: McpFormat,
    pub files: Vec<String>,
    pub project_files: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerFiles {
    pub user: Vec<String>,
    pub base: String,
    pub projects: Vec<String>,
}

/// mcp-launch.ts:285.
pub fn turn_server_files(agent: &TurnAgent, folder: &str, home: &str, store: Option<&str>, top: Option<&str>) -> ServerFiles {
    todo!()
}

/// mcp-launch.ts:296.
pub fn turn_servers(agent: &TurnAgent, configs: &LaunchConfigs) -> Vec<TurnServer> {
    todo!()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnConfig {
    pub files: Vec<String>,
    pub base: String,
}

/// mcp-launch.ts:310.
pub fn own_server_config(agent: &str, files: &[String], home: &str, store: Option<&str>) -> OwnConfig {
    todo!()
}

/// The held names a server reads that the launch cannot hand it, sorted (mcp-launch.ts:319).
pub fn launch_misses(agent: &str, format: McpFormat, server: &McpServer, held: &IndexSet<String>) -> Vec<String> {
    todo!()
}
