// SPDX-License-Identifier: AGPL-3.0-only
//! How each agent is set up to run on one computer, and the refusals a start, a setup and a config folder meet: the
//! setup a change leaves, the variables a setup may not name, the config folder rules, and the model and access word a
//! catalog does not take. Port of the functions of packages/protocol/src/thread-defaults.ts at the cut (383708c78) that
//! the host's command line and runtime read; its shapes and constants are wsp_wire::types::thread_defaults. See
//! spec/thread_defaults.md.

use indexmap::IndexMap;
use regex::Regex;
use wsp_wire::types::harness_picks::{efforts_for, every_model, marked_default, model_of};
use wsp_wire::types::thread_defaults::{
    AccessChoice, AgentSetup, AgentSetupSet, DefaultFrom, ModelPicker, ProjectOverrides, ResolvedAccess, ResolvedFolder, ResolvedPick,
    ThreadDefaults,
};
use wsp_wire::views::harness::{HarnessCatalog, HarnessCatalogSource, HarnessModel, HarnessOption};
use wsp_wire::views::preferences::Preferences;

use crate::exit::{refusal, usage_refusal, HostError, Kind};
use crate::semver::compare_versions;

// thread-defaults.ts:101
// export function setupPatched(kept: AgentSetup | undefined, set: AgentSetupSet): AgentSetup | undefined {
//   const env = { ...kept?.env };
//   for (const [name, value] of Object.entries(set.env ?? {})) {
//     if (value === null) delete env[name];
//     else env[name] = value;
//   }
//   const on = set.on === undefined ? kept?.on : set.on ? undefined : false;
//   const pick = <K extends "program" | "configDir" | "args">(key: K): AgentSetup[K] => (set[key] === undefined ? kept?.[key] : (set[key] ?? undefined)) as AgentSetup[K];
//   const next: AgentSetup = {
//     ...(on === false ? { on } : {}),
//     ...(pick("program") !== undefined ? { program: pick("program")! } : {}),
//     ...(pick("configDir") !== undefined ? { configDir: pick("configDir")! } : {}),
//     ...(pick("args") !== undefined ? { args: [...pick("args")!] } : {}),
//     ...(Object.keys(env).length > 0 ? { env } : {}),
//   };
//   return Object.keys(next).length === 0 ? undefined : next;
// }
/// A stored setup with a change over it; none where every field is back on the agent's own. A variable the change
/// names null is taken away and one it gives a value is set (a new one goes after the ones kept).
/// A field the change leaves out keeps what stands, and one it names null goes back on the agent's own.
fn pick<T: Clone>(set: &Option<Option<T>>, kept: Option<T>) -> Option<T> {
    match set {
        None => kept,
        Some(value) => value.clone(),
    }
}

pub fn setup_patched(kept: Option<&AgentSetup>, set: &AgentSetupSet) -> Option<AgentSetup> {
    let mut env: IndexMap<String, String> = kept.and_then(|kept| kept.env.clone()).unwrap_or_default();
    for (name, value) in set.env.iter().flatten() {
        match value {
            None => {
                env.shift_remove(&name.0);
            }
            Some(value) => {
                env.insert(name.0.clone(), value.clone());
            }
        }
    }
    let on = match set.on {
        None => kept.and_then(|kept| kept.on),
        Some(true) => None,
        Some(false) => Some(Default::default()),
    };
    let next = AgentSetup {
        on,
        program: pick(&set.program, kept.and_then(|kept| kept.program.clone())),
        config_dir: pick(&set.config_dir, kept.and_then(|kept| kept.config_dir.clone())),
        args: pick(&set.args, kept.and_then(|kept| kept.args.clone())),
        env: (!env.is_empty()).then_some(env),
    };
    (next != AgentSetup::default()).then_some(next)
}

// thread-defaults.ts:379
// export const noConfigDirLine = (agent: string): string => `${agent} takes no config folder of its own, so there is nothing to point at one`;
/// Why a config folder was refused for an agent wsp has no variable to point at one.
pub fn no_config_dir_line(agent: &str) -> String {
    format!("{agent} takes no config folder of its own, so there is nothing to point at one")
}

// thread-defaults.ts:375
// export const configDirIsHomeLine = (agent: string): string => `${agent}'s config folder cannot be the home folder itself; name a folder under it`;
fn config_dir_is_home_line(agent: &str) -> String {
    format!("{agent}'s config folder cannot be the home folder itself; name a folder under it")
}

