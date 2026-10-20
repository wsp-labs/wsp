// SPDX-License-Identifier: AGPL-3.0-only
//! The pure rules the Agents pages read the host's records by, over the types host-client generates from
//! `@wsp/protocol` (`agentsRows.ts`, `kinds/agents.ts`, `thread-defaults.ts`, `harness-picks.ts`), with their words.

use std::cmp::Ordering;

pub use wsp_host_client::protocol::AgentRow;
use wsp_host_client::protocol::{
    AgentRowSignIn, AgentRowSignInKind, BrokerStatus, BrokerStatusClaude, BrokerStatusCodex, HarnessCatalog, HarnessCatalogAccessKey,
    HarnessCatalogEffortsItem as Effort, HarnessCatalogHiddenModelsItem, HarnessCatalogLegacyModelsItem, HarnessCatalogModelsItem as Model,
    HarnessCatalogPermissionModesItem, HarnessCatalogSource, Preferences, PreferencesAgentDefaultsValue as AgentDefaults,
    PreferencesAgentDefaultsValueModels as Picker, ServerToolsAnswer, ServerToolsAnswerAuth, ServerToolsAnswerToolsItem,
};

use super::catalog::{AgentEntry, AGENTS};

/// One `HarnessModel` list's item as another list's: typify gives each list of the one shape a type of its own.
macro_rules! model_as {
    ($to:ident, $m:expr) => {{
        let m = $m;
        $to {
            added: m.added,
            context_windows: m.context_windows.clone(),
            default_effort: m.default_effort.clone(),
            description: m.description.clone(),
            efforts: m.efforts.clone(),
            fast: m.fast,
            is_default: m.is_default,
            label: m.label.clone(),
            short: m.short.clone(),
            value: m.value.clone(),
        }
    }};
}

/// The same for a `HarnessOption` list.
macro_rules! option_as {
    ($to:ident, $o:expr) => {{
        let o = $o;
        $to {
            description: o.description.clone(),
            is_default: o.is_default,
            label: o.label.clone(),
            short: o.short.clone(),
            value: o.value.clone(),
        }
    }};
}

/// The tools a server's answer lists, none where it listed none: only a server that connected lists them, and
/// typify reads the wire's absent list as an empty one.
pub fn tools_listed(answer: &ServerToolsAnswer) -> Option<&[ServerToolsAnswerToolsItem]> {
    (answer.auth == ServerToolsAnswerAuth::Connected || !answer.tools.is_empty()).then_some(answer.tools.as_slice())
}

/// The person's defaults for one agent (`AgentDefaults`), empty where none are kept.
pub fn own(prefs: Option<&Preferences>, agent: &str) -> AgentDefaults {
    prefs.and_then(|p| p.agent_defaults.get(agent)).cloned().unwrap_or_default()
}

pub fn entry(id: &str) -> Option<&'static AgentEntry> {
    AGENTS.iter().find(|a| a.id == id)
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

