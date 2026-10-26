// SPDX-License-Identifier: AGPL-3.0-only
//! `agents-help.ts:300-573` at 65f6b89c7: skills.sh and skills (search, show, add, remove, turn off and on), the
//! defaults of an agent and of a project, the default agent, an agent's setup on a computer, and the sentences and
//! words each of them says. Spec: `spec/agents-set.md`.
//!
//! The target a line names comes from `read.rs` (`agents_target(client, thread, on, usage, line, project)`, which
//! answers an `AgentsTarget`); `ProjectAsked` is declared there too. `preferences_of` and `project_defaults_of` are
//! `turns_help::picks`, `project_of` is `workspaces_help::rows`. The zod inputs of the tools (`SkillNameIn`,
//! `SkillProjectIn`, `AgentsThreadIn`, `AgentsOnIn`, the `describe` strings) are declared by the tool module; the
//! spec lists their text.

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use wsp_catalog::catalog::agent_name;
use wsp_wire::types::agents_report::{AgentRow, AgentsTarget, SkillAdded, SkillHit, SkillPreview, SKILL_PREVIEW_BYTES};
use wsp_wire::types::check::{env_name, tracker_repo};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::thread_defaults::{
    AccessChoice, AgentDefaults, AgentDefaultsPatch, AgentSetupSet, AgentSetupView, DefaultFrom, EnvName, ModelPicker,
    ProjectOverridesPatch, ThreadDefaults, TrackerRepo,
};
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::workspace::ProjectView;
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::js::js_trim;
use wsp_words::thread_defaults::config_dir_sign_in_line;

use super::read::{agents_target, js_object, read_reply, target_frame, ProjectAsked};
use crate::verbs::client::{absolute_path, access_word_of, cell, printable, table, Frame, HostClient};
use crate::verbs::turns_help::{preferences_of, project_defaults_of};
use crate::verbs::workspaces_help::project_of;

// ---------------------------------------------------------------------------------------------------------------
// skills.sh and skills (agents-help.ts:300-368)
// ---------------------------------------------------------------------------------------------------------------

/// `searchSkillsSh(client, q, limit)` (agents-help.ts:301): `skills.search {q, limit?}` (`limit` left out when absent,
/// sent when 0; `q` sent as given, an empty one included), its `skills` read as a list of `SkillHit`.
pub async fn search_skills_sh(client: &dyn HostClient, q: &str, limit: Option<u64>) -> Result<Vec<SkillHit>, HostError> {
    let mut params = Frame::new();
    params.insert("q".to_owned(), json!(q));
    if let Some(limit) = limit {
        params.insert("limit".to_owned(), json!(limit));
    }
    let reply = client.request("skills.search", params).await?;
    read_reply(&reply, "skills", "skills.search")
}

/// `noSkillHitsLine` (agents-help.ts:305).
pub const NO_SKILL_HITS_LINE: &str = "no skills on skills.sh match";

/// `SKILL_HIT_COLUMNS` (agents-help.ts:306).
pub const SKILL_HIT_COLUMNS: [&str; 3] = ["SKILL", "INSTALLS", "ADD WITH"];

/// `skillHitLines(hits)` (agents-help.ts:308): `NO_SKILL_HITS_LINE` for none, else `table` of the columns and one row
/// per hit: `cell(name)`, the install count, `cell(id)`.
pub fn skill_hit_lines(hits: &[SkillHit]) -> Vec<String> {
    if hits.is_empty() {
        return vec![NO_SKILL_HITS_LINE.to_owned()];
    }
    let mut rows = vec![SKILL_HIT_COLUMNS.map(str::to_owned).to_vec()];
    for hit in hits {
        rows.push(vec![cell(&hit.name), hit.installs.to_string(), cell(&hit.id)]);
    }
    table(&rows)
}

/// `onSkillsSh(skill)` (agents-help.ts:313, not exported): a skill named in three `/`-split parts (`a//c` too) is
/// one on skills.sh; any other count of parts names one already on the target.
pub fn on_skills_sh(skill: &str) -> bool {
    skill.split('/').count() == 3
}

/// `skillsShPlacelessLine(skill)` (agents-help.ts:315).
pub fn skills_sh_placeless_line(skill: &str) -> String {
    format!("{skill} is read off skills.sh, which names no computer or project.")
}

/// `skillShown(client, skill, thread, on, project, usage, line)` (agents-help.ts:318-325). A skills.sh id with a
/// thread, a computer or a project asked (`project.project`) is refused (`skills_sh_placeless_line`, usage `usage`)
/// before any frame, else `skills.get {skill}` and its `preview`. Any other name: `agents_target(client, thread, on,
/// usage, line, project.name)`, then `skills.preview {target, name, project: true?}` (`project` only when asked).
/// The preview is read as `SkillPreview`.
pub async fn skill_shown(
    client: &dyn HostClient,
    skill: &str,
    thread: Option<&str>,
    on: Option<&str>,
    project: &ProjectAsked,
    usage: &str,
    line: &str,
    cloud_on: bool,
) -> Result<SkillPreview, HostError> {
    if on_skills_sh(skill) {
        if thread.is_some() || on.is_some() || project.project {
            return Err(usage_refusal(&skills_sh_placeless_line(skill), usage));
        }
        let mut params = Frame::new();
        params.insert("skill".to_owned(), json!(skill));
        let reply = client.request("skills.get", params).await?;
        return read_reply(&reply, "preview", "skills.get");
    }
    let target = agents_target(client, thread, on, usage, line, project.name.as_deref(), cloud_on).await?;
    let mut params = target_frame(&target);
    params.insert("name".to_owned(), json!(skill));
    if project.project {
        params.insert("project".to_owned(), json!(true));
    }
    let reply = client.request("skills.preview", params).await?;
    read_reply(&reply, "preview", "skills.preview")
}

