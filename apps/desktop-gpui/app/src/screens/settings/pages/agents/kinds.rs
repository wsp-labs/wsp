// SPDX-License-Identifier: AGPL-3.0-only
//! The Tool servers, Skills and Plugins tabs (`agentKinds.tsx`, `kinds/servers.ts`, `kinds/skills.ts`,
//! `kinds/plugins.ts`, `AddServerRows.tsx`): the tab's search and add over one card per group the kind keeps its rows
//! in, the computer named in the first head with when it was read and its refresh, and one row per item with the
//! agents it is set up for, its state or the one step it needs, or its switch. A row opens the item's own page in
//! place of the list, and the add opens its own; the crumbs say how to go back.

use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::{App, Entity};
use serde_json::{json, Value};
use wsp_host_client::protocol::{
    AgentRow, AgentsReport as Report, AgentsReportProjectsItem as Project, AgentsReportServersItem as McpRow,
    AgentsReportServersItemAuth as Auth, AgentsReportServersItemScope as Scope, AgentsReportServersItemTransport as Transport,
    AgentsReportSkillsItem as SkillRow, AgentsReportSkillsItemScope as SkillScope, PluginRow, PluginRowMissing, PluginRowScope,
    ServerToolsAnswer as ToolsAnswer,
};

use super::catalog::LoginLine;
use super::model::{entry, tools_listed, w, Status, Tone};
use super::parts::Lead;
use super::store::{target, Doc, Flow, Key, Start, Store, Tab, ToolsState};
use super::Rows;
use crate::ui::{catalog, Act, Run};
pub use drawing::tab;

/// `SKILL_PREVIEW_BYTES`: how much of a SKILL.md a preview carries.
const PREVIEW_BYTES: u64 = 64 * 1024;

/// A row's target: a project's row names its project.
fn row_target(place: &str, project: Option<&str>) -> Value {
    match project {
        Some(id) => json!({ "placeId": place, "project": id }),
        None => target(place),
    }
}

/// A row's project as the report's list of them types it: typify gives each row's project a type of its own.
macro_rules! project_of {
    ($p:expr) => {
        $p.as_ref().map(|p| Project { id: p.id.clone(), name: p.name.clone(), path: p.path.clone() })
    };
}

/// The id of a row's project, its target's.
macro_rules! project_id {
    ($p:expr) => {
        $p.as_ref().map(|p| p.id.as_str())
    };
}

mod add;
mod drawing;
mod plugins;
mod servers;
mod skills;

// ---- What each kind's rows and pages say, pure over the report. ----

/// One entry per server: the same name reached the same way in the same scope and project.
#[derive(Clone)]
pub struct ServerEntry {
    key: String,
    name: String,
    project_scope: bool,
    project: Option<Project>,
    reach: String,
    stdio: bool,
    rows: Vec<McpRow>,
}

fn reach_of(row: &McpRow) -> String {
    match &row.transport {
        Transport::Stdio { line } => line.clone(),
        Transport::Http { host } => host.clone(),
        Transport::Unknown => String::new(),
    }
}

fn stdio(row: &McpRow) -> bool {
    matches!(row.transport, Transport::Stdio { .. })
}

/// `foldServers`.
fn fold_servers(rows: &[McpRow]) -> Vec<ServerEntry> {
    let mut out: Vec<(String, ServerEntry)> = Vec::new();
    for row in rows {
        let project_scope = matches!(row.scope, Scope::Project | Scope::Local);
        let scope = if project_scope { "project" } else { "global" };
        let mut parts = vec![scope.to_owned()];
        parts.extend(row.project.as_ref().map(|p| p.id.clone()));
        parts.extend([row.name.clone(), if stdio(row) { "stdio" } else { "http" }.to_owned(), reach_of(row)]);
        let key = parts.join("\0");
        match out.iter_mut().find(|(k, _)| *k == key) {
            Some((_, e)) => e.rows.push(row.clone()),
            None => out.push((
                key.clone(),
                ServerEntry {
                    key: format!("server-{}", key.replace('\0', "-")),
                    name: row.name.clone(),
                    project_scope,
                    project: project_of!(row.project),
                    reach: reach_of(row),
                    stdio: stdio(row),
                    rows: vec![row.clone()],
                },
            )),
        }
    }
    out.into_iter().map(|(_, e)| e).collect()
}

