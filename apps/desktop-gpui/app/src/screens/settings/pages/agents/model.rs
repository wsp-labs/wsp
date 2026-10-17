// SPDX-License-Identifier: AGPL-3.0-only
//! The records the Agents pages read off the host, in the shape `@wsp/protocol` gives them, and the pure rules the web
//! reads them by (`agentsRows.ts`, `kinds/agents.ts`, `thread-defaults.ts`, `harness-picks.ts`), with their words.

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::catalog::{AgentEntry, AGENTS};

/// A host record read again into this module's own shape.
pub fn reshape<T: DeserializeOwned>(from: &impl Serialize) -> Option<T> {
    serde_json::to_value(from).ok().and_then(|v| serde_json::from_value(v).ok())
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub read_at: String,
    #[serde(default)]
    pub agents: Vec<AgentRow>,
    #[serde(default)]
    pub skills: Vec<SkillRow>,
    #[serde(default)]
    pub servers: Vec<McpRow>,
    #[serde(default)]
    pub plugins: Option<Vec<PluginRow>>,
    #[serde(default)]
    pub refused: Vec<String>,
    #[serde(default)]
    pub projects: Option<Vec<Project>>,
    #[serde(default)]
    pub reach: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub latest: Option<String>,
    pub pinned: Option<String>,
    pub road: String,
    pub installs_on_first_run: Option<bool>,
    pub version_unread: Option<bool>,
    pub path: Option<String>,
    pub via: Option<String>,
    pub sign_in: String,
    pub sign_in_road: String,
    pub wsp_tools: bool,
    pub sign_in_detail: Option<String>,
    pub sign_in_kind: Option<String>,
    pub sign_in_plan: Option<String>,
    pub update: Option<Update>,
    pub setup: Option<Setup>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Update {
    pub to: String,
    pub command: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Setup {
    pub on: bool,
    pub program: Option<String>,
    pub config_dir: Option<String>,
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env_names: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpRow {
    pub agent: String,
    pub name: String,
    pub scope: String,
    pub file: Option<String>,
    pub launch: Option<bool>,
    pub transport: Transport,
    #[serde(default)]
    pub env_names: Vec<String>,
    pub auth: String,
    pub enabled: bool,
    pub in_recipe: Option<bool>,
    pub project: Option<Project>,
    pub sign_in_line: Option<String>,
    pub off: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Transport {
    pub kind: String,
    pub line: Option<String>,
    pub host: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillRow {
    pub name: String,
    pub description: Option<String>,
    pub paths: Vec<SkillPath>,
    pub scope: String,
    pub project: Option<Project>,
    pub plugin: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillPath {
    pub path: String,
    pub agent: Option<String>,
    pub link_to: Option<String>,
    pub off: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginRow {
    pub agent: String,
    pub id: String,
    pub name: String,
    pub marketplace: String,
    pub version: Option<String>,
    pub scope: String,
    pub project: Option<Project>,
    pub on: bool,
    pub missing: Option<String>,
    pub path: Option<String>,
    pub set_in: Option<String>,
    pub description: Option<String>,
    pub source: Option<String>,
    pub brings: Brings,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Brings {
    pub skills: Vec<String>,
    pub commands: Vec<String>,
    pub subagents: Vec<String>,
    pub hooks: Vec<String>,
    pub servers: Vec<String>,
    pub lsp: Vec<String>,
    pub apps: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolsAnswer {
    pub auth: String,
    pub tools: Option<Vec<Tool>>,
    pub holder: Option<String>,
    pub refused: Option<String>,
    pub read_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Tool {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SkillHit {
    pub id: String,
    pub source: String,
    pub skill_id: String,
    pub installs: u64,
}

/// The kind of credential the host's broker adds for machines at a provider, by agent.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Broker {
    pub claude: Option<String>,
    pub codex: Option<String>,
}

/// One harness's lists as `harnesses.list` gives them.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub harness: String,
    pub label: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub models: Vec<Model>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_models: Option<Vec<Model>>,
    #[serde(default)]
    pub legacy_listed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden_models: Option<Vec<Model>>,
    #[serde(default, skip_serializing)]
    pub unshaped: Option<Unshaped>,
    #[serde(default)]
    pub efforts: Vec<Opt>,
    #[serde(default)]
    pub permission_modes: Vec<Opt>,
    #[serde(default)]
    pub access: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unshaped {
    #[serde(default)]
    pub models: Vec<Model>,
    #[serde(default)]
    pub legacy_models: Option<Vec<Model>>,
    #[serde(default)]
    pub efforts: Vec<Opt>,
    #[serde(default)]
    pub permission_modes: Vec<Opt>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub is_default: Option<bool>,
    #[serde(default)]
    pub efforts: Option<Vec<String>>,
    #[serde(default)]
    pub default_effort: Option<String>,
    #[serde(default)]
    pub added: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Opt {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub is_default: Option<bool>,
}

/// The person's defaults for one agent (`AgentDefaults`).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct AgentDefaults {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub access: Option<String>,
    pub models: Option<Picker>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Picker {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom: Option<Vec<String>>,
}

impl Picker {
    pub fn is_empty(&self) -> bool {
        self.hide.is_none() && self.order.is_none() && self.custom.is_none()
    }
}

/// The two fields of the preferences the agents' defaults read.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    pub default_agent: Option<String>,
    #[serde(default)]
    pub agent_defaults: HashMap<String, AgentDefaults>,
}

impl Prefs {
    pub fn own(&self, agent: &str) -> AgentDefaults {
        self.agent_defaults.get(agent).cloned().unwrap_or_default()
    }
}

pub fn entry(id: &str) -> Option<&'static AgentEntry> {
    AGENTS.iter().find(|a| a.id == id)
}

/// `agentName`: an id the catalog does not know reads as itself.
pub fn agent_name(id: &str) -> String {
    entry(id).map(|e| e.name.to_owned()).unwrap_or_else(|| id.to_owned())
}

pub fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `listWords`: a, b and c.
pub fn list_words(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// `offlineFor`: minutes, then hours, then days.
pub fn offline_for(ms: u64) -> String {
    let minutes = ms / 60_000;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let hours = minutes / 60;
    if hours < 24 {
        format!("{hours} h")
    } else {
        format!("{} d", hours / 24)
    }
}

/// `ABOUT_WORDS.readWhen`.
pub fn read_when(ms: u64) -> String {
    if ms < 60_000 {
        "just now".to_owned()
    } else {
        format!("{} ago", offline_for(ms))
    }
}

/// `compareVersions`: the core numbers in order, and a release over its own prerelease.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn parts(v: &str) -> (Vec<String>, Option<Vec<String>>) {
        let v = v.trim().trim_start_matches('v');
        let v = v.split('+').next().unwrap_or_default();
        let (core, pre) = match v.split_once('-') {
            Some((core, pre)) => (core, Some(pre.split('.').map(str::to_owned).collect())),
            None => (v, None),
        };
        (core.split('.').map(str::to_owned).collect(), pre)
    }
    fn ident(a: &str, b: &str) -> Ordering {
        match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            _ => a.cmp(b),
        }
    }
    let ((lc, lp), (rc, rp)) = (parts(a), parts(b));
    for i in 0..lc.len().max(rc.len()) {
        let order = ident(lc.get(i).map_or("0", String::as_str), rc.get(i).map_or("0", String::as_str));
        if order != Ordering::Equal {
            return order;
        }
    }
    match (lp, rp) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(l), Some(r)) => {
            for i in 0..l.len().max(r.len()) {
                match (l.get(i), r.get(i)) {
                    (None, _) => return Ordering::Less,
                    (_, None) => return Ordering::Greater,
                    (Some(x), Some(y)) => {
                        let order = ident(x, y);
                        if order != Ordering::Equal {
                            return order;
                        }
                    }
                }
            }
            Ordering::Equal
        }
    }
}

/// `AGENTS_LIST_WORDS` and `AGENTS_PAGE_WORDS`, the ones these pages say.
pub mod w {
    pub const TABS: [(&str, &str); 4] = [("agents", "Agents"), ("servers", "Tool servers"), ("skills", "Skills"), ("plugins", "Plugins")];
    pub const NEW_THREADS: &str = "New threads";
    pub const DEFAULT_AGENT: &str = "Default agent";
    pub const DEFAULT_AGENT_DESCRIPTION: &str = "A project can set its own.";
    pub fn on(name: &str) -> String {
        format!("On {name}")
    }
    pub fn group_on(group: &str, name: &str) -> String {
        format!("{group} on {name}")
    }
    pub fn state_with(state: &str, count: &str) -> String {
        format!("{state} with {count}")
    }
    pub const DETAILS: &str = "Details";
    pub const NOT_READ: &str = "Not read";
    pub const SKILL_FILE: &str = "SKILL.md";
    pub const NOT_LISTED: &str = "Not listed yet.";
    pub const NOT_INSTALLED_HERE: &str = "Not installed here";
    pub fn checked_now(when: &str) -> String {
        format!("checked {when}")
    }
    pub const READ_AGENTS_AGAIN: &str = "Read the agents again";
    pub fn update_to(version: &str) -> String {
        format!("Update to {version}")
    }
    pub fn update_copied(command: &str, computer: &str) -> String {
        format!("Copied {command}. Run it in a terminal on {computer}.")
    }
    pub fn signed_in_as(kind: &str) -> &'static str {
        match kind {
            "api-key" => "Signed in with an API key",
            "subscription" => "Signed in with a subscription",
            _ => "Signed in with OAuth",
        }
    }
    pub const SIGNED_IN_WITH: &str = "Signed in with ";
    pub const MODEL: &str = "Model";
    pub const MODEL_DESCRIPTION: &str = "The composer still changes it for one thread.";
    pub const EFFORT: &str = "Effort";
    pub const ACCESS: &str = "Access";
    pub fn access_description(agent: &str) -> String {
        format!("A project can set its own. Passed to {agent} at every launch.")
    }
    pub const ACCESS_WORDS: [(&str, &str); 4] = [("ask", "Ask"), ("auto-edit", "Auto-edit"), ("full", "Full"), ("plan", "Plan")];
    pub const MODELS: &str = "Models";
    pub const MODELS_DESCRIPTION: &str = "The ones switched on are the composer's picker, in this order.";
    pub const DEFAULT_MODEL: &str = "default";
    pub fn no_threads_yet(agent: &str) -> String {
        format!("wsp starts no threads on {agent} yet, so it has nothing here to set.")
    }
    pub fn reorder(model: &str) -> String {
        format!("Drag to reorder {model}, or use the arrow keys")
    }
    pub const MODEL_ID_PLACEHOLDER: &str = "Add a model by its id";
    pub fn not_in_list(agent: &str) -> String {
        format!("Not in {agent}'s list")
    }
    pub const EDIT: &str = "Edit";
    pub const CHANGE: &str = "Change";
    pub const HOW_IT_RUNS: &str = "How it runs";
    pub const PROGRAM: &str = "Program";
    pub const CONFIG_FOLDER: &str = "Config folder";
    pub fn own_folder(agent: &str) -> String {
        format!("{agent}'s own folder")
    }
    pub const LAUNCH_ARGUMENTS: &str = "Launch arguments";
    pub fn arguments_count(n: usize) -> String {
        match n {
            0 => "No arguments.".to_owned(),
            1 => "1 argument.".to_owned(),
            n => format!("{n} arguments."),
        }
    }
    pub const ENVIRONMENT: &str = "Environment";
    pub fn variables_count(n: usize) -> String {
        match n {
            0 => "No variables.".to_owned(),
            1 => "1 variable, values hidden.".to_owned(),
            n => format!("{n} variables, values hidden."),
        }
    }
    pub const SAVE: &str = "Save";
    pub const CANCEL: &str = "Cancel";
    pub fn put_back(agent: &str) -> String {
        format!("Use {agent}'s own")
    }
    pub fn program_sheet(agent: &str, computer: &str) -> String {
        format!("The program run in {agent}'s place on {computer}: a path, or a word on its PATH.")
    }
    pub fn config_sheet(agent: &str, computer: &str) -> String {
        format!("The folder on {computer} {agent} keeps its config, sessions and sign-in in, under that computer's home.")
    }
    pub fn arguments_sheet(agent: &str, computer: &str) -> String {
        format!("Words added to every launch of {agent} on {computer}, as a shell would split them.")
    }
    pub const ARGUMENTS_UNCLOSED: &str = "A quote is never closed.";
    pub fn environment_sheet(agent: &str, computer: &str) -> String {
        format!("Variables every launch of {agent} on {computer} carries. A value is typed here once and never shown again.")
    }
    pub const VARIABLE_NAME: &str = "Name";
    pub const VARIABLE_VALUE: &str = "Value";
    pub const ADD: &str = "Add";
    pub fn remove_named(name: &str) -> String {
        format!("Remove {name}")
    }
    pub const NO_VARIABLES: &str = "No variables yet.";
    pub fn config_dir_sign_in(agent: &str) -> String {
        format!("{agent} keeps its sign-in under its config folder, so sign it in again there before its next thread")
    }
    pub const SIGNED_IN: &str = "signed in";
    pub const CONNECTED: &str = "connected";
    pub const CHECKING: &str = "checking";
    pub const FAILED: &str = "failed";
    pub const NO_SIGN_IN_NEEDED: &str = "no sign-in needed";
    pub const KEY_FROM_ENVIRONMENT: &str = "key from the environment";
    pub const NEEDS_SIGN_IN: &str = "needs sign-in";
    pub const WITH_TOKEN: &str = "signed in with a token";
    pub const USED_BY_CLOUDS: &str = "used by cloud threads through wsp";
    pub const NOT_CHECKED: &str = "not checked";
    pub const NOT_INSTALLED: &str = "not installed";
    pub const AVAILABLE_TO_INSTALL: &str = "Available to install";
    pub const SIGN_IN: &str = "Sign in";
    pub const UPDATE: &str = "Update";
    pub const INSTALL: &str = "Install";
    pub const UNINSTALL: &str = "Uninstall";
    pub const REMOVE: &str = "Remove";
    pub const TURN_ON: &str = "Turn on";
    pub const TURN_OFF: &str = "Turn off";
    pub const RECONNECT: &str = "Reconnect";
    pub const ADD_TOOLS: &str = "Add the wsp tools";
    pub const CHECK: &str = "Check";
    pub fn check_starts(computer: &str) -> String {
        format!("Check starts it on {computer}")
    }
    pub const LISTING: &str = "Listing";
    pub fn tools_of(name: &str) -> String {
        format!("Tools of {name}")
    }
    pub fn tools_count(n: usize) -> String {
        format!("{n} {}", if n == 1 { "tool" } else { "tools" })
    }
    pub fn holds_sign_in(agent: &str) -> String {
        format!("{agent} holds the sign-in")
    }
    pub fn keeps_sign_in(agent: &str) -> String {
        format!("{agent} keeps this server's sign-in, so wsp cannot list its tools yet.")
    }
    pub const SIGN_IN_TO_SEE: &str = "sign in to see its tools";
    pub const NOT_LISTED_YET: &str = "not listed yet";
    pub const WAITING_ON_YOU: &str = "waiting on you";
    pub const OPEN_IN_TERMINAL: &str = "Open in terminal";
    pub const OPEN: &str = "Open";
    pub const OPEN_PAGE: &str = "Open the page";
    pub const FINISH_IN_BROWSER: &str = "Finish in your browser";
    pub const LANDED_ADDRESS: &str = "The address your browser landed on";
    pub fn page_stays_here(computer: &str) -> String {
        format!("Its page returns to localhost, which wsp does not carry back to {computer} yet. Run this in a terminal on {computer}:")
    }
    pub const RUN_IN_TERMINAL: &str = "Run in your terminal";
    pub const PASTE_TOKEN: &str = "Paste the token";
    pub const PASTE_KEY: &str = "Paste the key";
    pub const TOOLS_EVERY_TURN: &str = "a thread there is handed the wsp tools with every turn";
    pub const ADD_TOOLS_HERE_ONLY: &str = "wsp adds its tools to an agent's config only on the computer the app runs on";
    pub const EVERY_THREAD: &str = "Every thread gets it";
    pub const ON_EVERY_LAUNCH: &str = "handed to every thread there, in no file";
    pub const NOT_YET: &str = "not in this wsp yet";
    pub const STATUS: &str = "Status";
    pub const VERSION: &str = "Version";
    pub const FROM_PANEL: &str = "from a task's own panel";
    pub const DESCRIPTION: &str = "Description";
    pub const PATH: &str = "Path";
    pub const SHARED: &str = "shared";
    pub const COMMAND: &str = "Command";
    pub const URL: &str = "URL";
    pub const HEADERS: &str = "Headers";
    pub const CONFIG_LOCATION: &str = "Config location";
    pub const TOOLS: &str = "Tools";
    pub const RECIPE: &str = "Recipe";
    pub const IN_RECIPE: &str = "yes";
    pub const NOT_IN_RECIPE: &str = "not in the recipe";
    pub const OWN_HOLD: &str = "installed by you, not by wsp";
    pub const SHIM_HOLD: &str = "runs through a shim wsp does not touch";
    pub const STARTS_ONCE: &str = "starts the server once";
    pub const ON: &str = "on";
    pub const ALWAYS_ON: &str = "always on";
    pub const KEPT_CURRENT: &str = "wsp keeps it current on every start";
    pub const IN_REPO: &str = "in the repo";
    pub fn lives_in_repo(path: &str) -> String {
        format!("lives in the repo at {path}")
    }
    pub const SEARCH_PLUGINS: &str = "Search plugins";
    pub fn no_plugins(on: &str) -> String {
        format!("No plugins on {on} yet.")
    }
    pub const MISSING: &str = "missing";
    pub const MARKETPLACE: &str = "Marketplace";
    pub const SCOPE: &str = "Scope";
    pub fn scope(scope: &str) -> &'static str {
        match scope {
            "user" => "User",
            "project" => "Project",
            _ => "Local",
        }
    }
    pub const SET_IN: &str = "Set in";
    pub const WHAT_IT_BRINGS: &str = "What it brings";
    pub const BRINGS: [&str; 7] = ["Skills", "Commands", "Subagents", "Hooks", "Tool servers", "Language servers", "Apps"];
    pub const COPY: &str = "Copy";
    pub const OFF: &str = "off";
    pub const ADD_SKILL: &str = "Add a skill";
    pub const SEARCH_SKILLS_SH: &str = "Search skills on skills.sh";
    pub const SKILLS_SH: &str = "skills.sh";
    pub const TYPE_TO_SEARCH: &str = "Type a name or a topic to find a skill.";
    pub fn no_hits(q: &str) -> String {
        format!("skills.sh has nothing for \"{q}\".")
    }
    pub const INSTALLED: &str = "installed";
    pub fn install_name(name: &str) -> String {
        format!("Install {name}")
    }
    pub const INSTALLING: &str = "Installing";
    pub const SOURCE: &str = "Source";
    pub const INSTALLS: &str = "Installs";
    pub const AGENTS: &str = "Agents";
    pub const WHERE: &str = "Where";
    pub const GLOBAL: &str = "Global";
    pub const READS_SHARED: &str = "reads the shared skills folder, so it has the skill anyway";
    pub fn already_on(computer: &str) -> String {
        format!("already on {computer}")
    }
    pub fn remove_title(name: &str) -> String {
        format!("Remove {name}?")
    }
    pub fn remove_body(computer: &str) -> String {
        format!("Its folder and every link to it leave {computer}.")
    }
    pub fn leaves_files(files: &[String], computer: &str) -> String {
        format!("It comes out of {} on {computer}.", files.join(" and "))
    }
    pub fn no_switch(agent: &str) -> String {
        format!("{agent} turns a server off per folder, in its own /mcp")
    }
    pub fn nothing_matches(q: &str) -> String {
        format!("Nothing matches \"{q}\".")
    }
    pub const ADD_SERVER: &str = "Add a tool server";
    pub const AGENT: &str = "Agent";
    pub const SERVER_NAME: &str = "Name";
    pub const REACHED_BY: &str = "Reached by";
    pub const BY_COMMAND: &str = "Command";
    pub const BY_ADDRESS: &str = "Address";
    pub const VARIABLES: &str = "Variables";
    pub const ADD_VARIABLE: &str = "Add a variable";
    pub const ADD_HEADER: &str = "Add a header";
    pub const VALUE: &str = "value";
    pub fn remove_pair(name: &str) -> String {
        if name.is_empty() {
            "Remove this line".to_owned()
        } else {
            format!("Remove {name}")
        }
    }
    pub const ADD_SERVER_GO: &str = "Add server";
    pub const ADDING: &str = "Adding";
    pub fn two_pairs_one_name(command: bool) -> &'static str {
        if command {
            "Two variables have the same name; keep one of them."
        } else {
            "Two headers have the same name; keep one of them."
        }
    }
    pub fn project_left(project: &str, computer: &str) -> String {
        format!("{project} is no longer on {computer}; pick where it goes.")
    }
    pub fn no_server_agents(computer: &str) -> String {
        format!("No agent on {computer} keeps tool servers in a file wsp writes.")
    }
    pub const UNCLOSED_QUOTE: &str = "The command has a quote it never closes, so nothing was written.";
    pub fn remove_name(act: &str, name: &str) -> String {
        format!("{act} {name}")
    }
    pub fn add_tools_no_config(agent: &str) -> String {
        format!("{agent} has no MCP config wsp knows, so the wsp tools were not written")
    }
    pub const FIRST_RUN: &str = "installs on first run";
    pub const VERSION_UNREAD: &str = "version unreadable";
    pub const NO_ANSWER: &str = "no answer";
}

/// What a status dot says at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Good,
    Waiting,
    Bad,
    Quiet,
}

/// A status as one dot and its word.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub state: String,
    pub tone: Tone,
    pub words: String,
    pub hover: Option<String>,
    pub count: Option<String>,
}