// thread-defaults.ts:385
// export const droppedEnvLine = (agent: string, name: string): string => `${name} never reaches ${agent}: its launch drops every variable of that kind`;
/// Why a variable was refused: the agent's own launch drops it, so it would never reach the agent.
pub fn dropped_env_line(agent: &str, name: &str) -> String {
    format!("{name} never reaches {agent}: its launch drops every variable of that kind")
}

// thread-defaults.ts:395
// export const processEnvLine = (name: string, agent: string): string => `${name} decides how ${agent} starts or what it loads, so an agent's setup does not set it`;
/// Why a variable was refused: it decides how the process starts or what it loads.
pub fn process_env_line(name: &str, agent: &str) -> String {
    format!("{name} decides how {agent} starts or what it loads, so an agent's setup does not set it")
}

// thread-defaults.ts:396
// export const wspEnvLine = (name: string): string => `${name} is wsp's own, so an agent's setup does not set it`;
/// Why a variable was refused: it is one of wsp's own.
pub fn wsp_env_line(name: &str) -> String {
    format!("{name} is wsp's own, so an agent's setup does not set it")
}

// thread-defaults.ts:391
// const PROCESS_ENV = new Set(["PATH", "HOME", "SHELL", "USER", "LOGNAME", "IFS", "TMPDIR", "NODE_OPTIONS", "NODE_PATH", "PYTHONPATH", "PYTHONSTARTUP", "PYTHONHOME", "RUBYOPT", "RUBYLIB", "PERL5OPT", "PERL5LIB", "BASH_ENV", "ENV", "ZDOTDIR", "PROMPT_COMMAND", "GIT_SSH", "GIT_SSH_COMMAND", "GIT_EXEC_PATH"]);
// const PROCESS_ENV_PREFIXES = ["LD_", "DYLD_"];
const PROCESS_ENV: [&str; 23] = [
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
const PROCESS_ENV_PREFIXES: [&str; 2] = ["LD_", "DYLD_"];

// thread-defaults.ts:402
// export function agentEnvRefusal(name: string, agent: string, dropped: readonly RegExp[] = []): string | null {
//   const upper = name.toUpperCase();
//   if (upper.startsWith("WSP_")) return wspEnvLine(name);
//   if (PROCESS_ENV.has(upper) || PROCESS_ENV_PREFIXES.some(prefix => upper.startsWith(prefix))) return processEnvLine(name, agent);
//   return dropped.some(pattern => pattern.test(name)) ? droppedEnvLine(agent, name) : null;
// }
/// Why a variable cannot be part of an agent's setup, or none where it may: one that decides how the process starts or
/// what it loads, any of wsp's own, and one the agent's own launch drops (`dropped`, its adapter's patterns).
pub fn agent_env_refusal(name: &str, agent: &str, dropped: &[Regex]) -> Option<String> {
    let upper = name.to_uppercase();
    if upper.starts_with("WSP_") {
        return Some(wsp_env_line(name));
    }
    if PROCESS_ENV.contains(&upper.as_str()) || PROCESS_ENV_PREFIXES.iter().any(|prefix| upper.starts_with(prefix)) {
        return Some(process_env_line(name, agent));
    }
    dropped.iter().any(|pattern| pattern.is_match(name)).then(|| dropped_env_line(agent, name))
}

// thread-defaults.ts:415
// const within = (path: string, folder: string): boolean => path === folder || path.startsWith(`${folder.replace(/\/+$/, "")}/`);
fn within(path: &str, folder: &str) -> bool {
    path == folder || path.starts_with(&format!("{}/", folder.trim_end_matches('/')))
}

// thread-defaults.ts:421
// export function configDirRefusal(agent: string, given: string, real: ResolvedFolder): string | null {
//   const home = real.home.replace(/\/+$/, "");
//   const folder = real.folder.replace(/\/+$/, "");
//   if (folder === home) return configDirIsHomeLine(agent);
//   if (!within(folder, home)) return `${agent}'s config folder has to be under the home folder ${home}, and ${given} is not`;
//   if (real.kept.some(kept => within(folder, kept.replace(/\/+$/, "")))) return `${given} is inside wsp's own folder, so it is no agent's config folder`;
//   return null;
// }
/// Why a config folder cannot stand: it is the home itself, outside the home once its links are followed, or inside one
/// of wsp's own folders. `given` is the path as the person named it, which the sentence names back.
pub fn config_dir_refusal(agent: &str, given: &str, real: &ResolvedFolder) -> Option<String> {
    let home = real.home.trim_end_matches('/');
    let folder = real.folder.trim_end_matches('/');
    if folder == home {
        return Some(config_dir_is_home_line(agent));
    }
    if !within(folder, home) {
        return Some(format!("{agent}'s config folder has to be under the home folder {home}, and {given} is not"));
    }
    if real.kept.iter().any(|kept| within(folder, kept.trim_end_matches('/'))) {
        return Some(format!("{given} is inside wsp's own folder, so it is no agent's config folder"));
    }
    None
}

// thread-defaults.ts:436
// export const configDirSignInLine = (agent: string): string => `${agent} keeps its sign-in under its config folder, so sign it in again there before its next thread`;
/// What a config folder change asks of the person: a login kept under the old folder does not follow it.
pub fn config_dir_sign_in_line(agent: &str) -> String {
    format!("{agent} keeps its sign-in under its config folder, so sign it in again there before its next thread")
}

/// What `startPicks` throws (the TS `e`): its sentence, the kind stamped on it, and where it listed options how many it
/// offered (`wire/helpers.ts:49` stamps `offered`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickError {
    pub message: String,
    pub kind: Option<String>,
    pub offered: Option<usize>,
}