/// The key a server's tools are kept under.
fn tools_key(row: &McpRow) -> String {
    format!("{}\0{}\0{}\0{}", row.agent, row.scope, row.project.as_ref().map_or("", |p| p.id.as_str()), row.name)
}

/// A command server of the person's own on a computer other than this one: it waits for Check.
fn held_command(row: &McpRow, rows: &Rows) -> bool {
    row.launch != Some(true) && stdio(row) && row.scope != Scope::Project && !rows.here
}

/// `checksOnShow`.
fn checks_on_show(row: &McpRow, rows: &Rows) -> bool {
    rows.held.is_none()
        && row.launch != Some(true)
        && row.enabled
        && row.off.is_none()
        && row.scope != Scope::Project
        && row.auth != Auth::EnvKey
        && !held_command(row, rows)
}

/// `rowState`.
fn row_state(row: &McpRow, s: Option<&ToolsState>, rows: &Rows) -> String {
    if !row.enabled || row.off.is_some() {
        return "off".into();
    }
    if let Some(answer) = s.and_then(|s| s.answer.as_ref()) {
        return answer.auth.to_string();
    }
    if s.is_some_and(|s| s.listing) {
        return "checking".into();
    }
    if s.is_some_and(|s| s.error.is_some()) {
        return "failed".into();
    }
    if checks_on_show(row, rows) {
        "checking".into()
    } else if held_command(row, rows) {
        "unknown".into()
    } else {
        row.auth.to_string()
    }
}

const WORST: [&str; 9] = ["failed", "needs-sign-in", "off", "checking", "unknown", "env-key", "open", "signed-in", "connected"];

#[derive(Clone)]
struct Standing {
    row: McpRow,
    state: String,
    answer: Option<ToolsAnswer>,
    listing: bool,
    error: Option<String>,
    held: Option<String>,
}

fn standings(entry: &ServerEntry, tools: &HashMap<Key, ToolsState>, rows: &Rows) -> Vec<Standing> {
    entry
        .rows
        .iter()
        .map(|row| {
            let s = tools.get(&(rows.place.clone(), tools_key(row)));
            let held = (s.is_none() && held_command(row, rows)).then(|| w::check_starts(&rows.name));
            Standing {
                row: row.clone(),
                state: row_state(row, s, rows),
                answer: s.and_then(|s| s.answer.clone()),
                listing: s.is_some_and(|s| s.listing),
                error: s.and_then(|s| s.error.clone()),
                held,
            }
        })
        .collect()
}

fn worst_of(all: &[Standing]) -> Standing {
    let rank = |s: &Standing| WORST.iter().position(|w| *w == s.state).unwrap_or(WORST.len());
    all.iter().min_by_key(|s| rank(s)).cloned().expect("an entry has a row")
}

fn asked_of(all: &[Standing]) -> Option<Standing> {
    all.iter()
        .find(|s| s.answer.as_ref().is_some_and(|a| tools_listed(a).is_some()))
        .or_else(|| all.iter().find(|s| s.answer.is_some() || s.error.is_some() || s.listing))
        .cloned()
}