impl Status {
    pub fn new(state: &str, tone: Tone, words: &str) -> Status {
        Status { state: state.to_owned(), tone, words: words.to_owned(), hover: None, count: None }
    }
    pub fn hover(mut self, hover: impl Into<String>) -> Status {
        self.hover = Some(hover.into());
        self
    }

    /// `statusWords`: capitalised, with the figure it answered with.
    pub fn said(&self) -> String {
        match &self.count {
            Some(count) => w::state_with(&capitalised(&self.words), count),
            None => capitalised(&self.words),
        }
    }
}

/// `signedIn`: a turn there needs no sign-in first.
pub fn signed_in(row: &AgentRow) -> bool {
    row.sign_in == "signed-in" || row.sign_in == "vault-key"
}

/// `signInWord`.
pub fn sign_in_word(row: &AgentRow) -> &'static str {
    match row.sign_in.as_str() {
        "signed-in" => w::SIGNED_IN,
        "vault-key" => w::WITH_TOKEN,
        "none" => w::NEEDS_SIGN_IN,
        _ => w::NOT_CHECKED,
    }
}

/// `signInStatus`.
pub fn sign_in_status(row: &AgentRow) -> Status {
    let tone = if signed_in(row) {
        Tone::Good
    } else if row.sign_in == "none" {
        Tone::Waiting
    } else {
        Tone::Quiet
    };
    Status::new(&row.sign_in, tone, sign_in_word(row))
}

