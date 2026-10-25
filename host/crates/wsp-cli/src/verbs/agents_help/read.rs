// SPDX-License-Identifier: AGPL-3.0-only
//! `agents-help.ts:1-298` at 65f6b89c7: the target a line names, the report and the rows of its four lists (agents,
//! skills, servers, plugins), the writes of a plugin and of a server, and sign-in. Spec: `spec/agents-read.md`.
//!
//! Declared by the tool module and not here (zod shapes and `describe` strings of the MCP tools): `AGENTS_FRAME`
//! (agents-help.ts:119, the report's shape less its four row lists), `PluginIdIn`, `PluginAgentIn`, `PLUGIN_CHANGE_WORDS`
//! (186-189), `ServerNameIn`, `ServerAgentIn`, `ServerScopeIn`, `ServerProjectIn`, `SERVER_CHANGE_WORDS` (255-263) and
//! `SERVER_TOOLS_WORDS` (265-266). Their text is listed in the spec.

use std::sync::Arc;

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;

use wsp_wire::types::agents_report::{AgentsProject, AgentsReport, AgentsTarget, McpScope, NappingLit, PluginBrings, PluginRow, ServerToolsAnswer};
use wsp_words::exit::HostError;

use crate::place_signin::BoxSignedIn;
use crate::verbs::client::{BoxFuture, Frame, HostClient, VerbContext};

// ---------------------------------------------------------------------------------------------------------------
// Drawing rows (agents-help.ts:11-34).
// ---------------------------------------------------------------------------------------------------------------

/// What a frame of `draw_rows` answers: the value `--json` prints and the rows a table prints.
pub struct Rows {
    pub value: Value,
    pub rows: Vec<String>,
}

/// The frame of `draw_rows`: handed the one socket, answers one reading of the host.
pub type RowFrame<'a> = &'a (dyn Fn(Arc<dyn HostClient>) -> BoxFuture<'static, Result<Rows, HostError>> + Send + Sync);

/// `drawRows(ctx, words, frame)` (agents-help.ts:11-34): a verb's rows, drawn once or redrawn where they stand.
/// Without `--watch`: one `ctx.client()` dial, the frame called once with it, `ctx.out.emit(value, rows joined by "\n")`
/// (an empty list of rows is an empty line), exit 0; a refusal of the frame rides out and nothing is printed.
/// With `--watch`: `watch_on(words, json, io.redraw)` is read first, before any dial, and a refusal
/// (`--json`, else no terminal) is thrown with no frame called; else one dial, then `watch_block` over the frame's
/// `rows`, handed `ctx.signals` where there is one, exit 0. Every frame reuses the one socket.
pub async fn draw_rows(ctx: &VerbContext, words: &str, frame: RowFrame<'_>) -> Result<i32, HostError> {
    let _ = (ctx, words, frame);
    todo!("agents-help.ts:11-34")
}

// ---------------------------------------------------------------------------------------------------------------
// The target a line names (agents-help.ts:36-110).
// ---------------------------------------------------------------------------------------------------------------

/// `aimedBothLine` (agents-help.ts:36).
pub const AIMED_BOTH_LINE: &str = "A thread already names its computer; give the thread or --on <computer>, not both.";

/// `aimedUsage(tool)` (agents-help.ts:39): `<tool> takes a thread or on, not both`.
pub fn aimed_usage(tool: &str) -> String {
    let _ = tool;
    todo!("agents-help.ts:39")
}

/// `agentsTarget(client, thread, on, usage, line, project?)` (agents-help.ts:47-52). Both a thread and a computer is a usage
/// refusal `AIMED_BOTH_LINE` with fix `usage`, before any frame. A thread: `thread_at_in(client, thread, line, true, cloud_on)` (`cloud_on` is the TS's `CLOUD_ON`, which only a thread's refusal reads; every helper below that names a target takes it last, as `verbs_for(cloud_on)` does, so a test runs both settings in one process; its
/// frames and refusals as `turns_help::target`), answers `{workspaceId}` (a project asked is not carried). A computer:
/// `place_named(client, on)` (one `places.list`), answers `{placeId, project?}` (`project` carried when given, an empty
/// string too). Neither: `{placeId: "here"}` with no frame.
pub async fn agents_target(client: &dyn HostClient, thread: Option<&str>, on: Option<&str>, usage: &str, line: &str, project: Option<&str>, cloud_on: bool) -> Result<AgentsTarget, HostError> {
    let _ = (client, thread, on, usage, line, project, cloud_on);
    todo!("agents-help.ts:47-52")
}

/// `ProjectAsked` (agents-help.ts:82-85): `--project` read against the target: not asked (`project` false), the thread's
/// own project (true, no name), or a project of a computer by name. Serialized as `{project, name?}`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ProjectAsked {
    pub project: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// `projectUnnamedLine` (agents-help.ts:87).
pub const PROJECT_UNNAMED_LINE: &str = "--project with --on names the project: --project <name>, as wsp projects shows it.";

/// `projectOffComputerLine(value)` (agents-help.ts:88).
pub fn project_off_computer_line(value: &str) -> String {
    let _ = value;
    todo!("agents-help.ts:88")
}

/// `toolsProjectBareLine` (agents-help.ts:89).
pub const TOOLS_PROJECT_BARE_LINE: &str = "--project on wsp servers tools names a project on --on <computer>; a thread finds its own project's servers.";

/// What `--project` or a tool's `project` is given as (agents-help.ts:93): a flag or a name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectValue {
    Flag(bool),
    Name(String),
}

