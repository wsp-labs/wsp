// SPDX-License-Identifier: AGPL-3.0-only
//! How wsp reads an agent's plugins on a computer and turns one on or off. Port of packages/catalog/src/plugins.ts and
//! plugins-claude.ts at a3b870a6e; the spec is spec/plugins.md. Codex's shelf is in plugins_codex.
use serde::Serialize;
use serde_json::Value;

use wsp_words::exit::HostError;
use wsp_wire::types::agents_report::PluginBrings;
use wsp_wire::types::agents_report::PluginMissing;
use wsp_wire::types::agents_report::PluginScope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootsFrom {
    pub files: Vec<String>,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeekDir {
    pub path: String,
    /// 1 or 2.
    pub depth: u8,
}

/// plugins.ts:277.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeekGroup {
    pub roots: Option<Vec<String>>,
    pub roots_from: Option<RootsFrom>,
    pub files: Option<Vec<String>>,
    pub dirs: Option<Vec<PeekDir>>,
}

/// What a peek found, by absolute path (plugins.ts:286).
pub trait Peeked {
    fn text(&self, path: &str) -> Option<String>;
    fn list(&self, dir: &str) -> Option<Vec<String>>;
    fn looked(&self, root: &str) -> bool;
}

/// The plain string values of `"key": "..."` with no escape inside (plugins.ts:294).
pub fn plain_values(text: &str, key: &str) -> Vec<String> {
    todo!()
}

/// What a plugin module may ask of the computer it reads (plugins.ts:298).
#[allow(async_fn_in_trait)]
pub trait PluginIo {
    fn home(&self) -> &str;
    fn store(&self) -> Option<&str>;
    fn seed(&self) -> Option<&str>;
    /// Nothing where the computer did not answer.
    async fn peek(&self, groups: &[PeekGroup]) -> Option<Box<dyn Peeked>>;
    /// The line's stdout where it exits 0.
    async fn run(&self, line: &str) -> Option<String>;
    /// Whether this road writes config files; the TS `edit?` being there.
    fn can_edit(&self) -> bool;
    /// The file changed to what `change` makes of it, inside `base`; the write's refusal is the error.
    async fn edit(&self, file: &str, base: &str, change: &dyn Fn(Option<&str>) -> Result<String, HostError>) -> Result<(), HostError>;
}

/// plugins.ts:314.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginProject {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginSkillDirs {
    pub dirs: Vec<String>,
    pub prefix: String,
}

/// plugins.ts:322.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundPlugin {
    pub id: String,
    pub name: String,
    pub marketplace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub scope: PluginScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<PluginProject>,
    pub on: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing: Option<PluginMissing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_in: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub brings: PluginBrings,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<PluginSkillDirs>,
}

/// plugins.ts:341.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct PluginsFound {
    pub plugins: Vec<FoundPlugin>,
    pub refused: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

/// One shelf per agent (plugins-claude.ts:183, plugins-codex.ts:343).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginShelf {
    Claude,
    Codex,
}

impl PluginShelf {
    /// The agent's plugins where it reads them.
    pub async fn read(self, io: &impl PluginIo, projects: &[PluginProject]) -> PluginsFound {
        todo!()
    }

    /// One plugin turned on or off for the login; the sentence where it did not take, `None` where it did.
    pub async fn turn(self, io: &impl PluginIo, plugin: &FoundPlugin, on: bool, read: &PluginsFound) -> Option<String> {
        todo!()
    }
}

/// Every list empty (plugins.ts:356).
pub fn nothing_brought() -> PluginBrings {
    todo!()
}

/// Split at the last `@`, an `@` at index 0 or none giving the whole id as the name (plugins.ts:359).
pub fn plugin_id_parts(id: &str) -> (String, String) {
    todo!()
}

/// A `~/<stateHome>/...` path under the store where one is named, else under the home (plugins.ts:365).
pub fn in_store(state_home: &str, path: &str, home: &str, store: Option<&str>) -> String {
    todo!()
}

/// The JSON a text holds, or nothing (plugins.ts:375).
pub fn json_of(text: Option<&str>) -> Option<Value> {
    todo!()
}

/// Absolute paths joined, `.` and `..` resolved, `..` past the root staying at the root (plugins.ts:386).
pub fn join_path(parts: &[&str]) -> String {
    todo!()
}

/// The last part after trailing slashes are taken off (plugins.ts:396).
pub fn base_name(path: &str) -> String {
    todo!()
}

/// Claude Code's shelf (plugins-claude.ts:187).
pub async fn read_claude(io: &impl PluginIo, projects: &[PluginProject]) -> PluginsFound {
    todo!()
}

/// plugins-claude.ts:281.
pub async fn turn_claude(io: &impl PluginIo, plugin: &FoundPlugin, on: bool) -> Option<String> {
    todo!()
}