impl From<wsp_wire::wire::helpers::ListedRefusal> for PickError {
    fn from(e: wsp_wire::wire::helpers::ListedRefusal) -> PickError {
        PickError { message: e.message, kind: Some(e.kind.to_string()), offered: Some(e.offered) }
    }
}

// thread-defaults.ts:313
// export function pickRefusal(e: unknown, catalog: Pick<HarnessCatalog, "source"> | undefined): Error {
//   const said = e instanceof Error ? e.message : String(e);
//   const clause = catalog?.source !== "table" ? "" : (e as { offered?: number }).offered === 0 ? BUILT_IN_TABLE_CLAUSE : BUILT_IN_LIST_CLAUSE;
//   return refusal(`${said}${clause}`, "Drop the flag, or give it a value the agent offers.", (e as { kind?: string }).kind);
// }
/// A model, effort or fast `startPicks` refused against a catalog, said with whose list it quotes and what to do.
pub fn pick_refusal(e: &PickError, catalog: Option<&HarnessCatalog>) -> HostError {
    let clause = match catalog.map(|catalog| catalog.source) {
        Some(HarnessCatalogSource::Table) => match e.offered {
            Some(0) => wsp_wire::types::thread_defaults::BUILT_IN_TABLE_CLAUSE,
            _ => wsp_wire::types::thread_defaults::BUILT_IN_LIST_CLAUSE,
        },
        _ => "",
    };
    refusal(&format!("{}{clause}", e.message), "Drop the flag, or give it a value the agent offers.", e.kind.as_deref().map(Kind::parse))
}

// thread-defaults.ts:150
// export function accessMode(catalog: Pick<HarnessCatalog, "access" | "permissionModes">, word: AccessChoice): string | undefined {
//   const mode = catalog.access?.[word];
//   return mode !== undefined && catalog.permissionModes.some(o => o.value === mode) ? mode : undefined;
// }
/// The harness's own mode a wsp word stands for, where its row maps the word to a mode it lists.
pub fn access_mode(catalog: &HarnessCatalog, word: AccessChoice) -> Option<&str> {
    let mode = catalog.access.as_ref()?.get(&word)?;
    catalog.permission_modes.iter().any(|option| option.value == *mode).then_some(mode.as_str())
}

const ACCESS_WORDS: [(AccessChoice, &str); 4] =
    [(AccessChoice::Ask, "ask"), (AccessChoice::AutoEdit, "auto-edit"), (AccessChoice::Full, "full"), (AccessChoice::Plan, "plan")];

fn access_word_text(word: AccessChoice) -> &'static str {
    ACCESS_WORDS.iter().find(|(choice, _)| *choice == word).map_or("", |(_, text)| text)
}

// thread-defaults.ts:294
// export const accessNotTakenLine = (label: string, word: string, takes: string): string => `${label} takes no ${word} access; it takes ${takes}`;
/// Why a word was refused: the agent maps it to none of its own modes, and these are the words it takes.
pub fn access_not_taken_line(label: &str, word: &str, takes: &str) -> String {
    format!("{label} takes no {word} access; it takes {takes}")
}

