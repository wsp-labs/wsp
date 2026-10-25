// SPDX-License-Identifier: AGPL-3.0-only
//! A recipe file as the person writes it: its kinds, what a sign-in reads as, and the options the picker offers. Port of the functions of packages/protocol/src/recipe-file.ts at
//! a3b870a6e; its shapes and flat constants are wsp_wire::types::recipe_file. See spec/recipe_file.md.

use wsp_wire::types::recipe_file::{GitHubSignIn, COPY_KEYS_WORD};
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::types::recipe_file::RecipeSignIn;
use wsp_wire::types::recipe_file::ServerSignIn;

use indexmap::IndexMap;
use serde::Serialize;

use wsp_wire::types::wsp_tools::MCP_SERVER_NAME;

use crate::js::js_trim;
use crate::usage::list_words;
use crate::words::base::plural;

// How an agent or the GitHub row signs in on the computer: the token or key this computer's vault holds, set in the
// environment of every run there, or the tool's own login run on that computer through the sign-in relay. An agent
// that mints a token here names which of the two it takes: `token`, minted here by its own command, or `key`, an API
// key the person put in the wsp home's .env, which bill differently.
// recipe-file.ts:17
// in wsp_wire::types::recipe_file: RecipeSignIn

// recipe-file.ts:18
// in wsp_wire::types::recipe_file: RecipeSignIn

// How the GitHub row signs gh in on the computer: either sign-in word, or skip, which leaves gh signed out there and
// a private repository a folder that waits on GitHub to clone.
// recipe-file.ts:22
// in wsp_wire::types::recipe_file: GitHubSignIn

// recipe-file.ts:23
// in wsp_wire::types::recipe_file: GitHubSignIn

/// The leading run of characters that are not JavaScript white space.
fn non_space_run(text: &str) -> &str {
    &text[..text.find(crate::js::is_js_whitespace).unwrap_or(text.len())]
}

/// The file name a kind of row is called in a sentence: "agent", "MCP server", "CLI" and so on.
fn kind_word(kind: RecipeKind) -> &'static str {
    match kind {
        RecipeKind::Agents => "agent",
        RecipeKind::Mcp => "MCP server",
        RecipeKind::Clis => "CLI",
        RecipeKind::Skills => "skill",
        RecipeKind::Plugins => "plugin",
        RecipeKind::Folders => "folder",
        RecipeKind::Configs => "config",
    }
}

/// Every object's keys sorted by UTF-16 code unit at every depth, the way the TS's `sorted` reads them.
fn sorted_keys(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => serde_json::Value::Array(items.iter().map(sorted_keys).collect()),
        serde_json::Value::Object(object) => {
            let mut entries: Vec<(&String, &serde_json::Value)> = object.iter().collect();
            entries.sort_by(|a, b| crate::js::utf16::cmp(a.0, b.0));
            serde_json::Value::Object(entries.into_iter().map(|(key, value)| (key.clone(), sorted_keys(value))).collect())
        }
        other => other.clone(),
    }
}

/// What `gh auth status` says of the first account it lists: the login it signs in as and its token's scopes. The
/// token itself is never read.
// recipe-file.ts:27
// export function ghStatusOf(said: string): { account?: string; scopes?: string[] } {
//   const account = /Logged in to \S+ (?:account|as) (\S+)/.exec(said)?.[1];
//   const scopes = /Token scopes:\s*(.+)/
//     .exec(said)?.[1]
//     ?.split(",")
//     .map(s => s.replace(/'/g, "").trim())
//     .filter(s => s !== "");
//   return { ...(account !== undefined ? { account } : {}), ...(scopes !== undefined && scopes.length > 0 ? { scopes } : {}) };
// }
/// The TS `{ account?, scopes? }`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct GhStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
}

