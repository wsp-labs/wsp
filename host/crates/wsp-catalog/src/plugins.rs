// SPDX-License-Identifier: AGPL-3.0-only
//! How wsp reads an agent's plugins on a computer and turns one on or off. Port of packages/catalog/src/plugins.ts and
//! plugins-claude.ts at a3b870a6e; the spec is spec/plugins.md. Codex's shelf is in plugins_codex.
use indexmap::{IndexMap, IndexSet};
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::json_parse::parse_json;
use crate::config_write::store_base;
use crate::json_edit::edit_json;
use crate::jsonc_tree::PathSeg;
use crate::plugins_codex::{read_codex_plugins, turn_codex};
use wsp_wire::types::agents_report::PluginBrings;
use wsp_wire::types::agents_report::PluginMissing;
use wsp_wire::types::agents_report::PluginScope;
use wsp_words::exit::HostError;

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

pub(crate) use wsp_words::js::{is_js_whitespace as js_space, js_trim, js_trim_start, JS_WS as WS};

/// The entries of a JSON object in JavaScript's order: whole-number keys first, ascending, then the rest as written.
pub(crate) fn js_entries(map: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut entries: Vec<(&String, &Value)> = map.iter().collect();
    entries.sort_by_key(|(k, _)| array_index(k).map_or(u64::MAX, u64::from));
    entries
}

fn array_index(key: &str) -> Option<u32> {
    let canonical = key == "0" || (!key.starts_with('0') && !key.is_empty());
    if !canonical || !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    key.parse::<u32>().ok().filter(|&i| i < u32::MAX)
}

/// Each string of a list, the list's order, sorted in UTF-16 order as JavaScript's `sort()` sorts.
pub(crate) fn sorted(mut items: Vec<String>) -> Vec<String> {
    items.sort_by(|a, b| wsp_words::js::utf16::cmp(a, b));
    items
}

/// Each item once, the first place it stands kept.
pub(crate) fn unique(items: impl IntoIterator<Item = String>) -> Vec<String> {
    items.into_iter().collect::<IndexSet<String>>().into_iter().collect()
}

/// The plain string values of `"key": "..."` with no escape inside (plugins.ts:294).
pub fn plain_values(text: &str, key: &str) -> Vec<String> {
    let pattern = format!(r#""{}"[{WS}]*:[{WS}]*"([^"\\]*)""#, regex::escape(key), WS = WS);
    let re = Regex::new(&pattern).expect("an escaped key is a valid pattern");
    re.captures_iter(text).map(|c| c[1].to_string()).collect()
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
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
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
        match self {
            PluginShelf::Claude => read_claude(io, projects).await,
            PluginShelf::Codex => read_codex_plugins(io).await,
        }
    }

    /// One plugin turned on or off for the login: `Ok(None)` where it took, the TS's returned `refused` sentence as
    /// `Ok(Some)`, and the error a failed write threw, with its kind, as `Err`.
    pub async fn turn(self, io: &impl PluginIo, plugin: &FoundPlugin, on: bool, read: &PluginsFound) -> Result<Option<String>, HostError> {
        match self {
            PluginShelf::Claude => turn_claude(io, plugin, on).await,
            PluginShelf::Codex => turn_codex(io, plugin, on, read).await,
        }
    }
}

/// Every list empty (plugins.ts:356).
pub fn nothing_brought() -> PluginBrings {
    PluginBrings { skills: vec![], commands: vec![], subagents: vec![], hooks: vec![], servers: vec![], lsp: vec![], apps: vec![] }
}

/// Split at the last `@`, an `@` at index 0 or none giving the whole id as the name (plugins.ts:359).
pub fn plugin_id_parts(id: &str) -> (String, String) {
    match id.rfind('@') {
        None | Some(0) => (id.to_string(), String::new()),
        Some(at) => (id[..at].to_string(), id[at + 1..].to_string()),
    }
}