// thread-defaults.ts:296
// export const noAccessWordsLine = (label: string): string => `${label} takes no access of wsp's words`;
/// Why a word was refused by an agent that maps none of wsp's words.
pub fn no_access_words_line(label: &str) -> String {
    format!("{label} takes no access of wsp's words")
}

// thread-defaults.ts:290
// export function accessRefusal(catalog: Pick<HarnessCatalog, "label" | "access" | "permissionModes">, word: AccessChoice): string | null {
//   if (accessMode(catalog, word) !== undefined) return null;
//   const takes = ACCESS_CHOICES.filter(w => accessMode(catalog, w) !== undefined);
//   return takes.length === 0 ? noAccessWordsLine(catalog.label) : accessNotTakenLine(catalog.label, word, takes.join(", "));
// }
/// Why a word was refused at a set or a start: the agent maps it to none of its own modes. None where it maps one.
pub fn access_refusal(catalog: &HarnessCatalog, word: AccessChoice) -> Option<String> {
    if access_mode(catalog, word).is_some() {
        return None;
    }
    let takes: Vec<&str> =
        ACCESS_WORDS.iter().filter(|(choice, _)| access_mode(catalog, *choice).is_some()).map(|(_, text)| *text).collect();
    Some(if takes.is_empty() {
        no_access_words_line(&catalog.label)
    } else {
        access_not_taken_line(&catalog.label, access_word_text(word), &takes.join(", "))
    })
}

// thread-defaults.ts:154
// export function accessWord(catalog: Pick<HarnessCatalog, "access">, mode: string): AccessChoice | undefined {
//   return ACCESS_CHOICES.find(word => catalog.access?.[word] === mode);
// }
/// The wsp word for one of a harness's own modes, where its row maps one to it.
pub fn access_word(catalog: &HarnessCatalog, mode: &str) -> Option<AccessChoice> {
    ACCESS_WORDS
        .iter()
        .map(|(choice, _)| *choice)
        .find(|word| catalog.access.as_ref().and_then(|access| access.get(word)).map(String::as_str) == Some(mode))
}

// thread-defaults.ts:365
// export function accessWordRefusal(catalog: Pick<HarnessCatalog, "label" | "access" | "permissionModes">, word: AccessChoice): Error | null {
//   const refused = accessRefusal(catalog, word);
//   return refused === null ? null : usageRefusal(refused, "Name one it takes, or drop --access.");
// }
/// An access word the agent maps to none of its modes, refused with what to do; none where it maps one.
pub fn access_word_refusal(catalog: &HarnessCatalog, word: AccessChoice) -> Option<HostError> {
    access_refusal(catalog, word).map(|refused| usage_refusal(&refused, "Name one it takes, or drop --access."))
}

// thread-defaults.ts:322
// export function olderThanTable(lists: Pick<HarnessCatalog, "source" | "version">, table: Pick<HarnessCatalog, "version"> | undefined): boolean | undefined {
//   const pin = /(\d+\.\d+\.\d+)/.exec(table?.version ?? "")?.[1];
//   if (lists.source !== "harness" || lists.version === null || pin === undefined) return undefined;
//   return compareVersions(lists.version, pin) < 0;
// }
/// Whether the binary that answered these lists is older than the release wsp's table for that agent was read off.
/// None where the lists are not a binary's own or either version cannot be read.
pub fn older_than_table(lists: &HarnessCatalog, table: Option<&HarnessCatalog>) -> Option<bool> {
    let version = table.and_then(|table| table.version.as_deref()).unwrap_or("");
    let pin = Regex::new(r"([0-9]+\.[0-9]+\.[0-9]+)").expect("a version pattern").captures(version)?.get(1)?.as_str().to_string();
    if lists.source != HarnessCatalogSource::Harness {
        return None;
    }
    Some(compare_versions(lists.version.as_deref()?, &pin).is_lt())
}

// thread-defaults.ts:332
// export const notRunLine = (agent: string, version: string, model: string, older: boolean): string =>
//   older ? `${agent} ${version} does not run ${model}; a newer ${agent} does` : `${agent} ${version} does not run ${model}`;
/// Why a start named a model wsp's own table lists that the agent's binary there does not run.
pub fn not_run_line(agent: &str, version: &str, model: &str, older: bool) -> String {
    if older {
        format!("{agent} {version} does not run {model}; a newer {agent} does")
    } else {
        format!("{agent} {version} does not run {model}")
    }
}