pub fn gh_status_of(said: &str) -> GhStatus {
    let mut account = None;
    for (at, _) in said.match_indices("Logged in to ") {
        let rest = &said[at + "Logged in to ".len()..];
        let host = non_space_run(rest);
        if host.is_empty() {
            continue;
        }
        let Some(after_host) = rest[host.len()..].strip_prefix(' ') else { continue };
        let Some(after_word) = after_host.strip_prefix("account ").or_else(|| after_host.strip_prefix("as ")) else { continue };
        let login = non_space_run(after_word);
        if !login.is_empty() {
            account = Some(login.to_string());
            break;
        }
    }
    let scopes = said.find("Token scopes:").map(|at| {
        let body = said[at + "Token scopes:".len()..].trim_start_matches(crate::js::is_js_whitespace);
        let line = &body[..body.find(['\n', '\r', '\u{2028}', '\u{2029}']).unwrap_or(body.len())];
        line.split(',').map(|scope| js_trim(&scope.replace('\'', "")).to_string()).filter(|scope| !scope.is_empty()).collect::<Vec<String>>()
    });
    GhStatus { account, scopes: scopes.filter(|scopes| !scopes.is_empty()) }
}

// How an MCP server signs in on the computer it goes to: with nothing, with a key or a token its definition
// carries and that travels with it, or through a browser sign-in run there (an http server the agent signs in, or
// mcp-remote's).
// recipe-file.ts:40
// in wsp_wire::types::recipe_file: ServerSignIn

// recipe-file.ts:41
// in wsp_wire::types::recipe_file: ServerSignIn

// A row's name, and the facts a row says beside it, made once each: every schema made costs the host its methods
// bound anew, held for good.
// recipe-file.ts:45
// private in the TS: port it as a private helper
// const NAME = z.string().min(1);

// recipe-file.ts:46
// private in the TS: port it as a private helper
// const NAMES = z.array(NAME);

// recipe-file.ts:47
// private in the TS: port it as a private helper
// const SIGN_IN = RecipeSignIn.optional();

// recipe-file.ts:48
// private in the TS: port it as a private helper
// const row = <T extends z.ZodRawShape>(shape: T) => z.object(shape);

// One recipe file, `<state dir>/recipes/<slug>.toml`. Every table is keyed by the name the row goes by here: an
// agent's catalog id, a server's name, a CLI's package, a skill's folder name, a plugin's `name@marketplace`, a
// folder's own key. `needs` is the one way a row asks for the C toolchain, which the floor carries only then.
// `copyKeys` is the person's one yes to copying the keys the ticked servers' definitions carry; without it every
// server that carries one is set aside.
// recipe-file.ts:55
// in wsp_wire::types::recipe_file: RecipeFile

// recipe-file.ts:80
// in wsp_wire::types::recipe_file: RecipeFile

/// wsp's own pieces, which every add puts on a computer itself and so are never a row to pick: its MCP server, which
/// every thread's launch there carries, and its command line, which the add installs.
// recipe-file.ts:84
// export const wspOwnServer = (name: string): boolean => name === MCP_SERVER_NAME;
pub fn wsp_own_server(name: &str) -> bool {
    name == MCP_SERVER_NAME
}

// recipe-file.ts:87
// in wsp_wire::types::recipe_file: WSP_COMMAND

/// A recipe less wsp's own pieces, which a recipe saved before they were left out can still hold: its server, and
/// the CLIs named in `wspPackages`, the packages that install the `wsp` command where the recipe is read.
// recipe-file.ts:91
// export function withoutWspOwn(file: RecipeFile, wspPackages: ReadonlySet<string>): RecipeFile {
//   const mcp = Object.fromEntries(Object.entries(file.mcp).filter(([name]) => !wspOwnServer(name)));
//   const clis = Object.fromEntries(Object.entries(file.clis).filter(([name]) => !wspPackages.has(name)));
//   return Object.keys(mcp).length === Object.keys(file.mcp).length && Object.keys(clis).length === Object.keys(file.clis).length ? file : { ...file, mcp, clis };
// }
pub fn without_wsp_own(file: &RecipeFile, wsp_packages: &indexmap::IndexSet<String>) -> RecipeFile {
    let mut out = file.clone();
    out.mcp.retain(|name, _| !wsp_own_server(&name.0));
    out.clis.retain(|name, _| !wsp_packages.contains(&name.0));
    out
}