/// `previewCutLine(kb)` (agents-help.ts:327): `(the first 64 KB of <kb> KB)`, the 64 from `SKILL_PREVIEW_BYTES / 1024`.
pub fn preview_cut_line(kb: u64) -> String {
    format!("(the first {} KB of {kb} KB)", SKILL_PREVIEW_BYTES / 1024)
}

/// `shownText(p)` (agents-help.ts:329-332): `printable(text)`, and where `size` is over `SKILL_PREVIEW_BYTES` (65536,
/// not at it) a blank line and `preview_cut_line(ceil(size / 1024))`.
pub fn shown_text(p: &SkillPreview) -> String {
    let text = printable(&p.text);
    let size = p.size.as_f64();
    if size > SKILL_PREVIEW_BYTES as f64 {
        format!("{text}\n\n{}", preview_cut_line((size / 1024.0).ceil() as u64))
    } else {
        text
    }
}

/// `skillAdded(client, skill, thread, on, agents, project, usage, line)` (agents-help.ts:334-337):
/// `agents_target(.., project.name)`, then `skills.add {target, skill, agents?, project: true?}`; `agents` is sent only
/// when non-empty, `project` only when asked. Its `added` read as `SkillAdded`.
#[allow(clippy::too_many_arguments)]
pub async fn skill_added(
    client: &dyn HostClient,
    skill: &str,
    thread: Option<&str>,
    on: Option<&str>,
    agents: Option<&[String]>,
    project: &ProjectAsked,
    usage: &str,
    line: &str,
    cloud_on: bool,
) -> Result<SkillAdded, HostError> {
    let target = agents_target(client, thread, on, usage, line, project.name.as_deref(), cloud_on).await?;
    let mut params = target_frame(&target);
    params.insert("skill".to_owned(), json!(skill));
    if let Some(agents) = agents.filter(|agents| !agents.is_empty()) {
        params.insert("agents".to_owned(), json!(agents));
    }
    if project.project {
        params.insert("project".to_owned(), json!(true));
    }
    let reply = client.request("skills.add", params).await?;
    read_reply(&reply, "added", "skills.add")
}

/// `isInLine(name, path)` (agents-help.ts:339).
pub fn is_in_line(name: &str, path: &str) -> String {
    format!("{name} is in {path}.")
}

/// `isInAlsoLine(name, path, copies)` (agents-help.ts:340).
pub fn is_in_also_line(name: &str, path: &str, copies: &str) -> String {
    format!("{name} is in {path}, and in {copies}.")
}

/// `agentCopyWords(agent, path)` (agents-help.ts:341): `<agent>'s <path>`.
pub fn agent_copy_words(agent: &str, path: &str) -> String {
    format!("{agent}'s {path}")
}

/// `addedLine(skill, a)` (agents-help.ts:343-346): the name is `cell` of the last `/` part of `skill` (empty for a
/// trailing `/`); with no agent copy `is_in_line(name, cell(path))`, else `is_in_also_line` with each copy as
/// `agent_copy_words(agentName(agent), cell(path))`, joined by `, ` in the order answered.
pub fn added_line(skill: &str, a: &SkillAdded) -> String {
    let name = cell(skill.rsplit('/').next().unwrap_or(skill));
    if a.agents.is_empty() {
        return is_in_line(&name, &cell(&a.path));
    }
    let copies: Vec<String> = a.agents.iter().map(|copy| agent_copy_words(&agent_name(&copy.agent), &cell(&copy.path))).collect();
    is_in_also_line(&name, &cell(&a.path), &copies.join(", "))
}

/// `goneFromLine(name, from)` (agents-help.ts:348).
pub fn gone_from_line(name: &str, from: &str) -> String {
    format!("{name} is gone from {from}.")
}

/// `turnedInLine(name, on, file)` (agents-help.ts:349): `<name> is on in <file>.` or `is off in`.
pub fn turned_in_line(name: &str, on: bool, file: &str) -> String {
    format!("{name} is {} in {file}.", on_word(on))
}

/// `removedLine(name, removed)` (agents-help.ts:351): `gone_from_line(cell(name), removed.map(cell).join(", "))`.
/// No folder at all reads `<name> is gone from .`.
pub fn removed_line(name: &str, removed: &[String]) -> String {
    let folders: Vec<String> = removed.iter().map(|folder| cell(folder)).collect();
    gone_from_line(&cell(name), &folders.join(", "))
}

/// The op `skill_changed` sends: `skills.remove` (reads `removed`) or `skills.toggle` (reads `paths`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillChange {
    Remove,
    Toggle,
}