// thread-defaults.ts:337
// export function notRunRefusal(lists: HarnessCatalog | undefined, table: HarnessCatalog | undefined, model: string | undefined, update: string | undefined): Error | null {
//   if (lists?.source !== "harness" || lists.version === null || table === undefined || model === undefined) return null;
//   if (everyModel(lists).some(m => m.value === model)) return null;
//   const known = everyModel(table).find(m => m.value === model);
//   if (known === undefined) return null;
//   const older = olderThanTable(lists, table) === true && update !== undefined;
//   const listed = lists.models.map(m => `${m.label} (${m.value})`).join(", ");
//   return older
//     ? usageRefusal(notRunLine(lists.label, lists.version, known.label, true), `Update it with ${update}, or pick a model it lists.`)
//     : usageRefusal(`${notRunLine(lists.label, lists.version, known.label, false)}; it lists ${listed}`, "Pick a model it lists.");
// }
/// A model refused against the lists a binary answered that wsp's table carries: said as the binary's own version not
/// running it, an older binary told to update and one newer than the table told to pick a model it lists. None where the
/// lists are not the binary's own, where they carry the model, or where the table does not know it either.
pub fn not_run_refusal(
    lists: Option<&HarnessCatalog>,
    table: Option<&HarnessCatalog>,
    model: Option<&str>,
    update: Option<&str>,
) -> Option<HostError> {
    let lists = lists.filter(|lists| lists.source == HarnessCatalogSource::Harness)?;
    let version = lists.version.as_deref()?;
    let (table, model) = (table?, model?);
    if every_model(lists).iter().any(|m| m.value == model) {
        return None;
    }
    let known = every_model(table).into_iter().find(|m| m.value == model)?;
    let older = older_than_table(lists, Some(table)) == Some(true) && update.is_some();
    let listed = lists.models.iter().map(|m| format!("{} ({})", m.label, m.value)).collect::<Vec<_>>().join(", ");
    Some(match update {
        Some(update) if older => usage_refusal(
            &not_run_line(&lists.label, version, &known.label, true),
            &format!("Update it with {update}, or pick a model it lists."),
        ),
        _ => usage_refusal(
            &format!("{}; it lists {listed}", not_run_line(&lists.label, version, &known.label, false)),
            "Pick a model it lists.",
        ),
    })
}

/// What `resolveThreadDefaults` reads (thread-defaults.ts:133): `runs` absent means the agents that have lists.
pub struct DefaultsAsk<'a> {
    /// The agent a thread runs when nothing names one and no default stands.
    pub first_agent: &'a str,
    /// Each agent's lists, its custom models in; none for an agent with no catalog row.
    pub catalog_of: &'a dyn Fn(&str) -> Option<HarnessCatalog>,
    pub runs: Option<&'a dyn Fn(&str) -> bool>,
    /// The agent the start names, or the one the thread already runs on.
    pub named: Option<&'a str>,
    pub project: Option<&'a ProjectOverrides>,
    /// Read for `default_agent` and `agent_defaults` only.
    pub prefs: &'a Preferences,
}

