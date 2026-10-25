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

impl Check for RecipeSignIn {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["vault", "machine", "token", "key"]))
    }
}

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

impl Check for GitHubSignIn {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["vault", "machine", "skip"]))
    }
}

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

impl Check for ServerSignIn {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["none", "key", "token", "oauth"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileAgentsValue {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub signin: Option<RecipeSignIn>,
}

impl Check for RecipeFileAgentsValue {
    /// Nested checks: signin.
    fn check(&self) -> Result<(), SchemaError> {
        self.signin.check().map_err(|e| e.under("signin"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<RecipeSignIn>(obj, "signin", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "signin" => <Option<RecipeSignIn> as Check>::describe(rest),
                _ => None,
            },
        }
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
        for (i, item) in self.agents.iter().enumerate() {
            crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i).under("agents"))?;
        }
        crate::types::check::items_at_least(self.agents.len(), 1).map_err(|e| e.under("agents"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<Vec<String>>(obj, "agents", &mut out, Self::rule_agents);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agents" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeFileMcpValue {
    fn rule_agents(x: &Vec<String>) -> Result<(), crate::types::check::SchemaError> {
        for (i, item) in (*x).iter().enumerate() {
            crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i))?;
        }
        crate::types::check::items_at_least((*x).len(), 1)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileClisValue {
    pub via: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub needs: Option<Vec<String>>,
}

impl Check for RecipeFileClisValue {
    /// Rules: via: at least 1 UTF-16 units; needs[]: at least 1 UTF-16 units.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.via, 1).map_err(|e| e.under("via"))?;
        if let Some(list) = &self.needs {
            for (i, item) in list.iter().enumerate() {
                crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i).under("needs"))?;
            }
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "via", &mut out, Self::rule_via);
        crate::types::keys::opt_key_with::<Vec<String>>(obj, "needs", &mut out, Self::rule_needs);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "via" => <String as Check>::describe(rest),
                "needs" => <Option<Vec<String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeFileClisValue {
    fn rule_via(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_needs(x: &Option<Vec<String>>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(list) = x {
            for (i, item) in list.iter().enumerate() {
                crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i))?;
            }
        }
        Ok(())
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
        crate::types::check::text_at_least(&self.from, 1).map_err(|e| e.under("from"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "from", &mut out, Self::rule_from);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "from" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeFileSkillsValue {
    fn rule_from(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFilePluginsValue {}

impl Check for RecipeFilePluginsValue {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            Some(crate::types::check::Zod::Word("object"))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileFoldersValue {
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub icon: Option<ProjectIcon>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub hue: Option<ProjectHue>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub image: Option<ProjectIconHash>,
    #[serde(default = "default_recipe_file_folders_value_keep")]
    pub keep: Vec<String>,
}

impl Check for RecipeFileFoldersValue {
    /// Rules: from: at least 1 UTF-16 units; name: at least 1 UTF-16 units; keep[]: at least 1 UTF-16 units.
    /// Nested checks: icon, hue, image.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.from, 1).map_err(|e| e.under("from"))?;
        if let Some(v) = &self.name {
            crate::types::check::text_at_least(v, 1).map_err(|e| e.under("name"))?;
        }
        self.icon.check().map_err(|e| e.under("icon"))?;
        self.hue.check().map_err(|e| e.under("hue"))?;
        for (i, item) in self.keep.iter().enumerate() {
            crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i).under("keep"))?;
        }
        self.image.check().map_err(|e| e.under("image"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "from", &mut out, Self::rule_from);
        crate::types::keys::opt_key_with::<String>(obj, "name", &mut out, Self::rule_name);
        crate::types::keys::opt_key::<ProjectIcon>(obj, "icon", &mut out);
        crate::types::keys::opt_key::<ProjectHue>(obj, "hue", &mut out);
        crate::types::keys::opt_key::<ProjectIconHash>(obj, "image", &mut out);
        crate::types::keys::any_key_with::<Vec<String>>(obj, "keep", &mut out, Self::rule_keep);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "from" => <String as Check>::describe(rest),
                "name" => <Option<String> as Check>::describe(rest),
                "icon" => <Option<ProjectIcon> as Check>::describe(rest),
                "hue" => <Option<ProjectHue> as Check>::describe(rest),
                "image" => None,
                "keep" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeFileFoldersValue {
    fn rule_from(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_name(x: &Option<String>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = x {
            crate::types::check::text_at_least(v, 1)?;
        }
        Ok(())
    }
    fn rule_keep(x: &Vec<String>) -> Result<(), crate::types::check::SchemaError> {
        for (i, item) in (*x).iter().enumerate() {
            crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i))?;
        }
        Ok(())
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
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            Some(crate::types::check::Zod::Word("object"))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileConfigsShell {}

impl Check for RecipeFileConfigsShell {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            Some(crate::types::check::Zod::Word("object"))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileConfigsGithub {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub signin: Option<GitHubSignIn>,
}

impl Check for RecipeFileConfigsGithub {
    /// Nested checks: signin.
    fn check(&self) -> Result<(), SchemaError> {
        self.signin.check().map_err(|e| e.under("signin"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<GitHubSignIn>(obj, "signin", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "signin" => <Option<GitHubSignIn> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeFileConfigs {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub git: Option<RecipeFileConfigsGit>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub shell: Option<RecipeFileConfigsShell>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub github: Option<RecipeFileConfigsGithub>,
}

impl Check for RecipeFileConfigs {
    /// Nested checks: git, shell, github.
    fn check(&self) -> Result<(), SchemaError> {
        self.git.check().map_err(|e| e.under("git"))?;
        self.shell.check().map_err(|e| e.under("shell"))?;
        self.github.check().map_err(|e| e.under("github"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<RecipeFileConfigsGit>(obj, "git", &mut out);
        crate::types::keys::opt_key::<RecipeFileConfigsShell>(obj, "shell", &mut out);
        crate::types::keys::opt_key::<RecipeFileConfigsGithub>(obj, "github", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "git" => <Option<RecipeFileConfigsGit> as Check>::describe(rest),
                "shell" => <Option<RecipeFileConfigsShell> as Check>::describe(rest),
                "github" => <Option<RecipeFileConfigsGithub> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// A key of a recipe table: the name a row goes by, which zod reads as a non-empty string.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecipeName(pub String);

impl Check for RecipeName {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.0, 1)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Word("string"))
    }
}

/// TS: packages/protocol/src/recipe-file.ts:55 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeFile {
    #[serde(deserialize_with = "crate::types::check::trimmed")]
    pub name: String,
    #[serde(
        default = "default_recipe_file_agents",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub agents: IndexMap<RecipeName, RecipeFileAgentsValue>,
    #[serde(
        default = "default_recipe_file_mcp",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub mcp: IndexMap<RecipeName, RecipeFileMcpValue>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copy_keys: Option<True>,
    #[serde(
        default = "default_recipe_file_clis",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub clis: IndexMap<RecipeName, RecipeFileClisValue>,
    #[serde(
        default = "default_recipe_file_skills",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub skills: IndexMap<RecipeName, RecipeFileSkillsValue>,
    #[serde(
        default = "default_recipe_file_plugins",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub plugins: IndexMap<RecipeName, RecipeFilePluginsValue>,
    #[serde(
        default = "default_recipe_file_folders",
        serialize_with = "crate::types::check::js_record",
        deserialize_with = "crate::types::kept::record"
    )]
    pub folders: IndexMap<RecipeName, RecipeFileFoldersValue>,
    #[serde(default = "default_recipe_file_configs")]
    pub configs: RecipeFileConfigs,
}

impl Check for RecipeFile {
    /// Rules: name: zod string check trim; name: at least 1 UTF-16 units; name: at most 64 UTF-16 units.
    /// Nested checks: agents, mcp, clis, skills, plugins, folders, configs.
    /// Strict: an unknown key is refused (zod .strict()).
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.name, 1).map_err(|e| e.under("name"))?;
        crate::types::check::text_at_most(&self.name, 64).map_err(|e| e.under("name"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.mcp.check().map_err(|e| e.under("mcp"))?;
        self.clis.check().map_err(|e| e.under("clis"))?;
        self.skills.check().map_err(|e| e.under("skills"))?;
        self.plugins.check().map_err(|e| e.under("plugins"))?;
        self.folders.check().map_err(|e| e.under("folders"))?;
        self.configs.check().map_err(|e| e.under("configs"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_trimmed_with(obj, "name", &mut out, Self::rule_name);
        crate::types::keys::any_key::<IndexMap<RecipeName, RecipeFileAgentsValue>>(obj, "agents", &mut out);
        crate::types::keys::any_key::<IndexMap<RecipeName, RecipeFileMcpValue>>(obj, "mcp", &mut out);
        crate::types::keys::opt_key::<True>(obj, "copyKeys", &mut out);
        crate::types::keys::any_key::<IndexMap<RecipeName, RecipeFileClisValue>>(obj, "clis", &mut out);
        crate::types::keys::any_key::<IndexMap<RecipeName, RecipeFileSkillsValue>>(obj, "skills", &mut out);
        crate::types::keys::any_key::<IndexMap<RecipeName, RecipeFilePluginsValue>>(obj, "plugins", &mut out);
        crate::types::keys::any_key::<IndexMap<RecipeName, RecipeFileFoldersValue>>(obj, "folders", &mut out);
        crate::types::keys::any_key::<RecipeFileConfigs>(obj, "configs", &mut out);
        let known: &[&str] = &["name", "agents", "mcp", "copyKeys", "clis", "skills", "plugins", "folders", "configs"];
        let extra: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
        if !extra.is_empty() {
            out.push(crate::types::check::unrecognized(Vec::new(), extra));
        }
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "agents" => <IndexMap<RecipeName, RecipeFileAgentsValue> as Check>::describe(rest),
                "mcp" => <IndexMap<RecipeName, RecipeFileMcpValue> as Check>::describe(rest),
                "copyKeys" => None,
                "clis" => <IndexMap<RecipeName, RecipeFileClisValue> as Check>::describe(rest),
                "skills" => <IndexMap<RecipeName, RecipeFileSkillsValue> as Check>::describe(rest),
                "plugins" => <IndexMap<RecipeName, RecipeFilePluginsValue> as Check>::describe(rest),
                "folders" => <IndexMap<RecipeName, RecipeFileFoldersValue> as Check>::describe(rest),
                "configs" => <RecipeFileConfigs as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeFile {
    fn rule_name(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        crate::types::check::text_at_most(x, 64)?;
        Ok(())
    }
}

fn default_recipe_file_agents() -> IndexMap<RecipeName, RecipeFileAgentsValue> {
    from_json("{}")
}

fn default_recipe_file_mcp() -> IndexMap<RecipeName, RecipeFileMcpValue> {
    from_json("{}")
}

fn default_recipe_file_clis() -> IndexMap<RecipeName, RecipeFileClisValue> {
    from_json("{}")
}

fn default_recipe_file_skills() -> IndexMap<RecipeName, RecipeFileSkillsValue> {
    from_json("{}")
}

fn default_recipe_file_plugins() -> IndexMap<RecipeName, RecipeFilePluginsValue> {
    from_json("{}")
}

fn default_recipe_file_folders() -> IndexMap<RecipeName, RecipeFileFoldersValue> {
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
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub saved_at: Option<String>,
    pub file: RecipeFile,
}

impl Check for RecipeView {
    /// Rules: name: at least 1 UTF-16 units; slug: at least 1 UTF-16 units; summary: at least 1 UTF-16 units; machines[]: at least 1 UTF-16 units.
    /// Nested checks: file.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.name, 1).map_err(|e| e.under("name"))?;
        crate::types::check::text_at_least(&self.slug, 1).map_err(|e| e.under("slug"))?;
        crate::types::check::text_at_least(&self.summary, 1).map_err(|e| e.under("summary"))?;
        for (i, item) in self.machines.iter().enumerate() {
            crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i).under("machines"))?;
        }
        self.file.check().map_err(|e| e.under("file"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "name", &mut out, Self::rule_name);
        crate::types::keys::req_key_with::<String>(obj, "slug", &mut out, Self::rule_slug);
        crate::types::keys::req_key_with::<String>(obj, "summary", &mut out, Self::rule_summary);
        crate::types::keys::req_key_with::<Vec<String>>(obj, "machines", &mut out, Self::rule_machines);
        crate::types::keys::opt_key::<String>(obj, "savedAt", &mut out);
        crate::types::keys::req_key::<RecipeFile>(obj, "file", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "slug" => <String as Check>::describe(rest),
                "summary" => <String as Check>::describe(rest),
                "machines" => <Vec<String> as Check>::describe(rest),
                "savedAt" => <Option<String> as Check>::describe(rest),
                "file" => <RecipeFile as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl RecipeView {
    fn rule_name(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_slug(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_summary(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
    fn rule_machines(x: &Vec<String>) -> Result<(), crate::types::check::SchemaError> {
        for (i, item) in (*x).iter().enumerate() {
            crate::types::check::text_at_least(item, 1).map_err(|e| e.under(i))?;
        }
        Ok(())
    }
}