/// `projectAsked(value, thread, on, usage)` (agents-help.ts:93-101): none or `false` is `{project: false}`. `true` or
/// the empty string: a usage refusal `PROJECT_UNNAMED_LINE` (fix `usage`) where a computer is named, else
/// `{project: true}`. A name: a usage refusal `project_off_computer_line(name)` (fix `usage`) with no computer, else
/// `{project: true, name}`. A thread given does not change the answer.
pub fn project_asked(value: Option<&ProjectValue>, thread: Option<&str>, on: Option<&str>, usage: &str) -> Result<ProjectAsked, HostError> {
    let _ = (value, thread, on, usage);
    todo!("agents-help.ts:93-101")
}

/// `toolsProject(value, thread, on, usage)` (agents-help.ts:105-109): none is none; the empty string a usage refusal
/// `TOOLS_PROJECT_BARE_LINE` (fix `usage`), computer or not; any other word goes through `project_asked` as a name and
/// answers its `name`.
pub fn tools_project(value: Option<&str>, thread: Option<&str>, on: Option<&str>, usage: &str) -> Result<Option<String>, HostError> {
    let _ = (value, thread, on, usage);
    todo!("agents-help.ts:105-109")
}

// ---------------------------------------------------------------------------------------------------------------
// The report and its rows (agents-help.ts:112-175).
// ---------------------------------------------------------------------------------------------------------------

/// `agentsReport(client, thread, on, usage, line)` (agents-help.ts:113-116): `agents_target` (no project), then one
/// `agents.read {target}`; its `report` read as an `AgentsReport` (keys it does not know dropped, the rest in the
/// schema's order).
pub async fn agents_report(client: &dyn HostClient, thread: Option<&str>, on: Option<&str>, usage: &str, line: &str, cloud_on: bool) -> Result<AgentsReport, HostError> {
    let _ = (client, thread, on, usage, line, cloud_on);
    todo!("agents-help.ts:113-116")
}

/// What `reportFacts` answers (agents-help.ts:122): the report's own facts beside the rows one list prints, in this
/// order, the absent optional ones left out. Serialized with camelCase keys.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportFacts {
    pub target: AgentsTarget,
    pub home: String,
    pub user: String,
    pub read_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale: Option<NappingLit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pause_mode: Option<wsp_wire::wire::capabilities::PauseMode>,
    pub refused: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projects: Option<Vec<AgentsProject>>,
}

/// `reportFacts(r)` (agents-help.ts:122-124): `target`, `home`, `user`, `readAt`, `stale` and `pauseMode` where present,
/// `refused`, `projects` where present (an empty list too). Never the rows or `reach`.
pub fn report_facts(r: &AgentsReport) -> ReportFacts {
    let _ = r;
    todo!("agents-help.ts:122-124")
}

/// `reportTail(r)` (agents-help.ts:128-130): the lines under every list: `<napped_word(pauseMode)>: this is what stood
/// there when it last ran` where `stale` is `napping`, then one `refused: <cell(line)>` per refused reader. Private:
/// reached through the four row functions.
#[allow(dead_code)]
fn report_tail(r: &AgentsReport) -> Vec<String> {
    let _ = r;
    todo!("agents-help.ts:128-130")
}