// thread-defaults.ts:168
/// The agent, model, effort and access a new thread starts on, each with where it came from. A layer that names what
/// the agent here does not take drops to the next one, so a stale override never refuses a start.
pub fn resolve_thread_defaults(ask: &DefaultsAsk) -> ThreadDefaults {
    let runs = |id: &str| match ask.runs {
        Some(runs) => runs(id),
        None => (ask.catalog_of)(id).is_some(),
    };
    let project = ask.project;
    let project_agent_name = project.and_then(|p| p.agent.as_deref());
    let default_agent = ask.prefs.default_agent.as_deref();
    let agent = match (ask.named, project_agent_name, default_agent) {
        (Some(named), _, _) => ResolvedPick { value: named.to_string(), from: DefaultFrom::Named },
        (None, Some(agent), _) if runs(agent) => ResolvedPick { value: agent.to_string(), from: DefaultFrom::Project },
        (None, _, Some(agent)) if runs(agent) => ResolvedPick { value: agent.to_string(), from: DefaultFrom::Default },
        _ => ResolvedPick { value: ask.first_agent.to_string(), from: DefaultFrom::Catalog },
    };
    let Some(catalog) = (ask.catalog_of)(&agent.value) else {
        return ThreadDefaults { agent, model: None, effort: None, access: None };
    };
    let project_agent = project_agent_name.is_none_or(|name| name == agent.value);
    let names_this_agent = project_agent_name == Some(agent.value.as_str());
    let own = ask.prefs.agent_defaults.get(&agent.value);
    let pick_from = |options: &[String],
                     open: bool,
                     marked: Option<String>,
                     from_project: Option<&String>,
                     from_default: Option<&String>|
     -> Option<ResolvedPick> {
        let applies = |value: &str, for_this_agent: bool| if open { for_this_agent } else { options.iter().any(|o| o == value) };
        if let Some(value) = from_project.filter(|value| project_agent && applies(value, names_this_agent)) {
            return Some(ResolvedPick { value: value.clone(), from: DefaultFrom::Project });
        }
        if let Some(value) = from_default.filter(|value| applies(value, true)) {
            return Some(ResolvedPick { value: value.clone(), from: DefaultFrom::Default });
        }
        marked.map(|value| ResolvedPick { value, from: DefaultFrom::Catalog })
    };
    let models = every_model(&catalog);
    let model_values: Vec<String> = models.iter().map(|m| m.value.clone()).collect();
    let model = pick_from(
        &model_values,
        models.is_empty(),
        marked_default(&models).map(|m| m.value.clone()),
        project.and_then(|p| p.model.as_ref()),
        own.and_then(|o| o.model.as_ref()),
    );
    let efforts = efforts_for(&catalog, model_of(&catalog, model.as_ref().map(|m| m.value.as_str())).as_ref());
    let effort_values: Vec<String> = efforts.iter().map(|e| e.value.clone()).collect();
    let effort = pick_from(
        &effort_values,
        catalog.efforts.is_empty(),
        marked_default(&efforts).map(|e| e.value.clone()),
        project.and_then(|p| p.effort.as_ref()),
        own.and_then(|o| o.effort.as_ref()),
    );
    let word = |from: DefaultFrom, value: Option<AccessChoice>| -> Option<ResolvedAccess> {
        let value = value?;
        let mode = access_mode(&catalog, value)?.to_string();
        Some(ResolvedAccess { value, mode, from })
    };
    let marked_mode = marked_default(&catalog.permission_modes).map(|o| o.value.as_str());
    let access = word(DefaultFrom::Project, project.and_then(|p| p.access))
        .or_else(|| word(DefaultFrom::Default, own.and_then(|o| o.access)))
        .or_else(|| word(DefaultFrom::Catalog, marked_mode.and_then(|mode| access_word(&catalog, mode))));
    ThreadDefaults { agent, model, effort, access }
}

// thread-defaults.ts:205
/// The list with its default mark on one value alone.
fn marked<T: Clone>(options: &[T], value: &str, value_of: impl Fn(&T) -> &str, set: impl Fn(&mut T, Option<bool>)) -> Vec<T> {
    options
        .iter()
        .map(|o| {
            let mut next = o.clone();
            set(&mut next, (value_of(o) == value).then_some(true));
            next
        })
        .collect()
}

fn marked_options(options: &[HarnessOption], value: &str) -> Vec<HarnessOption> {
    marked(options, value, |o| &o.value, |o, mark| o.is_default = mark)
}

fn marked_models(models: &[HarnessModel], value: &str) -> Vec<HarnessModel> {
    marked(models, value, |m| &m.value, |m, mark| m.is_default = mark)
}