/// `brokerRow`: what the broker says for an agent's row, `held` none where it holds nothing for it.
pub struct BrokerSays {
    pub held: Option<&'static str>,
    pub other_roads: Option<&'static str>,
}

pub fn broker_row(id: &str, broker: Option<&Broker>) -> Option<BrokerSays> {
    let broker = broker?;
    match id {
        "claude" => Some(BrokerSays {
            held: broker.claude.as_deref().and_then(|k| match k {
                "login" => Some("signed in with a subscription"),
                "token" => Some(w::WITH_TOKEN),
                "key" => Some("signed in with an API key"),
                "gateway" => Some("signed in through your gateway"),
                _ => None,
            }),
            other_roads: Some("a token: wsp agents key claude, a key: add --api-key"),
        }),
        "codex" => Some(BrokerSays {
            held: broker.codex.as_deref().and_then(|k| match k {
                "chatgpt" | "access-token" => Some("signed in with ChatGPT"),
                "key" => Some("signed in with an API key"),
                _ => None,
            }),
            other_roads: None,
        }),
        _ => None,
    }
}

/// `rowStatus`: the broker's word where it speaks for the row, else the row's own.
pub fn row_status(row: &AgentRow, broker: Option<&Broker>) -> Status {
    match broker_row(&row.id, broker) {
        None => sign_in_status(row),
        Some(BrokerSays { held: None, .. }) => Status::new("none", Tone::Waiting, w::NEEDS_SIGN_IN),
        Some(BrokerSays { held: Some(held), .. }) => Status::new("signed-in", Tone::Good, held),
    }
}