/// `agentRowLines(r)` (agents-help.ts:132-136): `table` of the head `AGENT, VERSION, LATEST, SIGN-IN, WSP TOOLS, UPDATE,
/// PATH` and one row per agent (the head alone for none), then the tail. Cells: the name; the version, else `installs on
/// first run`, else `version unreadable`, else `-`; the latest or `-`; the sign-in word (`not found` where not
/// installed, `sign-in unknown` where unknown, else `agent_sign_in_word`); `yes` or `no` for wsp tools; `<to>: <command>`
/// or `-`; the path or `-`.
pub fn agent_row_lines(r: &AgentsReport) -> Vec<String> {
    let _ = r;
    todo!("agents-help.ts:132-136")
}

/// `scopeWord(row)` (agents-help.ts:140): the row's scope, or with a project `local <name>` for a local scope and `project
/// <name>` for any other, the name through `cell`. Private: reached through the skill, server and plugin rows.
#[allow(dead_code)]
fn scope_word(scope: &str, project: Option<&AgentsProject>) -> String {
    let _ = (scope, project);
    todo!("agents-help.ts:140")
}

/// `skillRowLines(r)` (agents-help.ts:142-145): `no skills` for none, else `table` of `SKILL, KIND, WHERE` (name through
/// `cell`, `scope_word`, each folder's path through `cell`, with ` -> <link>` where it links, joined by `, `), then the tail.
pub fn skill_row_lines(r: &AgentsReport) -> Vec<String> {
    let _ = r;
    todo!("agents-help.ts:142-145")
}

/// `serverRowLines(r)` (agents-help.ts:147-152): `no MCP servers` for none, else `table` of `SERVER, AGENT, SCOPE, REACHED
/// BY, FILE, STATE`: the name, `agent_name(agent)`, `scope_word`, `stdio <line>` or `http <host>`, the file or `every
/// launch`, and the state: `on every thread` for a launch row, else `disabled`, else `off` where an off sentence is set,
/// else the auth word, then `, not in recipe` where `inRecipe` is false. Then one line per distinct off sentence of the
/// enabled servers (through `cell`, in row order), then the tail.
pub fn server_row_lines(r: &AgentsReport) -> Vec<String> {
    let _ = r;
    todo!("agents-help.ts:147-152")
}

/// `PLUGIN_KIND_WORDS` (agents-help.ts:155-163): each kind a plugin brings by its field, with its word for one and for
/// many, in this order.
pub const PLUGIN_KIND_WORDS: [(&str, &str, &str); 7] = [
    ("skills", "skill", "skills"),
    ("commands", "command", "commands"),
    ("subagents", "subagent", "subagents"),
    ("hooks", "hook", "hooks"),
    ("servers", "tool server", "tool servers"),
    ("lsp", "language server", "language servers"),
    ("apps", "app", "apps"),
];

/// `broughtWords(b)` (agents-help.ts:166-169): `<n> <word>` per kind that brings any, in the table's order, joined by
/// `, `; `-` for none.
pub fn brought_words(b: &PluginBrings) -> String {
    let _ = b;
    todo!("agents-help.ts:166-169")
}

/// `pluginRowLines(r)` (agents-help.ts:171-175): `no plugins` for none (an absent list too), else `table` of `PLUGIN,
/// AGENT, SCOPE, STATE, VERSION, BRINGS`: the id through `cell`, `agent_name`, `scope_word`, the state (`on` or `off`,
/// prefixed `missing, ` where `missing` is set), the version or `-`, `brought_words`; then the tail.
pub fn plugin_row_lines(r: &AgentsReport) -> Vec<String> {
    let _ = r;
    todo!("agents-help.ts:171-175")
}

// ---------------------------------------------------------------------------------------------------------------
// Plugins and servers written (agents-help.ts:177-253).
// ---------------------------------------------------------------------------------------------------------------

/// The `ask` of `pluginChanged` (agents-help.ts:178): keys in the order `agent`, `plugin`, `on`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PluginChange {
    pub agent: String,
    pub plugin: String,
    pub on: bool,
}

/// `pluginChanged(client, ask, thread, on, usage, line)` (agents-help.ts:178-181): `agents_target` (no project), then one
/// `plugins.toggle {target, agent, plugin, on}`; its `plugin` read as a `PluginRow` (unknown keys dropped).
pub async fn plugin_changed(client: &dyn HostClient, ask: &PluginChange, thread: Option<&str>, on: Option<&str>, usage: &str, line: &str, cloud_on: bool) -> Result<PluginRow, HostError> {
    let _ = (client, ask, thread, on, usage, line, cloud_on);
    todo!("agents-help.ts:178-181")
}

