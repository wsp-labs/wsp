// SPDX-License-Identifier: AGPL-3.0-only
//! `agents-help.ts:1-298` at 65f6b89c7: the target a line names, the report and the rows of its four lists (agents,
//! skills, servers, plugins), the writes of a plugin and of a server, and sign-in. Spec: `spec/agents-read.md`.
//!
//! Declared by the tool module and not here (zod shapes and `describe` strings of the MCP tools): `AGENTS_FRAME`
//! (agents-help.ts:119, the report's shape less its four row lists), `PluginIdIn`, `PluginAgentIn`, `PLUGIN_CHANGE_WORDS`
//! (186-189), `ServerNameIn`, `ServerAgentIn`, `ServerScopeIn`, `ServerProjectIn`, `SERVER_CHANGE_WORDS` (255-263) and
//! `SERVER_TOOLS_WORDS` (265-266). Their text is listed in the spec.

use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use serde_json::{json, Value};

use wsp_catalog::catalog::agent_name;
use wsp_wire::types::agents_report::{
    AgentRow, AgentRowSignIn, AgentsProject, AgentsReport, AgentsTarget, AgentsTargetV0, AgentsTargetV1, McpAuth, McpRow, McpRowTransport,
    McpScope, NappingLit, PluginBrings, PluginRow, PluginScope, ServerToolsAnswer, SignInLine, SkillRow, SkillScope,
    UNCLOSED_QUOTE_REFUSAL,
};
use wsp_wire::types::check::{parse, Check};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::session_events::{FIRST_RUN_WORD, VERSION_UNREAD_WORD};
use wsp_words::agents_report::{command_words, napped_word};
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::words::places::agent_sign_in_word;

use crate::place_signin::{relay_sign_in, target_link, BoxSignIn, BoxSignedIn, PlaceLink};
use crate::verbs::client::{cell, flag_set, other_version, place_named, table, BoxFuture, Frame, HostClient, VerbContext};
use crate::verbs::turns_help::thread_at_in;
use crate::watch::{watch_block, watch_on, Lines, WatchDeps, WatchOn};

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
    if !flag_set(&ctx.flags, "watch") {
        let client = ctx.client(None).await?;
        let rows = frame(client).await?;
        ctx.out.emit(&rows.value, Some(&rows.rows.join("\n")));
        return Ok(0);
    }
    let redraw = match watch_on(words, flag_set(&ctx.flags, "json"), ctx.io.redraw()) {
        WatchOn::Redraw(redraw) => redraw,
        WatchOn::Refusal(refusal) => return Err(refusal),
    };
    let client = ctx.client(None).await?;
    // The frames are asked over a channel: a watch's lines must be 'static, and the frame is only borrowed.
    let (asks, mut asked) = tokio::sync::mpsc::unbounded_channel::<tokio::sync::oneshot::Sender<Result<Vec<String>, HostError>>>();
    let lines: Lines = Arc::new(move || {
        let asks = asks.clone();
        Box::pin(async move {
            let (reply, answer) = tokio::sync::oneshot::channel();
            asks.send(reply).expect("the frame loop runs while the watch asks it");
            answer.await.expect("the frame loop answers every ask it takes")
        })
    });
    let serve = async {
        while let Some(reply) = asked.recv().await {
            let rows = frame(client.clone()).await.map(|drawn| drawn.rows);
            let _ = reply.send(rows);
        }
    };
    let deps = WatchDeps { redraw, every_ms: None, signals: ctx.signals.clone(), until: None };
    let (watched, ()) = tokio::join!(watch_block(lines, deps), serve);
    watched?;
    Ok(0)
}

// ---------------------------------------------------------------------------------------------------------------
// The target a line names (agents-help.ts:36-110).
// ---------------------------------------------------------------------------------------------------------------

/// `aimedBothLine` (agents-help.ts:36).
pub const AIMED_BOTH_LINE: &str = "A thread already names its computer; give the thread or --on <computer>, not both.";