impl SkillChange {
    /// The op's name on the wire.
    pub fn op(&self) -> &'static str {
        match self {
            SkillChange::Remove => "skills.remove",
            SkillChange::Toggle => "skills.toggle",
        }
    }
}

/// `skillChanged(client, op, name, thread, on, project, turn, usage, line)` (agents-help.ts:354-358):
/// `agents_target(.., project.name)`, then `<op> {target, name, project: true?, on: turn?}` (`project` only when
/// asked, `on` only when `turn` is given, for either op). Answers the list of strings read off `removed` for
/// `Remove` and off `paths` for `Toggle`, whatever else the reply carries.
#[allow(clippy::too_many_arguments)]
pub async fn skill_changed(
    client: &dyn HostClient,
    op: SkillChange,
    name: &str,
    thread: Option<&str>,
    on: Option<&str>,
    project: &ProjectAsked,
    turn: Option<bool>,
    usage: &str,
    line: &str,
    cloud_on: bool,
) -> Result<Vec<String>, HostError> {
    let target = agents_target(client, thread, on, usage, line, project.name.as_deref(), cloud_on).await?;
    let mut params = target_frame(&target);
    params.insert("name".to_owned(), json!(name));
    if project.project {
        params.insert("project".to_owned(), json!(true));
    }
    if let Some(turn) = turn {
        params.insert("on".to_owned(), json!(turn));
    }
    let reply = client.request(op.op(), params).await?;
    let key = match op {
        SkillChange::Remove => "removed",
        SkillChange::Toggle => "paths",
    };
    read_reply(&reply, key, op.op())
}

/// `turnedLine(name, on)` (agents-help.ts:360): `<name> is on.` or `is off.`
pub fn turned_line(name: &str, on: bool) -> String {
    format!("{name} is {}.", on_word(on))
}

/// `SKILL_CHANGE_WORDS` (agents-help.ts:367): the tool text of a skill change. Plain text, so declared here.
pub const SKILL_CHANGE_WORDS: &str = "The skill wsp writes and a plugin's are always on and are refused; a napping machine is not woken. The report there reads again at once.";

// ---------------------------------------------------------------------------------------------------------------
// --reset words, and the defaults of an agent (agents-help.ts:370-446)
// ---------------------------------------------------------------------------------------------------------------

/// `AGENT_SET_RESETS` (agents-help.ts:371).
pub const AGENT_SET_RESETS: [&str; 4] = ["model", "effort", "access", "models"];
/// `PROJECT_SET_RESETS` (agents-help.ts:372).
pub const PROJECT_SET_RESETS: [&str; 6] = ["agent", "model", "effort", "access", "after-worktree", "trackers"];
/// `AGENT_SETUP_RESETS` (agents-help.ts:373).
pub const AGENT_SETUP_RESETS: [&str; 3] = ["program", "config", "args"];

/// `resetsOf(fields, given)` (agents-help.ts:376-379, not exported): every word of `given` must be one of `fields`
/// (exact case); the first that is not is refused as usage: `--reset takes <fields, comma joined>, and got <word as
/// JSON>.` with the fix `Name one of those for each --reset.` Answers `given` as it is (duplicates kept).
pub fn resets_of(fields: &[&str], given: &[String]) -> Result<Vec<String>, HostError> {
    for word in given {
        if !fields.contains(&word.as_str()) {
            let said = format!("--reset takes {}, and got {}.", fields.join(", "), json!(word));
            return Err(usage_refusal(&said, "Name one of those for each --reset."));
        }
    }
    Ok(given.to_vec())
}

/// `AgentSetAsk` (agents-help.ts:387): what `wsp agents set` and its tool take. Read from JSON by the tests, with
/// the tool's own camelCase names.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSetAsk {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub access: Option<String>,
    pub hide: Option<Vec<String>>,
    pub show: Option<Vec<String>>,
    pub order: Option<Vec<String>>,
    pub add_model: Option<Vec<String>>,
    pub drop_model: Option<Vec<String>>,
    pub reset: Option<Vec<String>>,
}

/// `agentSetNothingLine` (agents-help.ts:399).
pub const AGENT_SET_NOTHING_LINE: &str =
    "wsp agents set takes --model, --effort, --access, a model to --hide, --show, --add-model or --drop-model, --order or --reset.";
/// `NOTHING_TO_SET_FIX` (agents-help.ts:401).
pub const NOTHING_TO_SET_FIX: &str = "Name at least one; with none of them there is nothing to change.";

