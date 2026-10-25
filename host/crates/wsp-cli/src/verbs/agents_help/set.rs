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

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wsp_wire::types::agents_report::{AgentRow, SkillAdded, SkillHit, SkillPreview, SKILL_PREVIEW_BYTES};
use wsp_wire::types::thread_defaults::{AgentDefaults, DefaultFrom, ThreadDefaults};
use wsp_wire::views::workspace::ProjectView;
use wsp_words::exit::HostError;

use super::read::ProjectAsked;
use crate::verbs::client::HostClient;

// ---------------------------------------------------------------------------------------------------------------
// skills.sh and skills (agents-help.ts:300-368)
// ---------------------------------------------------------------------------------------------------------------

/// `searchSkillsSh(client, q, limit)` (agents-help.ts:301): `skills.search {q, limit?}` (`limit` left out when absent,
/// sent when 0; `q` sent as given, an empty one included), its `skills` read as a list of `SkillHit`.
pub async fn search_skills_sh(client: &dyn HostClient, q: &str, limit: Option<u64>) -> Result<Vec<SkillHit>, HostError> {
    let _ = (client, q, limit);
    todo!("agents-help.ts:301")
}

/// `noSkillHitsLine` (agents-help.ts:305).
pub const NO_SKILL_HITS_LINE: &str = "no skills on skills.sh match";

/// `SKILL_HIT_COLUMNS` (agents-help.ts:306).
pub const SKILL_HIT_COLUMNS: [&str; 3] = ["SKILL", "INSTALLS", "ADD WITH"];

/// `skillHitLines(hits)` (agents-help.ts:308): `NO_SKILL_HITS_LINE` for none, else `table` of the columns and one row
/// per hit: `cell(name)`, the install count, `cell(id)`.
pub fn skill_hit_lines(hits: &[SkillHit]) -> Vec<String> {
    let _ = hits;
    todo!("agents-help.ts:308")
}

/// `onSkillsSh(skill)` (agents-help.ts:313, not exported): a skill named in three `/`-split parts (`a//c` too) is
/// one on skills.sh; any other count of parts names one already on the target.
pub fn on_skills_sh(skill: &str) -> bool {
    let _ = skill;
    todo!("agents-help.ts:313")
}

/// `skillsShPlacelessLine(skill)` (agents-help.ts:315).
pub fn skills_sh_placeless_line(skill: &str) -> String {
    let _ = skill;
    todo!("agents-help.ts:315")
}

/// `skillShown(client, skill, thread, on, project, usage, line)` (agents-help.ts:318-325). A skills.sh id with a
/// thread, a computer or a project asked (`project.project`) is refused (`skills_sh_placeless_line`, usage `usage`)
/// before any frame, else `skills.get {skill}` and its `preview`. Any other name: `agents_target(client, thread, on,
/// usage, line, project.name)`, then `skills.preview {target, name, project: true?}` (`project` only when asked).
/// The preview is read as `SkillPreview`.
pub async fn skill_shown(client: &dyn HostClient, skill: &str, thread: Option<&str>, on: Option<&str>, project: &ProjectAsked, usage: &str, line: &str, cloud_on: bool) -> Result<SkillPreview, HostError> {
    let _ = (client, skill, thread, on, project, usage, line, cloud_on);
    todo!("agents-help.ts:318-325")
}

/// `previewCutLine(kb)` (agents-help.ts:327): `(the first 64 KB of <kb> KB)`, the 64 from `SKILL_PREVIEW_BYTES / 1024`.
pub fn preview_cut_line(kb: u64) -> String {
    let _ = (kb, SKILL_PREVIEW_BYTES);
    todo!("agents-help.ts:327")
}

/// `shownText(p)` (agents-help.ts:329-332): `printable(text)`, and where `size` is over `SKILL_PREVIEW_BYTES` (65536,
/// not at it) a blank line and `preview_cut_line(ceil(size / 1024))`.
pub fn shown_text(p: &SkillPreview) -> String {
    let _ = p;
    todo!("agents-help.ts:329-332")
}