/// `aimedUsage(tool)` (agents-help.ts:39): `<tool> takes a thread or on, not both`.
pub fn aimed_usage(tool: &str) -> String {
    format!("{tool} takes a thread or on, not both")
}

/// `agentsTarget(client, thread, on, usage, line, project?)` (agents-help.ts:47-52). Both a thread and a computer is a usage
/// refusal `AIMED_BOTH_LINE` with fix `usage`, before any frame. A thread: `thread_at_in(client, thread, line, true, cloud_on)` (`cloud_on` is the TS's `CLOUD_ON`, which only a thread's refusal reads; every helper below that names a target takes it last, as `verbs_for(cloud_on)` does, so a test runs both settings in one process; its
/// frames and refusals as `turns_help::target`), answers `{workspaceId}` (a project asked is not carried). A computer:
/// `place_named(client, on)` (one `places.list`), answers `{placeId, project?}` (`project` carried when given, an empty
/// string too). Neither: `{placeId: "here"}` with no frame.
pub async fn agents_target(
    client: &dyn HostClient,
    thread: Option<&str>,
    on: Option<&str>,
    usage: &str,
    line: &str,
    project: Option<&str>,
    cloud_on: bool,
) -> Result<AgentsTarget, HostError> {
    if thread.is_some() && on.is_some() {
        return Err(usage_refusal(AIMED_BOTH_LINE, usage));
    }
    if let Some(thread) = thread {
        let (_, workspace) = thread_at_in(client, thread, line, true, cloud_on).await?;
        return Ok(AgentsTarget::V1(AgentsTargetV1 { workspace_id: workspace.id }));
    }
    if let Some(on) = on {
        let place = place_named(client, on).await?;
        return Ok(AgentsTarget::V0(AgentsTargetV0 { place_id: place.id, project: project.map(str::to_owned) }));
    }
    Ok(AgentsTarget::V0(AgentsTargetV0 { place_id: HERE_PLACE_ID.to_owned(), project: None }))
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
    format!("--project {value} names a project on a computer, which --on names; a thread's own project is --project alone.")
}

/// `toolsProjectBareLine` (agents-help.ts:89).
pub const TOOLS_PROJECT_BARE_LINE: &str =
    "--project on wsp servers tools names a project on --on <computer>; a thread finds its own project's servers.";

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
    let _ = thread;
    match value {
        None | Some(ProjectValue::Flag(false)) => Ok(ProjectAsked { project: false, name: None }),
        Some(ProjectValue::Flag(true)) => bare_project(on, usage),
        Some(ProjectValue::Name(name)) if name.is_empty() => bare_project(on, usage),
        Some(ProjectValue::Name(name)) => {
            if on.is_none() {
                return Err(usage_refusal(&project_off_computer_line(name), usage));
            }
            Ok(ProjectAsked { project: true, name: Some(name.clone()) })
        }
    }
}

fn bare_project(on: Option<&str>, usage: &str) -> Result<ProjectAsked, HostError> {
    if on.is_some() {
        return Err(usage_refusal(PROJECT_UNNAMED_LINE, usage));
    }
    Ok(ProjectAsked { project: true, name: None })
}