/// `pluginTurnedWords(id, on, agent)` (agents-help.ts:183): `<id> is on|off for <agent>; its next turn loads|leaves out
/// what it brings.`
pub fn plugin_turned_words(id: &str, on: bool, agent: &str) -> String {
    let _ = (id, on, agent);
    todo!("agents-help.ts:183")
}

/// `pluginTurnedLine(row)` (agents-help.ts:184): `plugin_turned_words(id, on, agent_name(agent))`.
pub fn plugin_turned_line(row: &PluginRow) -> String {
    let _ = row;
    todo!("agents-help.ts:184")
}

/// `SERVER_TOOL_COLUMNS` (agents-help.ts:191).
pub const SERVER_TOOL_COLUMNS: [&str; 2] = ["TOOL", "DESCRIPTION"];

/// `toolLines(name, a)` (agents-help.ts:194-199): the head `<name>: <auth>`, with `, <agent_name(holder)> holds the
/// sign-in` where a holder is named. Then `refused: <refused>` where the server refused (and nothing more); else nothing
/// where no tools were asked for; else `no tools` for an empty list, else `table` of `TOOL, DESCRIPTION` with the first
/// line of each description (`-` where none; an empty one stays empty; split on `\n` only).
pub fn tool_lines(name: &str, a: &ServerToolsAnswer) -> Vec<String> {
    let _ = (name, a);
    todo!("agents-help.ts:194-199")
}

/// `serverToolsOf(client, name, agent, thread, on, refresh, usage, line, project?)` (agents-help.ts:201-204):
/// `agents_target(.., project)`, then one `servers.tools {target, agent, name, refresh?}` (`refresh` only when true); its
/// `answer` read as a `ServerToolsAnswer` (unknown keys dropped).
pub async fn server_tools_of(client: &dyn HostClient, name: &str, agent: &str, thread: Option<&str>, on: Option<&str>, refresh: bool, usage: &str, line: &str, project: Option<&str>, cloud_on: bool) -> Result<ServerToolsAnswer, HostError> {
    let _ = (client, name, agent, thread, on, refresh, usage, line, project, cloud_on);
    todo!("agents-help.ts:201-204")
}

/// The op of `serverChanged` (agents-help.ts:207).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerOp {
    Add,
    Remove,
    Toggle,
}

impl ServerOp {
    pub fn op(self) -> &'static str {
        match self {
            ServerOp::Add => "servers.add",
            ServerOp::Remove => "servers.remove",
            ServerOp::Toggle => "servers.toggle",
        }
    }
}

/// What a server's write answers (agents-help.ts:207): the file written, nothing else of the reply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ServerFile {
    pub file: String,
}

/// `serverChanged(client, op, body, thread, on, usage, line, project?)` (agents-help.ts:207-210): `agents_target(..,
/// project)`, then one `<op> {target, ...body}` (the body's keys in its order after `target`); the reply read as
/// `{file}` and cut to it.
pub async fn server_changed(client: &dyn HostClient, op: ServerOp, body: &Frame, thread: Option<&str>, on: Option<&str>, usage: &str, line: &str, project: Option<&str>, cloud_on: bool) -> Result<ServerFile, HostError> {
    let _ = (client, op, body, thread, on, usage, line, project, cloud_on);
    todo!("agents-help.ts:207-210")
}

/// `unsetVariableLine(variable)` (agents-help.ts:212).
pub fn unset_variable_line(variable: &str) -> String {
    let _ = variable;
    todo!("agents-help.ts:212")
}

/// `notHeaderLine(pair)` (agents-help.ts:213).
pub fn not_header_line(pair: &str) -> String {
    let _ = pair;
    todo!("agents-help.ts:213")
}

/// What `serverValues` answers (agents-help.ts:217): `env` and `headers`, each only when its list was not empty. Each map is
/// ordered as a JS object orders its keys: names that are canonical array indexes (`0`, `7`, `10`, no sign, no leading
/// zero) first in ascending numeric order, every other name after them in the order first met; a repeated name keeps its
/// first place and its last value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ServerValues {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<IndexMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<IndexMap<String, String>>,
}