/// A server's state as its dot and word say it.
fn server_status(s: &Standing) -> Status {
    let state = s.state.as_str();
    if s.row.launch == Some(true) {
        return Status::new(state, Tone::Quiet, w::EVERY_THREAD);
    }
    let tools = s.answer.as_ref().and_then(tools_listed);
    match state {
        "connected" | "signed-in" => {
            let mut st = Status::new(state, Tone::Good, if state == "connected" { w::CONNECTED } else { w::SIGNED_IN });
            st.count = tools.map(|t| w::tools_count(t.len()));
            st
        }
        "checking" => Status::new(state, Tone::Quiet, w::CHECKING),
        "open" => Status::new(state, Tone::Quiet, w::NO_SIGN_IN_NEEDED),
        "env-key" => Status::new(state, Tone::Quiet, w::KEY_FROM_ENVIRONMENT),
        "needs-sign-in" => Status::new(state, Tone::Waiting, w::NEEDS_SIGN_IN).hover(w::SIGN_IN_TO_SEE),
        "failed" => {
            let st = Status::new(state, Tone::Bad, w::FAILED);
            match s.error.clone().or_else(|| s.answer.as_ref().and_then(|a| a.refused.clone())) {
                Some(why) => st.hover(why),
                None => st,
            }
        }
        "off" => {
            let st = Status::new(state, Tone::Quiet, w::OFF);
            match (&s.row.off, s.row.enabled) {
                (Some(off), true) => st.hover(off.clone()),
                _ => st,
            }
        }
        _ => {
            let st = Status::new(state, Tone::Quiet, w::NOT_CHECKED);
            match (s.answer.as_ref().and_then(|a| a.holder.clone()), &s.held) {
                (Some(holder), _) => st.hover(w::holds_sign_in(&catalog::name(&holder))),
                (None, Some(held)) => st.hover(held.clone()),
                _ => st,
            }
        }
    }
}

/// `serverSignInStart`: the server's own sign-in in a watched terminal there, or the line the person runs.
fn server_sign_in(row: &McpRow, reach: &str, rows: &Rows) -> Option<Start> {
    let (command, pasted) = match entry(&row.agent)?.mcp.as_ref()?.login.as_ref()?.lines(&row.name) {
        LoginLine::Inside(line) => return Some(Start::Copy { line, why: None }),
        LoginLine::Run { line, pasted } => (line, pasted),
    };
    let server = Some((row.name.clone(), row.scope.to_string(), row.project.as_ref().map(|p| p.id.clone())));
    let run = |finish| Start::Run {
        agent: row.agent.clone(),
        server: server.clone(),
        finish: Some(finish),
        pastes: true,
        cloud: false,
        terminal: false,
    };
    if reach != "none" {
        return Some(run("callback"));
    }
    match pasted {
        Some(_) => Some(run("address")),
        None => Some(Start::Copy { line: row.sign_in_line.clone().unwrap_or(command), why: Some(w::page_stays_here(&rows.name)) }),
    }
}

/// One line of a detail (`Fact`).
#[derive(Clone, Default)]
struct Fact {
    id: String,
    label: String,
    value: Option<String>,
    agent: Option<String>,
    status: Option<Status>,
    note: Option<String>,
    note_status: Option<Status>,
    copy: bool,
    muted: bool,
    act: Option<Act>,
    href: Option<String>,
    line: bool,
    prose: bool,
}

impl Fact {
    fn new(id: &str, label: &str) -> Fact {
        Fact { id: id.to_owned(), label: label.to_owned(), ..Fact::default() }
    }

    fn value(mut self, value: impl Into<String>) -> Fact {
        self.value = Some(value.into());
        self
    }

    fn muted(mut self, value: impl Into<String>) -> Fact {
        self.value = Some(value.into());
        self.muted = true;
        self
    }
}

/// One row of what a detail lists under it: a tool, or a kind of thing a plugin brings.
struct UnderRow {
    key: String,
    title: String,
    subtext: Option<String>,
    count: Option<String>,
}

/// What a detail lists under it, with when it was read.
struct Under {
    title: String,
    reading: bool,
    rows: Option<Vec<UnderRow>>,
    read_at: Option<String>,
    refused: Option<String>,
    empty: Option<String>,
    refresh: Option<Run>,
}

/// One option of a pick.
#[derive(Clone)]
struct ChoiceOption {
    value: String,
    label: String,
    agent: Option<String>,
    held: Option<String>,
}

type SetPick = Rc<dyn Fn(Vec<String>, &mut App)>;
type Load = Rc<dyn Fn(&mut App)>;

/// A pick a detail asks for before its first act: several ticks, or one of a few.
#[derive(Clone)]
struct Choice {
    id: &'static str,
    label: &'static str,
    many: bool,
    options: Vec<ChoiceOption>,
    value: Vec<String>,
    lost: Option<String>,
    set: SetPick,
}

/// A document a detail draws whole under its facts.
struct DocView {
    key: String,
    doc: Doc,
    load: Option<Load>,
}