/// `toolsProject(value, thread, on, usage)` (agents-help.ts:105-109): none is none; the empty string a usage refusal
/// `TOOLS_PROJECT_BARE_LINE` (fix `usage`), computer or not; any other word goes through `project_asked` as a name and
/// answers its `name`.
pub fn tools_project(value: Option<&str>, thread: Option<&str>, on: Option<&str>, usage: &str) -> Result<Option<String>, HostError> {
    match value {
        None => Ok(None),
        Some("") => Err(usage_refusal(TOOLS_PROJECT_BARE_LINE, usage)),
        Some(name) => Ok(project_asked(Some(&ProjectValue::Name(name.to_owned())), thread, on, usage)?.name),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The report and its rows (agents-help.ts:112-175).
// ---------------------------------------------------------------------------------------------------------------

/// `agentsReport(client, thread, on, usage, line)` (agents-help.ts:113-116): `agents_target` (no project), then one
/// `agents.read {target}`; its `report` read as an `AgentsReport` (keys it does not know dropped, the rest in the
/// schema's order).
pub async fn agents_report(
    client: &dyn HostClient,
    thread: Option<&str>,
    on: Option<&str>,
    usage: &str,
    line: &str,
    cloud_on: bool,
) -> Result<AgentsReport, HostError> {
    let target = agents_target(client, thread, on, usage, line, None, cloud_on).await?;
    let reply = client.request("agents.read", target_frame(&target)).await?;
    read_reply(&reply, "report", "agents.read")
}

/// The one key of a write or a read frame: `{target}`.
pub(super) fn target_frame(target: &AgentsTarget) -> Frame {
    let mut params = Frame::new();
    params.insert("target".to_owned(), json!(target));
    params
}

/// A reply's field read into the type its shape names (zod's parse): a field the reply lacks or shapes otherwise is the
/// other-version refusal the host's answer earns.
pub(super) fn read_reply<T: Check + serde::de::DeserializeOwned>(reply: &Frame, key: &str, op: &str) -> Result<T, HostError> {
    parse::<T>(reply.get(key).unwrap_or(&Value::Null)).map_err(|_| HostError::new(other_version(op)))
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
    ReportFacts {
        target: r.target.clone(),
        home: r.home.clone(),
        user: r.user.clone(),
        read_at: r.read_at.clone(),
        stale: r.stale,
        pause_mode: r.pause_mode,
        refused: r.refused.clone(),
        projects: r.projects.clone(),
    }
}

/// `reportTail(r)` (agents-help.ts:128-130): the lines under every list: `<napped_word(pauseMode)>: this is what stood
/// there when it last ran` where `stale` is `napping`, then one `refused: <cell(line)>` per refused reader. Private:
/// reached through the four row functions.
fn report_tail(r: &AgentsReport) -> Vec<String> {
    let napping = r.stale.map(|_| format!("{}: this is what stood there when it last ran", napped_word(r.pause_mode.as_ref())));
    napping.into_iter().chain(r.refused.iter().map(|line| format!("refused: {}", cell(line)))).collect()
}

/// `agentRowLines(r)` (agents-help.ts:132-136): `table` of the head `AGENT, VERSION, LATEST, SIGN-IN, WSP TOOLS, UPDATE,
/// PATH` and one row per agent (the head alone for none), then the tail. Cells: the name; the version, else `installs on
/// first run`, else `version unreadable`, else `-`; the latest or `-`; the sign-in word (`not found` where not
/// installed, `sign-in unknown` where unknown, else `agent_sign_in_word`); `yes` or `no` for wsp tools; `<to>: <command>`
/// or `-`; the path or `-`.
pub fn agent_row_lines(r: &AgentsReport) -> Vec<String> {
    let word = |a: &AgentRow| -> String {
        if !a.installed {
            return "not found".to_owned();
        }
        match a.sign_in {
            AgentRowSignIn::AgentSignInState(state) => agent_sign_in_word(state),
            AgentRowSignIn::V1(_) => "sign-in unknown".to_owned(),
        }
    };
    let version = |a: &AgentRow| -> String {
        match &a.version {
            Some(version) => version.clone(),
            None if a.installs_on_first_run.is_some() => FIRST_RUN_WORD.to_owned(),
            None if a.version_unread.is_some() => VERSION_UNREAD_WORD.to_owned(),
            None => "-".to_owned(),
        }
    };
    let update = |a: &AgentRow| -> String { a.update.as_ref().map_or_else(|| "-".to_owned(), |u| format!("{}: {}", u.to, u.command)) };
    let mut rows = vec![["AGENT", "VERSION", "LATEST", "SIGN-IN", "WSP TOOLS", "UPDATE", "PATH"].map(str::to_owned).to_vec()];
    for a in &r.agents {
        rows.push(vec![
            a.name.clone(),
            version(a),
            a.latest.clone().unwrap_or_else(|| "-".to_owned()),
            word(a),
            if a.wsp_tools { "yes" } else { "no" }.to_owned(),
            update(a),
            a.path.clone().unwrap_or_else(|| "-".to_owned()),
        ]);
    }
    let mut lines = table(&rows);
    lines.extend(report_tail(r));
    lines
}

/// `scopeWord(row)` (agents-help.ts:140): the row's scope, or with a project `local <name>` for a local scope and `project
/// <name>` for any other, the name through `cell`. Private: reached through the skill, server and plugin rows.
fn scope_word(scope: &str, project: Option<&AgentsProject>) -> String {
    match project {
        Some(project) => format!("{} {}", if scope == "local" { "local" } else { "project" }, cell(&project.name)),
        None => scope.to_owned(),
    }
}

fn mcp_scope_word(scope: McpScope) -> &'static str {
    match scope {
        McpScope::User => "user",
        McpScope::Home => "home",
        McpScope::Local => "local",
        McpScope::Project => "project",
    }
}

fn skill_scope_word(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::User => "user",
        SkillScope::Project => "project",
        SkillScope::Plugin => "plugin",
    }
}