/// A `~/<stateHome>/...` path under the store where one is named, else under the home (plugins.ts:365).
pub fn in_store(state_home: &str, path: &str, home: &str, store: Option<&str>) -> String {
    let own = format!("~/{state_home}");
    if let Some(store) = store {
        if path == own || path.starts_with(&format!("{own}/")) {
            return format!("{store}{}", &path[own.len()..]);
        }
    }
    if path == "~" {
        return home.to_string();
    }
    match path.strip_prefix("~/") {
        Some(rest) => format!("{home}/{rest}"),
        None => path.to_string(),
    }
}

/// The JSON a text holds, or nothing (plugins.ts:375).
pub fn json_of(text: Option<&str>) -> Option<Value> {
    parse_json(text?).ok()
}

/// Absolute paths joined, `.` and `..` resolved, `..` past the root staying at the root (plugins.ts:386).
pub fn join_path(parts: &[&str]) -> String {
    let joined = parts.join("/");
    let mut out: Vec<&str> = Vec::new();
    for seg in joined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    format!("/{}", out.join("/"))
}

/// The last part after trailing slashes are taken off (plugins.ts:396).
pub fn base_name(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string()
}

/// Codex's and Claude Code's `enabledPlugins`-free install record, read off installed_plugins.json (plugins-claude.ts:28).
#[derive(Debug, Clone)]
struct Install {
    scope: PluginScope,
    install_path: String,
    version: Option<String>,
    project_path: Option<String>,
}

/// One plugin row to list: its user install, a project's or local install, or one only the image's seed holds.
#[derive(Clone)]
struct Listed {
    id: String,
    install: Install,
    project: Option<PluginProject>,
    seed: Option<Install>,
    seed_only: bool,
}

struct Row {
    id: String,
    install: Install,
    project: Option<PluginProject>,
    seeded: bool,
}

struct Manifest {
    entry: Option<Map<String, Value>>,
    there: bool,
    m: Map<String, Value>,
}

struct Brought {
    brings: PluginBrings,
    skill_dirs: Vec<String>,
}

/// Two peeks read as one: the top's answer where it has one (plugins-claude.ts:162).
struct Both<'a> {
    top: Option<&'a dyn Peeked>,
    base: &'a dyn Peeked,
}

fn both<'a>(base: &'a dyn Peeked, top: Option<&'a dyn Peeked>) -> Both<'a> {
    Both { top, base }
}

impl Peeked for Both<'_> {
    fn text(&self, path: &str) -> Option<String> {
        self.top.and_then(|t| t.text(path)).or_else(|| self.base.text(path))
    }
    fn list(&self, dir: &str) -> Option<Vec<String>> {
        self.top.and_then(|t| t.list(dir)).or_else(|| self.base.list(dir))
    }
    fn looked(&self, root: &str) -> bool {
        self.top.is_some_and(|t| t.looked(root)) || self.base.looked(root)
    }
}

const CLAUDE_STATE: &str = ".claude";
const INDEX: &str = "~/.claude/plugins/installed_plugins.json";
const MARKETPLACES: &str = "~/.claude/plugins/known_marketplaces.json";
const SETTINGS: &str = "~/.claude/settings.json";
/// A project's shared settings, then the login's own for it, which wins.
const PROJECT_SETTINGS: &str = ".claude/settings.json";
const LOCAL_SETTINGS: &str = ".claude/settings.local.json";
/// What a plugin's folder holds where Claude Code looks by default (plugins-claude.ts:93).
const FOLDER_FILES: [&str; 4] = [".claude-plugin/plugin.json", "hooks/hooks.json", ".mcp.json", ".lsp.json"];
const MARKETPLACE_FILE: &str = ".claude-plugin/marketplace.json";
/// A seed folder holds what the config folder's plugins folder does (code.claude.com/docs/en/plugins/org).
const SEED_INDEX: &str = "installed_plugins.json";
const SEED_MARKETPLACES: &str = "known_marketplaces.json";