/// `newerThan`: the newer version its vendor publishes.
pub fn newer_than(row: &AgentRow) -> Option<&str> {
    let (version, latest) = (row.version.as_deref()?, row.latest.as_deref()?);
    (compare_versions(latest, version) == Ordering::Greater).then_some(latest)
}

/// `versionLine`: the line under an installed agent's name.
pub fn version_line(row: &AgentRow, broker: Option<&Broker>) -> Option<String> {
    let version = match (&row.version, row.installs_on_first_run, row.version_unread) {
        (Some(v), _, _) => Some(format!("v{v}")),
        (None, Some(true), _) => Some(w::FIRST_RUN.to_owned()),
        (None, _, Some(true)) => Some(w::VERSION_UNREAD.to_owned()),
        _ => None,
    };
    let Some(cloud) = broker_row(&row.id, broker) else { return version };
    let clouds = match cloud.other_roads {
        Some(other) => format!("{}; {other}", w::USED_BY_CLOUDS),
        None => w::USED_BY_CLOUDS.to_owned(),
    };
    Some(match version {
        Some(v) => format!("{v}, {clouds}"),
        None => capitalised(&clouds),
    })
}

/// `signInHead`: how the sign-in stands for the head of an agent's page, and the whole sentence for its hover.
pub struct SignInHead {
    pub line: String,
    pub whole: String,
}