/// The one row of an item (`RowView`).
struct RowView {
    key: String,
    title: String,
    lead: Lead,
    marks: Vec<String>,
    subtext: Option<String>,
    status: Option<Status>,
    quick: Option<Act>,
}

/// An item's own page (`DetailView`).
struct Detail {
    title: String,
    lead: Lead,
    marks: Vec<String>,
    about: Option<String>,
    facts: Vec<Fact>,
    acts: Vec<Act>,
    flow: Option<(String, Flow)>,
    refused: Option<String>,
    under: Option<Under>,
    choices: Vec<Choice>,
    doc: Option<DocView>,
}

/// One item of a tab.
#[derive(Clone)]
enum Item {
    Server(Box<ServerEntry>),
    Skill(Box<SkillRow>),
    Plugin(Box<PluginRow>),
}

/// A group of a tab's items, by its name and a project's folder.
struct Group {
    id: String,
    label: Option<String>,
    path: Option<String>,
    items: Vec<Item>,
}

/// What every kind reads beside its item: the store, the computer, the report while one stands.
struct K<'a> {
    store: &'a Entity<Store>,
    rows: &'a Rows,
    report: Option<&'a Report>,
    /// Privacy's server icons: whether a server's own icon is asked and drawn.
    icons: bool,
}

impl<'a> K<'a> {
    fn agents(&self) -> &'a [AgentRow] {
        self.report.map_or(&[], |r| &r.agents)
    }

    fn projects(&self) -> &'a [Project] {
        self.report.map_or(&[], |r| &r.projects)
    }

    fn skills(&self) -> &'a [SkillRow] {
        self.report.map_or(&[], |r| &r.skills)
    }

    fn servers(&self) -> &'a [McpRow] {
        self.report.map_or(&[], |r| &r.servers)
    }

    fn refused(&self) -> &'a [String] {
        self.report.map_or(&[], |r| &r.refused)
    }

    /// `reach`: how a server's sign-in page finds its way back here.
    fn reach(&self) -> String {
        self.report.and_then(|r| r.reach).map_or_else(|| "none".to_owned(), |r| r.to_string())
    }

    fn s<'b>(&self, cx: &'b App) -> &'b Store {
        self.store.read(cx)
    }

    fn key(&self, key: &str) -> Key {
        (self.rows.place.clone(), key.to_owned())
    }
}

/// `projectGroups`: one group per project, by name, its folder beside it.
fn project_groups(items: Vec<(Option<Project>, Item)>) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for (project, item) in items {
        let id = format!("project-{}", project.as_ref().map_or("", |p| p.id.as_str()));
        match groups.iter_mut().find(|g| g.id == id) {
            Some(g) => g.items.push(item),
            None => {
                groups.push(Group { id, label: project.as_ref().map(|p| p.name.clone()), path: project.map(|p| p.path), items: vec![item] })
            }
        }
    }
    groups.sort_by(|a, b| by_name(a.label.as_deref().unwrap_or_default(), b.label.as_deref().unwrap_or_default()));
    groups
}

/// The agents a skill lives in folders of.
fn skill_agents(row: &SkillRow) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in row.paths.iter().filter_map(|p| p.agent.clone()) {
        if !out.contains(&a) {
            out.push(a);
        }
    }
    out
}

/// The folder a skill really lives in: the one that is no link, else the first.
fn real_path(row: &SkillRow) -> String {
    row.paths.iter().find(|p| p.link_to.is_none()).or(row.paths.first()).map(|p| p.path.clone()).unwrap_or_default()
}

fn skill_off(row: &SkillRow) -> bool {
    row.paths.iter().all(|p| p.off == Some(true))
}

fn skill_source(row: &SkillRow) -> &'static str {
    if super::catalog::is_system_skill(&row.name) {
        "system"
    } else if row.scope == SkillScope::Project {
        "project"
    } else {
        "user"
    }
}

/// `skillKey`.
fn skill_key(row: &SkillRow) -> String {
    format!("{}:{}{}", row.scope, row.project.as_ref().map(|p| format!("{}:", p.id)).unwrap_or_default(), row.name)
}