// The kinds of row a recipe holds, in the order a person reads them.
// recipe-file.ts:98
// in wsp_wire::types::recipe_file: RECIPE_KINDS

// recipe-file.ts:99
// export type RecipeKind = (typeof RECIPE_KINDS)[number];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecipeKind {
    Agents,
    Mcp,
    Clis,
    Skills,
    Plugins,
    Folders,
    Configs,
}

// recipe-file.ts:101
// private in the TS: port it as a private helper
// const KIND_WORDS: Record<RecipeKind, string> = { agents: "agent", mcp: "MCP server", clis: "CLI", skills: "skill", plugins: "plugin", folders: "folder", configs: "config" };

/// The file name a recipe's name is kept under: lower case, every run of anything but a letter or a digit one dash.
/// Empty for a name with neither, which the writer refuses.
// recipe-file.ts:105
// export const recipeSlug = (name: string): string =>
//   name
//     .trim()
//     .toLowerCase()
//     .replace(/[^a-z0-9]+/g, "-")
//     .replace(/^-+|-+$/g, "")
//     .slice(0, 64);
pub fn recipe_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut in_dash_run = false;
    for c in js_trim(name).to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            slug.push(c);
            in_dash_run = false;
        } else if !in_dash_run {
            slug.push('-');
            in_dash_run = true;
        }
    }
    slug.trim_matches('-').chars().take(64).collect()
}

// recipe-file.ts:114
// in wsp_wire::types::recipe_file: NO_RECIPE

/// The rows of one recipe by kind, configs counted by the ones it ticks.
// recipe-file.ts:117
// export function recipeCounts(file: RecipeFile): Record<RecipeKind, number> {
//   return {
//     agents: Object.keys(file.agents).length,
//     mcp: Object.keys(file.mcp).length,
//     clis: Object.keys(file.clis).length,
//     skills: Object.keys(file.skills).length,
//     plugins: Object.keys(file.plugins).length,
//     folders: Object.keys(file.folders).length,
//     configs: Object.values(file.configs).filter(v => v !== undefined).length,
//   };
// }
pub fn recipe_counts(file: &RecipeFile) -> IndexMap<RecipeKind, usize> {
    let configs = [file.configs.git.is_some(), file.configs.shell.is_some(), file.configs.github.is_some()].into_iter().filter(|present| *present).count();
    IndexMap::from([
        (RecipeKind::Agents, file.agents.len()),
        (RecipeKind::Mcp, file.mcp.len()),
        (RecipeKind::Clis, file.clis.len()),
        (RecipeKind::Skills, file.skills.len()),
        (RecipeKind::Plugins, file.plugins.len()),
        (RecipeKind::Folders, file.folders.len()),
        (RecipeKind::Configs, configs),
    ])
}

/// One line of what a recipe holds: `2 agents, 3 CLIs, 20 skills, 1 folder, 2 configs`.
// recipe-file.ts:130
// export function recipeSummary(file: RecipeFile): string {
//   const counts = recipeCounts(file);
//   const said = RECIPE_KINDS.flatMap(kind => (counts[kind] > 0 ? [plural(counts[kind], KIND_WORDS[kind])] : []));
//   return said.length === 0 ? "nothing picked" : said.join(", ");
// }
pub fn recipe_summary(file: &RecipeFile) -> String {
    let said: Vec<String> = recipe_counts(file).into_iter().filter(|(_, n)| *n > 0).map(|(kind, n)| plural(n as f64, kind_word(kind))).collect();
    if said.is_empty() { "nothing picked".to_string() } else { said.join(", ") }
}