pub fn sign_in_head(row: &AgentRow, computer: &str, plan: Option<String>) -> SignInHead {
    let kind = row.sign_in_kind.as_deref();
    let planned = plan.is_some() && (kind == Some("subscription") || (kind.is_none() && row.sign_in == "signed-in"));
    let line = match (&plan, kind) {
        (Some(plan), _) if planned => format!("{}{plan}", w::SIGNED_IN_WITH),
        (_, Some(kind)) => w::signed_in_as(kind).to_owned(),
        _ => capitalised(sign_in_word(row)),
    };
    let whole = match &row.sign_in_detail {
        None => line.clone(),
        Some(detail) => {
            let an = if kind == Some("api-key") { "an " } else { "" };
            format!("{}{an}{}", w::SIGNED_IN_WITH, detail.replace("the machine", computer))
        }
    };
    SignInHead { line, whole }
}

/// `planWord`: the plan word a vendor names its plan by.
pub fn plan_word(plan: &str, brand: Option<&str>) -> String {
    let word = capitalised(plan);
    match brand {
        Some(brand) => format!("{brand} {word}"),
        None => word,
    }
}

/// `markedDefault`.
fn marked_default<'a>(options: impl IntoIterator<Item = (&'a str, Option<bool>)>) -> Option<&'a str> {
    options.into_iter().find(|(_, d)| *d == Some(true)).map(|(v, _)| v)
}