/// `agentDefaultsSet(client, agent, ask)` (agents-help.ts:405-425). In this order: the reset words are checked
/// (`resets_of(AGENT_SET_RESETS)`), then the patch is built; no frame is sent before it is whole. `reset models` makes
/// `models` null and reads nothing. Otherwise, when any of hide, show, order, addModel or dropModel is non-empty,
/// `preferences.get` is read once and the agent's kept `models` merged: hide = (kept hide without the shown ones) then
/// the asked hide, de-duplicated first-seen; custom = (kept custom without the dropped ones) then addModel,
/// de-duplicated; order = the asked order when non-empty (not de-duplicated), else the kept one; the picker keeps
/// only the lists that are non-empty, in the order hide, order, custom (an empty picker `{}` is sent as it is).
/// A model, effort or access given is sent as given (access through `access_word_of`, refused before any frame
/// when it is not one of ask, auto-edit, full, plan); one not given is null where its word is in `reset`, else left
/// out. The patch's key order is model, effort, access, models. An empty patch is refused
/// (`AGENT_SET_NOTHING_LINE`, fix `NOTHING_TO_SET_FIX`). Then `preferences.set {patch: {agentDefaults: {<agent>:
/// patch}}}`, and the answer is the agent's `agentDefaults` entry of the reply's `Preferences` (`{}` when none).
pub async fn agent_defaults_set(client: &dyn HostClient, agent: &str, ask: &AgentSetAsk) -> Result<AgentDefaults, HostError> {
    let resets = resets_of(&AGENT_SET_RESETS, ask.reset.as_deref().unwrap_or(&[]))?;
    let reset = |word: &str| resets.iter().any(|r| r == word);
    let lists = [&ask.hide, &ask.show, &ask.order, &ask.add_model, &ask.drop_model]
        .into_iter()
        .any(|list| list.as_ref().is_some_and(|l| !l.is_empty()));
    let mut models: Option<Option<ModelPicker>> = reset("models").then_some(None);
    if models.is_none() && lists {
        let kept = preferences_of(client).await?.agent_defaults.get(agent).and_then(|entry| entry.models.clone()).unwrap_or(ModelPicker {
            hide: None,
            order: None,
            custom: None,
        });
        let shown = ask.show.clone().unwrap_or_default();
        let dropped = ask.drop_model.clone().unwrap_or_default();
        let mut hide: IndexSet<String> = kept.hide.unwrap_or_default().into_iter().filter(|m| !shown.contains(m)).collect();
        hide.extend(ask.hide.clone().unwrap_or_default());
        let mut custom: IndexSet<String> = kept.custom.unwrap_or_default().into_iter().filter(|m| !dropped.contains(m)).collect();
        custom.extend(ask.add_model.clone().unwrap_or_default());
        let order = match &ask.order {
            Some(order) if !order.is_empty() => Some(order.clone()),
            _ => kept.order,
        };
        models = Some(Some(ModelPicker {
            hide: non_empty(hide.into_iter().collect()),
            order: order.filter(|order| !order.is_empty()),
            custom: non_empty(custom.into_iter().collect()),
        }));
    }
    let access = ask.access.as_deref().map(access_word_of).transpose()?;
    let patch = AgentDefaultsPatch {
        model: patched(ask.model.clone(), reset("model")),
        effort: patched(ask.effort.clone(), reset("effort")),
        access: patched(access, reset("access")),
        models,
    };
    if patch.model.is_none() && patch.effort.is_none() && patch.access.is_none() && patch.models.is_none() {
        return Err(usage_refusal(AGENT_SET_NOTHING_LINE, NOTHING_TO_SET_FIX));
    }
    let mut entry = Frame::new();
    entry.insert(agent.to_owned(), serde_json::to_value(&patch).expect("a patch writes as JSON"));
    let mut patch_frame = Frame::new();
    patch_frame.insert("agentDefaults".to_owned(), Value::Object(entry));
    let mut params = Frame::new();
    params.insert("patch".to_owned(), Value::Object(patch_frame));
    let reply = client.request("preferences.set", params).await?;
    let prefs: Preferences = read_reply(&reply, "preferences", "preferences.set")?;
    Ok(prefs.agent_defaults.get(agent).map(|entry| entry.clone().into_inner()).unwrap_or(AgentDefaults {
        model: None,
        effort: None,
        access: None,
        models: None,
    }))
}

/// `PICKER_WORDS.hide` (agents-help.ts:428): `hides <models>`.
pub fn picker_hide(models: &str) -> String {
    format!("hides {models}")
}
/// `PICKER_WORDS.order` (agents-help.ts:428): `lists <models> first`.
pub fn picker_order(models: &str) -> String {
    format!("lists {models} first")
}
/// `PICKER_WORDS.custom` (agents-help.ts:428): `adds <models>`.
pub fn picker_custom(models: &str) -> String {
    format!("adds {models}")
}

/// `startsOnOwnLine(agent)` (agents-help.ts:429).
pub fn starts_on_own_line(agent: &str) -> String {
    format!("{agent} starts on its own defaults.")
}
/// `startsOnLine(agent, said)` (agents-help.ts:430).
pub fn starts_on_line(agent: &str, said: &str) -> String {
    format!("{agent} starts on {said}.")
}