/// A recipe with what this computer has for each of its rows right now, keyed `<kind>/<row>`: a CLI's version, a
/// skill folder's digest at its real path, a config's digest after its cuts, a server's entry digest. A row this
/// computer no longer has reads empty. The hash over this is what a computer that applied the recipe is compared
/// against.
// recipe-file.ts:140
// export interface ResolvedRecipe {
//   file: RecipeFile;
//   items: Record<string, string>;
// }
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRecipe {
    pub file: RecipeFile,
    pub items: IndexMap<String, String>,
}

/// The text a resolved recipe hashes to: every key in order at every depth, so two files that differ only in the
/// order their rows were written resolve to one hash.
// recipe-file.ts:147
// export function recipeCanon(resolved: ResolvedRecipe): string {
//   const sorted = (v: unknown): unknown => {
//     if (Array.isArray(v)) return v.map(sorted);
//     if (v === null || typeof v !== "object") return v;
//     return Object.fromEntries(
//       Object.keys(v as Record<string, unknown>)
//         .sort()
//         .map(k => [k, sorted((v as Record<string, unknown>)[k])]),
//     );
//   };
//   return JSON.stringify(sorted(resolved));
// }
pub fn recipe_canon(resolved: &ResolvedRecipe) -> String {
    let value = serde_json::json!({ "file": serde_json::to_value(&resolved.file).expect("a recipe file serializes"), "items": resolved.items });
    serde_json::to_string(&sorted_keys(&value)).expect("a sorted recipe serializes")
}

// One recipe as the list and the show answer it: its file, its slug, the line of what it holds, the computers that
// follow it, by name, and when its file was last written, ISO.
// recipe-file.ts:162
// in wsp_wire::types::recipe_file: RecipeView

// recipe-file.ts:163
// in wsp_wire::types::recipe_file: RecipeView