impl Catalog {
    /// `everyModel`.
    pub fn every_model(&self) -> Vec<&Model> {
        self.models.iter().chain(self.legacy_models.iter().flatten()).chain(self.hidden_models.iter().flatten()).collect()
    }

    /// `modelOf`: a slug the catalog does not list still counts, named by itself.
    pub fn model_of(&self, value: &str) -> Model {
        self.every_model().into_iter().find(|m| m.value == value).cloned().unwrap_or_else(|| Model {
            value: value.to_owned(),
            label: value.to_owned(),
            ..Model::default()
        })
    }

    /// `effortsFor`.
    pub fn efforts_for(&self, model: Option<&Model>) -> Vec<Opt> {
        let options: Vec<Opt> = match model.and_then(|m| m.efforts.as_ref()) {
            Some(subset) => self.efforts.iter().filter(|o| subset.contains(&o.value)).cloned().collect(),
            None => self.efforts.clone(),
        };
        match model.and_then(|m| m.default_effort.as_ref()) {
            None => options,
            Some(own) => options.into_iter().map(|o| Opt { is_default: (o.value == *own).then_some(true), ..o }).collect(),
        }
    }

    /// `accessMode`.
    pub fn access_mode(&self, word: &str) -> Option<&str> {
        let mode = self.access.as_ref()?.get(word)?;
        self.permission_modes.iter().any(|o| o.value == *mode).then_some(mode.as_str())
    }

    /// `unmarked`: the lists as the agent gives them, before the picker shaped them.
    pub fn unmarked(&self) -> Catalog {
        match &self.unshaped {
            None => self.clone(),
            Some(u) => Catalog {
                models: u.models.clone(),
                legacy_models: u.legacy_models.clone(),
                efforts: u.efforts.clone(),
                permission_modes: u.permission_modes.clone(),
                hidden_models: None,
                unshaped: None,
                ..self.clone()
            },
        }
    }

    /// `withCustomModels`.
    pub fn with_custom(&self, picker: Option<&Picker>) -> Catalog {
        let known: Vec<String> = self.every_model().iter().map(|m| m.value.clone()).collect();
        let added: Vec<Model> = picker
            .and_then(|p| p.custom.as_ref())
            .into_iter()
            .flatten()
            .filter(|id| !known.contains(id))
            .map(|id| Model { value: id.clone(), label: id.clone(), added: Some(true), ..Model::default() })
            .collect();
        let mut next = self.clone();
        next.models.extend(added);
        next
    }

    /// `shapeModels`: hidden models apart, the person's order first.
    pub fn shaped(&self, picker: Option<&Picker>) -> Catalog {
        let Some(picker) = picker else { return self.clone() };
        let hide = picker.hide.clone().unwrap_or_default();
        let order = picker.order.clone().unwrap_or_default();
        let rank = |m: &Model| order.iter().position(|v| *v == m.value).unwrap_or(order.len());
        let shown = |list: &[Model]| {
            let mut kept: Vec<Model> = list.iter().filter(|m| !hide.contains(&m.value)).cloned().collect();
            kept.sort_by_key(rank);
            kept
        };
        let models = shown(&self.models);
        let legacy = self.legacy_models.as_ref().map(|l| shown(l));
        let hidden: Vec<Model> = self
            .models
            .iter()
            .chain(self.legacy_models.iter().flatten())
            .filter(|m| hide.contains(&m.value))
            .map(|m| Model { is_default: None, ..m.clone() })
            .collect();
        let had = marked_default(self.every_model().into_iter().map(|m| (m.value.as_str(), m.is_default))).is_some();
        let has = marked_default(models.iter().chain(legacy.iter().flatten()).map(|m| (m.value.as_str(), m.is_default))).is_some();
        let models = if had && !has && !models.is_empty() {
            let first = models[0].value.clone();
            models.into_iter().map(|m| Model { is_default: (m.value == first).then_some(true), ..m }).collect()
        } else {
            models
        };
        Catalog { models, legacy_models: legacy, hidden_models: (!hidden.is_empty()).then_some(hidden), ..self.clone() }
    }
}

/// A resolved pick and where it came from: `default` the person's, `catalog` the list's own mark.
#[derive(Clone, Debug, PartialEq)]
pub struct Picked {
    pub value: String,
    pub from_default: bool,
}

/// `ThreadDefaults` past the agent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Defaults {
    pub model: Option<Picked>,
    pub effort: Option<Picked>,
    pub access: Option<Picked>,
}

