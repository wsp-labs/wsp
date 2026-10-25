// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/recipe-file.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::from_json;
use crate::types::check::{Check, SchemaError};
use crate::types::project_look::ProjectHue;
use crate::types::project_look::ProjectIcon;
use crate::types::project_look::ProjectIconHash;
use indexmap::IndexMap;
use wsp_frames::True;

/// TS: packages/protocol/src/recipe-file.ts:87 at a3b870a6e.
pub const WSP_COMMAND: &str = "wsp";

/// TS: packages/protocol/src/recipe-file.ts:98 at a3b870a6e.
pub const RECIPE_KINDS: &[&str] = &["agents", "mcp", "clis", "skills", "plugins", "folders", "configs"];

/// TS: packages/protocol/src/recipe-file.ts:114 at a3b870a6e.
pub const NO_RECIPE: &str = "none";

/// TS: packages/protocol/src/recipe-file.ts:189 at a3b870a6e.
pub const COPY_KEYS_WORD: &str = "Copy the keys";

/// TS: packages/protocol/src/recipe-file.ts:198 at a3b870a6e.
pub const RECIPE_NAME_REFUSAL: &str = "a recipe's name needs a letter or a digit";

/// TS: packages/protocol/src/recipe-file.ts:205 at a3b870a6e.
pub const NO_RECIPES: &str = "this runtime keeps no recipes; the host that serves the app wires them";

/// TS: packages/protocol/src/recipe-file.ts:215 at a3b870a6e.
pub const RECIPES_TICKET_REFUSAL: &str =
    "a socket let in on a ticket cannot see or change the recipes this host keeps; run wsp recipes on the computer the host runs on";

/// TS: packages/protocol/src/recipe-file.ts:17 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecipeSignIn {
    #[serde(rename = "vault")]
    Vault,
    #[serde(rename = "machine")]
    Machine,
    #[serde(rename = "token")]
    Token,
    #[serde(rename = "key")]
    Key,
}

impl Check for RecipeSignIn {}

/// TS: packages/protocol/src/recipe-file.ts:22 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GitHubSignIn {
    #[serde(rename = "vault")]
    Vault,
    #[serde(rename = "machine")]
    Machine,
    #[serde(rename = "skip")]
    Skip,
}

impl Check for GitHubSignIn {}

/// TS: packages/protocol/src/recipe-file.ts:40 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServerSignIn {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "key")]
    Key,
    #[serde(rename = "token")]
    Token,
    #[serde(rename = "oauth")]
    Oauth,
}

impl Check for ServerSignIn {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileAgentsValue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signin: Option<RecipeSignIn>,
}

impl Check for RecipeFileAgentsValue {
    /// Nested checks: signin.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileMcpValue {
    pub agents: Vec<String>,
}

impl Check for RecipeFileMcpValue {
    /// Rules: agents[]: at least 1 UTF-16 units; agents: at least 1 items.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileClisValue {
    pub via: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<Vec<String>>,
}

impl Check for RecipeFileClisValue {
    /// Rules: via: at least 1 UTF-16 units; needs[]: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileSkillsValue {
    pub from: String,
}

impl Check for RecipeFileSkillsValue {
    /// Rules: from: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFilePluginsValue {}

impl Check for RecipeFilePluginsValue {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileFoldersValue {
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<ProjectIcon>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<ProjectHue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ProjectIconHash>,
    #[serde(default = "default_recipe_file_folders_value_keep")]
    pub keep: Vec<String>,
}

impl Check for RecipeFileFoldersValue {
    /// Rules: from: at least 1 UTF-16 units; name: at least 1 UTF-16 units; keep[]: at least 1 UTF-16 units.
    /// Nested checks: icon, hue, image.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

fn default_recipe_file_folders_value_keep() -> Vec<String> {
    from_json("[]")
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileConfigsGit {}

impl Check for RecipeFileConfigsGit {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileConfigsShell {}

impl Check for RecipeFileConfigsShell {
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileConfigsGithub {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signin: Option<GitHubSignIn>,
}

impl Check for RecipeFileConfigsGithub {
    /// Nested checks: signin.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileConfigs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<RecipeFileConfigsGit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<RecipeFileConfigsShell>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github: Option<RecipeFileConfigsGithub>,
}

impl Check for RecipeFileConfigs {
    /// Nested checks: git, shell, github.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/recipe-file.ts:55 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeFile {
    pub name: String,
    #[serde(default = "default_recipe_file_agents")]
    pub agents: IndexMap<String, RecipeFileAgentsValue>,
    #[serde(default = "default_recipe_file_mcp")]
    pub mcp: IndexMap<String, RecipeFileMcpValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_keys: Option<True>,
    #[serde(default = "default_recipe_file_clis")]
    pub clis: IndexMap<String, RecipeFileClisValue>,
    #[serde(default = "default_recipe_file_skills")]
    pub skills: IndexMap<String, RecipeFileSkillsValue>,
    #[serde(default = "default_recipe_file_plugins")]
    pub plugins: IndexMap<String, RecipeFilePluginsValue>,
    #[serde(default = "default_recipe_file_folders")]
    pub folders: IndexMap<String, RecipeFileFoldersValue>,
    #[serde(default = "default_recipe_file_configs")]
    pub configs: RecipeFileConfigs,
}

impl Check for RecipeFile {
    /// Rules: name: zod string check trim; name: at least 1 UTF-16 units; name: at most 64 UTF-16 units.
    /// Nested checks: agents, mcp, clis, skills, plugins, folders, configs.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

fn default_recipe_file_agents() -> IndexMap<String, RecipeFileAgentsValue> {
    from_json("{}")
}

fn default_recipe_file_mcp() -> IndexMap<String, RecipeFileMcpValue> {
    from_json("{}")
}

fn default_recipe_file_clis() -> IndexMap<String, RecipeFileClisValue> {
    from_json("{}")
}

fn default_recipe_file_skills() -> IndexMap<String, RecipeFileSkillsValue> {
    from_json("{}")
}

fn default_recipe_file_plugins() -> IndexMap<String, RecipeFilePluginsValue> {
    from_json("{}")
}

fn default_recipe_file_folders() -> IndexMap<String, RecipeFileFoldersValue> {
    from_json("{}")
}

fn default_recipe_file_configs() -> RecipeFileConfigs {
    from_json("{}")
}

/// TS: packages/protocol/src/recipe-file.ts:162 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeView {
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub machines: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_at: Option<String>,
    pub file: RecipeFile,
}

impl Check for RecipeView {
    /// Rules: name: at least 1 UTF-16 units; slug: at least 1 UTF-16 units; summary: at least 1 UTF-16 units; machines[]: at least 1 UTF-16 units.
    /// Nested checks: file.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