/// `pluginKey`.
fn plugin_key(row: &PluginRow) -> String {
    format!("{}:{}:{}{}", row.agent, row.scope, row.project.as_ref().map(|p| format!("{}:", p.id)).unwrap_or_default(), row.id)
}

fn row_key(head: &[&str], project: Option<&str>, name: &str) -> String {
    let mut parts: Vec<String> = head.iter().map(|s| s.to_string()).collect();
    parts.extend(project.map(str::to_owned));
    parts.push(name.to_owned());
    parts.join("-")
}

/// `pluginMissingLine`.
fn plugin_missing(row: &PluginRow, computer: &str) -> Option<String> {
    let missing = row.missing?;
    let agent = catalog::name(&row.agent);
    Some(match missing {
        PluginRowMissing::Folder => format!(
            "{agent} has {} {}, but its folder is not on {computer}, so no turn loads it.",
            row.id,
            if row.on { "on" } else { "installed" }
        ),
        PluginRowMissing::Uninstalled => format!(
            "The config.toml on {computer} names {}, but {agent} has it in its marketplace {} and not installed, so no turn loads it.",
            row.id, row.marketplace
        ),
        other => {
            let unlisted = format!("The config.toml on {computer} names {}, but {agent} lists no such plugin there.", row.id);
            if other == PluginRowMissing::Marketplace {
                format!("{unlisted} Its marketplace, {}, is a folder that is not on {computer}.", row.marketplace)
            } else {
                unlisted
            }
        }
    })
}

/// `byName`: names in `localeCompare`'s order, the root collation Chromium's ICU sorts by: letters before their accents,
/// accents before their case, a lower case first.
fn by_name(a: &str, b: &str) -> std::cmp::Ordering {
    static ROOT: std::sync::LazyLock<icu_collator::CollatorBorrowed<'static>> = std::sync::LazyLock::new(|| {
        icu_collator::Collator::try_new(Default::default(), Default::default()).expect("the root collation is compiled in")
    });
    ROOT.compare(a, b)
}

