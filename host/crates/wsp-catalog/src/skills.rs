// SPDX-License-Identifier: AGPL-3.0-only
//! Where each agent loads skills from, the skills its own install brings, and how its plugins go to another computer.
//! Port of packages/catalog/src/skills.ts at a3b870a6e; the spec is spec/skills_context.md.
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lands {
    Copy,
    Link,
}

/// skills.ts:13.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillRoot {
    pub dir: String,
    pub lands: Lands,
}

pub const SHARED_SKILLS: &str = "~/.agents/skills";
pub const PROJECT_SHARED_SKILLS: &str = ".agents/skills";
pub const XDG_SHARED_SKILLS: &str = "~/.config/agents/skills";

/// skills.ts:29: `user` is never empty, the agent's own folder first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillRoots {
    pub user: Vec<SkillRoot>,
    pub project: Vec<SkillRoot>,
}

/// skills.ts:36.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginSkills {
    pub index: String,
}

/// skills.ts:54.
pub fn claude_plugin_skills() -> PluginSkills {
    todo!()
}

/// One module per agent whose install seeds skills (skills.ts:66).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BundledSkills {
    Hermes,
}

impl BundledSkills {
    /// "~/.hermes/skills/.bundled_manifest".
    pub fn manifest(self) -> &'static str {
        todo!()
    }

    /// Each line's text before its first `:`, trimmed, the empty ones left out (skills.ts:68).
    pub fn names(self, text: &str) -> Vec<String> {
        todo!()
    }
}

/// One module per agent with plugins (skills.ts:120).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginRoad {
    Claude,
}

/// What a forked machine reads its plugins from (skills.ts:105).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginSeed {
    pub road: PluginRoad,
}

/// Reads one file of a seed folder by its path in it; the TS `read` callback.
#[allow(async_fn_in_trait)]
pub trait SeedReader {
    async fn read(&self, rel: &str) -> Option<String>;
}

impl PluginRoad {
    pub fn marketplaces(self) -> &'static str {
        todo!()
    }

    pub fn settings(self) -> &'static str {
        todo!()
    }

    /// The plugins `enabledPlugins` switches on (`=== true`), in key order; none off text that does not parse
    /// (skills.ts:143).
    pub fn enabled(self, text: &str) -> Vec<String> {
        todo!()
    }

    /// A github source's repo, else its url (skills.ts:122).
    pub fn source_of(self, text: &str, marketplace: &str) -> Option<String> {
        todo!()
    }

    /// Every github repo matching OWNER_REPO (skills.ts:130).
    pub fn github_repos(self, text: &str) -> Vec<String> {
        todo!()
    }

    /// skills.ts:152.
    pub fn install(self, plugin: &str, source: &str) -> String {
        todo!()
    }

    /// skills.ts:153.
    pub fn asked(self, out: &str, plugin: &str) -> Option<String> {
        todo!()
    }

    /// skills.ts:157.
    pub fn uninstall(self, plugin: &str) -> String {
        todo!()
    }

    /// skills.ts:160.
    pub fn copied_before(self) -> Vec<String> {
        todo!()
    }

    pub fn seed(self) -> Option<PluginSeed> {
        todo!()
    }

    /// The settings with every marketplace no switched-on plugin comes from taken out; the text as it was where it does
    /// not parse, names none, or cannot be edited in place (skills.ts:197).
    pub fn used_only(self, text: &str) -> String {
        todo!()
    }
}

impl PluginSeed {
    /// CLAUDE_PLUGIN_SEED.
    pub fn dir(self) -> &'static str {
        todo!()
    }

    /// PLUGIN_SEED_ENV.
    pub fn env(self) -> &'static str {
        todo!()
    }

    /// skills.ts:166.
    pub fn build(self, plugin: &str, source: &str) -> String {
        todo!()
    }

    /// `[link, target]` for each plugin its marketplace pins to a commit whose first twelve differ from its folder,
    /// reading each marketplace's file once (skills.ts:177).
    pub async fn links(self, read: &impl SeedReader) -> Vec<(String, String)> {
        todo!()
    }
}

/// skills.ts:218.
pub fn own_skill_folder(roots: &SkillRoots, project: bool) -> Option<SkillRoot> {
    todo!()
}

/// skills.ts:225.
pub fn skills_dir_of(roots: &SkillRoots) -> String {
    todo!()
}

pub const WSP_SKILL_NAME: &str = "wsp";

/// skills.ts:232.
pub fn is_system_skill(name: &str) -> bool {
    todo!()
}