/// `resolveThreadDefaults` for a named agent with no project: the person's default where the list takes it, else the
/// list's own mark.
pub fn resolve(catalog: &Catalog, own: &AgentDefaults) -> Defaults {
    fn pick(options: &[(String, Option<bool>)], open: bool, own: Option<&String>) -> Option<Picked> {
        if let Some(value) = own {
            if open || options.iter().any(|(v, _)| v == value) {
                return Some(Picked { value: value.clone(), from_default: true });
            }
        }
        options.iter().find(|(_, d)| *d == Some(true)).map(|(v, _)| Picked { value: v.clone(), from_default: false })
    }
    let models: Vec<(String, Option<bool>)> = catalog.every_model().iter().map(|m| (m.value.clone(), m.is_default)).collect();
    let model = pick(&models, models.is_empty(), own.model.as_ref());
    let picked = model.as_ref().map(|m| catalog.model_of(&m.value));
    let efforts: Vec<(String, Option<bool>)> = catalog.efforts_for(picked.as_ref()).into_iter().map(|o| (o.value, o.is_default)).collect();
    let effort = pick(&efforts, catalog.efforts.is_empty(), own.effort.as_ref());
    let word = |value: Option<&str>, from_default: bool| {
        let value = value?;
        catalog.access_mode(value).map(|_| Picked { value: value.to_owned(), from_default })
    };
    let marked = catalog.permission_modes.iter().find(|o| o.is_default == Some(true)).map(|o| o.value.clone());
    let marked_word = marked.and_then(|mode| {
        ACCESS.iter().find(|word| catalog.access.as_ref().and_then(|a| a.get(**word)) == Some(&mode)).map(|w| w.to_string())
    });
    let access = word(own.access.as_deref(), true).or_else(|| word(marked_word.as_deref(), false));
    Defaults { model, effort, access }
}

/// `ACCESS_CHOICES`.
pub const ACCESS: [&str; 4] = ["ask", "auto-edit", "full", "plan"];

/// `newThreadDefaults`: what a new thread on an agent starts with where its project names nothing.
pub fn new_thread_defaults(catalog: &Catalog, prefs: &Prefs) -> Defaults {
    let own = prefs.own(&catalog.harness);
    resolve(&catalog.unmarked().with_custom(own.models.as_ref()), &own)
}

/// `accessRefusal`: why a word maps to none of the agent's modes.
pub fn access_held(catalog: &Catalog, word: &str) -> Option<String> {
    if catalog.access_mode(word).is_some() {
        return None;
    }
    let takes: Vec<&str> = ACCESS.iter().copied().filter(|w| catalog.access_mode(w).is_some()).collect();
    Some(if takes.is_empty() {
        format!("{} takes no access of wsp's words", catalog.label)
    } else {
        format!("{} takes no {word} access; it takes {}", catalog.label, takes.join(", "))
    })
}

/// `modelIdRefusal`.
pub fn model_id_refusal(id: &str) -> Option<String> {
    if id.is_empty() {
        return Some("A model id cannot be empty".to_owned());
    }
    if id.chars().any(char::is_whitespace) {
        return Some("No model id has a space in it".to_owned());
    }
    if id.starts_with('-') {
        return Some("No model id starts with \"-\"".to_owned());
    }
    let ok = |c: char| c.is_ascii_alphanumeric() || "._-:/@+[]".contains(c);
    id.chars().find(|c| !ok(*c)).map(|c| format!("No model id has {} in it", serde_json::to_string(&c.to_string()).unwrap_or_default()))
}

/// `agentEnvRefusal`, without the adapter's dropped patterns.
pub fn env_refusal(name: &str, agent: &str) -> Option<String> {
    const PROCESS: [&str; 23] = [
        "PATH",
        "HOME",
        "SHELL",
        "USER",
        "LOGNAME",
        "IFS",
        "TMPDIR",
        "NODE_OPTIONS",
        "NODE_PATH",
        "PYTHONPATH",
        "PYTHONSTARTUP",
        "PYTHONHOME",
        "RUBYOPT",
        "RUBYLIB",
        "PERL5OPT",
        "PERL5LIB",
        "BASH_ENV",
        "ENV",
        "ZDOTDIR",
        "PROMPT_COMMAND",
        "GIT_SSH",
        "GIT_SSH_COMMAND",
        "GIT_EXEC_PATH",
    ];
    let upper = name.to_uppercase();
    if upper.starts_with("WSP_") {
        return Some(format!("{name} is wsp's own, so an agent's setup does not set it"));
    }
    if PROCESS.contains(&upper.as_str()) || upper.starts_with("LD_") || upper.starts_with("DYLD_") {
        return Some(format!("{name} decides how {agent} starts or what it loads, so an agent's setup does not set it"));
    }
    None
}

/// `commandWords`: a line split as a shell would, or none where a quote is never closed.
pub fn command_words(line: &str) -> Option<Vec<String>> {
    let chars: Vec<char> = line.chars().collect();
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            let end = chars[i + 1..].iter().position(|x| *x == '\'')? + i + 1;
            word.get_or_insert_with(String::new).extend(&chars[i + 1..end]);
            i = end;
        } else if c == '"' {
            let w = word.get_or_insert_with(String::new);
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                w.push(chars[i]);
                i += 1;
            }
            if i >= chars.len() {
                return None;
            }
        } else if c == '\\' && i + 1 < chars.len() {
            i += 1;
            word.get_or_insert_with(String::new).push(chars[i]);
        } else if c.is_whitespace() {
            words.extend(word.take());
        } else {
            word.get_or_insert_with(String::new).push(c);
        }
        i += 1;
    }
    words.extend(word);
    Some(words)
}