fn folder_dirs() -> Vec<PeekDir> {
    [(".", 1), ("skills", 2), ("commands", 1), ("agents", 1)]
        .into_iter()
        .map(|(path, depth)| PeekDir { path: path.to_string(), depth })
        .collect()
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// The strings a field holds: the one string, or the strings of a list.
fn strings_of(field: Option<&Value>) -> Vec<String> {
    match field {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(items)) => items.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

/// A manifest path resolved in the plugin's folder; nothing for one that would leave it (plugins-claude.ts:69).
fn inside(root: &str, path: &str) -> Option<String> {
    let at = join_path(&[root, path]);
    (at == root || at.starts_with(&format!("{root}/"))).then_some(at)
}

/// A manifest's paths for one component, inside the plugin (plugins-claude.ts:102).
fn paths_of(root: &str, field: Option<&Value>) -> Vec<String> {
    strings_of(field).iter().filter_map(|p| inside(root, p)).collect()
}

/// The `.json` files a hooks, servers or language servers field names (plugins-claude.ts:105).
fn json_files(root: &str, field: Option<&Value>) -> Vec<String> {
    let items: Vec<&Value> = match field {
        None => vec![],
        Some(Value::Array(items)) => items.iter().collect(),
        Some(v) => vec![v],
    };
    items.into_iter().filter_map(Value::as_str).filter(|p| p.ends_with(".json")).filter_map(|p| inside(root, p)).collect()
}

fn stem(file: &str) -> String {
    let base = base_name(file);
    base.strip_suffix(".md").map_or(base.clone(), str::to_string)
}

fn markdown(names: &[String]) -> Vec<String> {
    sorted(names.iter().filter(|n| n.ends_with(".md")).cloned().collect())
}

/// The folder's listing, nothing where the peek has none.
fn listing(seen: &dyn Peeked, dir: &str) -> Vec<String> {
    seen.list(dir).unwrap_or_default()
}

fn installs_of(index: Option<&Value>) -> IndexMap<String, Vec<Install>> {
    let mut out: IndexMap<String, Vec<Install>> = IndexMap::new();
    let Some(plugins) = index.and_then(Value::as_object).and_then(|o| o.get("plugins")).and_then(Value::as_object) else {
        return out;
    };
    for (id, records) in js_entries(plugins) {
        let Some(records) = records.as_array() else {
            continue;
        };
        for record in records {
            let Some(r) = record.as_object() else {
                continue;
            };
            let Some(install_path) = r.get("installPath").and_then(Value::as_str).filter(|p| p.starts_with('/')) else {
                continue;
            };
            let scope = match r.get("scope").and_then(Value::as_str) {
                Some("user") => PluginScope::User,
                Some("project") => PluginScope::Project,
                Some("local") => PluginScope::Local,
                _ => continue,
            };
            out.entry(id.clone()).or_default().push(Install {
                scope,
                install_path: install_path.trim_end_matches('/').to_string(),
                version: r.get("version").and_then(Value::as_str).map(String::from),
                project_path: r.get("projectPath").and_then(Value::as_str).map(String::from),
            });
        }
    }
    out
}

/// What a settings file says of a plugin: on, off, or nothing (plugins-claude.ts:51).
fn switch_in(settings: Option<&Value>, id: &str) -> Option<bool> {
    settings?.get("enabledPlugins")?.as_object()?.get(id)?.as_bool()
}

/// Where a marketplace comes from, as known_marketplaces.json says (plugins-claude.ts:57).
fn source_of(known: &Map<String, Value>, marketplace: &str) -> Option<String> {
    let source = known.get(marketplace)?.as_object()?.get("source")?.as_object()?;
    let value = match source.get("source").and_then(Value::as_str) {
        Some("github") => source.get("repo"),
        Some("directory") => source.get("path"),
        _ => source.get("url"),
    };
    value?.as_str().map(String::from)
}

/// The manifest Claude Code loads: plugin.json with a marketplace entry's commands, agents and skills appended and its
/// hooks merged where the entry is not `strict: false`; the entry alone where the plugin has no plugin.json (plugins-claude.ts:79).
fn effective(manifest: Option<&Map<String, Value>>, entry: Option<&Map<String, Value>>) -> Map<String, Value> {
    let Some(manifest) = manifest else {
        return entry.cloned().unwrap_or_default();
    };
    let Some(entry) = entry.filter(|e| e.get("strict") != Some(&Value::Bool(false))) else {
        return manifest.clone();
    };
    let mut out = manifest.clone();
    for key in ["commands", "agents", "skills"] {
        let Some(added) = entry.get(key) else {
            continue;
        };
        let value = if matches!(manifest.get(key), Some(Value::Object(_))) || added.is_object() {
            let mut merged = Map::new();
            if let Some(from_manifest) = manifest.get(key).and_then(Value::as_object) {
                merged.extend(from_manifest.clone());
            }
            if let Some(from_entry) = added.as_object() {
                merged.extend(from_entry.clone());
            }
            Value::Object(merged)
        } else {
            let mut list = strings_of(manifest.get(key));
            list.extend(strings_of(Some(added)));
            Value::Array(list.into_iter().map(Value::String).collect())
        };
        out.insert(key.to_string(), value);
    }
    if let Some(added) = entry.get("hooks") {
        let mut hooks = match manifest.get("hooks") {
            None => vec![],
            Some(Value::Array(items)) => items.clone(),
            Some(v) => vec![v.clone()],
        };
        hooks.push(added.clone());
        out.insert("hooks".to_string(), Value::Array(hooks));
    }
    out
}

/// The keys of the maps a field names, its default file first: inline maps, and `.json` files holding the map under
/// `wrap` or bare (plugins-claude.ts:119).
fn keys_of(root: &str, field: Option<&Value>, file: &str, wrap: &str, seen: &dyn Peeked) -> Vec<String> {
    let parts: Vec<&Value> = match field {
        None => vec![],
        Some(Value::Array(items)) => items.iter().collect(),
        Some(v) => vec![v],
    };
    let mut paths = vec![join_path(&[root, file])];
    paths.extend(json_files(root, field));
    let mut maps: Vec<Map<String, Value>> = paths
        .iter()
        .filter_map(|p| {
            let json = json_of(seen.text(p).as_deref())?;
            let obj = json.as_object()?;
            Some(obj.get(wrap).and_then(Value::as_object).unwrap_or(obj).clone())
        })
        .collect();
    maps.extend(parts.into_iter().filter_map(|p| p.as_object().cloned()));
    unique(maps.iter().flat_map(|m| js_entries(m).into_iter().map(|(k, _)| k.clone())))
}

/// The skills a folder holds: itself where it holds a SKILL.md, else each folder in it that does (plugins-claude.ts:129).
fn skills_in(dir: &str, seen: &dyn Peeked) -> Vec<String> {
    let names = listing(seen, dir);
    if names.iter().any(|n| n == "SKILL.md") {
        return vec![base_name(dir)];
    }
    names.iter().filter(|n| !n.contains('/') && !n.starts_with('.') && names.contains(&format!("{n}/SKILL.md"))).cloned().collect()
}

/// What one plugin's folder brings, each by the name a turn gives it, off what the peeks read (plugins-claude.ts:136).
fn brought_by(root: &str, prefix: &str, m: &Map<String, Value>, seen: &dyn Peeked) -> Brought {
    let named = |names: Vec<String>| -> Vec<String> { unique(names).into_iter().map(|n| format!("{prefix}:{n}")).collect() };
    let skill_dirs: Vec<String> = std::iter::once(join_path(&[root, "skills"])).chain(paths_of(root, m.get("skills"))).collect();
    let mut skills: Vec<String> = skill_dirs.iter().flat_map(|d| skills_in(d, seen)).collect();
    if skills.is_empty() && m.get("skills").is_none() && listing(seen, root).iter().any(|n| n == "SKILL.md") {
        skills.push(prefix.to_string());
    }
    let md_in = |dir: &str| -> Vec<String> { markdown(&listing(seen, dir)).iter().map(|f| stem(f)).collect() };
    let commands: Vec<String> = match m.get("commands") {
        None => md_in(&join_path(&[root, "commands"])),
        Some(Value::Object(map)) => js_entries(map).into_iter().map(|(k, _)| k.clone()).collect(),
        Some(v) => paths_of(root, Some(v)).iter().flat_map(|p| if p.ends_with(".md") { vec![stem(p)] } else { md_in(p) }).collect(),
    };
    let subagents: Vec<String> = match m.get("agents") {
        None => md_in(&join_path(&[root, "agents"])),
        Some(Value::Object(map)) => js_entries(map).into_iter().map(|(k, _)| k.clone()).collect(),
        Some(v) => paths_of(root, Some(v)).iter().filter(|p| p.ends_with(".md")).map(|p| stem(p)).collect(),
    };
    let brings = PluginBrings {
        skills: sorted(named(skills)),
        commands: sorted(named(commands)),
        subagents: sorted(named(subagents)),
        hooks: sorted(keys_of(root, m.get("hooks"), "hooks/hooks.json", "hooks", seen)),
        servers: sorted(
            keys_of(root, m.get("mcpServers"), ".mcp.json", "mcpServers", seen)
                .into_iter()
                .map(|s| format!("plugin:{prefix}:{s}"))
                .collect(),
        ),
        lsp: sorted(keys_of(root, m.get("lspServers"), ".lsp.json", "lspServers", seen)),
        apps: vec![],
    };
    let skill_dirs =
        skill_dirs.into_iter().filter(|d| !skills_in(d, seen).is_empty() && !listing(seen, d).iter().any(|n| n == "SKILL.md")).collect();
    Brought { brings, skill_dirs }
}

/// What a manifest names outside the default places, which a second peek reads: skill and command folders, and the
/// `.json` files of its hooks and servers (plugins-claude.ts:109).
fn declared(root: &str, m: &Map<String, Value>) -> (Vec<PeekDir>, Vec<String>) {
    let rel = |p: &str| -> String {
        if p == root {
            ".".to_string()
        } else {
            p[root.len() + 1..].to_string()
        }
    };
    let mut dirs: Vec<PeekDir> = paths_of(root, m.get("skills")).iter().map(|p| PeekDir { path: rel(p), depth: 2 }).collect();
    if !matches!(m.get("commands"), Some(Value::Object(_))) {
        dirs.extend(paths_of(root, m.get("commands")).iter().filter(|p| !p.ends_with(".md")).map(|p| PeekDir { path: rel(p), depth: 1 }));
    }
    let files =
        [m.get("hooks"), m.get("mcpServers"), m.get("lspServers")].into_iter().flat_map(|f| json_files(root, f)).map(|p| rel(&p)).collect();
    (dirs, files)
}

/// A marketplace's own folder, as known_marketplaces.json says (plugins-claude.ts:165).
fn location_of(known: &Map<String, Value>, marketplace: &str) -> Option<String> {
    let at = known.get(marketplace)?.as_object()?;
    at.get("installLocation")?.as_str().filter(|p| p.starts_with('/')).map(String::from)
}

/// One marketplace's plugin entries by name, off the marketplace.json in its folder (plugins-claude.ts:171).
fn entries_of(known: &Map<String, Value>, marketplace: &str, seen: &dyn Peeked) -> IndexMap<String, Map<String, Value>> {
    let Some(at) = location_of(known, marketplace) else {
        return IndexMap::new();
    };
    let json = json_of(seen.text(&join_path(&[at.as_str(), MARKETPLACE_FILE])).as_deref());
    let plugins: &[Value] = json.as_ref().and_then(|j| j.get("plugins")).and_then(Value::as_array).map_or(&[], Vec::as_slice);
    plugins
        .iter()
        .filter_map(|p| {
            let o = p.as_object()?;
            Some((o.get("name")?.as_str()?.to_string(), o.clone()))
        })
        .collect()
}

/// The installs a seed index holds, first seed winning (plugins-claude.ts:206).
fn seed_installs(seed_indexes: &[Option<Value>]) -> IndexMap<String, Install> {
    let mut in_seed: IndexMap<String, Install> = IndexMap::new();
    for index in seed_indexes {
        for (id, installs) in installs_of(index.as_ref()) {
            let Some(mine) = installs.into_iter().find(|i| i.scope == PluginScope::User) else {
                continue;
            };
            if !in_seed.contains_key(&id) {
                in_seed.insert(id, mine);
            }
        }
    }
    in_seed
}

struct SettingsOf {
    shared: Option<Value>,
    local: Option<Value>,
}

/// Claude Code's shelf: one peek reads the config and every plugin's folder, a second only for a path the first could not
/// read off the file, a third for a manifest that keeps a component outside its default place (plugins-claude.ts:187).
pub async fn read_claude(io: &impl PluginIo, projects: &[PluginProject]) -> PluginsFound {
    let (home, store) = (io.home(), io.store());
    let path = |p: &str| in_store(CLAUDE_STATE, p, home, store);
    let seeds: Vec<String> = io.seed().unwrap_or("").split(':').filter(|d| d.starts_with('/')).map(String::from).collect();
    let index_path = path(INDEX);
    let marketplaces_path = path(MARKETPLACES);
    let settings_path = path(SETTINGS);
    let seed_index: Vec<String> = seeds.iter().map(|s| join_path(&[s.as_str(), SEED_INDEX])).collect();
    let seed_marketplaces: Vec<String> = seeds.iter().map(|s| join_path(&[s.as_str(), SEED_MARKETPLACES])).collect();
    let groups = [
        PeekGroup {
            roots: Some(strings(&["/"])),
            files: Some(vec![index_path.clone(), marketplaces_path.clone(), settings_path.clone()]),
            ..Default::default()
        },
        PeekGroup {
            roots: Some(projects.iter().map(|p| p.path.clone()).collect()),
            files: Some(strings(&[PROJECT_SETTINGS, LOCAL_SETTINGS])),
            ..Default::default()
        },
        PeekGroup {
            roots_from: Some(RootsFrom { files: [vec![index_path.clone()], seed_index.clone()].concat(), key: "installPath".to_string() }),
            files: Some(strings(&FOLDER_FILES)),
            dirs: Some(folder_dirs()),
            ..Default::default()
        },
        PeekGroup {
            roots_from: Some(RootsFrom {
                files: [vec![marketplaces_path.clone()], seed_marketplaces.clone()].concat(),
                key: "installLocation".to_string(),
            }),
            files: Some(strings(&[MARKETPLACE_FILE])),
            ..Default::default()
        },
        PeekGroup { roots: Some(seeds.clone()), files: Some(strings(&[SEED_INDEX, SEED_MARKETPLACES])), ..Default::default() },
    ];
    let refused = |message: String| PluginsFound { plugins: vec![], refused: vec![message], version: None, file: None };
    let Some(first) = io.peek(&groups).await else {
        return refused("plugins: Claude Code's plugin files could not be read".to_string());
    };
    let index_text = first.text(&index_path);
    let seed_indexes: Vec<Option<Value>> = seed_index.iter().map(|p| json_of(first.text(p).as_deref())).collect();
    // Before its first turn a machine's config folder has no index, and every plugin it loads is in the seed.
    if index_text.is_none() && seed_indexes.iter().all(Option::is_none) {
        return PluginsFound::default();
    }
    let index = match index_text {
        None => Value::Object(Map::new()),
        Some(text) => match json_of(Some(&text)) {
            Some(index) => index,
            None => return refused(format!("plugins: {index_path} does not parse, so Claude Code's plugins were not read")),
        },
    };
    // Each startup a seed's marketplace overwrites the config folder's entry of that name, the first seed's winning.
    let mut known: Map<String, Value> = Map::new();
    let config_known = json_of(first.text(&marketplaces_path).as_deref());
    let seed_known = seed_marketplaces.iter().map(|p| json_of(first.text(p).as_deref()));
    for source in std::iter::once(config_known).chain(seed_known.collect::<Vec<_>>().into_iter().rev()).flatten() {
        if let Value::Object(m) = source {
            known.extend(m);
        }
    }
    let in_seed = seed_installs(&seed_indexes);
    let user = json_of(first.text(&settings_path).as_deref());
    let settings_of = |p: &PluginProject| SettingsOf {
        shared: json_of(first.text(&join_path(&[p.path.as_str(), PROJECT_SETTINGS])).as_deref()),
        local: json_of(first.text(&join_path(&[p.path.as_str(), LOCAL_SETTINGS])).as_deref()),
    };
    // A project or local install is a row only in a project the read covers; one for a folder gone is dropped. A user
    // install the seed also holds keeps the seed's beside it, which a turn loads where the config folder's is gone.
    let mut listed: Vec<Listed> = Vec::new();
    for (id, installs) in installs_of(Some(&index)) {
        let mine = installs.iter().find(|i| i.scope == PluginScope::User).cloned();
        if let Some(mine) = mine {
            listed.push(Listed { id: id.clone(), install: mine, project: None, seed: in_seed.get(&id).cloned(), seed_only: false });
        }
        for scope in [PluginScope::Project, PluginScope::Local] {
            for p in projects {
                if let Some(install) = installs.iter().find(|i| i.scope == scope && i.project_path.as_deref() == Some(p.path.as_str())) {
                    listed.push(Listed {
                        id: id.clone(),
                        install: install.clone(),
                        project: Some(p.clone()),
                        seed: None,
                        seed_only: false,
                    });
                }
            }
        }
    }
    for (id, seed) in &in_seed {
        if !listed.iter().any(|r| &r.id == id && r.project.is_none()) {
            listed.push(Listed { id: id.clone(), install: seed.clone(), project: None, seed: Some(seed.clone()), seed_only: true });
        }
    }
    let unread: Vec<String> = unique(
        listed.iter().flat_map(|r| std::iter::once(r.install.install_path.clone()).chain(r.seed.iter().map(|s| s.install_path.clone()))),
    )
    .into_iter()
    .filter(|r| !first.looked(r))
    .collect();
    let unknown: Vec<String> = unique(listed.iter().filter_map(|r| location_of(&known, &plugin_id_parts(&r.id).1)))
        .into_iter()
        .filter(|r| !first.looked(r))
        .collect();
    let folder_peek = if unread.is_empty() && unknown.is_empty() {
        None
    } else {
        io.peek(&[
            PeekGroup { roots: Some(unread), files: Some(strings(&FOLDER_FILES)), dirs: Some(folder_dirs()), ..Default::default() },
            PeekGroup { roots: Some(unknown), files: Some(strings(&[MARKETPLACE_FILE])), ..Default::default() },
        ])
        .await
    };
    let folders = both(first.as_ref(), folder_peek.as_deref());
    let rows: Vec<Row> = listed
        .into_iter()
        .map(|r| {
            let seeded = r.seed.is_some() && (r.seed_only || folders.list(&r.install.install_path).is_none());
            match r.seed {
                Some(seed) if seeded => Row { id: r.id, install: seed, project: r.project, seeded: true },
                _ => Row { id: r.id, install: r.install, project: r.project, seeded: false },
            }
        })
        .collect();
    let mut entries: IndexMap<String, IndexMap<String, Map<String, Value>>> = IndexMap::new();
    let manifests: Vec<Manifest> = rows
        .iter()
        .map(|row| {
            let (name, marketplace) = plugin_id_parts(&row.id);
            let marketplace_entries = entries.entry(marketplace.clone()).or_insert_with(|| entries_of(&known, &marketplace, &folders));
            let entry = marketplace_entries.get(&name).cloned();
            let there = folders.list(&row.install.install_path).is_some();
            let manifest = if there {
                json_of(folders.text(&join_path(&[row.install.install_path.as_str(), FOLDER_FILES[0]])).as_deref())
            } else {
                None
            };
            let m = effective(manifest.as_ref().and_then(Value::as_object), entry.as_ref());
            Manifest { entry, there, m }
        })
        .collect();
    let mut extra: Vec<PeekGroup> = Vec::new();
    for (row, manifest) in rows.iter().zip(&manifests) {
        let (dirs, files) = if manifest.there { declared(&row.install.install_path, &manifest.m) } else { (vec![], vec![]) };
        if !dirs.is_empty() || !files.is_empty() {
            extra.push(PeekGroup {
                roots: Some(vec![row.install.install_path.clone()]),
                files: Some(files),
                dirs: Some(dirs),
                ..Default::default()
            });
        }
    }
    let extra_peek = if extra.is_empty() { None } else { io.peek(&extra).await };
    let seen = both(&folders, extra_peek.as_deref());
    let plugins: Vec<FoundPlugin> = rows
        .iter()
        .zip(&manifests)
        .map(|(row, manifest)| {
            let (name, marketplace) = plugin_id_parts(&row.id);
            let m = &manifest.m;
            let entry = manifest.entry.as_ref();
            let prefix = m.get("name").and_then(Value::as_str).filter(|s| !s.is_empty()).map_or(name.clone(), String::from);
            // A seed's plugins are not on of themselves: a turn loads one only where a settings file switches it on (2.1.296).
            let fallback = if row.seeded {
                false
            } else {
                entry
                    .and_then(|e| e.get("defaultEnabled"))
                    .and_then(Value::as_bool)
                    .or_else(|| m.get("defaultEnabled").and_then(Value::as_bool))
                    .unwrap_or(true)
            };
            let set = row.project.as_ref().map(&settings_of);
            let local = set.as_ref().and_then(|s| switch_in(s.local.as_ref(), &row.id));
            let shared = set.as_ref().and_then(|s| switch_in(s.shared.as_ref(), &row.id));
            // The order Claude Code merges in, measured on 2.1.296: local over project over user, then the plugin's default.
            let on = local.or(shared).or(switch_in(user.as_ref(), &row.id)).unwrap_or(fallback);
            let read = manifest.there.then(|| brought_by(&row.install.install_path, &prefix, m, &seen));
            let description = m
                .get("description")
                .and_then(Value::as_str)
                .or_else(|| entry.and_then(|e| e.get("description")).and_then(Value::as_str))
                .map(String::from);
            let set_in = row.project.as_ref().map(|p| {
                let file = if local.is_some() || (shared.is_none() && row.install.scope == PluginScope::Local) {
                    LOCAL_SETTINGS
                } else {
                    PROJECT_SETTINGS
                };
                join_path(&[p.path.as_str(), file])
            });
            let (brings, skills) = match read {
                Some(read) => {
                    let skills = (!read.skill_dirs.is_empty()).then(|| PluginSkillDirs { dirs: read.skill_dirs, prefix: prefix.clone() });
                    (read.brings, skills)
                }
                None => (nothing_brought(), None),
            };
            FoundPlugin {
                id: row.id.clone(),
                name,
                marketplace: marketplace.clone(),
                version: row.install.version.clone(),
                scope: row.install.scope,
                project: row.project.clone(),
                on,
                missing: if manifest.there { None } else { Some(PluginMissing::Folder) },
                path: Some(row.install.install_path.clone()),
                set_in,
                description,
                source: source_of(&known, &marketplace),
                brings,
                skills,
            }
        })
        .collect();
    PluginsFound { plugins, refused: vec![], version: None, file: None }
}

/// The switch (plugins-claude.ts:281): the store's settings get `enabledPlugins[id]` set, inside the store's base.
/// A write that fails answers the error `edit` threw, kind and all.
pub async fn turn_claude(io: &impl PluginIo, plugin: &FoundPlugin, on: bool) -> Result<Option<String>, HostError> {
    if !io.can_edit() {
        return Ok(Some(format!("{} was not switched: this road writes no settings file.", plugin.id)));
    }
    let (home, store) = (io.home(), io.store());
    let file = in_store(CLAUDE_STATE, SETTINGS, home, store);
    let base = store_base(&in_store(CLAUDE_STATE, "~/.claude", home, store), home);
    let edit = vec![(vec![PathSeg::Key("enabledPlugins".to_string()), PathSeg::Key(plugin.id.clone())], Some(Value::Bool(on)))];
    let change = move |text: Option<&str>| -> Result<String, HostError> { edit_json(text.unwrap_or("{}"), &edit) };
    io.edit(&file, &base, &change).await.map(|()| None)
}