// thread-defaults.ts:215
/// The catalog with its marks moved onto what a new thread here starts on, so the composer's pickers show it and a
/// start fills it in. Only a value the person's layers picked moves a mark; the catalog's own stand. `defaults.agent`
/// is not read.
pub fn marked_for(catalog: &HarnessCatalog, defaults: &ThreadDefaults) -> HarnessCatalog {
    let moved = |from: DefaultFrom| !matches!(from, DefaultFrom::Catalog | DefaultFrom::Named);
    let mut next = catalog.clone();
    if let Some(pick) = defaults.model.as_ref().filter(|p| moved(p.from)) {
        let in_models = catalog.models.iter().any(|m| m.value == pick.value);
        next.models = if in_models {
            marked_models(&next.models, &pick.value)
        } else {
            next.models.iter().map(|m| HarnessModel { is_default: None, ..m.clone() }).collect()
        };
        if let Some(legacy) = next.legacy_models.take() {
            next.legacy_models = Some(if in_models { legacy } else { marked_models(&legacy, &pick.value) });
        }
    }
    if let Some(pick) = defaults.effort.as_ref().filter(|p| moved(p.from)) {
        let takes = |m: &HarnessModel| m.efforts.as_ref().is_none_or(|efforts| efforts.contains(&pick.value));
        let re_mark = |m: &HarnessModel| {
            if takes(m) && m.default_effort.is_some() {
                HarnessModel { default_effort: Some(pick.value.clone()), ..m.clone() }
            } else {
                m.clone()
            }
        };
        next.efforts = marked_options(&next.efforts, &pick.value);
        next.models = next.models.iter().map(re_mark).collect();
        next.legacy_models = next.legacy_models.take().map(|legacy| legacy.iter().map(re_mark).collect());
    }
    if let Some(pick) = defaults.access.as_ref().filter(|p| moved(p.from)) {
        next.permission_modes = marked_options(&next.permission_modes, &pick.mode);
    }
    next
}

/// What an open list carries that no mark can: the value the person set, for a start to run where it names none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenDefaults {
    pub model: Option<String>,
    pub effort: Option<String>,
}

// thread-defaults.ts:246
/// The model and effort a new thread starts on where the agent's list is open (its CLI takes any value).
pub fn open_defaults(catalog: &HarnessCatalog, defaults: &ThreadDefaults) -> OpenDefaults {
    let set = |pick: Option<&ResolvedPick>, open: bool| {
        pick.filter(|p| open && matches!(p.from, DefaultFrom::Project | DefaultFrom::Default)).map(|p| p.value.clone())
    };
    OpenDefaults {
        model: set(defaults.model.as_ref(), every_model(catalog).is_empty()),
        effort: set(defaults.effort.as_ref(), catalog.efforts.is_empty()),
    }
}

// thread-defaults.ts:258
/// Why an id typed by hand is no model id: the page asks it under the field and the host where it keeps one.
pub fn model_id_refusal(id: &str) -> Option<String> {
    if id.is_empty() {
        return Some("A model id cannot be empty".to_string());
    }
    if id.chars().any(crate::js::is_js_whitespace) {
        return Some("No model id has a space in it".to_string());
    }
    if id.starts_with('-') {
        return Some("No model id starts with \"-\"".to_string());
    }
    let plain = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':' | '/' | '@' | '+' | '[' | ']');
    let odd = id.chars().find(|c| !plain(*c))?;
    Some(format!("No model id has {} in it", serde_json::to_string(&odd.to_string()).unwrap_or_default()))
}

// thread-defaults.ts:267
/// The catalog with the person's own model ids added after the binary's, so a start and a default may name one.
pub fn with_custom_models(catalog: &HarnessCatalog, picker: Option<&ModelPicker>) -> HarnessCatalog {
    let known: Vec<String> = every_model(catalog).into_iter().map(|m| m.value).collect();
    let mut added: Vec<HarnessModel> = Vec::new();
    for id in picker.and_then(|p| p.custom.as_ref()).into_iter().flatten() {
        if known.contains(id) {
            continue;
        }
        added.push(HarnessModel {
            value: id.clone(),
            label: id.clone(),
            description: None,
            is_default: None,
            short: None,
            efforts: None,
            default_effort: None,
            context_windows: None,
            fast: None,
            added: Some(wsp_frames::True),
        });
    }
    if added.is_empty() {
        return catalog.clone();
    }
    let mut next = catalog.clone();
    next.models.extend(added);
    next
}

// thread-defaults.ts:432
/// Why a launch did not start: the config folder kept at setup now leads somewhere `why` refuses.
pub fn config_dir_launch_refusal(agent: &str, agent_id: &str, folder: &str, why: &str) -> String {
    format!("{agent} does not start with its config folder {folder}: {why}. Set another with wsp agents setup {agent_id} --config, or put its own back with --reset config.")
}

// thread-defaults.ts:374
/// Why a start named an agent that is off on the computer its workspace stands on.
pub fn agent_off_line(agent: &str, computer: &str) -> String {
    format!("{agent} is off on {computer}, so no thread there runs it")
}