/// `agentDefaultsLine(agent, d)` (agents-help.ts:433-438): the words said, in order: model, effort, access (each as
/// stored, not cleaned), then the picker's hide, order, custom lists (each only when non-empty, models joined by a
/// space, through `picker_hide`, `picker_order`, `picker_custom`); none at all is `starts_on_own_line`, else
/// `starts_on_line` with the words joined by `; `. The agent is named by `agentName(agent)` (its id when the catalog has
/// none).
pub fn agent_defaults_line(agent: &str, d: &AgentDefaults) -> String {
    let listed = |models: Option<&Vec<String>>, words: fn(&str) -> String| -> Option<String> {
        models.filter(|m| !m.is_empty()).map(|m| words(&m.join(" ")))
    };
    let hide = d.models.as_ref().and_then(|m| m.hide.as_ref());
    let order = d.models.as_ref().and_then(|m| m.order.as_ref());
    let custom = d.models.as_ref().and_then(|m| m.custom.as_ref());
    let picker = [listed(hide, picker_hide), listed(order, picker_order), listed(custom, picker_custom)];
    let said: Vec<String> = [d.model.clone(), d.effort.clone(), d.access.map(|access| access_word(access).to_owned())]
        .into_iter()
        .chain(picker)
        .flatten()
        .collect();
    if said.is_empty() {
        starts_on_own_line(&agent_name(agent))
    } else {
        starts_on_line(&agent_name(agent), &said.join("; "))
    }
}

/// The answer of `default_agent_set`: `{defaultAgent}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultAgentSet {
    pub default_agent: String,
}

/// `defaultAgentSet(client, agent)` (agents-help.ts:441-444): `preferences.set {patch: {defaultAgent: agent}}`, the
/// reply not read; answers `{defaultAgent: agent}` (an empty agent is sent as it is; the host refuses).
pub async fn default_agent_set(client: &dyn HostClient, agent: &str) -> Result<DefaultAgentSet, HostError> {
    let mut patch = Frame::new();
    patch.insert("defaultAgent".to_owned(), json!(agent));
    let mut params = Frame::new();
    params.insert("patch".to_owned(), Value::Object(patch));
    client.request("preferences.set", params).await?;
    Ok(DefaultAgentSet { default_agent: agent.to_owned() })
}

/// `defaultAgentLine(agent)` (agents-help.ts:446): names the agent by `agentName`.
pub fn default_agent_line(agent: &str) -> String {
    format!("A new thread that names no agent runs {}, where its project names none.", agent_name(agent))
}

// ---------------------------------------------------------------------------------------------------------------
// An agent's setup (agents-help.ts:448-503)
// ---------------------------------------------------------------------------------------------------------------

/// `envNameLine(named, quoted)` (agents-help.ts:448).
pub fn env_name_line(named: &str, quoted: &str) -> String {
    format!("{named} takes a variable's name, and got {quoted}.")
}
/// `ENV_NAME_FIX` (agents-help.ts:449).
pub const ENV_NAME_FIX: &str = "Name the variable alone; its value is asked for where nothing echoes it.";

/// `envNameOf(name, named)` (agents-help.ts:452-455): the name as given when it is letters, digits and `_` and does
/// not start with a digit (`EnvName`); else refused as usage with `env_name_line(named, <name as JSON>)` and
/// `ENV_NAME_FIX`. Whether the variable is one the host forbids is the host's to say.
pub fn env_name_of(name: &str, named: &str) -> Result<String, HostError> {
    if !env_name(name) {
        return Err(usage_refusal(&env_name_line(named, &json!(name).to_string()), ENV_NAME_FIX));
    }
    Ok(name.to_owned())
}

/// `AgentSetupAsk` (agents-help.ts:458): what `wsp agents setup` and its tool take.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AgentSetupAsk {
    pub on: Option<String>,
    pub enabled: Option<bool>,
    pub program: Option<String>,
    pub config: Option<String>,
    pub args: Option<Vec<String>>,
    pub unset_env: Option<Vec<String>>,
    pub reset: Option<Vec<String>>,
}

/// `agentSetupNothingLine` (agents-help.ts:468).
pub const AGENT_SETUP_NOTHING_LINE: &str =
    "wsp agents setup takes --enable or --disable, --program, --config, --arg, --env, --unset-env or --reset.";