fn plugin_scope_word(scope: PluginScope) -> &'static str {
    match scope {
        PluginScope::User => "user",
        PluginScope::Project => "project",
        PluginScope::Local => "local",
    }
}

fn auth_word(auth: McpAuth) -> &'static str {
    match auth {
        McpAuth::Open => "open",
        McpAuth::EnvKey => "env-key",
        McpAuth::Connected => "connected",
        McpAuth::SignedIn => "signed-in",
        McpAuth::NeedsSignIn => "needs-sign-in",
        McpAuth::Failed => "failed",
        McpAuth::Unknown => "unknown",
    }
}

/// `skillRowLines(r)` (agents-help.ts:142-145): `no skills` for none, else `table` of `SKILL, KIND, WHERE` (name through
/// `cell`, `scope_word`, each folder's path through `cell`, with ` -> <link>` where it links, joined by `, `), then the tail.
pub fn skill_row_lines(r: &AgentsReport) -> Vec<String> {
    let where_of = |s: &SkillRow| -> String {
        let folders: Vec<String> = s
            .paths
            .iter()
            .map(|p| match &p.link_to {
                None => cell(&p.path),
                Some(link) => format!("{} -> {}", cell(&p.path), cell(link)),
            })
            .collect();
        folders.join(", ")
    };
    let mut lines = if r.skills.is_empty() {
        vec!["no skills".to_owned()]
    } else {
        let mut rows = vec![["SKILL", "KIND", "WHERE"].map(str::to_owned).to_vec()];
        for s in &r.skills {
            rows.push(vec![cell(&s.name), scope_word(skill_scope_word(s.scope), s.project.as_ref()), where_of(s)]);
        }
        table(&rows)
    };
    lines.extend(report_tail(r));
    lines
}