/// `skillAdded(client, skill, thread, on, agents, project, usage, line)` (agents-help.ts:334-337):
/// `agents_target(.., project.name)`, then `skills.add {target, skill, agents?, project: true?}`; `agents` is sent only
/// when non-empty, `project` only when asked. Its `added` read as `SkillAdded`.
#[allow(clippy::too_many_arguments)]
pub async fn skill_added(client: &dyn HostClient, skill: &str, thread: Option<&str>, on: Option<&str>, agents: Option<&[String]>, project: &ProjectAsked, usage: &str, line: &str, cloud_on: bool) -> Result<SkillAdded, HostError> {
    let _ = (client, skill, thread, on, agents, project, usage, line, cloud_on);
    todo!("agents-help.ts:334-337")
}

/// `isInLine(name, path)` (agents-help.ts:339).
pub fn is_in_line(name: &str, path: &str) -> String {
    let _ = (name, path);
    todo!("agents-help.ts:339")
}

/// `isInAlsoLine(name, path, copies)` (agents-help.ts:340).
pub fn is_in_also_line(name: &str, path: &str, copies: &str) -> String {
    let _ = (name, path, copies);
    todo!("agents-help.ts:340")
}

/// `agentCopyWords(agent, path)` (agents-help.ts:341): `<agent>'s <path>`.
pub fn agent_copy_words(agent: &str, path: &str) -> String {
    let _ = (agent, path);
    todo!("agents-help.ts:341")
}

/// `addedLine(skill, a)` (agents-help.ts:343-346): the name is `cell` of the last `/` part of `skill` (empty for a
/// trailing `/`); with no agent copy `is_in_line(name, cell(path))`, else `is_in_also_line` with each copy as
/// `agent_copy_words(agentName(agent), cell(path))`, joined by `, ` in the order answered.
pub fn added_line(skill: &str, a: &SkillAdded) -> String {
    let _ = (skill, a);
    todo!("agents-help.ts:343-346")
}

/// `goneFromLine(name, from)` (agents-help.ts:348).
pub fn gone_from_line(name: &str, from: &str) -> String {
    let _ = (name, from);
    todo!("agents-help.ts:348")
}

/// `turnedInLine(name, on, file)` (agents-help.ts:349): `<name> is on in <file>.` or `is off in`.
pub fn turned_in_line(name: &str, on: bool, file: &str) -> String {
    let _ = (name, on, file);
    todo!("agents-help.ts:349")
}