/// `serverValues(env, names, headers, usage)` (agents-help.ts:217-232): the variables named, read off `env` in order (the
/// first one `env` lacks is a usage refusal `unset_variable_line` with fix `usage`; an empty value is a value), then each
/// header `Name=VAR` in order (an `=` at the front or the end, or none, is `not_header_line(pair)`; the split is at the
/// first `=`, so the rest, equals signs included, is the variable's name). The variables are read before any header.
pub fn server_values(env: &IndexMap<String, String>, names: &[String], headers: &[String], usage: &str) -> Result<ServerValues, HostError> {
    let _ = (env, names, headers, usage);
    todo!("agents-help.ts:217-232")
}

/// What `serverCommand` answers (agents-help.ts:235): the program and its arguments, or neither.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ServerCommand {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

/// `serverCommand(line, usage)` (agents-help.ts:235-241): nothing without a line; else `command_words(line)` (a line with
/// a quote it never closes is a usage refusal `unclosed_quote_refusal` with fix `usage`); the first word is the command
/// (`""` where there are none), the rest its arguments (an empty list included).
pub fn server_command(line: Option<&str>, usage: &str) -> Result<ServerCommand, HostError> {
    let _ = (line, usage);
    todo!("agents-help.ts:235-241")
}

/// `projectScopeLine(word)` (agents-help.ts:243).
pub fn project_scope_line(word: &str) -> String {
    let _ = word;
    todo!("agents-help.ts:243")
}

/// What `serverScope` answers (agents-help.ts:247): the scope, or none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ServerScope {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<McpScope>,
}

/// `serverScope(word, usage, project)` (agents-help.ts:247-253): no word is `project` where `--project` was asked, else no
/// scope. A word that is not user, home, local or project (case as given) is a usage refusal `--scope is user, home, local
/// or project, not <word>.` with fix `usage`, read before `--project` is looked at. With `--project` asked only `project`
/// and `local` stand; any other word is a usage refusal `project_scope_line(word)`.
pub fn server_scope(word: Option<&str>, usage: &str, project: &ProjectAsked) -> Result<ServerScope, HostError> {
    let _ = (word, usage, project);
    todo!("agents-help.ts:247-253")
}

// ---------------------------------------------------------------------------------------------------------------
// The wsp tools and sign-in (agents-help.ts:268-298).
// ---------------------------------------------------------------------------------------------------------------

/// `toolsAddedLine(agent, file)` (agents-help.ts:268): `<agent> now has the wsp tools: <file>`.
pub fn tools_added_line(agent: &str, file: &str) -> String {
    let _ = (agent, file);
    todo!("agents-help.ts:268")
}

/// `addTools(client, agent)` (agents-help.ts:271-273): one `agents.addTools {target: {placeId: "here"}, agent}` (no
/// `places.list`), the reply read as `{file}` and cut to it.
pub async fn add_tools(client: &dyn HostClient, agent: &str) -> Result<ServerFile, HostError> {
    let _ = (client, agent);
    todo!("agents-help.ts:271-273")
}

/// The `ask` of `signInHere` (agents-help.ts:276): the agent, and the name of a server where it is one's sign-in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignInAsk {
    pub agent: String,
    pub name: Option<String>,
}

/// `signInHere(ctx, target, ask, where)` (agents-help.ts:276-292): interface only, it needs a pty relay (a spec note).
/// One `ctx.client()` dial, `agents.signInLine {target, agent, name?}` (its `line` read as a `SignInLine`), the target's
/// link (`target_link`), then `relay_sign_in` with `where`, the link, the agent only where no server is named, the line,
/// `ctx.terminal` or this process's terminal, and `ctx.open` or an opener that opens nothing; the link is closed on every
/// road out.
pub async fn sign_in_here(ctx: &VerbContext, target: &AgentsTarget, ask: &SignInAsk, r#where: &str) -> Result<BoxSignedIn, HostError> {
    let _ = (ctx, target, ask, r#where);
    todo!("agents-help.ts:276-292")
}

/// `signedInLine(what, where, answer)` (agents-help.ts:295-298): `<what> is signed in on <where>` and ` (<detail>)` where
/// a detail is set (an empty one too), then `.`; or `<what> is not signed in on <where>` and `: <said>` where it said
/// something (an empty one too), then `.`.
pub fn signed_in_line(what: &str, r#where: &str, answer: &BoxSignedIn) -> String {
    let _ = (what, r#where, answer);
    todo!("agents-help.ts:295-298")
}