/// `serverRowLines(r)` (agents-help.ts:147-152): `no MCP servers` for none, else `table` of `SERVER, AGENT, SCOPE, REACHED
/// BY, FILE, STATE`: the name, `agent_name(agent)`, `scope_word`, `stdio <line>` or `http <host>`, the file or `every
/// launch`, and the state: `on every thread` for a launch row, else `disabled`, else `off` where an off sentence is set,
/// else the auth word, then `, not in recipe` where `inRecipe` is false. Then one line per distinct off sentence of the
/// enabled servers (through `cell`, in row order), then the tail.
pub fn server_row_lines(r: &AgentsReport) -> Vec<String> {
    let reach = |s: &McpRow| -> String {
        match &s.transport {
            McpRowTransport::Stdio(t) => format!("stdio {}", t.line),
            McpRowTransport::Http(t) => format!("http {}", t.host),
        }
    };
    let state = |s: &McpRow| -> String {
        if s.launch.is_some() {
            return "on every thread".to_owned();
        }
        let first = if !s.enabled {
            "disabled".to_owned()
        } else if s.off.is_some() {
            "off".to_owned()
        } else {
            auth_word(s.auth).to_owned()
        };
        let mut parts = vec![first];
        if s.in_recipe == Some(false) {
            parts.push("not in recipe".to_owned());
        }
        parts.join(", ")
    };
    let off: IndexSet<String> = r.servers.iter().filter(|s| s.enabled).filter_map(|s| s.off.as_deref()).map(cell).collect();
    let mut lines = if r.servers.is_empty() {
        vec!["no MCP servers".to_owned()]
    } else {
        let mut rows = vec![["SERVER", "AGENT", "SCOPE", "REACHED BY", "FILE", "STATE"].map(str::to_owned).to_vec()];
        for s in &r.servers {
            rows.push(vec![
                s.name.clone(),
                agent_name(&s.agent),
                scope_word(mcp_scope_word(s.scope), s.project.as_ref()),
                reach(s),
                s.file.clone().unwrap_or_else(|| "every launch".to_owned()),
                state(s),
            ]);
        }
        table(&rows)
    };
    lines.extend(off);
    lines.extend(report_tail(r));
    lines
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
    let counts = [b.skills.len(), b.commands.len(), b.subagents.len(), b.hooks.len(), b.servers.len(), b.lsp.len(), b.apps.len()];
    let said: Vec<String> = PLUGIN_KIND_WORDS
        .iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .map(|((_, one, many), n)| format!("{n} {}", if n == 1 { one } else { many }))
        .collect();
    if said.is_empty() {
        "-".to_owned()
    } else {
        said.join(", ")
    }
}