/// `agentSetupSet(client, agent, ask, values, usage)` (agents-help.ts:471-486). In this order: `resets_of(AGENT_SETUP_RESETS)`;
/// `agents_target(client, None, ask.on, usage, "wsp agents setup")` (so with `on` a `places.list` is sent here, and a
/// refusal below comes after it); `env_name_of(name, "--unset-env")` for each unset name; then the change: `on` from
/// `enabled`, `program`, `configDir` (`absolute_path("--config is a folder on that computer", config)`, never
/// resolved or expanded), `args` (empty is not given), each as given, null where its reset word is named and it is
/// not given; `env` = `values` then each unset name as null (a name in both keeps its first place and is null),
/// sent only when non-empty. An empty change is refused (`AGENT_SETUP_NOTHING_LINE`, fix `NOTHING_TO_SET_FIX`).
/// Then `agents.setup {placeId, agent, ..change}` with `placeId` the target's, else `HERE_PLACE_ID`; answers the
/// reply's `agent` as an `AgentRow`.
pub async fn agent_setup_set(
    client: &dyn HostClient,
    agent: &str,
    ask: &AgentSetupAsk,
    values: &IndexMap<String, String>,
    usage: &str,
) -> Result<AgentRow, HostError> {
    let resets = resets_of(&AGENT_SETUP_RESETS, ask.reset.as_deref().unwrap_or(&[]))?;
    let reset = |word: &str| resets.iter().any(|r| r == word);
    // No thread is named here, so the cloud setting is never read.
    let target = agents_target(client, None, ask.on.as_deref(), usage, "wsp agents setup", None, false).await?;
    let unset: Vec<String> = ask.unset_env.iter().flatten().map(|name| env_name_of(name, "--unset-env")).collect::<Result<_, _>>()?;
    let mut pairs: Vec<(String, Option<String>)> = values.iter().map(|(name, value)| (name.clone(), Some(value.clone()))).collect();
    pairs.extend(unset.into_iter().map(|name| (name, None)));
    let env: IndexMap<EnvName, Option<String>> = js_object(pairs).into_iter().map(|(name, value)| (EnvName(name), value)).collect();
    let config = ask.config.as_deref().map(|config| absolute_path("--config is a folder on that computer", config)).transpose()?;
    let change = AgentSetupSet {
        on: ask.enabled,
        program: patched(ask.program.clone(), reset("program")),
        config_dir: patched(config, reset("config")),
        args: patched(ask.args.clone().filter(|args| !args.is_empty()), reset("args")),
        env: non_empty_map(env),
    };
    if change.on.is_none() && change.program.is_none() && change.config_dir.is_none() && change.args.is_none() && change.env.is_none() {
        return Err(usage_refusal(AGENT_SETUP_NOTHING_LINE, NOTHING_TO_SET_FIX));
    }
    let place_id = match &target {
        AgentsTarget::V0(place) => place.place_id.clone(),
        AgentsTarget::V1(_) => HERE_PLACE_ID.to_owned(),
    };
    let mut params = Frame::new();
    params.insert("placeId".to_owned(), json!(place_id));
    params.insert("agent".to_owned(), json!(agent));
    if let Value::Object(fields) = serde_json::to_value(&change).expect("a change writes as JSON") {
        params.extend(fields);
    }
    let reply = client.request("agents.setup", params).await?;
    read_reply(&reply, "agent", "agents.setup")
}

/// `setupOnLine(name, on)` (agents-help.ts:488): `<name> is on there.` or `is off there.`
pub fn setup_on_line(name: &str, on: bool) -> String {
    format!("{name} is {} there.", on_word(on))
}

/// `SETUP_WORDS` (agents-help.ts:490): the label each fact of a setup is printed under.
pub struct SetupWords {
    pub program: &'static str,
    pub config_dir: &'static str,
    pub args: &'static str,
    pub env_names: &'static str,
}
pub const SETUP_WORDS: SetupWords =
    SetupWords { program: "program", config_dir: "config folder", args: "launch words", env_names: "variables" };

/// `agentSetupLines(row, configMoved)` (agents-help.ts:493-503): the row's `setup` (when it has none, `{on: true,
/// envNames: []}`) as `setup_on_line(row.name, on)` (the row's own name, not the catalog's), then one line each, in
/// order, for `program` (`cell`), `configDir` (`cell`), `args` (each `cell`, joined by a space; an empty list is
/// printed as the label and nothing after it), `envNames` (only when non-empty, joined by a space); then, when
/// `config_moved` and the setup has a `configDir`, `configDirSignInLine(row.name)` and a full stop.
pub fn agent_setup_lines(row: &AgentRow, config_moved: bool) -> Vec<String> {
    let setup =
        row.setup.clone().unwrap_or(AgentSetupView { on: true, program: None, config_dir: None, args: None, env_names: Vec::new() });
    let mut lines = vec![setup_on_line(&row.name, setup.on)];
    if let Some(program) = &setup.program {
        lines.push(format!("{}: {}", SETUP_WORDS.program, cell(program)));
    }
    if let Some(config_dir) = &setup.config_dir {
        lines.push(format!("{}: {}", SETUP_WORDS.config_dir, cell(config_dir)));
    }
    if let Some(args) = &setup.args {
        let words: Vec<String> = args.iter().map(|arg| cell(arg)).collect();
        lines.push(format!("{}: {}", SETUP_WORDS.args, words.join(" ")));
    }
    if !setup.env_names.is_empty() {
        lines.push(format!("{}: {}", SETUP_WORDS.env_names, setup.env_names.join(" ")));
    }
    if config_moved && setup.config_dir.is_some() {
        lines.push(format!("{}.", config_dir_sign_in_line(&row.name)));
    }
    lines
}

// ---------------------------------------------------------------------------------------------------------------
// A project's defaults (agents-help.ts:505-567)
// ---------------------------------------------------------------------------------------------------------------

/// `ProjectSetAsk` (agents-help.ts:506): what `wsp projects set` and its tool take.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectSetAsk {
    pub agent: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub access: Option<String>,
    pub after_worktree: Option<String>,
    pub trackers: Option<Vec<String>>,
    pub reset: Option<Vec<String>>,
}

/// `projectSetNothingLine` (agents-help.ts:516).
pub const PROJECT_SET_NOTHING_LINE: &str =
    "wsp projects set takes --agent, --model, --effort, --access, --after-worktree, --tracker or --reset.";
