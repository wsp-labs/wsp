// SPDX-License-Identifier: AGPL-3.0-only
//! Where each agent loads skills from, the skills its own install brings, and how its plugins go to another computer.
//! Port of packages/catalog/src/skills.ts at a3b870a6e; the spec is spec/skills_context.md.
use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use wsp_wire::types::env::PLUGIN_SEED_ENV;
use wsp_wire::types::projects::OWNER_REPO;
use wsp_wire::types::shell_quote::shell_quote;

use crate::json_parse::parse_json;
use crate::context::SKILL_NAME;
use crate::json_edit::{edit_json, JsonEdit};
use crate::jsonc_tree::PathSeg;
use crate::plugins::{js_entries, js_trim, json_of, WS};
use crate::roads::{CLAUDE_CONFIG_DIR, CLAUDE_PLUGIN_SEED};

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
    PluginSkills { index: "~/.claude/plugins/installed_plugins.json".to_string() }
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
        match self {
            BundledSkills::Hermes => "~/.hermes/skills/.bundled_manifest",
        }
    }

    /// Each line's text before its first `:`, trimmed, the empty ones left out (skills.ts:68).
    pub fn names(self, text: &str) -> Vec<String> {
        match self {
            BundledSkills::Hermes => text
                .split('\n')
                .map(|line| js_trim(line.split(':').next().unwrap_or("")))
                .filter(|name| !name.is_empty())
                .map(String::from)
                .collect(),
        }
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

/// `OWNER_REPO` as the TS tests it: ASCII word characters only.
static OWNER_REPO_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!("(?-u){OWNER_REPO}")).expect("OWNER_REPO is a valid pattern"));

/// The `shownCommand` sha256 of a plugin install that asks to run a command (skills.ts:153).
static SHOWN_COMMAND_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#""shownCommand"[{WS}]*:[{WS}]*\{{[^}}]*"sha256"[{WS}]*:[{WS}]*"([0-9a-f]{{64}})""#, WS = WS))
        .expect("the shownCommand pattern is valid")
});

/// The twelve-hex prefix of a pinned commit, the folder a turn looks for it under (skills.ts:192).
fn is_twelve_hex(sha: &str) -> bool {
    sha.len() == 12 && sha.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl PluginRoad {
    pub fn marketplaces(self) -> &'static str {
        match self {
            PluginRoad::Claude => "~/.claude/plugins/known_marketplaces.json",
        }
    }

    pub fn settings(self) -> &'static str {
        match self {
            PluginRoad::Claude => "~/.claude/settings.json",
        }
    }

    /// The plugins `enabledPlugins` switches on (`=== true`), in key order; none off text that does not parse
    /// (skills.ts:143).
    pub fn enabled(self, text: &str) -> Vec<String> {
        match self {
            PluginRoad::Claude => {
                let parsed: Option<Value> = parse_json(text).ok();
                let on = parsed.as_ref().and_then(|v| v.get("enabledPlugins")).and_then(Value::as_object);
                on.map_or_else(Vec::new, |on| {
                    js_entries(on).into_iter().filter(|(_, v)| v.as_bool() == Some(true)).map(|(k, _)| k.clone()).collect()
                })
            }
        }
    }

    /// A github source's repo, else its url (skills.ts:122).
    pub fn source_of(self, text: &str, marketplace: &str) -> Option<String> {
        match self {
            PluginRoad::Claude => {
                let parsed = parse_json(text).ok()?;
                let at = parsed.get(marketplace)?.get("source")?;
                let value = if at.get("source").and_then(Value::as_str) == Some("github") { at.get("repo") } else { at.get("url") };
                value?.as_str().map(String::from)
            }
        }
    }

    /// Every github repo matching OWNER_REPO (skills.ts:130).
    pub fn github_repos(self, text: &str) -> Vec<String> {
        match self {
            PluginRoad::Claude => {
                let Ok(known) = parse_json(text) else {
                    return vec![];
                };
                let Some(known) = known.as_object() else {
                    return vec![];
                };
                js_entries(known)
                    .into_iter()
                    .filter_map(|(_, m)| {
                        let at = m.as_object()?.get("source")?.as_object()?;
                        if at.get("source").and_then(Value::as_str) != Some("github") {
                            return None;
                        }
                        let repo = at.get("repo").and_then(Value::as_str)?;
                        OWNER_REPO_RE.is_match(repo).then(|| repo.to_string())
                    })
                    .collect()
            }
        }
    }

    /// skills.ts:152.
    pub fn install(self, plugin: &str, source: &str) -> String {
        match self {
            PluginRoad::Claude => [
                format!("claude plugin marketplace add {} || true", shell_quote(source)),
                format!("claude plugin install {} --json </dev/null", shell_quote(plugin)),
            ]
            .join("\n"),
        }
    }

    /// skills.ts:153.
    pub fn asked(self, out: &str, plugin: &str) -> Option<String> {
        match self {
            PluginRoad::Claude => {
                let sha = SHOWN_COMMAND_RE.captures(out)?[1].to_string();
                Some(format!("asks to run a command its marketplace declares; to accept it, run claude plugin install {plugin} --accept-command {sha} on that computer"))
            }
        }
    }

    /// skills.ts:157.
    pub fn uninstall(self, plugin: &str) -> String {
        match self {
            PluginRoad::Claude => {
                format!("claude plugin uninstall {} </dev/null", shell_quote(plugin))
            }
        }
    }

    /// skills.ts:160.
    pub fn copied_before(self) -> Vec<String> {
        match self {
            PluginRoad::Claude => vec![
                format!("{CLAUDE_CONFIG_DIR}/plugins/known_marketplaces.json"),
                format!("{CLAUDE_CONFIG_DIR}/plugins/installed_plugins.json"),
            ],
        }
    }

    pub fn seed(self) -> Option<PluginSeed> {
        match self {
            PluginRoad::Claude => Some(PluginSeed { road: self }),
        }
    }

    /// The settings with every marketplace no switched-on plugin comes from taken out; the text as it was where it does
    /// not parse, names none, or cannot be edited in place (skills.ts:197).
    pub fn used_only(self, text: &str) -> String {
        match self {
            PluginRoad::Claude => {
                let Ok(parsed) = parse_json(text) else {
                    return text.to_string();
                };
                let Some(known) = parsed.get("extraKnownMarketplaces").and_then(Value::as_object) else {
                    return text.to_string();
                };
                let on = self.enabled(text);
                let unused: Vec<String> = js_entries(known)
                    .into_iter()
                    .map(|(k, _)| k.clone())
                    .filter(|m| !on.iter().any(|p| p.ends_with(&format!("@{m}"))))
                    .collect();
                if unused.is_empty() {
                    return text.to_string();
                }
                let edits: Vec<JsonEdit> = unused
                    .iter()
                    .map(|m| (vec![PathSeg::Key("extraKnownMarketplaces".to_string()), PathSeg::Key(m.clone())], None))
                    .collect();
                edit_json(text, &edits).unwrap_or_else(|_| text.to_string())
            }
        }
    }
}