/// `pluginRowLines(r)` (agents-help.ts:171-175): `no plugins` for none (an absent list too), else `table` of `PLUGIN,
/// AGENT, SCOPE, STATE, VERSION, BRINGS`: the id through `cell`, `agent_name`, `scope_word`, the state (`on` or `off`,
/// prefixed `missing, ` where `missing` is set), the version or `-`, `brought_words`; then the tail.
pub fn plugin_row_lines(r: &AgentsReport) -> Vec<String> {
    let plugins = r.plugins.clone().unwrap_or_default();
    let state = |p: &PluginRow| -> String {
        let on = if p.on { "on" } else { "off" };
        if p.missing.is_some() {
            format!("missing, {on}")
        } else {
            on.to_owned()
        }
    };
    let mut lines = if plugins.is_empty() {
        vec!["no plugins".to_owned()]
    } else {
        let mut rows = vec![["PLUGIN", "AGENT", "SCOPE", "STATE", "VERSION", "BRINGS"].map(str::to_owned).to_vec()];
        for p in &plugins {
            rows.push(vec![
                cell(&p.id),
                agent_name(&p.agent),
                scope_word(plugin_scope_word(p.scope), p.project.as_ref()),
                state(p),
                p.version.clone().unwrap_or_else(|| "-".to_owned()),
                brought_words(&p.brings),
            ]);
        }
        table(&rows)
    };
    lines.extend(report_tail(r));
    lines
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
pub async fn plugin_changed(
    client: &dyn HostClient,
    ask: &PluginChange,
    thread: Option<&str>,
    on: Option<&str>,
    usage: &str,
    line: &str,
    cloud_on: bool,
) -> Result<PluginRow, HostError> {
    let target = agents_target(client, thread, on, usage, line, None, cloud_on).await?;
    let mut params = target_frame(&target);
    params.insert("agent".to_owned(), json!(ask.agent));
    params.insert("plugin".to_owned(), json!(ask.plugin));
    params.insert("on".to_owned(), json!(ask.on));
    let reply = client.request("plugins.toggle", params).await?;
    read_reply(&reply, "plugin", "plugins.toggle")
}

/// `pluginTurnedWords(id, on, agent)` (agents-help.ts:183): `<id> is on|off for <agent>; its next turn loads|leaves out
/// what it brings.`
pub fn plugin_turned_words(id: &str, on: bool, agent: &str) -> String {
    let (word, verb) = if on { ("on", "loads") } else { ("off", "leaves out") };
    format!("{id} is {word} for {agent}; its next turn {verb} what it brings.")
}

/// `pluginTurnedLine(row)` (agents-help.ts:184): `plugin_turned_words(id, on, agent_name(agent))`.
pub fn plugin_turned_line(row: &PluginRow) -> String {
    plugin_turned_words(&row.id, row.on, &agent_name(&row.agent))
}

/// `SERVER_TOOL_COLUMNS` (agents-help.ts:191).
pub const SERVER_TOOL_COLUMNS: [&str; 2] = ["TOOL", "DESCRIPTION"];

/// `toolLines(name, a)` (agents-help.ts:194-199): the head `<name>: <auth>`, with `, <agent_name(holder)> holds the
/// sign-in` where a holder is named. Then `refused: <refused>` where the server refused (and nothing more); else nothing
/// where no tools were asked for; else `no tools` for an empty list, else `table` of `TOOL, DESCRIPTION` with the first
/// line of each description (`-` where none; an empty one stays empty; split on `\n` only).
pub fn tool_lines(name: &str, a: &ServerToolsAnswer) -> Vec<String> {
    let holder = a.holder.as_deref().map(|holder| format!(", {} holds the sign-in", agent_name(holder))).unwrap_or_default();
    let head = format!("{name}: {}{holder}", auth_word(a.auth));
    if let Some(refused) = &a.refused {
        return vec![head, format!("refused: {refused}")];
    }
    let Some(tools) = &a.tools else { return vec![head] };
    if tools.is_empty() {
        return vec![head, "no tools".to_owned()];
    }
    let mut rows = vec![SERVER_TOOL_COLUMNS.map(str::to_owned).to_vec()];
    for t in tools {
        let description = t.description.as_deref().unwrap_or("-");
        rows.push(vec![t.name.clone(), description.split('\n').next().unwrap_or_default().to_owned()]);
    }
    let mut lines = vec![head];
    lines.extend(table(&rows));
    lines
}

/// `serverToolsOf(client, name, agent, thread, on, refresh, usage, line, project?)` (agents-help.ts:201-204):
/// `agents_target(.., project)`, then one `servers.tools {target, agent, name, refresh?}` (`refresh` only when true); its
/// `answer` read as a `ServerToolsAnswer` (unknown keys dropped).
pub async fn server_tools_of(
    client: &dyn HostClient,
    name: &str,
    agent: &str,
    thread: Option<&str>,
    on: Option<&str>,
    refresh: bool,
    usage: &str,
    line: &str,
    project: Option<&str>,
    cloud_on: bool,
) -> Result<ServerToolsAnswer, HostError> {
    let target = agents_target(client, thread, on, usage, line, project, cloud_on).await?;
    let mut params = target_frame(&target);
    params.insert("agent".to_owned(), json!(agent));
    params.insert("name".to_owned(), json!(name));
    if refresh {
        params.insert("refresh".to_owned(), json!(true));
    }
    let reply = client.request("servers.tools", params).await?;
    read_reply(&reply, "answer", "servers.tools")
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
pub async fn server_changed(
    client: &dyn HostClient,
    op: ServerOp,
    body: &Frame,
    thread: Option<&str>,
    on: Option<&str>,
    usage: &str,
    line: &str,
    project: Option<&str>,
    cloud_on: bool,
) -> Result<ServerFile, HostError> {
    let target = agents_target(client, thread, on, usage, line, project, cloud_on).await?;
    let mut params = target_frame(&target);
    for (key, value) in body {
        params.insert(key.clone(), value.clone());
    }
    let reply = client.request(op.op(), params).await?;
    file_of(&reply, op.op())
}

/// `z.object({file: z.string()}).parse(reply)` cut to `{file}`: a reply without a string `file` is the other-version refusal.
fn file_of(reply: &Frame, op: &str) -> Result<ServerFile, HostError> {
    let file = reply.get("file").and_then(Value::as_str).ok_or_else(|| HostError::new(other_version(op)))?;
    Ok(ServerFile { file: file.to_owned() })
}

/// `unsetVariableLine(variable)` (agents-help.ts:212).
pub fn unset_variable_line(variable: &str) -> String {
    format!("{variable} is not set in this environment, so there is no value to write.")
}

/// `notHeaderLine(pair)` (agents-help.ts:213).
pub fn not_header_line(pair: &str) -> String {
    format!("{pair} is not <header>=<variable>.")
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
    let read = |variable: &str| -> Result<String, HostError> {
        env.get(variable).cloned().ok_or_else(|| usage_refusal(&unset_variable_line(variable), usage))
    };
    let mut vars = Vec::with_capacity(names.len());
    for name in names {
        vars.push((name.clone(), read(name)?));
    }
    let mut heads = Vec::with_capacity(headers.len());
    for pair in headers {
        let at = match pair.find('=') {
            Some(at) if at > 0 && at + 1 < pair.len() => at,
            _ => return Err(usage_refusal(&not_header_line(pair), usage)),
        };
        heads.push((pair[..at].to_owned(), read(&pair[at + 1..])?));
    }
    Ok(ServerValues { env: (!names.is_empty()).then(|| js_object(vars)), headers: (!headers.is_empty()).then(|| js_object(heads)) })
}

/// The keys of an object built from these pairs, in the order a JS object gives them: canonical array indexes first in
/// ascending order, every other key in the order first set; a repeated key keeps its first place and its last value.
pub(super) fn js_object<V>(pairs: Vec<(String, V)>) -> IndexMap<String, V> {
    let mut set: IndexMap<String, V> = IndexMap::new();
    for (key, value) in pairs {
        set.insert(key, value);
    }
    let mut indexed: Vec<(u64, String, V)> = Vec::new();
    let mut named: Vec<(String, V)> = Vec::new();
    for (key, value) in set {
        match array_index(&key) {
            Some(index) => indexed.push((index, key, value)),
            None => named.push((key, value)),
        }
    }
    indexed.sort_by_key(|(index, _, _)| *index);
    indexed.into_iter().map(|(_, key, value)| (key, value)).chain(named).collect()
}

fn array_index(key: &str) -> Option<u64> {
    let digits = !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit()) && (key == "0" || !key.starts_with('0'));
    let index: u64 = if digits { key.parse().ok()? } else { return None };
    (index <= 4_294_967_294).then_some(index)
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
    let Some(line) = line else { return Ok(ServerCommand::default()) };
    let Some(words) = command_words(line) else {
        return Err(usage_refusal(UNCLOSED_QUOTE_REFUSAL, usage));
    };
    let mut words = words.into_iter();
    let command = words.next().unwrap_or_default();
    Ok(ServerCommand { command: Some(command), args: Some(words.collect()) })
}

/// `projectScopeLine(word)` (agents-help.ts:243).
pub fn project_scope_line(word: &str) -> String {
    format!("--project names a project, which only the project and local scopes take, not {word}; give one of the two.")
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
    let Some(word) = word else {
        return Ok(ServerScope { scope: project.project.then_some(McpScope::Project) });
    };
    let scope = match word {
        "user" => McpScope::User,
        "home" => McpScope::Home,
        "local" => McpScope::Local,
        "project" => McpScope::Project,
        _ => return Err(usage_refusal(&format!("--scope is user, home, local or project, not {word}."), usage)),
    };
    if project.project && scope != McpScope::Project && scope != McpScope::Local {
        return Err(usage_refusal(&project_scope_line(word), usage));
    }
    Ok(ServerScope { scope: Some(scope) })
}

// ---------------------------------------------------------------------------------------------------------------
// The wsp tools and sign-in (agents-help.ts:268-298).
// ---------------------------------------------------------------------------------------------------------------

/// `toolsAddedLine(agent, file)` (agents-help.ts:268): `<agent> now has the wsp tools: <file>`.
pub fn tools_added_line(agent: &str, file: &str) -> String {
    format!("{agent} now has the wsp tools: {file}")
}

/// `addTools(client, agent)` (agents-help.ts:271-273): one `agents.addTools {target: {placeId: "here"}, agent}` (no
/// `places.list`), the reply read as `{file}` and cut to it.
pub async fn add_tools(client: &dyn HostClient, agent: &str) -> Result<ServerFile, HostError> {
    let mut params = Frame::new();
    params.insert("target".to_owned(), json!({ "placeId": HERE_PLACE_ID }));
    params.insert("agent".to_owned(), json!(agent));
    let reply = client.request("agents.addTools", params).await?;
    file_of(&reply, "agents.addTools")
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
    let client = ctx.client(None).await?;
    let mut params = target_frame(target);
    params.insert("agent".to_owned(), json!(ask.agent));
    if let Some(name) = &ask.name {
        params.insert("name".to_owned(), json!(name));
    }
    let reply = client.request("agents.signInLine", params).await?;
    let road = target_link(client.clone(), target).await?;
    let answer = relay_here(ctx, &road, ask, r#where, &reply).await;
    road.close().await;
    answer
}

/// The relay of a sign-in the host planned, on this line's terminal; the reply's `line` is read first.
async fn relay_here(ctx: &VerbContext, road: &PlaceLink, ask: &SignInAsk, r#where: &str, reply: &Frame) -> Result<BoxSignedIn, HostError> {
    let line = read_reply::<SignInLine>(reply, "line", "agents.signInLine")?;
    let open: Arc<dyn Fn(String) -> BoxFuture<'static, bool> + Send + Sync> = match &ctx.open {
        Some(opener) => {
            let opener = opener.clone();
            Arc::new(move |url: String| {
                let opener = opener.clone();
                Box::pin(async move { opener.open(&url).await })
            })
        }
        None => Arc::new(|_url: String| Box::pin(async { false })),
    };
    let sign_in = BoxSignIn {
        r#where: r#where.to_owned(),
        link: road.link.clone(),
        agent: ask.name.is_none().then(|| ask.agent.clone()),
        line,
        open,
        timeout_ms: None,
    };
    let Some(terminal) = ctx.terminal.clone() else {
        todo!("this process's own terminal: signin_relay.rs has no RelayTerminal built from stdin and stdout yet")
    };
    let mut terminal = terminal.lock().await;
    relay_sign_in(sign_in, &mut terminal).await
}

/// `signedInLine(what, where, answer)` (agents-help.ts:295-298): `<what> is signed in on <where>` and ` (<detail>)` where
/// a detail is set (an empty one too), then `.`; or `<what> is not signed in on <where>` and `: <said>` where it said
/// something (an empty one too), then `.`.
pub fn signed_in_line(what: &str, r#where: &str, answer: &BoxSignedIn) -> String {
    if answer.signed_in {
        let detail = answer.detail.as_ref().map(|detail| format!(" ({detail})")).unwrap_or_default();
        return format!("{} is signed in on {}{}.", what, r#where, detail);
    }
    let said = answer.said.as_ref().map(|said| format!(": {said}")).unwrap_or_default();
    format!("{} is not signed in on {}{}.", what, r#where, said)
}