/// What a recipe can pick from on this computer: every row a computer of the person's can take, Mac-only things
/// left out. A CLI that builds from source names the C toolchain under `needs`.
// recipe-file.ts:167
// export interface RecipeOptions {
//   /** `kind` is how the agent's own sign-in works (a token minted here, a key, a browser page, a device code), and
//    * `bytes` what it weighs on a box where a build measured it. */
//   agents: { id: string; name: string; signins: RecipeSignIn[]; kind?: string; bytes?: number }[];
//   /** `kind` is how the server signs in, as the collector read its definition; `keys` what the values it carries go
//    * by, names only, on a server that goes only on a yes to copying them. */
//   mcp: { name: string; agents: string[]; kind?: ServerSignIn; keys?: string[] }[];
//   /** `calls` is how many times the person's agents ran the CLI, off their session histories; never run, it is left off. */
//   clis: { name: string; via: string; version?: string; needs?: string[]; bytes?: number; calls?: number }[];
//   skills: { name: string; from: string; linked: boolean }[];
//   /** `on` where the agent's own settings here switch the plugin on. */
//   plugins: { name: string; on?: true }[];
//   /** The GitHub row carries the sign-in words it can take here: the vault only where this computer holds a gh login,
//    * with the account that login signs in as and its token's scopes where gh said them. */
//   configs: { id: "git" | "shell" | "github"; label: string; signins?: GitHubSignIn[]; account?: string; scopes?: string[] }[];
//   /** This computer's own projects, each a folder a box can take: what its history and kept files weigh, the commits
//    * no remote holds, and whether GitHub refuses an anonymous read of it, which is a repository that needs GitHub
//    * signed in there to clone. */
//   folders?: { name: string; path: string; remote?: string; private?: boolean; unpushed?: number; bytes?: number }[];
// }
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct RecipeOptions {
    pub agents: Vec<AgentOption>,
    pub mcp: Vec<McpOption>,
    pub clis: Vec<CliOption>,
    pub skills: Vec<SkillOption>,
    pub plugins: Vec<PluginOption>,
    pub configs: Vec<ConfigOption>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Vec<FolderOption>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentOption {
    pub id: String,
    pub name: String,
    pub signins: Vec<RecipeSignIn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct McpOption {
    pub name: String,
    pub agents: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<ServerSignIn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CliOption {
    pub name: String,
    pub via: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calls: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SkillOption {
    pub name: String,
    pub from: String,
    pub linked: bool,
}

/// `on` is the TS `true` or absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PluginOption {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigId {
    Git,
    Shell,
    Github,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConfigOption {
    pub id: ConfigId,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signins: Option<Vec<GitHubSignIn>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FolderOption {
    pub name: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unpushed: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<f64>,
}

// recipe-file.ts:189
// in wsp_wire::types::recipe_file: COPY_KEYS_WORD

/// Why a ticked server stayed off a computer for want of a yes to copying its keys: what they go by, and where that
/// yes is given, which is the page of the recipe the computer follows, by its name, or the server's own row in the
/// computer's setup where it follows none.
// recipe-file.ts:194
// export const keysKeptLine = (keys: readonly string[], recipe?: string): string =>
//   `not copied: it needs ${keys.length === 0 ? "a key" : listWords(keys)}; to send ${keys.length === 1 ? "it" : "them"}, ${recipe === undefined ? `press ${COPY_KEYS_WORD} on this row in the computer's Setup in Settings > Computers` : `pick ${COPY_KEYS_WORD} under MCP servers on the recipe ${recipe} in Settings > Recipes`}`;
pub fn keys_kept_line(keys: &[impl AsRef<str>], recipe: Option<&str>) -> String {
    let needs = if keys.is_empty() { "a key".to_string() } else { list_words(keys) };
    let pronoun = if keys.len() == 1 { "it" } else { "them" };
    let place = match recipe {
        None => format!("press {COPY_KEYS_WORD} on this row in the computer's Setup in Settings > Computers"),
        Some(recipe) => format!("pick {COPY_KEYS_WORD} under MCP servers on the recipe {recipe} in Settings > Recipes"),
    };
    format!("not copied: it needs {needs}; to send {pronoun}, {place}")
}

// recipe-file.ts:198
// in wsp_wire::types::recipe_file: RECIPE_NAME_REFUSAL

/// The refusal a word naming no saved recipe gets, naming the ones there are.
// recipe-file.ts:201
// export const noSuchRecipeRefusal = (word: string, held: readonly string[]): string =>
//   held.length === 0 ? `no recipe named ${word}; none is saved yet` : `no recipe named ${word}; you have ${held.join(", ")}`;
pub fn no_such_recipe_refusal(word: &str, held: &[impl AsRef<str>]) -> String {
    if held.is_empty() {
        return format!("no recipe named {word}; none is saved yet");
    }
    let held: Vec<&str> = held.iter().map(AsRef::as_ref).collect();
    format!("no recipe named {word}; you have {}", held.join(", "))
}

// recipe-file.ts:205
// in wsp_wire::types::recipe_file: NO_RECIPES

/// The refusal a save from this computer, or from a cloud, gets: picks are what a computer of the person's was set
/// up with, and this one is where they are picked from.
// recipe-file.ts:209
// export const recipeFromHereRefusal = (word: string): string => `${word} is not a computer you added; a recipe is saved from one that was set up with picks`;
pub fn recipe_from_here_refusal(word: &str) -> String {
    format!("{word} is not a computer you added; a recipe is saved from one that was set up with picks")
}

/// The refusal a save from a computer set up before picks were kept gets.
// recipe-file.ts:212
// export const noPicksRefusal = (name: string): string => `${name} was set up before recipes, so it carries no picks to save; a recipe is written from the app's Add a computer, or from a computer set up with one`;
pub fn no_picks_refusal(name: &str) -> String {
    format!("{name} was set up before recipes, so it carries no picks to save; a recipe is written from the app's Add a computer, or from a computer set up with one")
}

// recipe-file.ts:215
// in wsp_wire::types::recipe_file: RECIPES_TICKET_REFUSAL