/// `trackerShapeLine(word)` (agents-help.ts:517): the word as JSON.
pub fn tracker_shape_line(word: &str) -> String {
    format!("--tracker {} names no repo.", json!(word))
}
/// `TRACKER_SHAPE_FIX` (agents-help.ts:518).
pub const TRACKER_SHAPE_FIX: &str = "Name it as owner/name, the way gh repo view does.";
/// `trackersLine(trackers)` (agents-help.ts:520): the repos joined by `, `.
pub fn trackers_line(trackers: &[String]) -> String {
    format!("gh on a sub-thread's machine may also comment on: {}", trackers.join(", "))
}
/// `afterWorktreeBlankLine` (agents-help.ts:521).
pub const AFTER_WORKTREE_BLANK_LINE: &str = "--after-worktree was given a blank command, which a new worktree cannot run.";
/// `AFTER_WORKTREE_BLANK_FIX` (agents-help.ts:522).
pub const AFTER_WORKTREE_BLANK_FIX: &str = "Name the shell line a new worktree runs, or take the command away with --reset after-worktree.";
/// `afterWorktreeLine(command)` (agents-help.ts:524).
pub fn after_worktree_line(command: &str) -> String {
    format!("after a new worktree: {command}")
}
/// `noDefaultsAnsweredLine(project)` (agents-help.ts:525): the project's name as it is, not cleaned.
pub fn no_defaults_answered_line(project: &str) -> String {
    format!("the host answered no defaults for {project}")
}

/// The answer of `project_defaults_set`: the project as the host resolved it, what a new thread on it starts on, and
/// the two overrides the preferences reply echoes beside the picks (each left out when the reply has none, an empty
/// list kept).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDefaultsSet {
    pub project: ProjectView,
    pub defaults: ThreadDefaults,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trackers: Option<Vec<String>>,
}

/// `projectDefaultsSet(client, ref, ask)` (agents-help.ts:528-549). Refusals in this order, each before any frame:
/// an after-worktree command blank after `trim` (`AFTER_WORKTREE_BLANK_LINE`, `AFTER_WORKTREE_BLANK_FIX`); the first
/// tracker that is not `owner/name` (one `/`, no space, no empty side: `tracker_shape_line`, `TRACKER_SHAPE_FIX`);
/// `resets_of(PROJECT_SET_RESETS)`; the access word (`access_word_of`); an empty patch (`PROJECT_SET_NOTHING_LINE`,
/// `NOTHING_TO_SET_FIX`). The patch's key order is agent, model, effort, access, afterWorktree, trackers; a field
/// given is sent as given (the after-worktree command untrimmed, the trackers de-duplicated first-seen, an empty
/// list is not given); one not given is null where its reset word (`after-worktree`, `trackers`) is named, else left
/// out. Then, in order: `projects.resolve {ref}` (`project_of`), `preferences.set {patch: {projectDefaults:
/// {<project.id>: patch}}}`, `projects.defaults` (`project_defaults_of`). When the defaults hold no entry for the
/// project id, a plain error `no_defaults_answered_line(project.name)`. The answer carries the project, its defaults,
/// and the reply's `projectDefaults[project.id]` `afterWorktree` and `trackers` where present.
pub async fn project_defaults_set(client: &dyn HostClient, r#ref: &str, ask: &ProjectSetAsk) -> Result<ProjectDefaultsSet, HostError> {
    if ask.after_worktree.as_deref().is_some_and(|command| js_trim(command).is_empty()) {
        return Err(usage_refusal(AFTER_WORKTREE_BLANK_LINE, AFTER_WORKTREE_BLANK_FIX));
    }
    if let Some(word) = ask.trackers.iter().flatten().find(|word| !tracker_repo(word.as_str())) {
        return Err(usage_refusal(&tracker_shape_line(word), TRACKER_SHAPE_FIX));
    }
    let resets = resets_of(&PROJECT_SET_RESETS, ask.reset.as_deref().unwrap_or(&[]))?;
    let reset = |word: &str| resets.iter().any(|r| r == word);
    let access = ask.access.as_deref().map(access_word_of).transpose()?;
    let trackers = ask.trackers.as_ref().filter(|trackers| !trackers.is_empty()).map(|trackers| {
        let unique: IndexSet<String> = trackers.iter().cloned().collect();
        unique.into_iter().map(TrackerRepo).collect::<Vec<TrackerRepo>>()
    });
    let patch = ProjectOverridesPatch {
        agent: patched(ask.agent.clone(), reset("agent")),
        model: patched(ask.model.clone(), reset("model")),
        effort: patched(ask.effort.clone(), reset("effort")),
        access: patched(access, reset("access")),
        after_worktree: patched(ask.after_worktree.clone(), reset("after-worktree")),
        trackers: patched(trackers, reset("trackers")),
    };
    if patch.agent.is_none()
        && patch.model.is_none()
        && patch.effort.is_none()
        && patch.access.is_none()
        && patch.after_worktree.is_none()
        && patch.trackers.is_none()
    {
        return Err(usage_refusal(PROJECT_SET_NOTHING_LINE, NOTHING_TO_SET_FIX));
    }
    let project = project_of(client, r#ref).await?;
    let mut by_id = Frame::new();
    by_id.insert(project.id.clone(), serde_json::to_value(&patch).expect("a patch writes as JSON"));
    let mut patch_frame = Frame::new();
    patch_frame.insert("projectDefaults".to_owned(), Value::Object(by_id));
    let mut params = Frame::new();
    params.insert("patch".to_owned(), Value::Object(patch_frame));
    let reply = client.request("preferences.set", params).await?;
    let defaults = project_defaults_of(client)
        .await?
        .get(&project.id)
        .cloned()
        .ok_or_else(|| HostError::new(no_defaults_answered_line(&project.name)))?;
    let prefs: Preferences = read_reply(&reply, "preferences", "preferences.set")?;
    let entry = prefs.project_defaults.get(&project.id).map(|entry| entry.clone().into_inner());
    Ok(ProjectDefaultsSet {
        project,
        defaults,
        after_worktree: entry.as_ref().and_then(|entry| entry.after_worktree.clone()),
        trackers: entry.and_then(|entry| entry.trackers).map(|trackers| trackers.into_iter().map(|tracker| tracker.0).collect()),
    })
}

/// `FROM_WORDS` (agents-help.ts:552): where a resolved value came from, as a person reads it.
pub fn from_word(from: DefaultFrom) -> &'static str {
    match from {
        DefaultFrom::Named => "named on the start",
        DefaultFrom::Project => "this project's",
        DefaultFrom::Default => "your default",
        DefaultFrom::Catalog => "the catalog's",
    }
}
/// `newThreadsHeadLine(project)` (agents-help.ts:553).
pub fn new_threads_head_line(project: &str) -> String {
    format!("A new thread on {project} starts on:")
}