/// `shellLine`: words as one line a shell splits back into them.
pub fn shell_line(words: &[String]) -> String {
    words
        .iter()
        .map(|word| {
            let plain = !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || "_-./:=@%+,".contains(c));
            if plain {
                word.clone()
            } else {
                format!("'{}'", word.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `refusedLines`: each `<reader>: <reason>` the report could not read.
pub fn refused_lines(refused: &[String]) -> Vec<(String, Option<String>)> {
    refused
        .iter()
        .map(|line| match line.find(": ") {
            Some(at) if at > 0 => (capitalised(&line[..at]), Some(line[at + 2..].to_owned())),
            _ => (line.clone(), None),
        })
        .collect()
}

/// `compactCount`: 3.6M, 201K, 12.
pub fn compact_count(n: u64) -> String {
    let short = |v: f64, unit: &str| {
        let rounded = (v * 10.).round() / 10.;
        if rounded.fract() == 0. {
            format!("{}{unit}", rounded as u64)
        } else {
            format!("{rounded:.1}{unit}")
        }
    };
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 if (n as f64 / 1e3 * 10.).round() < 10_000. => short(n as f64 / 1e3, "K"),
        1_000..=999_999_999 => short(n as f64 / 1e6, "M"),
        _ => short(n as f64 / 1e9, "B"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        serde_json::from_value(serde_json::json!({
            "harness": "claude", "label": "Claude Code", "source": "harness", "version": "2.1.0",
            "models": [{ "value": "opus", "label": "Opus", "isDefault": true, "efforts": ["low", "high"], "defaultEffort": "high" }, { "value": "sonnet", "label": "Sonnet" }],
            "efforts": [{ "value": "low", "label": "Low" }, { "value": "medium", "label": "Medium", "isDefault": true }, { "value": "high", "label": "High" }],
            "permissionModes": [{ "value": "default", "label": "Default", "isDefault": true }, { "value": "plan", "label": "Plan" }],
            "access": { "ask": "default", "plan": "plan" }
        }))
        .unwrap()
    }

    #[test]
    fn versions_order_as_semver_does() {
        assert_eq!(compare_versions("2.1.10", "2.1.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0-beta", "1.0.0"), Ordering::Less);
        assert_eq!(compare_versions("v1.2", "1.2.0"), Ordering::Equal);
    }

    #[test]
    fn a_new_thread_starts_on_the_marks_until_the_person_picks() {
        let c = catalog();
        let none = resolve(&c, &AgentDefaults::default());
        assert_eq!(none.model.unwrap().value, "opus");
        assert_eq!(none.effort.unwrap().value, "high");
        assert_eq!(none.access.unwrap().value, "ask");
        let own = AgentDefaults { model: Some("sonnet".into()), access: Some("full".into()), ..AgentDefaults::default() };
        let picked = resolve(&c, &own);
        assert_eq!(picked.model, Some(Picked { value: "sonnet".into(), from_default: true }));
        assert_eq!(picked.effort.unwrap().value, "medium");
        assert_eq!(picked.access.unwrap().value, "ask");
        assert_eq!(access_held(&c, "full").as_deref(), Some("Claude Code takes no full access; it takes ask, plan"));
        assert_eq!(access_held(&c, "plan"), None);
    }

    #[test]
    fn the_picker_hides_orders_and_adds() {
        let picker = Picker { hide: Some(vec!["opus".into()]), order: None, custom: Some(vec!["x-1".into()]) };
        let shaped = catalog().with_custom(Some(&picker)).shaped(Some(&picker));
        assert_eq!(shaped.models.iter().map(|m| m.value.as_str()).collect::<Vec<_>>(), ["sonnet", "x-1"]);
        assert_eq!(shaped.models[0].is_default, Some(true));
        assert_eq!(shaped.hidden_models.unwrap()[0].value, "opus");
    }

    #[test]
    fn words_read_as_the_web_writes_them() {
        assert_eq!(command_words(r#"npx -y "a b" 'c d' e\ f"#).unwrap(), ["npx", "-y", "a b", "c d", "e f"]);
        assert_eq!(command_words("a 'b"), None);
        assert_eq!(shell_line(&["--x".into(), "a b".into()]), "--x 'a b'");
        assert_eq!(compact_count(3_600_000), "3.6M");
        assert_eq!(compact_count(201_000), "201K");
        assert_eq!(compact_count(12), "12");
        assert_eq!(read_when(30_000), "just now");
        assert_eq!(read_when(3 * 3_600_000), "3 h ago");
        assert_eq!(model_id_refusal("a b").as_deref(), Some("No model id has a space in it"));
        assert_eq!(refused_lines(&["skills: no folder".into()])[0], ("Skills".into(), Some("no folder".into())));
    }

    #[test]
    fn the_head_names_the_plan_where_the_host_read_one() {
        let row = AgentRow { sign_in: "signed-in".into(), sign_in_kind: Some("subscription".into()), ..AgentRow::default() };
        let head = sign_in_head(&row, "box", Some(plan_word("max", Some("Claude"))));
        assert_eq!(head.line, "Signed in with Claude Max");
        assert_eq!(head.whole, "Signed in with Claude Max");
    }
}