/// `compareVersions` (`packages/protocol/src/semver.mjs`), the one version order: the dotted core as numbers, a part the
/// shorter does not carry read as 0, then a release above its own prereleases; build metadata after a `+` counts for
/// nothing.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn parts(v: &str) -> (Vec<&str>, Option<Vec<&str>>) {
        let rest = v.split('+').next().unwrap_or_default();
        match rest.split_once('-') {
            Some((core, pre)) => (core.split('.').collect(), Some(pre.split('.').collect())),
            None => (rest.split('.').collect(), None),
        }
    }
    fn numeric(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
    }
    fn ident(a: &str, b: &str) -> Ordering {
        match (numeric(a), numeric(b)) {
            (true, true) => a.parse::<f64>().unwrap_or(0.).total_cmp(&b.parse::<f64>().unwrap_or(0.)),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => a.cmp(b),
        }
    }
    let ((left, left_pre), (right, right_pre)) = (parts(a), parts(b));
    for i in 0..left.len().max(right.len()) {
        let order = ident(left.get(i).unwrap_or(&"0"), right.get(i).unwrap_or(&"0"));
        if order != Ordering::Equal {
            return order;
        }
    }
    match (left_pre, right_pre) {
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
    use wsp_host_client::protocol::{AgentRowSignInKind, PluginRowScope};

    pub const TABS: [(&str, &str); 4] = [("agents", "Agents"), ("servers", "Tool servers"), ("skills", "Skills"), ("plugins", "Plugins")];
    pub const NEW_THREADS: &str = "New threads";
    pub const DEFAULT_AGENT: &str = "Default agent";
    pub const DEFAULT_AGENT_DESCRIPTION: &str = "A project can set its own.";
    pub const AGENT_HEAD_LINE: &str = "Version, sign-in, update and whether it is on";
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
    pub fn signed_in_as(kind: AgentRowSignInKind) -> &'static str {
        match kind {
            AgentRowSignInKind::ApiKey => "Signed in with an API key",
            AgentRowSignInKind::Subscription => "Signed in with a subscription",
            AgentRowSignInKind::Oauth => "Signed in with OAuth",
            AgentRowSignInKind::Unknown => "Signed in",
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
    pub fn reorder(model: &str) -> String {
        format!("Drag to reorder {model}, or use the arrow keys")
    }

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
    pub const CODE_ASK: &str = "Paste the code from the page";
    pub const CODE_SUBMIT: &str = "Submit";
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
    pub fn scope(scope: PluginRowScope) -> &'static str {
        match scope {
            PluginRowScope::User | PluginRowScope::Unknown => "User",
            PluginRowScope::Project => "Project",
            PluginRowScope::Local => "Local",
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
    matches!(row.sign_in, AgentRowSignIn::SignedIn | AgentRowSignIn::VaultKey)
}

/// `signInWord`.
pub fn sign_in_word(row: &AgentRow) -> &'static str {
    match row.sign_in {
        AgentRowSignIn::SignedIn => w::SIGNED_IN,
        AgentRowSignIn::VaultKey => w::WITH_TOKEN,
        AgentRowSignIn::None => w::NEEDS_SIGN_IN,
        AgentRowSignIn::Unknown => w::NOT_CHECKED,
    }
}

/// `signInStatus`.
pub fn sign_in_status(row: &AgentRow) -> Status {
    let tone = if signed_in(row) {
        Tone::Good
    } else if row.sign_in == AgentRowSignIn::None {
        Tone::Waiting
    } else {
        Tone::Quiet
    };
    Status::new(&row.sign_in.to_string(), tone, sign_in_word(row))
}

/// `brokerRow`: what the broker says for an agent's row, `held` none where it holds nothing for it.
pub struct BrokerSays {
    pub held: Option<&'static str>,
    pub other_roads: Option<&'static str>,
}

pub fn broker_row(id: &str, broker: Option<&BrokerStatus>) -> Option<BrokerSays> {
    let broker = broker?;
    match id {
        "claude" => Some(BrokerSays {
            held: broker.claude.map(|k| match k {
                BrokerStatusClaude::Login => "signed in with a subscription",
                BrokerStatusClaude::Token => w::WITH_TOKEN,
                BrokerStatusClaude::Key => "signed in with an API key",
                BrokerStatusClaude::Gateway => "signed in through your gateway",
                BrokerStatusClaude::Unknown => "signed in",
            }),
            other_roads: Some("a token: wsp agents key claude, a key: add --api-key"),
        }),
        "codex" => Some(BrokerSays {
            held: broker.codex.map(|k| match k {
                BrokerStatusCodex::Chatgpt | BrokerStatusCodex::AccessToken => "signed in with ChatGPT",
                BrokerStatusCodex::Key => "signed in with an API key",
                BrokerStatusCodex::Unknown => "signed in",
            }),
            other_roads: None,
        }),
        _ => None,
    }
}

/// `rowStatus`: the broker's word where it speaks for the row, else the row's own.
pub fn row_status(row: &AgentRow, broker: Option<&BrokerStatus>) -> Status {
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
pub fn version_line(row: &AgentRow, broker: Option<&BrokerStatus>) -> Option<String> {
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
    let kind = row.sign_in_kind;
    let planned =
        plan.is_some() && (kind == Some(AgentRowSignInKind::Subscription) || (kind.is_none() && row.sign_in == AgentRowSignIn::SignedIn));
    let line = match (&plan, kind) {
        (Some(plan), _) if planned => format!("{}{plan}", w::SIGNED_IN_WITH),
        (_, Some(kind)) => w::signed_in_as(kind).to_owned(),
        _ => capitalised(sign_in_word(row)),
    };
    let whole = match &row.sign_in_detail {
        None => line.clone(),
        Some(detail) => {
            let an = if kind == Some(AgentRowSignInKind::ApiKey) { "an " } else { "" };
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

/// A model no list names, named by itself.
fn bare_model(value: &str, added: Option<bool>) -> Model {
    Model {
        added,
        context_windows: Vec::new(),
        default_effort: None,
        description: None,
        efforts: Vec::new(),
        fast: None,
        is_default: None,
        label: value.to_owned(),
        short: None,
        value: value.to_owned(),
    }
}

/// The models a picker lists: the current ones, then the legacy ones.
pub fn listed_models(catalog: &HarnessCatalog) -> Vec<Model> {
    catalog.models.iter().cloned().chain(catalog.legacy_models.iter().map(|m| model_as!(Model, m))).collect()
}

/// `everyModel`: the listed ones, then the ones the person hid.
pub fn every_model(catalog: &HarnessCatalog) -> Vec<Model> {
    let mut all = listed_models(catalog);
    all.extend(catalog.hidden_models.iter().map(|m| model_as!(Model, m)));
    all
}

/// A model's slug without the context window Claude Code writes after it, as in `claude-opus-5[1m]`.
pub fn base_model(model: &str) -> &str {
    model.strip_suffix("[1m]").unwrap_or(model)
}

/// `modelOf`: a slug the catalog does not list still counts, named by itself.
pub fn model_of(catalog: &HarnessCatalog, value: &str) -> Model {
    every_model(catalog).into_iter().find(|m| m.value == value).unwrap_or_else(|| bare_model(value, None))
}

/// `effortsFor`. A model whose wire list is empty reads here as one with no list of its own, as typify reads both alike.
pub fn efforts_for(catalog: &HarnessCatalog, model: Option<&Model>) -> Vec<Effort> {
    let options: Vec<Effort> = match model.map(|m| &m.efforts).filter(|e| !e.is_empty()) {
        Some(subset) => catalog.efforts.iter().filter(|o| subset.contains(&o.value)).cloned().collect(),
        None => catalog.efforts.clone(),
    };
    match model.and_then(|m| m.default_effort.as_ref()) {
        None => options,
        Some(own) => options.into_iter().map(|o| Effort { is_default: (o.value == *own).then_some(true), ..o }).collect(),
    }
}

/// `accessMode`.
pub fn access_mode<'a>(catalog: &'a HarnessCatalog, word: &str) -> Option<&'a str> {
    let mode = catalog.access.get(&word.parse::<HarnessCatalogAccessKey>().ok()?)?;
    catalog.permission_modes.iter().any(|o| o.value == *mode).then_some(mode.as_str())
}

/// `unmarked`: the lists as the agent gives them, before the picker shaped them.
pub fn unmarked(catalog: &HarnessCatalog) -> HarnessCatalog {
    match &catalog.unshaped {
        None => catalog.clone(),
        Some(u) => HarnessCatalog {
            models: u.models.iter().map(|m| model_as!(Model, m)).collect(),
            legacy_models: u.legacy_models.iter().map(|m| model_as!(HarnessCatalogLegacyModelsItem, m)).collect(),
            efforts: u.efforts.iter().map(|o| option_as!(Effort, o)).collect(),
            permission_modes: u.permission_modes.iter().map(|o| option_as!(HarnessCatalogPermissionModesItem, o)).collect(),
            hidden_models: Vec::new(),
            unshaped: None,
            ..catalog.clone()
        },
    }
}

/// `withCustomModels`.
pub fn with_custom(catalog: &HarnessCatalog, picker: Option<&Picker>) -> HarnessCatalog {
    let known: Vec<String> = every_model(catalog).into_iter().map(|m| m.value).collect();
    let added = picker.into_iter().flat_map(|p| &p.custom).filter(|id| !known.contains(id)).map(|id| bare_model(id, Some(true)));
    let mut next = catalog.clone();
    next.models.extend(added);
    next
}

/// `shapeModels`: hidden models apart, the person's order first.
pub fn shaped(catalog: &HarnessCatalog, picker: Option<&Picker>) -> HarnessCatalog {
    let Some(picker) = picker else { return catalog.clone() };
    let (hide, order) = (&picker.hide, &picker.order);
    let rank = |value: &str| order.iter().position(|v| v == value).unwrap_or(order.len());
    let mut models: Vec<Model> = catalog.models.iter().filter(|m| !hide.contains(&m.value)).cloned().collect();
    models.sort_by_key(|m| rank(&m.value));
    let mut legacy: Vec<HarnessCatalogLegacyModelsItem> =
        catalog.legacy_models.iter().filter(|m| !hide.contains(&m.value)).cloned().collect();
    legacy.sort_by_key(|m| rank(&m.value));
    let hidden: Vec<HarnessCatalogHiddenModelsItem> = listed_models(catalog)
        .iter()
        .filter(|m| hide.contains(&m.value))
        .map(|m| HarnessCatalogHiddenModelsItem { is_default: None, ..model_as!(HarnessCatalogHiddenModelsItem, m) })
        .collect();
    let every = every_model(catalog);
    let had = marked_default(every.iter().map(|m| (m.value.as_str(), m.is_default))).is_some();
    let has = marked_default(
        models.iter().map(|m| (m.value.as_str(), m.is_default)).chain(legacy.iter().map(|m| (m.value.as_str(), m.is_default))),
    )
    .is_some();
    if had && !has && !models.is_empty() {
        let first = models[0].value.clone();
        models = models.into_iter().map(|m| Model { is_default: (m.value == first).then_some(true), ..m }).collect();
    }
    HarnessCatalog { models, legacy_models: legacy, hidden_models: hidden, ..catalog.clone() }
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
pub fn resolve(catalog: &HarnessCatalog, own: &AgentDefaults) -> Defaults {
    fn pick(options: &[(String, Option<bool>)], open: bool, own: Option<&String>) -> Option<Picked> {
        if let Some(value) = own {
            if open || options.iter().any(|(v, _)| v == value) {
                return Some(Picked { value: value.clone(), from_default: true });
            }
        }
        options.iter().find(|(_, d)| *d == Some(true)).map(|(v, _)| Picked { value: v.clone(), from_default: false })
    }
    let models: Vec<(String, Option<bool>)> = every_model(catalog).into_iter().map(|m| (m.value, m.is_default)).collect();
    let model = pick(&models, models.is_empty(), own.model.as_ref());
    let picked = model.as_ref().map(|m| model_of(catalog, &m.value));
    let efforts: Vec<(String, Option<bool>)> = efforts_for(catalog, picked.as_ref()).into_iter().map(|o| (o.value, o.is_default)).collect();
    let effort = pick(&efforts, catalog.efforts.is_empty(), own.effort.as_ref());
    let word = |value: Option<&str>, from_default: bool| {
        let value = value?;
        access_mode(catalog, value).map(|_| Picked { value: value.to_owned(), from_default })
    };
    let marked = catalog.permission_modes.iter().find(|o| o.is_default == Some(true)).map(|o| o.value.clone());
    let marked_word = marked.and_then(|mode| {
        ACCESS.iter().find(|word| word.parse().ok().and_then(|key| catalog.access.get(&key)) == Some(&mode)).map(|w| w.to_string())
    });
    let access = word(own.access.map(|a| a.to_string()).as_deref(), true).or_else(|| word(marked_word.as_deref(), false));
    Defaults { model, effort, access }
}

/// The first `d.d.d` in a version line, as `/(\d+\.\d+\.\d+)/` finds it.
fn version_in(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let digits = |from: usize| bytes[from..].iter().take_while(|b| b.is_ascii_digit()).count();
    (0..bytes.len()).find_map(|start| {
        let mut at = start;
        for part in 0..3 {
            let run = digits(at);
            if run == 0 {
                return None;
            }
            at += run;
            if part < 2 {
                if bytes.get(at) != Some(&b'.') {
                    return None;
                }
                at += 1;
            }
        }
        Some(&line[start..at])
    })
}

/// `olderThanTable`: whether the binary that answered these lists is older than the release the table was read off;
/// none where the lists are not a binary's own or either version does not read.
pub fn older_than_table(lists: &HarnessCatalog, table: Option<&HarnessCatalog>) -> Option<bool> {
    let pin = version_in(table?.version.as_deref()?)?;
    let version = lists.version.as_deref().filter(|_| lists.source == HarnessCatalogSource::Harness)?;
    Some(compare_versions(version, pin) == Ordering::Less)
}

/// `defaultNotRunLine`: the person's default model for new threads that the agent on the picked computer does not run,
/// with the model that runs instead; none where it runs there, or where the lists are not that computer's own.
pub fn default_not_run_line(lists: &HarnessCatalog, table: Option<&HarnessCatalog>, prefs: Option<&Preferences>) -> Option<String> {
    if lists.source != HarnessCatalogSource::Harness {
        return None;
    }
    let table = table?;
    let own = own(prefs, &lists.harness);
    let wanted = own.model.as_deref()?;
    if every_model(lists).iter().any(|m| m.value == wanted) {
        return None;
    }
    let known = every_model(table).into_iter().find(|m| m.value == wanted)?;
    let runs = model_of(lists, &resolve(lists, &own).model?.value).label;
    Some(if older_than_table(lists, Some(table)) == Some(true) {
        format!("{} needs a newer {} here; running {runs}", known.label, lists.label)
    } else {
        format!("{} here does not run {}; running {runs}", lists.label, known.label)
    })
}

/// `ACCESS_CHOICES`.
pub const ACCESS: [&str; 4] = ["ask", "auto-edit", "full", "plan"];

/// `newThreadDefaults`: what a new thread on an agent starts with where its project names nothing.
pub fn new_thread_defaults(catalog: &HarnessCatalog, prefs: Option<&Preferences>) -> Defaults {
    let own = own(prefs, &catalog.harness);
    resolve(&with_custom(&unmarked(catalog), own.models.as_ref()), &own)
}

/// `accessRefusal`: why a word maps to none of the agent's modes.
pub fn access_held(catalog: &HarnessCatalog, word: &str) -> Option<String> {
    if access_mode(catalog, word).is_some() {
        return None;
    }
    let takes: Vec<&str> = ACCESS.iter().copied().filter(|w| access_mode(catalog, w).is_some()).collect();
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

/// `PROCESS_ENV`: the variables that decide how a process starts or what it loads.
pub const PROCESS_ENV: [&str; 23] = [
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

/// `PROCESS_ENV_PREFIXES`.
pub const PROCESS_ENV_PREFIXES: [&str; 2] = ["LD_", "DYLD_"];

/// `agentEnvRefusal`, without the adapter's dropped patterns.
pub fn env_refusal(name: &str, agent: &str) -> Option<String> {
    let upper = name.to_uppercase();
    if upper.starts_with("WSP_") {
        return Some(format!("{name} is wsp's own, so an agent's setup does not set it"));
    }
    if PROCESS_ENV.contains(&upper.as_str()) || PROCESS_ENV_PREFIXES.iter().any(|p| upper.starts_with(p)) {
        return Some(format!("{name} decides how {agent} starts or what it loads, so an agent's setup does not set it"));
    }
    None
}

/// `agentOfRow`: the agent an agents-rung recipe row names, none for a tool server's row and every other rung.
pub fn agent_of_row(id: &str) -> Option<&str> {
    id.strip_prefix("agents/").filter(|_| !id.starts_with("agents/mcp/"))
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

/// Host records for a test, each from the fields it names over the ones its wire shape requires.
#[cfg(test)]
pub mod records {
    use serde_json::{json, Value};
    use wsp_host_client::protocol::{AgentsReport, HarnessCatalog, Preferences};

    use super::AgentRow;

    fn over(mut base: Value, given: Value) -> Value {
        if let (Some(base), Value::Object(given)) = (base.as_object_mut(), given) {
            base.extend(given);
        }
        base
    }

    pub fn catalog(given: Value) -> HarnessCatalog {
        let base = json!({
            "harness": "claude", "label": "Claude Code", "source": "table", "version": null, "models": [], "efforts": [],
            "contextWindows": [], "permissionModes": [], "steers": false, "renames": false, "images": false
        });
        serde_json::from_value(over(base, given)).expect("a catalog")
    }

    pub fn agent(given: Value) -> AgentRow {
        let base = json!({ "id": "claude", "name": "Claude Code", "installed": true, "road": "wsp", "signIn": "unknown", "signInRoad": "none", "wspTools": false });
        serde_json::from_value(over(base, given)).expect("an agent row")
    }

    pub fn prefs(given: Value) -> Preferences {
        let base = json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
            "projectLook": {}, "projectOrder": [], "labs": false });
        serde_json::from_value(over(base, given)).expect("preferences")
    }

    pub fn report(given: Value) -> AgentsReport {
        let base = json!({
            "target": { "placeId": "here" }, "home": "/home/me", "user": "me", "readAt": "2026-10-18T00:00:00.000Z", "agents": [],
            "skills": [], "servers": [], "refused": []
        });
        serde_json::from_value(over(base, given)).expect("a report")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::records::{agent, catalog as catalog_of, prefs};
    use super::*;

    fn catalog() -> HarnessCatalog {
        catalog_of(json!({
            "harness": "claude", "label": "Claude Code", "source": "harness", "version": "2.1.0",
            "models": [{ "value": "opus", "label": "Opus", "isDefault": true, "efforts": ["low", "high"], "defaultEffort": "high" }, { "value": "sonnet", "label": "Sonnet" }],
            "efforts": [{ "value": "low", "label": "Low" }, { "value": "medium", "label": "Medium", "isDefault": true }, { "value": "high", "label": "High" }],
            "permissionModes": [{ "value": "default", "label": "Default", "isDefault": true }, { "value": "plan", "label": "Plan" }],
            "access": { "ask": "default", "plan": "plan" }
        }))
    }

    #[test]
    fn versions_order_as_semver_mjs_orders_them() {
        assert_eq!(compare_versions("2.1.10", "2.1.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0-beta", "1.0.0"), Ordering::Less);
        assert_eq!(compare_versions("0.2", "0.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "1.0.0-rc.2"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0-rc.10", "1.0.0-rc.2"), Ordering::Greater);
        assert_eq!(compare_versions("1.0.0-rc", "1.0.0-rc.1"), Ordering::Less);
        assert_eq!(compare_versions("0.3.7+abc", "0.3.7"), Ordering::Equal);
        assert_eq!(compare_versions("1.02", "1.2"), Ordering::Equal, "numbers as numbers");
        assert_eq!(compare_versions("1.0.0-rc.99999999999999999999", "1.0.0-rc.9"), Ordering::Greater, "past u64, still a number");
        assert_eq!(compare_versions("v1.2", "1.2.0"), Ordering::Greater, "semver.mjs reads no leading v: the word v1 is above 1");
        assert_eq!(compare_versions(" 1.2", "1.2"), Ordering::Greater, "nor trims");
    }

    #[test]
    fn a_new_thread_starts_on_the_marks_until_the_person_picks() {
        let c = catalog();
        let none = resolve(&c, &AgentDefaults::default());
        assert_eq!(none.model.unwrap().value, "opus");
        assert_eq!(none.effort.unwrap().value, "high");
        assert_eq!(none.access.unwrap().value, "ask");
        let own: AgentDefaults = serde_json::from_value(json!({ "model": "sonnet", "access": "full" })).unwrap();
        let picked = resolve(&c, &own);
        assert_eq!(picked.model, Some(Picked { value: "sonnet".into(), from_default: true }));
        assert_eq!(picked.effort.unwrap().value, "medium");
        assert_eq!(picked.access.unwrap().value, "ask");
        assert_eq!(access_held(&c, "full").as_deref(), Some("Claude Code takes no full access; it takes ask, plan"));
        assert_eq!(access_held(&c, "plan"), None);
    }

    #[test]
    fn a_default_the_computer_does_not_run_is_said_with_what_runs() {
        let table = catalog_of(json!({
            "harness": "opencode", "label": "OpenCode", "source": "table", "version": "opencode 1.18.18 (read 2026-10-01)",
            "models": [{ "value": "big", "label": "Big" }, { "value": "small", "label": "Small", "isDefault": true }]
        }));
        let lists = |version: &str, source: &str| {
            catalog_of(json!({
                "harness": "opencode", "label": "OpenCode", "source": source, "version": version,
                "models": [{ "value": "small", "label": "Small", "isDefault": true }]
            }))
        };
        let wanting = |model: &str| prefs(json!({ "agentDefaults": { "opencode": { "model": model } } }));
        let older = lists("1.2.0", "harness");
        assert_eq!(older_than_table(&older, Some(&table)), Some(true));
        assert_eq!(
            default_not_run_line(&older, Some(&table), Some(&wanting("big"))).as_deref(),
            Some("Big needs a newer OpenCode here; running Small")
        );
        let newer = lists("1.20.0", "harness");
        assert_eq!(older_than_table(&newer, Some(&table)), Some(false));
        assert_eq!(
            default_not_run_line(&newer, Some(&table), Some(&wanting("big"))).as_deref(),
            Some("OpenCode here does not run Big; running Small")
        );
        assert_eq!(default_not_run_line(&newer, Some(&table), Some(&wanting("small"))), None, "a default it runs");
        assert_eq!(default_not_run_line(&newer, Some(&table), Some(&wanting("unknown"))), None, "a default the table does not know");
        assert_eq!(default_not_run_line(&newer, Some(&table), None), None, "no default");
        assert_eq!(
            default_not_run_line(&lists("1.2.0", "table"), Some(&table), Some(&wanting("big"))),
            None,
            "lists not the computer's own"
        );
        assert_eq!(default_not_run_line(&older, None, Some(&wanting("big"))), None, "no table");
        assert_eq!(version_in("v0.155.1-beta"), Some("0.155.1"));
        assert_eq!(version_in("1.2 and 3.4.5"), Some("3.4.5"));
        assert_eq!(version_in("1.2"), None);
    }

    #[test]
    fn the_picker_hides_orders_and_adds() {
        let picker: Picker = serde_json::from_value(json!({ "hide": ["opus"], "custom": ["x-1"] })).unwrap();
        let shaped = shaped(&with_custom(&catalog(), Some(&picker)), Some(&picker));
        assert_eq!(shaped.models.iter().map(|m| m.value.as_str()).collect::<Vec<_>>(), ["sonnet", "x-1"]);
        assert_eq!(shaped.models[0].is_default, Some(true));
        assert_eq!(shaped.hidden_models[0].value, "opus");
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
        let row = agent(json!({ "signIn": "signed-in", "signInKind": "subscription" }));
        let head = sign_in_head(&row, "box", Some(plan_word("max", Some("Claude"))));
        assert_eq!(head.line, "Signed in with Claude Max");
        assert_eq!(head.whole, "Signed in with Claude Max");
    }

    #[test]
    fn a_server_lists_its_tools_only_once_it_connected() {
        let answer = |v: serde_json::Value| -> ServerToolsAnswer { serde_json::from_value(v).unwrap() };
        assert!(tools_listed(&answer(json!({ "auth": "needs-sign-in", "readAt": "x" }))).is_none());
        assert_eq!(tools_listed(&answer(json!({ "auth": "connected", "tools": [], "readAt": "x" }))).map(<[_]>::len), Some(0));
        assert_eq!(
            tools_listed(&answer(json!({ "auth": "connected", "tools": [{ "name": "a" }], "readAt": "x" }))).map(<[_]>::len),
            Some(1)
        );
    }
}