/// `DEFAULTS_LABELS` (agents-help.ts:556): the word each resolved value is printed under.
pub struct DefaultsLabels {
    pub agent: &'static str,
    pub model: &'static str,
    pub effort: &'static str,
    pub access: &'static str,
}
pub const DEFAULTS_LABELS: DefaultsLabels = DefaultsLabels { agent: "agent", model: "model", effort: "effort", access: "access" };

/// `defaultsValueLine(label, value, from)` (agents-help.ts:557): `<label> <value>: <from>`.
pub fn defaults_value_line(label: &str, value: &str, from: &str) -> String {
    format!("{label} {value}: {from}")
}

/// `threadDefaultsLines(d)` (agents-help.ts:560-563): one `defaults_value_line` for the agent (shown by `agentName`
/// of its value), then for the model, the effort and the access that are present, in that order, each value as it is
/// (the access's value, not its mode) with `from_word` of where it came from.
pub fn thread_defaults_lines(d: &ThreadDefaults) -> Vec<String> {
    let mut lines = vec![defaults_value_line(DEFAULTS_LABELS.agent, &agent_name(&d.agent.value), from_word(d.agent.from))];
    if let Some(model) = &d.model {
        lines.push(defaults_value_line(DEFAULTS_LABELS.model, &model.value, from_word(model.from)));
    }
    if let Some(effort) = &d.effort {
        lines.push(defaults_value_line(DEFAULTS_LABELS.effort, &effort.value, from_word(effort.from)));
    }
    if let Some(access) = &d.access {
        lines.push(defaults_value_line(DEFAULTS_LABELS.access, access_word(access.value), from_word(access.from)));
    }
    lines
}

/// `defaultsCell(d)` (agents-help.ts:566-567): `-` for none read, else the agent, the model and the access (not the
/// effort) that are present, joined by a space, each followed by ` (project)` where it came from the project.
pub fn defaults_cell(d: Option<&ThreadDefaults>) -> String {
    let Some(d) = d else { return "-".to_owned() };
    let mut picks = vec![pick_cell(&d.agent.value, d.agent.from)];
    if let Some(model) = &d.model {
        picks.push(pick_cell(&model.value, model.from));
    }
    if let Some(access) = &d.access {
        picks.push(pick_cell(access_word(access.value), access.from));
    }
    picks.join(" ")
}

/// `AGENTS_ON_WORDS` (agents-help.ts:572): the tool text of `on`. Plain text, so declared here.
pub const AGENTS_ON_WORDS: &str =
    "the computer to read, by the name wsp computers shows; this computer without it, and a thread names its own";
/// `AGENTS_READ_WORDS` (agents-help.ts:573): the tool text of a read. Plain text, so declared here.
pub const AGENTS_READ_WORDS: &str = "Read as the login the computer was added with, off each agent's config and whether its files are there: no MCP server is started and no login file is opened. A napping machine answers what stood there when it last ran, marked stale with its pauseMode (disk: the nap stopped it, memory: it naps), and is not woken.";

fn on_word(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

fn patched<T>(value: Option<T>, reset: bool) -> Option<Option<T>> {
    match value {
        Some(value) => Some(Some(value)),
        None => reset.then_some(None),
    }
}

fn non_empty(items: Vec<String>) -> Option<Vec<String>> {
    (!items.is_empty()).then_some(items)
}

fn non_empty_map(env: IndexMap<EnvName, Option<String>>) -> Option<IndexMap<EnvName, Option<String>>> {
    (!env.is_empty()).then_some(env)
}

fn access_word(access: AccessChoice) -> &'static str {
    match access {
        AccessChoice::Ask => "ask",
        AccessChoice::AutoEdit => "auto-edit",
        AccessChoice::Full => "full",
        AccessChoice::Plan => "plan",
    }
}

fn pick_cell(value: &str, from: DefaultFrom) -> String {
    if from == DefaultFrom::Project {
        format!("{value} (project)")
    } else {
        value.to_owned()
    }
}