impl PluginSeed {
    /// CLAUDE_PLUGIN_SEED.
    pub fn dir(self) -> &'static str {
        match self.road {
            PluginRoad::Claude => CLAUDE_PLUGIN_SEED,
        }
    }

    /// PLUGIN_SEED_ENV.
    pub fn env(self) -> &'static str {
        PLUGIN_SEED_ENV
    }

    /// skills.ts:166.
    pub fn build(self, plugin: &str, source: &str) -> String {
        [
            r#"seed_config="$(mktemp -d)""#.to_string(),
            format!(
                "CLAUDE_CODE_PLUGIN_CACHE_DIR={} CLAUDE_CONFIG_DIR=\"$seed_config\" sh -c {}",
                shell_quote(self.dir()),
                shell_quote(&self.road.install(plugin, source))
            ),
            "built=$?".to_string(),
            r#"rm -rf "$seed_config""#.to_string(),
            r#"exit "$built""#.to_string(),
        ]
        .join("\n")
    }

    /// `[link, target]` for each plugin its marketplace pins to a commit whose first twelve differ from its folder,
    /// reading each marketplace's file once (skills.ts:177).
    pub async fn links(self, read: &impl SeedReader) -> Vec<(String, String)> {
        let index_text = read.read("installed_plugins.json").await;
        let Some(index) = json_of(index_text.as_deref()).and_then(|v| v.get("plugins").cloned()) else {
            return vec![];
        };
        let Some(index) = index.as_object() else {
            return vec![];
        };
        let mut out = Vec::new();
        let mut manifests: HashMap<String, Option<String>> = HashMap::new();
        for (id, records) in js_entries(index) {
            let Some(at) = id.rfind('@').filter(|&at| at > 0) else {
                continue;
            };
            let Some(install_path) = records.as_array().and_then(|r| r.first()).and_then(|r| r.get("installPath")).and_then(Value::as_str)
            else {
                continue;
            };
            let (name, marketplace) = (&id[..at], &id[at + 1..]);
            if !manifests.contains_key(marketplace) {
                let text = read.read(&format!("marketplaces/{marketplace}/.claude-plugin/marketplace.json")).await;
                manifests.insert(marketplace.to_string(), text);
            }
            let listed = json_of(manifests.get(marketplace).and_then(|t| t.as_deref())).and_then(|m| m.get("plugins").cloned());
            let entry = listed
                .as_ref()
                .and_then(Value::as_array)
                .and_then(|ps| ps.iter().find(|p| p.get("name").and_then(Value::as_str) == Some(name)));
            let sha: Option<String> = entry
                .and_then(|e| e.get("source"))
                .and_then(|s| s.get("sha"))
                .and_then(Value::as_str)
                .map(|s| s.chars().take(12).collect());
            let folder = install_path.rsplit('/').next().unwrap_or("");
            if let Some(sha) = sha.filter(|s| is_twelve_hex(s) && s != folder) {
                out.push((format!("cache/{marketplace}/{name}/{sha}"), folder.to_string()));
            }
        }
        out
    }
}

/// Where an install puts a skill for one agent, a project's with `project`: nothing where the agent reads the shared
/// folder already, else its own folder with how that folder takes a skill (skills.ts:218).
pub fn own_skill_folder(roots: &SkillRoots, project: bool) -> Option<SkillRoot> {
    let (folders, shared) = if project { (&roots.project, PROJECT_SHARED_SKILLS) } else { (&roots.user, SHARED_SKILLS) };
    if folders.iter().any(|r| r.dir == shared) {
        return None;
    }
    folders.first().cloned()
}

/// The folder the agent's own skills go in, where the wsp skill is written (skills.ts:225).
pub fn skills_dir_of(roots: &SkillRoots) -> String {
    roots.user[0].dir.clone()
}

pub const WSP_SKILL_NAME: &str = "wsp";

/// skills.ts:232.
pub fn is_system_skill(name: &str) -> bool {
    name == WSP_SKILL_NAME || name == SKILL_NAME
}