impl Tab {
    fn search(self) -> Option<&'static str> {
        match self {
            Tab::Servers => Some("Search tool servers"),
            Tab::Skills => Some("Search skills"),
            Tab::Plugins => Some(w::SEARCH_PLUGINS),
            Tab::Agents => None,
        }
    }

    fn add(self) -> Option<&'static str> {
        match self {
            Tab::Servers => Some("Add a tool server"),
            Tab::Skills => Some("Add skill"),
            _ => None,
        }
    }

    fn empty(self, on: &str) -> String {
        match self {
            Tab::Servers => format!("No tool servers on {on} yet."),
            Tab::Skills => format!("No skills on {on} yet."),
            Tab::Plugins => w::no_plugins(on),
            Tab::Agents => format!("No agents found on {on}."),
        }
    }

    fn items(self, report: &Report) -> Vec<Item> {
        match self {
            Tab::Servers => {
                let mut all = fold_servers(&report.servers);
                all.sort_by(|a, b| by_name(&a.name, &b.name));
                all.into_iter().map(|e| Item::Server(Box::new(e))).collect()
            }
            Tab::Skills => {
                let mut all: Vec<SkillRow> = report.skills.iter().filter(|s| s.scope != SkillScope::Plugin).cloned().collect();
                all.sort_by(|a, b| by_name(&a.name, &b.name));
                all.into_iter().map(|s| Item::Skill(Box::new(s))).collect()
            }
            Tab::Plugins => {
                let mut all: Vec<PluginRow> = report.plugins.clone();
                all.sort_by(|a, b| by_name(&a.name, &b.name));
                all.into_iter().map(|p| Item::Plugin(Box::new(p))).collect()
            }
            Tab::Agents => Vec::new(),
        }
    }

    fn groups(self, items: Vec<Item>) -> Vec<Group> {
        match self {
            Tab::Servers => {
                let (global, projects): (Vec<Item>, Vec<Item>) =
                    items.into_iter().partition(|i| matches!(i, Item::Server(e) if !e.project_scope));
                let mut out = Vec::new();
                if !global.is_empty() {
                    out.push(Group { id: "global".into(), label: Some(w::GLOBAL.into()), path: None, items: global });
                }
                out.extend(project_groups(
                    projects
                        .into_iter()
                        .map(|i| {
                            let p = if let Item::Server(e) = &i { e.project.clone() } else { None };
                            (p, i)
                        })
                        .collect(),
                ));
                out
            }
            Tab::Skills => {
                let source = |i: &Item| if let Item::Skill(s) = i { skill_source(s) } else { "" };
                let mut out = Vec::new();
                for (id, label) in [("system", "System"), ("user", "Global")] {
                    let hit: Vec<Item> = items.iter().filter(|i| source(i) == id).cloned().collect();
                    if !hit.is_empty() {
                        out.push(Group { id: format!("source-{id}"), label: Some(label.into()), path: None, items: hit });
                    }
                }
                out.extend(project_groups(
                    items
                        .into_iter()
                        .filter(|i| source(i) == "project")
                        .map(|i| {
                            let p = if let Item::Skill(s) = &i { project_of!(s.project) } else { None };
                            (p, i)
                        })
                        .collect(),
                ));
                out
            }
            Tab::Plugins => {
                let (mine, theirs): (Vec<Item>, Vec<Item>) =
                    items.into_iter().partition(|i| matches!(i, Item::Plugin(p) if p.scope == PluginRowScope::User));
                let mut agents: Vec<String> = Vec::new();
                for i in &mine {
                    if let Item::Plugin(p) = i {
                        if !agents.contains(&p.agent) {
                            agents.push(p.agent.clone());
                        }
                    }
                }
                agents.sort_by_key(|a| (super::catalog::AGENTS.iter().position(|c| c.id == a.as_str()).unwrap_or(usize::MAX), a.clone()));
                let mut out: Vec<Group> = agents
                    .into_iter()
                    .map(|agent| Group {
                        id: format!("agent-{agent}"),
                        label: Some(catalog::name(&agent)),
                        path: None,
                        items: mine.iter().filter(|i| matches!(i, Item::Plugin(p) if p.agent == agent)).cloned().collect(),
                    })
                    .collect();
                out.extend(project_groups(
                    theirs
                        .into_iter()
                        .map(|i| {
                            let p = if let Item::Plugin(p) = &i { project_of!(p.project) } else { None };
                            (p, i)
                        })
                        .collect(),
                ));
                out
            }
            Tab::Agents => Vec::new(),
        }
    }
}

impl Item {
    fn key(&self) -> String {
        match self {
            Item::Server(e) => e.key.clone(),
            Item::Skill(s) => row_key(&["skill", &s.scope.to_string()], project_id!(s.project), &s.name),
            Item::Plugin(p) => plugin_key(p),
        }
    }

    fn matches(&self, q: &str) -> bool {
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        let words: Vec<String> = match self {
            Item::Server(e) => {
                let mut w = vec![e.name.clone(), e.reach.clone()];
                w.extend(e.project.as_ref().map(|p| p.name.clone()));
                for r in &e.rows {
                    w.push(catalog::name(&r.agent));
                    w.push(r.file.clone().unwrap_or_default());
                }
                w
            }
            Item::Skill(s) => {
                let mut w = vec![s.name.clone(), s.description.clone().unwrap_or_default(), real_path(s)];
                w.extend(s.project.as_ref().map(|p| p.name.clone()));
                w.extend(skill_agents(s).iter().map(|a| catalog::name(a)));
                w
            }
            Item::Plugin(p) => {
                let mut w = vec![
                    p.name.clone(),
                    p.id.clone(),
                    p.marketplace.clone(),
                    p.description.clone().unwrap_or_default(),
                    catalog::name(&p.agent),
                ];
                w.extend(p.project.as_ref().map(|p| p.name.clone()));
                for b in [
                    &p.brings.skills,
                    &p.brings.commands,
                    &p.brings.subagents,
                    &p.brings.hooks,
                    &p.brings.servers,
                    &p.brings.lsp,
                    &p.brings.apps,
                ] {
                    w.extend(b.iter().cloned());
                }
                w
            }
        };
        words.iter().any(|w| w.to_lowercase().contains(&q))
    }
}

#[cfg(test)]
mod tests;