/// `removedLine(name, removed)` (agents-help.ts:351): `gone_from_line(cell(name), removed.map(cell).join(", "))`.
/// No folder at all reads `<name> is gone from .`.
pub fn removed_line(name: &str, removed: &[String]) -> String {
    let _ = (name, removed);
    todo!("agents-help.ts:351")
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
pub async fn skill_changed(client: &dyn HostClient, op: SkillChange, name: &str, thread: Option<&str>, on: Option<&str>, project: &ProjectAsked, turn: Option<bool>, usage: &str, line: &str, cloud_on: bool) -> Result<Vec<String>, HostError> {
    let _ = (client, op, name, thread, on, project, turn, usage, line, cloud_on);
    todo!("agents-help.ts:354-358")
}

/// `turnedLine(name, on)` (agents-help.ts:360): `<name> is on.` or `is off.`
pub fn turned_line(name: &str, on: bool) -> String {
    let _ = (name, on);
    todo!("agents-help.ts:360")
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
    let _ = (fields, given);
    todo!("agents-help.ts:376-379")
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
pub const AGENT_SET_NOTHING_LINE: &str = "wsp agents set takes --model, --effort, --access, a model to --hide, --show, --add-model or --drop-model, --order or --reset.";
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
    let _ = (client, agent, ask);
    todo!("agents-help.ts:405-425")
}

/// `PICKER_WORDS.hide` (agents-help.ts:428): `hides <models>`.
pub fn picker_hide(models: &str) -> String {
    let _ = models;
    todo!("agents-help.ts:428")
}
/// `PICKER_WORDS.order` (agents-help.ts:428): `lists <models> first`.
pub fn picker_order(models: &str) -> String {
    let _ = models;
    todo!("agents-help.ts:428")
}
/// `PICKER_WORDS.custom` (agents-help.ts:428): `adds <models>`.
pub fn picker_custom(models: &str) -> String {
    let _ = models;
    todo!("agents-help.ts:428")
}

/// `startsOnOwnLine(agent)` (agents-help.ts:429).
pub fn starts_on_own_line(agent: &str) -> String {
    let _ = agent;
    todo!("agents-help.ts:429")
}
/// `startsOnLine(agent, said)` (agents-help.ts:430).
pub fn starts_on_line(agent: &str, said: &str) -> String {
    let _ = (agent, said);
    todo!("agents-help.ts:430")
}

/// `agentDefaultsLine(agent, d)` (agents-help.ts:433-438): the words said, in order: model, effort, access (each as
/// stored, not cleaned), then the picker's hide, order, custom lists (each only when non-empty, models joined by a
/// space, through `picker_hide`, `picker_order`, `picker_custom`); none at all is `starts_on_own_line`, else
/// `starts_on_line` with the words joined by `; `. The agent is named by `agentName(agent)` (its id when the catalog has
/// none).
pub fn agent_defaults_line(agent: &str, d: &AgentDefaults) -> String {
    let _ = (agent, d);
    todo!("agents-help.ts:433-438")
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
    let _ = (client, agent);
    todo!("agents-help.ts:441-444")
}

/// `defaultAgentLine(agent)` (agents-help.ts:446): names the agent by `agentName`.
pub fn default_agent_line(agent: &str) -> String {
    let _ = agent;
    todo!("agents-help.ts:446")
}

// ---------------------------------------------------------------------------------------------------------------
// An agent's setup (agents-help.ts:448-503)
// ---------------------------------------------------------------------------------------------------------------

/// `envNameLine(named, quoted)` (agents-help.ts:448).
pub fn env_name_line(named: &str, quoted: &str) -> String {
    let _ = (named, quoted);
    todo!("agents-help.ts:448")
}
/// `ENV_NAME_FIX` (agents-help.ts:449).
pub const ENV_NAME_FIX: &str = "Name the variable alone; its value is asked for where nothing echoes it.";

/// `envNameOf(name, named)` (agents-help.ts:452-455): the name as given when it is letters, digits and `_` and does
/// not start with a digit (`EnvName`); else refused as usage with `env_name_line(named, <name as JSON>)` and
/// `ENV_NAME_FIX`. Whether the variable is one the host forbids is the host's to say.
pub fn env_name_of(name: &str, named: &str) -> Result<String, HostError> {
    let _ = (name, named);
    todo!("agents-help.ts:452-455")
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
pub const AGENT_SETUP_NOTHING_LINE: &str = "wsp agents setup takes --enable or --disable, --program, --config, --arg, --env, --unset-env or --reset.";

/// `agentSetupSet(client, agent, ask, values, usage)` (agents-help.ts:471-486). In this order: `resets_of(AGENT_SETUP_RESETS)`;
/// `agents_target(client, None, ask.on, usage, "wsp agents setup")` (so with `on` a `places.list` is sent here, and a
/// refusal below comes after it); `env_name_of(name, "--unset-env")` for each unset name; then the change: `on` from
/// `enabled`, `program`, `configDir` (`absolute_path("--config is a folder on that computer", config)`, never
/// resolved or expanded), `args` (empty is not given), each as given, null where its reset word is named and it is
/// not given; `env` = `values` then each unset name as null (a name in both keeps its first place and is null),
/// sent only when non-empty. An empty change is refused (`AGENT_SETUP_NOTHING_LINE`, fix `NOTHING_TO_SET_FIX`).
/// Then `agents.setup {placeId, agent, ..change}` with `placeId` the target's, else `HERE_PLACE_ID`; answers the
/// reply's `agent` as an `AgentRow`.
pub async fn agent_setup_set(client: &dyn HostClient, agent: &str, ask: &AgentSetupAsk, values: &IndexMap<String, String>, usage: &str) -> Result<AgentRow, HostError> {
    let _ = (client, agent, ask, values, usage);
    todo!("agents-help.ts:471-486")
}

/// `setupOnLine(name, on)` (agents-help.ts:488): `<name> is on there.` or `is off there.`
pub fn setup_on_line(name: &str, on: bool) -> String {
    let _ = (name, on);
    todo!("agents-help.ts:488")
}

/// `SETUP_WORDS` (agents-help.ts:490): the label each fact of a setup is printed under.
pub struct SetupWords {
    pub program: &'static str,
    pub config_dir: &'static str,
    pub args: &'static str,
    pub env_names: &'static str,
}
pub const SETUP_WORDS: SetupWords = SetupWords { program: "program", config_dir: "config folder", args: "launch words", env_names: "variables" };

/// `agentSetupLines(row, configMoved)` (agents-help.ts:493-503): the row's `setup` (when it has none, `{on: true,
/// envNames: []}`) as `setup_on_line(row.name, on)` (the row's own name, not the catalog's), then one line each, in
/// order, for `program` (`cell`), `configDir` (`cell`), `args` (each `cell`, joined by a space; an empty list is
/// printed as the label and nothing after it), `envNames` (only when non-empty, joined by a space); then, when
/// `config_moved` and the setup has a `configDir`, `configDirSignInLine(row.name)` and a full stop.
pub fn agent_setup_lines(row: &AgentRow, config_moved: bool) -> Vec<String> {
    let _ = (row, config_moved);
    todo!("agents-help.ts:493-503")
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
pub const PROJECT_SET_NOTHING_LINE: &str = "wsp projects set takes --agent, --model, --effort, --access, --after-worktree, --tracker or --reset.";
/// `trackerShapeLine(word)` (agents-help.ts:517): the word as JSON.
pub fn tracker_shape_line(word: &str) -> String {
    let _ = word;
    todo!("agents-help.ts:517")
}
/// `TRACKER_SHAPE_FIX` (agents-help.ts:518).
pub const TRACKER_SHAPE_FIX: &str = "Name it as owner/name, the way gh repo view does.";
/// `trackersLine(trackers)` (agents-help.ts:520): the repos joined by `, `.
pub fn trackers_line(trackers: &[String]) -> String {
    let _ = trackers;
    todo!("agents-help.ts:520")
}
/// `afterWorktreeBlankLine` (agents-help.ts:521).
pub const AFTER_WORKTREE_BLANK_LINE: &str = "--after-worktree was given a blank command, which a new worktree cannot run.";
/// `AFTER_WORKTREE_BLANK_FIX` (agents-help.ts:522).
pub const AFTER_WORKTREE_BLANK_FIX: &str = "Name the shell line a new worktree runs, or take the command away with --reset after-worktree.";
/// `afterWorktreeLine(command)` (agents-help.ts:524).
pub fn after_worktree_line(command: &str) -> String {
    let _ = command;
    todo!("agents-help.ts:524")
}
/// `noDefaultsAnsweredLine(project)` (agents-help.ts:525): the project's name as it is, not cleaned.
pub fn no_defaults_answered_line(project: &str) -> String {
    let _ = project;
    todo!("agents-help.ts:525")
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
    let _ = (client, r#ref, ask);
    todo!("agents-help.ts:528-549")
}

/// `FROM_WORDS` (agents-help.ts:552): where a resolved value came from, as a person reads it.
pub fn from_word(from: DefaultFrom) -> &'static str {
    let _ = from;
    todo!("agents-help.ts:552")
}
/// `newThreadsHeadLine(project)` (agents-help.ts:553).
pub fn new_threads_head_line(project: &str) -> String {
    let _ = project;
    todo!("agents-help.ts:553")
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
    let _ = (label, value, from);
    todo!("agents-help.ts:557")
}

/// `threadDefaultsLines(d)` (agents-help.ts:560-563): one `defaults_value_line` for the agent (shown by `agentName`
/// of its value), then for the model, the effort and the access that are present, in that order, each value as it is
/// (the access's value, not its mode) with `from_word` of where it came from.
pub fn thread_defaults_lines(d: &ThreadDefaults) -> Vec<String> {
    let _ = d;
    todo!("agents-help.ts:560-563")
}

/// `defaultsCell(d)` (agents-help.ts:566-567): `-` for none read, else the agent, the model and the access (not the
/// effort) that are present, joined by a space, each followed by ` (project)` where it came from the project.
pub fn defaults_cell(d: Option<&ThreadDefaults>) -> String {
    let _ = d;
    todo!("agents-help.ts:566-567")
}

/// `AGENTS_ON_WORDS` (agents-help.ts:572): the tool text of `on`. Plain text, so declared here.
pub const AGENTS_ON_WORDS: &str = "the computer to read, by the name wsp computers shows; this computer without it, and a thread names its own";
/// `AGENTS_READ_WORDS` (agents-help.ts:573): the tool text of a read. Plain text, so declared here.
pub const AGENTS_READ_WORDS: &str = "Read as the login the computer was added with, off each agent's config and whether its files are there: no MCP server is started and no login file is opened. A napping machine answers what stood there when it last ran, marked stale with its pauseMode (disk: the nap stopped it, memory: it naps), and is not woken.";
