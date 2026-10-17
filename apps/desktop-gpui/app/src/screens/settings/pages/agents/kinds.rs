// SPDX-License-Identifier: AGPL-3.0-only
//! The Tool servers, Skills and Plugins tabs (`agentKinds.tsx`, `kinds/servers.ts`, `kinds/skills.ts`,
//! `kinds/plugins.ts`, `AddServerRows.tsx`): the tab's search and add over one card per group the kind keeps its rows
//! in, the computer named in the first head with when it was read and its refresh, and one row per item with the
//! agents it is set up for, its state or the one step it needs, or its switch. A row opens the item's own page in
//! place of the list, and the add opens its own; the crumbs say how to go back.

use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Entity, SharedString, Window};
use serde_json::{json, Value};
use wsp_host_client::ops::settings_agents::{Preview, SkillHits, Tools};
use wsp_host_client::protocol::{
    PluginsToggleRequest, ServersAddRequest, ServersRemoveRequest, ServersToggleRequest, ServersToolsRequest, SkillsAddRequest,
    SkillsGetRequest, SkillsPreviewRequest, SkillsRemoveRequest, SkillsSearchRequest, SkillsToggleRequest,
};

use super::keyed;
use super::model::{
    agent_name, command_words, compact_count, entry, read_when, refused_lines, reshape, w, McpRow, PluginRow, Project, Report, SkillHit,
    SkillRow, Status, Tone, ToolsAnswer,
};
use super::parts::{act_button, agent_marks, field, lead, on_head, plain_head, refusal, status_word, step_mark, Act, Lead, Run};
use super::store::{server_row, target, Doc, Flow, Key, Picks, Start, Store, Tab, ToolsState};
use super::{flow, headed, not_read, with_under, Rows};
use crate::screens::settings::model::req;
use crate::screens::settings::page::{Ctx, Level};
use crate::screens::settings::Acts;
use crate::ui::{
    glyph, size, text, tip, ActivePalette, Button, Card, CardItem, CopyRow, Glyph, HeadRow, Line, Row, RowSkeleton, Segment,
    SegmentedControl, Select, Skeleton, Switch, WordClass,
};

/// `SKILL_PREVIEW_BYTES`: how much of a SKILL.md a preview carries.
const PREVIEW_BYTES: u64 = 64 * 1024;

/// A row's target: a project's row names its project.
fn row_target(place: &str, project: Option<&Project>) -> Value {
    match project {
        Some(p) => json!({ "placeId": place, "project": p.id }),
        None => target(place),
    }
}

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
    if row.transport.kind == "stdio" {
        row.transport.line.clone().unwrap_or_default()
    } else {
        row.transport.host.clone().unwrap_or_default()
    }
}

/// `foldServers`.
fn fold_servers(rows: &[McpRow]) -> Vec<ServerEntry> {
    let mut out: Vec<(String, ServerEntry)> = Vec::new();
    for row in rows {
        let project_scope = row.scope == "project" || row.scope == "local";
        let scope = if project_scope { "project" } else { "global" };
        let mut parts = vec![scope.to_owned()];
        parts.extend(row.project.as_ref().map(|p| p.id.clone()));
        parts.extend([row.name.clone(), row.transport.kind.clone(), reach_of(row)]);
        let key = parts.join("\0");
        match out.iter_mut().find(|(k, _)| *k == key) {
            Some((_, e)) => e.rows.push(row.clone()),
            None => out.push((
                key.clone(),
                ServerEntry {
                    key: format!("server-{}", key.replace('\0', "-")),
                    name: row.name.clone(),
                    project_scope,
                    project: row.project.clone(),
                    reach: reach_of(row),
                    stdio: row.transport.kind == "stdio",
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
    row.launch != Some(true) && row.transport.kind == "stdio" && row.scope != "project" && !rows.here
}

/// `checksOnShow`.
fn checks_on_show(row: &McpRow, rows: &Rows) -> bool {
    rows.held.is_none()
        && row.launch != Some(true)
        && row.enabled
        && row.off.is_none()
        && row.scope != "project"
        && row.auth != "env-key"
        && !held_command(row, rows)
}

/// `rowState`.
fn row_state(row: &McpRow, s: Option<&ToolsState>, rows: &Rows) -> String {
    if !row.enabled || row.off.is_some() {
        return "off".into();
    }
    if let Some(answer) = s.and_then(|s| s.answer.as_ref()) {
        return answer.auth.clone();
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
        row.auth.clone()
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
        .find(|s| s.answer.as_ref().is_some_and(|a| a.tools.is_some()))
        .or_else(|| all.iter().find(|s| s.answer.is_some() || s.error.is_some() || s.listing))
        .cloned()
}

/// A server's state as its dot and word say it.
fn server_status(s: &Standing) -> Status {
    let state = s.state.as_str();
    if s.row.launch == Some(true) {
        return Status::new(state, Tone::Quiet, w::EVERY_THREAD);
    }
    let tools = s.answer.as_ref().and_then(|a| a.tools.as_ref());
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
                (Some(holder), _) => st.hover(w::holds_sign_in(&agent_name(&holder))),
                (None, Some(held)) => st.hover(held.clone()),
                _ => st,
            }
        }
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// `serverSignInStart`: the server's own sign-in in a watched terminal there, or the line the person runs.
fn server_sign_in(row: &McpRow, reach: &str, rows: &Rows) -> Option<Start> {
    let name = shell_quote(&row.name);
    let (command, pasted) = match row.agent.as_str() {
        "claude" => (format!("claude mcp login {name}"), Some(format!("claude mcp login {name} --no-browser"))),
        "codex" => (format!("codex mcp login {name}"), None),
        "opencode" => (format!("opencode mcp auth {name}"), None),
        "gemini" => return Some(Start::Copy { line: format!("/mcp auth {}", row.name), why: None }),
        _ => return None,
    };
    let server = Some((row.name.clone(), row.scope.clone(), row.project.as_ref().map(|p| p.id.clone())));
    let run = |finish| Start::Run {
        agent: row.agent.clone(),
        server: server.clone(),
        finish: Some(finish),
        pastes: true,
        terminal: false,
        cloud: false,
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

/// What every kind reads beside its item: the store, the computer, the report.
struct K<'a> {
    store: &'a Entity<Store>,
    rows: &'a Rows,
    report: &'a Report,
}

impl K<'_> {
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
    groups.sort_by_key(|g| g.label.clone().unwrap_or_default());
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
    if row.name == "wsp" || row.name == "wsp-machine" {
        "system"
    } else if row.scope == "project" {
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

fn row_key(head: &[&str], project: Option<&Project>, name: &str) -> String {
    let mut parts: Vec<String> = head.iter().map(|s| s.to_string()).collect();
    parts.extend(project.map(|p| p.id.clone()));
    parts.push(name.to_owned());
    parts.join("-")
}

/// `pluginMissingLine`.
fn plugin_missing(row: &PluginRow, computer: &str) -> Option<String> {
    let missing = row.missing.as_deref()?;
    let agent = agent_name(&row.agent);
    Some(match missing {
        "folder" => format!(
            "{agent} has {} {}, but its folder is not on {computer}, so no turn loads it.",
            row.id,
            if row.on { "on" } else { "installed" }
        ),
        "uninstalled" => format!(
            "The config.toml on {computer} names {}, but {agent} has it in its marketplace {} and not installed, so no turn loads it.",
            row.id, row.marketplace
        ),
        other => {
            let unlisted = format!("The config.toml on {computer} names {}, but {agent} lists no such plugin there.", row.id);
            if other == "marketplace" {
                format!("{unlisted} Its marketplace, {}, is a folder that is not on {computer}.", row.marketplace)
            } else {
                unlisted
            }
        }
    })
}

const CATALOG_ORDER: [&str; 11] = ["claude", "codex", "gemini", "opencode", "pi", "hermes", "crush", "qwen", "goose", "amp", "cursor"];

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
                all.sort_by(|a, b| a.name.cmp(&b.name));
                all.into_iter().map(|e| Item::Server(Box::new(e))).collect()
            }
            Tab::Skills => {
                let mut all: Vec<SkillRow> = report.skills.iter().filter(|s| s.scope != "plugin").cloned().collect();
                all.sort_by(|a, b| a.name.cmp(&b.name));
                all.into_iter().map(|s| Item::Skill(Box::new(s))).collect()
            }
            Tab::Plugins => {
                let mut all: Vec<PluginRow> = report.plugins.clone().unwrap_or_default();
                all.sort_by(|a, b| a.name.cmp(&b.name));
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
                            let p = if let Item::Skill(s) = &i { s.project.clone() } else { None };
                            (p, i)
                        })
                        .collect(),
                ));
                out
            }
            Tab::Plugins => {
                let (mine, theirs): (Vec<Item>, Vec<Item>) =
                    items.into_iter().partition(|i| matches!(i, Item::Plugin(p) if p.scope == "user"));
                let mut agents: Vec<String> = Vec::new();
                for i in &mine {
                    if let Item::Plugin(p) = i {
                        if !agents.contains(&p.agent) {
                            agents.push(p.agent.clone());
                        }
                    }
                }
                agents.sort_by_key(|a| (CATALOG_ORDER.iter().position(|c| c == a).unwrap_or(usize::MAX), a.clone()));
                let mut out: Vec<Group> = agents
                    .into_iter()
                    .map(|agent| Group {
                        id: format!("agent-{agent}"),
                        label: Some(agent_name(&agent)),
                        path: None,
                        items: mine.iter().filter(|i| matches!(i, Item::Plugin(p) if p.agent == agent)).cloned().collect(),
                    })
                    .collect();
                out.extend(project_groups(
                    theirs
                        .into_iter()
                        .map(|i| {
                            let p = if let Item::Plugin(p) = &i { p.project.clone() } else { None };
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
            Item::Skill(s) => row_key(&["skill", &s.scope], s.project.as_ref(), &s.name),
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
                    w.push(agent_name(&r.agent));
                    w.push(r.file.clone().unwrap_or_default());
                }
                w
            }
            Item::Skill(s) => {
                let mut w = vec![s.name.clone(), s.description.clone().unwrap_or_default(), real_path(s)];
                w.extend(s.project.as_ref().map(|p| p.name.clone()));
                w.extend(skill_agents(s).iter().map(|a| agent_name(a)));
                w
            }
            Item::Plugin(p) => {
                let mut w = vec![
                    p.name.clone(),
                    p.id.clone(),
                    p.marketplace.clone(),
                    p.description.clone().unwrap_or_default(),
                    agent_name(&p.agent),
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

// ---- Servers. ----

struct ServerActs {
    all: Vec<Standing>,
    worst: Standing,
    asked: Option<Standing>,
    acts: Vec<Act>,
    sign_ins: Vec<(String, Act)>,
    flow: Option<(String, Flow)>,
}

fn server_acts(e: &ServerEntry, k: &K, cx: &App) -> ServerActs {
    let s = k.s(cx);
    let all = standings(e, &s.tools, k.rows);
    let worst = worst_of(&all);
    let asked = asked_of(&all);
    if e.rows.iter().all(|r| r.launch == Some(true)) {
        return ServerActs { all, worst, asked, acts: Vec::new(), sign_ins: Vec::new(), flow: None };
    }
    let place = k.rows.place.clone();
    let list = {
        let (store, place) = (k.store.clone(), place.clone());
        move |row: McpRow, refresh: bool, cx: &mut App| store.update(cx, |s, cx| list_tools(s, &place, &row, refresh, cx))
    };
    let check = {
        let (rows, list) = (e.rows.clone(), list.clone());
        Act::new("check", w::CHECK, Some(Glyph::Activity)).hover(w::STARTS_ONCE).run(move |_, cx| {
            for r in &rows {
                list(r.clone(), false, cx);
            }
        })
    };
    let primary = asked.as_ref().map_or_else(|| worst.row.clone(), |a| a.row.clone());
    let listing = asked.as_ref().is_some_and(|a| a.listing);
    let reconnect = if listing {
        Act::new("reconnect", w::LISTING, Some(Glyph::RefreshCw)).busy()
    } else {
        let target = if worst.state == "failed" { worst.row.clone() } else { primary };
        let list = list.clone();
        Act::new("reconnect", w::RECONNECT, Some(Glyph::RefreshCw)).run(move |_, cx| list(target.clone(), true, cx))
    };
    let busy = s.busy.contains(&k.key(&e.key));
    let unswitched = e.rows.iter().find(|r| !entry(&r.agent).and_then(|a| a.mcp.as_ref()).is_some_and(|m| m.switch));
    let turn = |on: bool| {
        let (id, label, glyph) = if on { ("turn-on", w::TURN_ON, Glyph::Power) } else { ("turn-off", w::TURN_OFF, Glyph::PowerOff) };
        if let Some(u) = unswitched {
            return Act::not_yet(id, label, glyph).hover(w::no_switch(&agent_name(&u.agent)));
        }
        if busy {
            return Act::new(id, label, Some(glyph)).busy();
        }
        let (store, key, rows, place) =
            (k.store.clone(), e.key.clone(), e.rows.iter().filter(|r| r.enabled != on).cloned().collect::<Vec<_>>(), place.clone());
        Act::new(id, label, Some(glyph))
            .run(move |_, cx| store.update(cx, |s, cx| server_write(s, &place, &key, rows.clone(), Some(on), cx)))
    };
    let turn_off = turn(false);
    let turn_on = turn(true);
    let mut files: Vec<String> = Vec::new();
    for f in e.rows.iter().filter_map(|r| r.file.clone()) {
        if !files.contains(&f) {
            files.push(f);
        }
    }
    let remove = {
        let act = Act::new("remove", w::REMOVE, Some(Glyph::Trash))
            .destructive()
            .confirm(w::remove_title(&e.name), w::leaves_files(&files, &k.rows.name));
        if busy {
            act.busy()
        } else {
            let (store, key, rows, place) = (k.store.clone(), e.key.clone(), e.rows.clone(), place.clone());
            act.run(move |_, cx| store.update(cx, |s, cx| server_write(s, &place, &key, rows.clone(), None, cx)))
        }
    };
    let reach = k.report.reach.clone().unwrap_or_else(|| "none".into());
    let mut sign_ins: Vec<(String, Act)> = Vec::new();
    let mut flow: Option<(String, Flow)> = None;
    for st in &all {
        let id = server_row(&st.row.agent, &st.row.scope, st.row.project.as_ref().map(|p| p.id.as_str()), &st.row.name);
        let f = s.flows.get(&k.key(&id));
        let act = super::sign_in_act(k.store, k.rows, &id, server_sign_in(&st.row, &reach, k.rows), f);
        if flow.is_none() {
            flow = f.cloned().map(|f| (id.clone(), f));
        }
        if st.state == "needs-sign-in" || act.id == "cancel" || (st.state == "unknown" && !e.stdio) {
            sign_ins.push((st.row.agent.clone(), act));
        }
    }
    let sign_in = sign_ins.iter().find(|(a, _)| *a == worst.row.agent).or(sign_ins.first()).map(|(_, a)| a.clone());
    let with_sign_in: Vec<Act> = sign_in.into_iter().collect();
    let acts = match worst.state.as_str() {
        "needs-sign-in" => [with_sign_in, vec![turn_off, remove]].concat(),
        "failed" => vec![reconnect, turn_off, remove],
        "off" => vec![turn_on, remove],
        "checking" => vec![turn_off, remove],
        _ if asked.as_ref().is_some_and(|a| a.answer.is_some()) => [with_sign_in, vec![reconnect, turn_off, remove]].concat(),
        _ if e.rows.iter().any(|r| held_command(r, k.rows)) => vec![check, turn_off, remove],
        _ => [with_sign_in, vec![turn_off, remove]].concat(),
    };
    let held = k.rows.held.is_some();
    ServerActs { all, worst, asked, acts: k.rows.hold(acts), sign_ins, flow: if held { None } else { flow } }
}

/// One server's tools asked of the host: `servers.tools`, its answer kept by the row.
fn list_tools(s: &mut Store, place: &str, row: &McpRow, refresh: bool, cx: &mut gpui_kit::Context<Store>) {
    let key = (place.to_owned(), tools_key(row));
    let was = s.tools.get(&key).and_then(|t| t.answer.clone());
    s.tools.insert(key.clone(), ToolsState { listing: true, answer: was, error: None });
    let mut params = json!({ "target": row_target(place, row.project.as_ref()), "agent": row.agent, "name": row.name });
    if refresh {
        params["refresh"] = json!(true);
    }
    s.call(req::<ServersToolsRequest>(params), cx, move |s, answer: Result<Tools, String>, _| {
        let state = s.tools.entry(key).or_default();
        state.listing = false;
        match answer.map(|t| reshape::<ToolsAnswer>(&t.answer)) {
            Ok(Some(answer)) => {
                state.answer = Some(answer);
                state.error = None;
            }
            Ok(None) => state.error = Some("The tools did not read.".into()),
            Err(said) => state.error = Some(said),
        }
    });
    cx.notify();
}

/// One entry's rows turned on or off, or removed with `None`, one agent after another; busy while they run.
fn server_write(s: &mut Store, place: &str, key: &str, rows: Vec<McpRow>, on: Option<bool>, cx: &mut gpui_kit::Context<Store>) {
    let k = (place.to_owned(), key.to_owned());
    s.busy.insert(k.clone());
    s.refused.remove(&k);
    write_next(s, place.to_owned(), k, rows, on, cx);
    cx.notify();
}

fn write_next(s: &mut Store, place: String, k: Key, mut rows: Vec<McpRow>, on: Option<bool>, cx: &mut gpui_kit::Context<Store>) {
    if rows.is_empty() {
        s.busy.remove(&k);
        s.refresh(&place, cx);
        return;
    }
    let row = rows.remove(0);
    let mut params =
        json!({ "target": row_target(&place, row.project.as_ref()), "agent": row.agent, "name": row.name, "scope": row.scope });
    let then = move |s: &mut Store, answer: Result<(), String>, cx: &mut gpui_kit::Context<Store>| match answer {
        Ok(()) => write_next(s, place, k, rows, on, cx),
        Err(said) => {
            s.busy.remove(&k);
            s.refused.insert(k, said);
            s.refresh(&place, cx);
        }
    };
    match on {
        Some(on) => {
            params["on"] = json!(on);
            s.call(req::<ServersToggleRequest>(params), cx, move |s, a, cx| then(s, a.map(|_| ()), cx));
        }
        None => s.call(req::<ServersRemoveRequest>(params), cx, move |s, a, cx| then(s, a.map(|_| ()), cx)),
    }
}

fn server_row_view(e: &ServerEntry, k: &K, cx: &App) -> RowView {
    let a = server_acts(e, k, cx);
    let status = server_status(&a.worst);
    let find = |ids: &[&str]| a.acts.iter().find(|x| ids.contains(&x.id)).cloned();
    let step = match a.worst.state.as_str() {
        "needs-sign-in" => find(&["sign-in", "cancel"]),
        "failed" => find(&["reconnect"]),
        "off" => find(&["turn-on"]),
        _ => find(&["check"]),
    };
    let waiting = a.flow.as_ref().is_some_and(|(_, f)| f.waits());
    let mut status = status;
    if waiting {
        status.words = w::WAITING_ON_YOU.into();
    }
    RowView {
        key: e.key.clone(),
        title: e.name.clone(),
        lead: Lead::Box(Glyph::Server),
        marks: e.rows.iter().map(|r| r.agent.clone()).collect(),
        subtext: Some(e.reach.clone()),
        status: Some(status),
        quick: if waiting { None } else { step },
    }
}

fn server_detail(e: &ServerEntry, k: &K, cx: &App) -> Detail {
    let a = server_acts(e, k, cx);
    let listed = a.asked.as_ref().and_then(|s| s.answer.as_ref()).and_then(|x| x.tools.clone());
    let mut status = server_status(&a.worst);
    status.count = None;
    let names: Vec<String> = {
        let mut out: Vec<String> = Vec::new();
        for n in e.rows.iter().flat_map(|r| r.env_names.iter()) {
            if !out.contains(n) {
                out.push(n.clone());
            }
        }
        out
    };
    let disagree = a.all.iter().any(|s| s.state != a.all[0].state);
    let holder = a.asked.as_ref().and_then(|s| s.answer.as_ref()).and_then(|x| x.holder.clone());
    let refused = a.asked.as_ref().and_then(|s| s.error.clone().or_else(|| s.answer.as_ref().and_then(|x| x.refused.clone())));
    let wrote = k.s(cx).refused.get(&k.key(&e.key)).cloned();
    let tools_fact = match (&listed, &holder) {
        (Some(t), _) => Fact::new("tools", w::TOOLS).value(w::tools_count(t.len())),
        (None, Some(h)) => Fact::new("tools", w::TOOLS).muted(w::keeps_sign_in(&agent_name(h))),
        _ => Fact::new("tools", w::TOOLS).muted(if a.worst.state == "needs-sign-in" { w::SIGN_IN_TO_SEE } else { w::NOT_LISTED_YET }),
    };
    let recipe = e.rows.iter().find_map(|r| r.in_recipe);
    let launched = e.rows.iter().all(|r| r.launch == Some(true));
    let mut facts = vec![Fact {
        status: Some(status.clone()),
        note: (status.state == "failed").then(|| status.hover.clone()).flatten(),
        ..Fact::new("status", w::STATUS)
    }];
    facts.push(Fact { copy: true, ..Fact::new("reach", if e.stdio { w::COMMAND } else { w::URL }).value(e.reach.clone()) });
    if !names.is_empty() {
        facts.push(Fact::new("names", if e.stdio { w::ENVIRONMENT } else { w::HEADERS }).muted(names.join(", ")));
    }
    for (at, s) in a.all.iter().enumerate() {
        let act =
            if disagree && e.rows.len() > 1 { a.sign_ins.iter().find(|(ag, _)| *ag == s.row.agent).map(|(_, x)| x.clone()) } else { None };
        let act = match (&k.rows.held, act) {
            (Some(why), Some(act)) => Some(act.held(why)),
            (_, act) => act,
        };
        let mut fact = Fact {
            agent: Some(s.row.agent.clone()),
            act,
            ..Fact::new(&format!("config-{}", s.row.agent), if at == 0 { w::CONFIG_LOCATION } else { "" })
        };
        match &s.row.file {
            None => fact = fact.muted(w::ON_EVERY_LAUNCH),
            Some(file) => {
                fact = fact.value(file.clone());
                fact.copy = true;
            }
        }
        if disagree {
            fact.note_status = Some(server_status(s));
        }
        facts.push(fact);
    }
    if !launched {
        facts.push(tools_fact);
    }
    if let Some(recipe) = recipe {
        let f = Fact::new("recipe", w::RECIPE);
        facts.push(if recipe { f.value(w::IN_RECIPE) } else { f.muted(w::NOT_IN_RECIPE) });
    }
    let tools_row = a.asked.as_ref().map_or_else(|| e.rows[0].clone(), |s| s.row.clone());
    let refresh: Option<Run> = (!launched && k.rows.held.is_none()).then(|| {
        let (store, place) = (k.store.clone(), k.rows.place.clone());
        Rc::new(move |_: &mut Window, cx: &mut App| store.update(cx, |s, cx| list_tools(s, &place, &tools_row, true, cx))) as Run
    });
    let off = (a.worst.state == "off" && a.worst.row.enabled).then(|| a.worst.row.off.clone()).flatten();
    let quiet = if launched {
        Some(w::TOOLS_EVERY_TURN.to_owned())
    } else if refused.is_some() || listed.is_some() {
        None
    } else if let Some(h) = &holder {
        Some(w::keeps_sign_in(&agent_name(h)))
    } else if a.worst.state == "needs-sign-in" {
        Some(w::SIGN_IN_TO_SEE.to_owned())
    } else {
        off
    };
    Detail {
        title: e.name.clone(),
        lead: Lead::Box(Glyph::Server),
        marks: e.rows.iter().map(|r| r.agent.clone()).collect(),
        about: None,
        facts,
        acts: a.acts,
        flow: a.flow,
        refused: wrote,
        under: Some(Under {
            title: w::tools_of(&e.name),
            reading: a.asked.as_ref().is_some_and(|s| s.listing) || a.worst.state == "checking",
            rows: listed.map(|t| {
                t.into_iter().map(|t| UnderRow { key: t.name.clone(), title: t.name, subtext: t.description, count: None }).collect()
            }),
            read_at: a.asked.as_ref().and_then(|s| s.answer.as_ref()).map(|x| x.read_at.clone()),
            refused,
            empty: quiet,
            refresh,
        }),
        choices: Vec::new(),
        doc: None,
    }
}

/// `shown`: each time a report stands, the servers that are checked the moment the tab shows them are asked.
fn servers_shown(store: &Entity<Store>, rows: &Rows, report: &Report, cx: &mut App) {
    let wanted: Vec<McpRow> = report
        .servers
        .iter()
        .filter(|r| checks_on_show(r, rows))
        .filter(|r| store.read(cx).tools.get(&(rows.place.clone(), tools_key(r))).is_none_or(|t| !t.listing))
        .cloned()
        .collect();
    let read_at = report.read_at.clone();
    store.update(cx, |s, cx| {
        let was = s.shown.insert(rows.place.clone(), read_at.clone());
        if was.as_deref() == Some(read_at.as_str()) {
            return;
        }
        for row in wanted {
            list_tools(s, &rows.place, &row, false, cx);
        }
    });
}

// ---- Skills. ----

fn skill_acts(row: &SkillRow, k: &K, cx: &App) -> Vec<Act> {
    let source = skill_source(row);
    if source == "system" {
        return Vec::new();
    }
    let key = skill_key(row);
    let busy = k.s(cx).busy.contains(&k.key(&key));
    let off = skill_off(row);
    let turn = if source == "project" {
        Act::not_yet("turn-off", w::TURN_OFF, Glyph::PowerOff).hover(w::lives_in_repo(&real_path(row)))
    } else {
        let (id, label, g) = if off { ("turn-on", w::TURN_ON, Glyph::Power) } else { ("turn-off", w::TURN_OFF, Glyph::PowerOff) };
        let act = Act::new(id, label, Some(g));
        if busy {
            act
        } else {
            let (store, place, row) = (k.store.clone(), k.rows.place.clone(), row.clone());
            act.run(move |_, cx| {
                let params = json!({ "target": row_target(&place, row.project.as_ref()), "name": row.name, "project": row.scope == "project", "on": off });
                store.update(cx, |s, cx| write(s, &place, &skill_key(&row), req::<SkillsToggleRequest>(params), cx))
            })
        }
    };
    let remove = {
        let act = Act::new("remove", w::REMOVE, Some(Glyph::Trash))
            .destructive()
            .confirm(w::remove_title(&row.name), w::remove_body(&k.rows.name));
        if busy {
            act
        } else {
            let (store, place, row) = (k.store.clone(), k.rows.place.clone(), row.clone());
            act.run(move |_, cx| {
                let params =
                    json!({ "target": row_target(&place, row.project.as_ref()), "name": row.name, "project": row.scope == "project" });
                store.update(cx, |s, cx| write(s, &place, &skill_key(&row), req::<SkillsRemoveRequest>(params), cx))
            })
        }
    };
    k.rows.hold(vec![turn, remove])
}

/// One write: busy while it runs, its refusal kept until the next try, the report read again after.
fn write<O>(s: &mut Store, place: &str, key: &str, op: O, cx: &mut gpui_kit::Context<Store>)
where
    O: wsp_host_client::Op + Send + 'static,
    O::Reply: Send + 'static,
{
    let k = (place.to_owned(), key.to_owned());
    s.busy.insert(k.clone());
    s.refused.remove(&k);
    let place = place.to_owned();
    s.call(op, cx, move |s, answer, cx| {
        s.busy.remove(&k);
        if let Err(said) = answer {
            s.refused.insert(k, said);
        }
        s.refresh(&place, cx);
    });
    cx.notify();
}

fn skill_row_view(row: &SkillRow) -> RowView {
    RowView {
        key: row_key(&["skill", &row.scope], row.project.as_ref(), &row.name),
        title: row.name.clone(),
        lead: Lead::Glyph(Glyph::ScrollText),
        marks: skill_agents(row),
        subtext: Some(real_path(row)),
        status: None,
        quick: None,
    }
}

fn skill_detail(row: &SkillRow, k: &K, cx: &App) -> Detail {
    let source = skill_source(row);
    let on = || Status::new("on", Tone::Good, w::ON);
    let off = || Status::new("off", Tone::Quiet, w::OFF);
    let status = match source {
        "system" => Fact {
            status: Some(Status::new("on", Tone::Good, w::ALWAYS_ON)),
            note: Some(w::KEPT_CURRENT.into()),
            ..Fact::new("status", w::STATUS)
        },
        "project" => Fact {
            status: Some(if skill_off(row) { off() } else { on() }),
            note: Some(w::IN_REPO.into()),
            ..Fact::new("status", w::STATUS)
        },
        _ => Fact { status: Some(if skill_off(row) { off() } else { on() }), ..Fact::new("status", w::STATUS) },
    };
    let mut facts = vec![status];
    if let Some(d) = &row.description {
        facts.push(Fact { prose: true, ..Fact::new("description", w::DESCRIPTION).value(d.clone()) });
    }
    let mut ordered = row.paths.clone();
    ordered.sort_by_key(|p| p.agent.is_some());
    for (at, p) in ordered.iter().enumerate() {
        let mut f = Fact { copy: true, ..Fact::new(&format!("path-{at}"), if at == 0 { w::PATH } else { "" }).value(p.path.clone()) };
        match &p.agent {
            None => f.note = Some(w::SHARED.into()),
            Some(a) => f.agent = Some(a.clone()),
        }
        facts.push(f);
    }
    let key = (k.rows.place.clone(), skill_key(row));
    let s = k.s(cx);
    let doc = s.previews.get(&key).cloned().unwrap_or(Doc::default());
    let load = {
        let (store, place, row) = (k.store.clone(), k.rows.place.clone(), row.clone());
        Rc::new(move |cx: &mut App| {
            store.update(cx, |s, cx| {
                let key = (place.clone(), skill_key(&row));
                if s.previews.contains_key(&key) {
                    return;
                }
                s.previews.insert(key.clone(), Doc::default());
                let params =
                    json!({ "target": row_target(&place, row.project.as_ref()), "name": row.name, "project": row.scope == "project" });
                s.call(req::<SkillsPreviewRequest>(params), cx, move |s, answer: Result<Preview, String>, _| {
                    s.previews.insert(key, doc_of(answer));
                });
            })
        }) as Load
    };
    Detail {
        title: row.name.clone(),
        lead: Lead::Glyph(Glyph::ScrollText),
        marks: skill_agents(row),
        about: None,
        facts,
        acts: skill_acts(row, k, cx),
        flow: None,
        refused: s.refused.get(&key).cloned(),
        under: None,
        choices: Vec::new(),
        doc: Some(DocView { key: skill_key(row), doc, load: Some(load) }),
    }
}

fn doc_of(answer: Result<Preview, String>) -> Doc {
    match answer {
        Ok(p) => {
            let v = serde_json::to_value(&p.preview).unwrap_or(Value::Null);
            Doc {
                text: v.get("text").and_then(Value::as_str).map(str::to_owned),
                size: v.get("size").and_then(Value::as_u64).unwrap_or(0),
                error: None,
            }
        }
        Err(said) => Doc { error: Some(said), ..Doc::default() },
    }
}

/// The skill already in the folder a hit installs into.
fn installed_as<'a>(hit: &SkillHit, report: &'a Report, project: Option<&Project>) -> Option<&'a SkillRow> {
    report.skills.iter().find(|s| {
        s.name == hit.skill_id
            && match project {
                None => s.scope == "user",
                Some(p) => s.scope == "project" && s.project.as_ref().is_some_and(|x| x.id == p.id),
            }
    })
}

/// `whereNow`: where an add goes, the home or a project, and the line that says a picked project has left.
struct WhereNow {
    options: Vec<ChoiceOption>,
    value: String,
    project: Option<Project>,
    lost: Option<String>,
}

fn where_now(report: &Report, pick: Option<&(String, String)>, computer: &str) -> WhereNow {
    let mut projects = report.projects.clone().unwrap_or_default();
    projects.sort_by(|a, b| a.name.cmp(&b.name));
    let options = if projects.is_empty() && pick.is_none() {
        Vec::new()
    } else {
        let mut o = vec![ChoiceOption { value: "home".into(), label: w::GLOBAL.into(), agent: None, held: None }];
        o.extend(projects.iter().map(|p| ChoiceOption { value: p.id.clone(), label: p.name.clone(), agent: None, held: None }));
        o
    };
    match pick {
        None => WhereNow { options, value: "home".into(), project: None, lost: None },
        Some((id, name)) => match projects.iter().find(|p| p.id == *id) {
            Some(p) => WhereNow { options, value: id.clone(), project: Some(p.clone()), lost: None },
            None => WhereNow { options, value: id.clone(), project: None, lost: Some(w::project_left(name, computer)) },
        },
    }
}

fn remote_detail(hit: &SkillHit, k: &K, cx: &App) -> Detail {
    let s = k.s(cx);
    let installed: Vec<String> = k.report.agents.iter().filter(|a| a.installed).map(|a| a.id.clone()).collect();
    let picks = s.picks.get(&hit.id).cloned().unwrap_or_default();
    let picked_agents = picks.agents.clone().unwrap_or_else(|| installed.clone());
    let place = where_now(k.report, picks.project.as_ref(), &k.rows.name);
    let there = if place.lost.is_none() { installed_as(hit, k.report, place.project.as_ref()) } else { None };
    let busy = s.busy.contains(&k.key(&hit.id));
    let project_pick = picks.project.is_some();
    let offered: Vec<ChoiceOption> = k
        .report
        .agents
        .iter()
        .filter(|a| a.installed)
        .map(|a| {
            let shared = entry(&a.id).is_some_and(|e| !if project_pick { e.own_skills.1 } else { e.own_skills.0 });
            ChoiceOption {
                value: a.id.clone(),
                label: a.name.clone(),
                agent: Some(a.id.clone()),
                held: shared.then(|| w::READS_SHARED.into()),
            }
        })
        .collect();
    let set_picks = {
        let (store, id) = (k.store.clone(), hit.id.clone());
        move |next: Picks, cx: &mut App| {
            let id = id.clone();
            store.update(cx, |s, cx| {
                s.picks.insert(id, next);
                cx.notify();
            })
        }
    };
    let agents_choice = {
        let (set, picks) = (set_picks.clone(), picks.clone());
        Choice {
            id: "agents",
            label: w::AGENTS,
            many: true,
            options: offered.clone(),
            value: offered.iter().filter(|o| o.held.is_some() || picked_agents.contains(&o.value)).map(|o| o.value.clone()).collect(),
            lost: None,
            set: Rc::new(move |agents, cx| set(Picks { agents: Some(agents), ..picks.clone() }, cx)),
        }
    };
    let mut choices = vec![agents_choice];
    if !place.options.is_empty() {
        let (set, picks, projects) = (set_picks.clone(), picks.clone(), k.report.projects.clone().unwrap_or_default());
        choices.push(Choice {
            id: "where",
            label: w::WHERE,
            many: false,
            options: place.options.clone(),
            value: vec![place.value.clone()],
            lost: place.lost.clone(),
            set: Rc::new(move |v, cx| {
                let project = v.first().and_then(|v| projects.iter().find(|p| p.id == *v)).map(|p| (p.id.clone(), p.name.clone()));
                set(Picks { project, ..picks.clone() }, cx)
            }),
        });
    }
    let mut install =
        Act::new("install", if busy { w::INSTALLING.to_owned() } else { w::install_name(&hit.skill_id) }, Some(Glyph::Download));
    if busy {
        install = install.busy();
    } else if let Some(lost) = &place.lost {
        install = install.hover(lost.clone());
    } else if there.is_some() {
        install = install.hover(w::already_on(&k.rows.name));
    } else {
        let agents: Vec<String> =
            picked_agents.iter().filter(|a| offered.iter().any(|o| o.value == **a && o.held.is_none())).cloned().collect();
        let (store, place_id, id, project) = (k.store.clone(), k.rows.place.clone(), hit.id.clone(), place.project.clone());
        install = install.run(move |_, cx| {
            let params =
                json!({ "target": row_target(&place_id, project.as_ref()), "skill": id, "agents": agents, "project": project.is_some() });
            store.update(cx, |s, cx| write(s, &place_id, &id, req::<SkillsAddRequest>(params), cx))
        });
    }
    let doc = s.remote.get(&hit.id).cloned().unwrap_or(Doc::default());
    let load = {
        let (store, id) = (k.store.clone(), hit.id.clone());
        Rc::new(move |cx: &mut App| {
            store.update(cx, |s, cx| {
                if s.remote.contains_key(&id) {
                    return;
                }
                s.remote.insert(id.clone(), Doc::default());
                let id = id.clone();
                s.call(req::<SkillsGetRequest>(json!({ "skill": id })), cx, move |s, answer: Result<Preview, String>, _| {
                    s.remote.insert(id, doc_of(answer));
                });
            })
        }) as Load
    };
    let status = match there {
        None => Status::new("not-installed", Tone::Quiet, w::NOT_INSTALLED),
        Some(_) => Status::new("installed", Tone::Good, w::INSTALLED),
    };
    Detail {
        title: hit.skill_id.clone(),
        lead: Lead::Glyph(Glyph::ScrollText),
        marks: Vec::new(),
        about: None,
        facts: vec![
            Fact { status: Some(status), ..Fact::new("status", w::STATUS) },
            Fact::new("source", w::SOURCE).value(hit.source.clone()),
            Fact::new("installs", w::INSTALLS).value(compact_count(hit.installs)),
        ],
        acts: k.rows.hold(vec![install]),
        flow: None,
        refused: s.refused.get(&k.key(&hit.id)).cloned(),
        under: None,
        choices: if there.is_none() || !place.options.is_empty() { choices } else { Vec::new() },
        doc: Some(DocView { key: hit.id.clone(), doc, load: Some(load) }),
    }
}

// ---- Plugins. ----

fn plugin_status(row: &PluginRow, computer: &str) -> Option<Status> {
    if let Some(missing) = plugin_missing(row, computer) {
        return Some(Status::new("missing", Tone::Waiting, w::MISSING).hover(missing));
    }
    if row.scope == "user" {
        return None;
    }
    let st = if row.on { Status::new("on", Tone::Good, w::ON) } else { Status::new("off", Tone::Quiet, w::OFF) };
    Some(match &row.set_in {
        Some(file) => st.hover(format!("{} is set in the repo at {file}, so it is not switched here.", row.id)),
        None => st,
    })
}

fn plugin_acts(row: &PluginRow, k: &K, cx: &App) -> Vec<Act> {
    if row.scope != "user" || row.missing.is_some() {
        return Vec::new();
    }
    let busy = k.s(cx).busy.contains(&k.key(&plugin_key(row)));
    let (id, label, g) = if row.on { ("turn-off", w::TURN_OFF, Glyph::PowerOff) } else { ("turn-on", w::TURN_ON, Glyph::Power) };
    let act = Act::new(id, label, Some(g));
    let act = if busy {
        act.busy()
    } else {
        let (store, place, row) = (k.store.clone(), k.rows.place.clone(), row.clone());
        act.run(move |_, cx| {
            let params = json!({ "target": row_target(&place, row.project.as_ref()), "agent": row.agent, "plugin": row.id, "on": !row.on });
            store.update(cx, |s, cx| write(s, &place, &plugin_key(&row), req::<PluginsToggleRequest>(params), cx))
        })
    };
    k.rows.hold(vec![act])
}

/// A marketplace's own page where its source is a GitHub repo or an address.
fn source_page(source: Option<&str>) -> Option<String> {
    let source = source?;
    if source.starts_with("https://") {
        return Some(source.to_owned());
    }
    let parts: Vec<&str> = source.split('/').collect();
    let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '-');
    (parts.len() == 2 && parts.iter().all(|p| ok(p))).then(|| format!("https://github.com/{source}"))
}

fn plugin_row_view(row: &PluginRow, computer: &str) -> RowView {
    RowView {
        key: plugin_key(row),
        title: row.name.clone(),
        lead: Lead::Glyph(Glyph::Puzzle),
        marks: vec![row.agent.clone()],
        subtext: Some(row.marketplace.clone()),
        status: plugin_status(row, computer),
        quick: None,
    }
}

fn plugin_detail(row: &PluginRow, k: &K, cx: &App) -> Detail {
    let status = plugin_status(row, &k.rows.name).unwrap_or_else(|| {
        if row.on {
            Status::new("on", Tone::Good, w::ON)
        } else {
            Status::new("off", Tone::Quiet, w::OFF)
        }
    });
    let mut facts = vec![Fact { status: Some(status), ..Fact::new("status", w::STATUS) }];
    facts
        .push(Fact { href: source_page(row.source.as_deref()), ..Fact::new("marketplace", w::MARKETPLACE).value(row.marketplace.clone()) });
    if let Some(v) = &row.version {
        facts.push(Fact::new("version", w::VERSION).value(v.clone()));
    }
    facts.push(Fact::new("scope", w::SCOPE).value(w::scope(&row.scope)));
    if let Some(p) = &row.path {
        facts.push(Fact { copy: true, ..Fact::new("path", w::PATH).value(p.clone()) });
    }
    if let Some(p) = &row.set_in {
        facts.push(Fact { copy: true, ..Fact::new("set-in", w::SET_IN).value(p.clone()) });
    }
    let b = &row.brings;
    let brought: Vec<UnderRow> = [&b.skills, &b.commands, &b.subagents, &b.hooks, &b.servers, &b.lsp, &b.apps]
        .into_iter()
        .zip(w::BRINGS)
        .filter(|(names, _)| !names.is_empty())
        .map(|(names, title)| UnderRow {
            key: title.to_owned(),
            title: title.to_owned(),
            subtext: Some(names.join(", ")),
            count: Some(names.len().to_string()),
        })
        .collect();
    Detail {
        title: row.name.clone(),
        lead: Lead::Glyph(Glyph::Puzzle),
        marks: vec![row.agent.clone()],
        about: plugin_missing(row, &k.rows.name).or_else(|| row.description.clone()),
        facts,
        acts: plugin_acts(row, k, cx),
        flow: None,
        refused: k.s(cx).refused.get(&k.key(&plugin_key(row))).cloned(),
        under: (!brought.is_empty()).then_some(Under {
            title: w::WHAT_IT_BRINGS.into(),
            reading: false,
            rows: Some(brought),
            read_at: None,
            refused: None,
            empty: None,
            refresh: None,
        }),
        choices: Vec::new(),
        doc: None,
    }
}

// ---- Drawing. ----

fn row_view(item: &Item, k: &K, cx: &App) -> RowView {
    match item {
        Item::Server(e) => server_row_view(e, k, cx),
        Item::Skill(s) => skill_row_view(s),
        Item::Plugin(p) => plugin_row_view(p, &k.rows.name),
    }
}

fn detail(item: &Item, k: &K, cx: &App) -> Detail {
    match item {
        Item::Server(e) => server_detail(e, k, cx),
        Item::Skill(s) => skill_detail(s, k, cx),
        Item::Plugin(p) => plugin_detail(p, k, cx),
    }
}

/// The one step a row stands in its state's place (`stepOf`).
fn step_of(row: &RowView) -> Option<Act> {
    row.quick.clone().filter(|q| q.id != "turn-on" && (q.run.is_some() || q.busy)).map(Act::word_only)
}

/// The step of a failed state, drawn as its glyph beside the state (`failedStep`).
fn failed_step(row: &RowView, status: Option<&Status>, step: Option<&Act>) -> Option<Act> {
    (status.is_some_and(|s| s.tone == Tone::Bad) && step.is_some() && row.quick.as_ref().is_some_and(|q| q.glyph.is_some()))
        .then(|| row.quick.clone())
        .flatten()
}

/// The switch an item's acts turn it on and off by, where the host can (`turnOf`).
fn turn_of(acts: &[Act]) -> Option<(bool, Act)> {
    let act = acts.iter().find(|a| a.id == "turn-on" || a.id == "turn-off")?;
    act.hover.is_none().then(|| (act.id == "turn-off", act.clone()))
}

fn turn_switch(key: &str, turn: (bool, Act)) -> AnyElement {
    let (on, act) = turn;
    let switch = Switch::new(SharedString::from(format!("{key}-on")), on).held(act.run.is_none());
    match act.run {
        Some(run) => switch.on_change(move |_, window, cx| run(window, cx)).into_any_element(),
        None => switch.into_any_element(),
    }
}

fn kind_status(key: &str, status: &Status, cx: &App) -> AnyElement {
    status_word(SharedString::from(format!("{key}-status")), &status.said(), status.tone, status.hover.as_deref(), cx)
}

/// What stands at a row's or a head's right: the failed state's step and the state, the step, the switch, or the state.
fn right(key: &str, row: &RowView, d: &Detail, with_turn: bool, acts: &Acts, cx: &App) -> Option<AnyElement> {
    let step = step_of(row);
    let status = row.status.clone().or_else(|| d.facts.iter().find(|f| f.id == "status").and_then(|f| f.status.clone()));
    let turn = if with_turn && row.status.is_none() { turn_of(&d.acts) } else { None };
    let failed = failed_step(row, status.as_ref(), step.as_ref());
    if let Some(failed) = failed {
        return Some(
            h_flex()
                .gap_3()
                .child(step_mark(format!("{key}-step"), &failed))
                .child(kind_status(key, status.as_ref()?, cx))
                .into_any_element(),
        );
    }
    if let Some(step) = step {
        return Some(act_button(format!("{key}-{}", step.id), &step, acts));
    }
    if let Some(turn) = turn {
        return Some(turn_switch(key, turn));
    }
    status.map(|s| kind_status(key, &s, cx))
}

fn kind_row(item: &Item, k: &K, window: &mut Window, cx: &mut App) -> CardItem {
    let row = row_view(item, k, cx);
    let d = detail(item, k, cx);
    let acts = k.s(cx).acts().clone();
    let control = right(&row.key, &row, &d, true, &acts, cx);
    let mut line = Row::new(SharedString::from(row.key.clone()), row.title.clone()).lead(lead(&row.lead, &row.title, cx)).mono().clip();
    if let Some(marks) = agent_marks(format!("{}-marks", row.key), &row.marks, cx) {
        line = line.mark(marks);
    }
    if let Some(sub) = &row.subtext {
        line = line.note(sub.clone());
    }
    if let Some(control) = control {
        line = line.control(control);
    }
    let level = Level { key: format!("item:{}", row.key).into(), name: row.title.clone().into(), up: None };
    let open = acts.clone();
    line = line.on_open(move |_, _, cx| open.open_level(Some(level.clone()), cx));
    let under = match (&d.flow, &d.refused) {
        (Some((row_id, flow)), _) => Some(flow::view(k.store, &k.rows.place, row_id, flow, false, window, cx)),
        (None, Some(said)) => Some(
            div()
                .text_size(text::FACT)
                .line_height(px(18.))
                .text_color(cx.palette().error_foreground)
                .child(said.clone())
                .into_any_element(),
        ),
        _ => None,
    };
    with_under(line, under)
}

/// A group's head: its name, and a project's folder after it.
fn group_head(group: &Group, first: Option<String>, cx: &App) -> AnyElement {
    super::parts::group_head(first.or_else(|| group.label.clone()).unwrap_or_default(), group.path.clone(), cx)
}

/// The field a tab's search or an add's is typed into, at the row controls' height.
fn search_field(input: &Entity<gpui_kit::component::input::InputState>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let el = Styled::h(gpui_kit::component::input::Input::new(input), px(30.))
        .prefix(Icon::new(Glyph::Search).with_size(px(16.)).text_color(p.muted_foreground))
        .rounded(px(7.))
        .px(px(10.))
        .py_0()
        .border_color(p.input)
        .bg(p.input_fill)
        .text_size(text::FACT)
        .w_full();
    div().flex_1().min_w_0().max_w(px(320.)).child(el).into_any_element()
}

/// One tab: its list, or the page of one of its items or of its add, as the crumbs' level names it.
pub fn tab(ctx: &Ctx, store: &Entity<Store>, rows: &Rows, tab: Tab, window: &mut Window, cx: &mut App) -> AnyElement {
    let (report, reading, error, read_at) = {
        let s = store.read(cx);
        let read = s.reads.get(&rows.place);
        (
            read.and_then(|r| r.report.clone()),
            read.is_some_and(|r| r.reading),
            read.and_then(|r| r.error.clone()),
            read.and_then(|r| r.read_at),
        )
    };
    let empty = Report::default();
    let k = K { store, rows, report: report.as_ref().unwrap_or(&empty) };
    if tab == Tab::Servers {
        if let Some(report) = &report {
            servers_shown(store, rows, report, cx);
        }
    }
    let items = report.as_ref().map(|r| tab.items(r)).unwrap_or_default();
    let level = ctx.level.map(|l| l.key.to_string());
    let held = rows.held.is_some();
    let adds = !held && tab.add().is_some();
    if let Some(level) = &level {
        if let Some(key) = level.strip_prefix("item:") {
            match items.iter().find(|i| i.key() == key) {
                Some(item) => return item_page(Some(&row_view(item, &k, cx)), &detail(item, &k, cx), &k, window, cx),
                None if report.is_some() => ctx.acts.open_level(None, cx),
                None => {}
            }
        } else if let Some(rest) = level.strip_prefix("found:") {
            let (key, query) = rest.split_once('\n').unwrap_or((rest, ""));
            let hit =
                store.read(cx).searches.get(query.trim()).and_then(|s| s.hits.clone()).and_then(|h| h.into_iter().find(|h| h.id == key));
            match hit {
                Some(hit) if tab == Tab::Skills => return item_page(None, &remote_detail(&hit, &k, cx), &k, window, cx),
                _ if report.is_some() => ctx.acts.open_level(None, cx),
                _ => {}
            }
        } else if level == "add" && adds {
            match tab {
                Tab::Skills => return add_skill(ctx, &k, window, cx),
                Tab::Servers => return add_server(&k, window, cx),
                _ => {}
            }
        } else if report.is_some() {
            ctx.acts.open_level(None, cx);
        }
    }
    kind_list(ctx, &k, tab, items, report.is_some(), reading, error, read_at, window, cx)
}

#[allow(clippy::too_many_arguments)]
fn kind_list(
    ctx: &Ctx,
    k: &K,
    tab: Tab,
    items: Vec<Item>,
    has_report: bool,
    reading: bool,
    error: Option<String>,
    read_at: Option<std::time::SystemTime>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let name = k.rows.name.clone();
    let mut toolbar = h_flex().debug_selector(|| "kind-toolbar".into()).justify_between().gap_3();
    let mut query = String::new();
    if let Some(search) = tab.search() {
        let input = keyed::typed(format!("kind-search-{}-{}", k.rows.place, tab.id()), search, "", false, |_, _, _| {}, window, cx);
        query = keyed::words(&input, cx);
        toolbar = toolbar.child(search_field(&input, cx));
    }
    if let Some(add) = tab.add() {
        let held = k.rows.held.is_some() || !has_report;
        let mut button = Button::add(format!("kind-add-{}", tab.id()), add).height(px(30.)).held(held);
        if !held {
            let acts = ctx.acts.clone();
            let name = if tab == Tab::Skills { w::ADD_SKILL } else { w::ADD_SERVER };
            button = button.on_click(move |_, _, cx| acts.open_level(Some(Level { key: "add".into(), name: name.into(), up: None }), cx));
        }
        let el = match (&k.rows.held, has_report && held) {
            (Some(why), true) => div().id("kind-add-hover").tooltip(tip(why.clone())).child(button).into_any_element(),
            _ => button.into_any_element(),
        };
        toolbar = toolbar.child(el);
    }
    let matching: Vec<Item> = items.into_iter().filter(|i| i.matches(&query)).collect();
    let groups: Vec<Group> = tab.groups(matching).into_iter().filter(|g| !g.items.is_empty()).collect();
    let when = read_at.map(|at| w::checked_now(&read_when(at.elapsed().unwrap_or_default().as_millis() as u64)));
    let refresh: Run = {
        let (store, place) = (k.store.clone(), k.rows.place.clone());
        Rc::new(move |_, cx| store.update(cx, |s, cx| s.refresh(&place, cx)))
    };
    let head = |text: AnyElement, cx: &App| {
        on_head(&format!("kind-{}", tab.id()), text, when.clone(), reading, Some(refresh.clone()), w::READ_AGENTS_AGAIN, cx)
    };
    let mut cards: Vec<AnyElement> = Vec::new();
    if !has_report && reading {
        cards.push(Card::new("kind-reading").head(w::on(&name)).body(RowSkeleton::new("agents-reading")).into_any_element());
    } else if groups.is_empty() {
        let mut card = Card::new("kind-none");
        if has_report {
            let q = query.trim();
            card = card.item(Line::new("kind-none", if q.is_empty() { tab.empty(&name) } else { w::nothing_matches(q) }).empty());
        }
        cards.push(headed(head(w::on(&name).into_any_element(), cx), card));
    } else {
        for (at, group) in groups.iter().enumerate() {
            let items: Vec<CardItem> = group.items.iter().map(|i| kind_row(i, k, window, cx)).collect();
            let card = Card::new(SharedString::from(format!("kind-{}", group.id))).items(items);
            let title = if at == 0 {
                let words = match &group.label {
                    None => w::on(&name),
                    Some(label) => w::group_on(label, &name),
                };
                head(group_head(group, Some(words), cx), cx)
            } else {
                plain_head(group_head(group, None, cx), cx)
            };
            cards.push(headed(title, card));
        }
    }
    let mut lines = refused_lines(&k.report.refused);
    lines.extend(k.rows.misses.iter().cloned());
    if !has_report {
        if let Some(error) = error {
            lines.push((error, None));
        }
    }
    cards.extend(not_read(lines));
    v_flex().gap_3().child(toolbar).child(v_flex().gap(px(30.)).children(cards)).into_any_element()
}

/// Acts the item's page draws in a place of their own, or whose work the page already shows.
const PAGE_OWN: [&str; 4] = ["turn-on", "turn-off", "remove", "reconnect"];

/// One item's own page: its head with its state or its step and its switch, how it stands, its picks, what it lists,
/// its document, and Remove at the foot.
fn item_page(row: Option<&RowView>, d: &Detail, k: &K, window: &mut Window, cx: &mut App) -> AnyElement {
    if let Some(load) = d.doc.as_ref().and_then(|doc| doc.load.clone()) {
        load(cx);
    }
    let acts = k.s(cx).acts().clone();
    let step = row.and_then(step_of);
    let status = row.and_then(|r| r.status.clone()).or_else(|| d.facts.iter().find(|f| f.id == "status").and_then(|f| f.status.clone()));
    let failed = row.and_then(|r| failed_step(r, status.as_ref(), step.as_ref()));
    let turn = turn_of(&d.acts);
    let remove = d.acts.iter().find(|a| a.id == "remove").cloned();
    let page_acts: Vec<Act> = d
        .acts
        .iter()
        .filter(|a| !PAGE_OWN.contains(&a.id) && Some(a.id) != step.as_ref().map(|s| s.id) && (a.run.is_some() || a.busy))
        .cloned()
        .map(Act::word_only)
        .collect();
    let mut slot = h_flex().gap_3();
    for act in &page_acts {
        slot = slot.child(act_button(format!("kind-head-{}", act.id), act, &acts));
    }
    if let Some(failed) = failed {
        slot = slot.child(step_mark("kind-head-step", &failed));
        if let Some(status) = &status {
            slot = slot.child(kind_status("kind-head", status, cx));
        }
    } else if let Some(step) = &step {
        slot = slot.child(act_button(format!("kind-head-{}", step.id), step, &acts));
    } else if let (Some(status), true) = (&status, page_acts.is_empty()) {
        slot = slot.child(kind_status("kind-head", status, cx));
    }
    if let Some(turn) = turn {
        slot = slot.child(turn_switch("kind-head", turn));
    }
    let mark = match &d.lead {
        Lead::Agent(id) => super::marks::harness_mark(id, &d.title, px(16.), cx),
        Lead::Box(g) | Lead::Glyph(g) => glyph(*g, cx).into_any_element(),
    };
    let mut head = HeadRow::new("kind-head", mark, d.title.clone()).slot(slot);
    if let Some(marks) = agent_marks("kind-head-marks", &d.marks, cx) {
        head = head.mark(marks);
    }
    if let Some(sub) = row.and_then(|r| r.subtext.clone()) {
        head = head.line(div().id("kind-head-line").truncate().tooltip(tip(sub.clone())).child(sub));
    }
    let p = cx.palette().clone();
    let mut section = v_flex().gap_3().child(Card::new("kind-head").item(head));
    if let Some(about) = &d.about {
        section =
            section.child(div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(about.clone()));
    }
    if let Some((row_id, f)) = &d.flow {
        section = section.child(flow::view(k.store, &k.rows.place, row_id, f, true, window, cx));
    } else if let Some(said) = &d.refused {
        section = section.child(refusal(Some(said)));
    }
    let mut out = vec![section.into_any_element()];
    let facts: Vec<&Fact> = d.facts.iter().filter(|f| f.id != "status" && (f.id != "tools" || d.under.is_none())).collect();
    let ones: Vec<&Choice> = d.choices.iter().filter(|c| !c.many).collect();
    if !facts.is_empty() || !ones.is_empty() {
        let mut card = Card::new("kind-facts").head(w::DETAILS);
        let mut label_for = String::new();
        for f in &facts {
            if !f.label.is_empty() {
                label_for = f.label.clone();
            }
            card = card.item(fact_line(f, &label_for, &acts, cx));
        }
        for c in ones {
            card = card.item(one_of(c, window, cx));
        }
        out.push(card.into_any_element());
    }
    for c in d.choices.iter().filter(|c| c.many) {
        out.push(choice_card(c, cx));
    }
    if let Some(under) = &d.under {
        out.push(under_card(under, cx));
    }
    if let Some(doc) = &d.doc {
        out.push(Card::new("kind-doc").head(w::SKILL_FILE).body(skill_preview(&doc.key, &doc.doc, window, cx)).into_any_element());
    }
    if let Some(remove) = remove {
        let row = Row::new("kind-remove", w::remove_name(&remove.label, &d.title))
            .note(remove.confirm.as_ref().map(|(_, b)| b.clone()).unwrap_or_default())
            .control(act_button("kind-remove-act", &remove.clone().word_only(), &acts));
        out.push(Card::new("kind-remove").item(row).into_any_element());
    }
    v_flex().gap(px(30.)).children(out).into_any_element()
}

/// One fact as a line: its label, and at its right the agent it is for, its value, its note and its act.
fn fact_line(f: &Fact, label_for: &str, acts: &Acts, cx: &App) -> CardItem {
    let p = cx.palette().clone();
    let id = SharedString::from(format!("fact-{}", f.id));
    if f.prose {
        return Row::new(id, f.label.clone()).note(f.value.clone().unwrap_or_default()).into();
    }
    if f.line {
        let (label, value) = (f.label.clone(), f.value.clone().unwrap_or_default());
        let ground = p.card;
        return CardItem::placed(move |corners| {
            corners
                .apply(v_flex())
                .px(corners.inset)
                .py_3()
                .min_h(size::LINE_FLOOR)
                .justify_center()
                .gap_2()
                .child(div().text_size(text::SETTING_TITLE.0).line_height(text::SETTING_TITLE.1).child(label))
                .child(CopyRow::new(id.clone(), value).on(ground))
                .into_any_element()
        });
    }
    let mut right = h_flex().group("fact").flex_wrap().justify_end().gap_x_3().gap_y_1().min_w_0();
    if let (true, Some(value)) = (f.copy, &f.value) {
        let value = value.clone();
        right = right.child(
            div().opacity(0.).group_hover("fact", |s| s.opacity(1.)).child(
                Button::icon_xs(SharedString::from(format!("fact-{}-copy", f.id)), Glyph::Copy)
                    .tooltip(format!("{} {}", w::COPY, label_for.to_lowercase()))
                    .on_click(move |_, _, cx| cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(value.clone()))),
            ),
        );
    }
    if let Some(agent) = &f.agent {
        right = right.child(super::marks::harness_mark(agent, &agent_name(agent), px(14.), cx));
    }
    if let Some(status) = &f.status {
        right = right.child(kind_status(&format!("fact-{}", f.id), status, cx));
    }
    if let Some(value) = &f.value {
        right = right.child(match &f.href {
            Some(href) => {
                let href = href.clone();
                h_flex()
                    .id(SharedString::from(format!("fact-{}-link", f.id)))
                    .min_w_0()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(text::VALUE)
                    .text_color(p.foreground)
                    .hover(|s| s.underline())
                    .tooltip(tip(href.clone()))
                    .on_click(move |_, _, cx| cx.open_url(&href))
                    .child(div().truncate().child(value.clone()))
                    .child(Icon::new(Glyph::ExternalLink).with_size(px(12.)).text_color(p.muted_foreground))
                    .into_any_element()
            }
            None => {
                crate::ui::word(value.clone(), if f.muted { WordClass::Fact } else { WordClass::Value }, cx).text_right().into_any_element()
            }
        });
    }
    if let Some(note) = &f.note {
        right = right.child(crate::ui::word(note.clone(), WordClass::Fact, cx));
    }
    if let Some(status) = &f.note_status {
        right = right.child(kind_status(&format!("fact-{}-note", f.id), status, cx));
    }
    if let Some(act) = &f.act {
        right = right.child(act_button(format!("fact-{}-act", f.id), &act.clone().word_only(), acts));
    }
    Line::new(id, f.label.clone()).right(right).into()
}

/// One pick of a few as a line with its select.
fn one_of(c: &Choice, window: &mut Window, cx: &mut App) -> CardItem {
    let items = c.options.iter().map(|o| (o.value.clone(), SharedString::from(o.label.clone()), None)).collect();
    let select = keyed::pick(format!("choice-{}", c.id), items, c.value.first().cloned(), window, cx);
    let set = c.set.clone();
    let select = Select::new(&select).on_change(move |v: &String, _, cx| set(vec![v.clone()], cx));
    let right = v_flex().items_end().gap_1().child(select).children(c.lost.clone().map(|lost| crate::ui::word(lost, WordClass::Fact, cx)));
    Line::new(SharedString::from(format!("choice-{}", c.id)), c.label).right(right).into()
}

/// Several picks as rows, each with its switch, a held one standing on with its reason under it.
fn choice_card(c: &Choice, cx: &App) -> AnyElement {
    let rows = c.options.iter().map(|o| {
        let on = c.value.contains(&o.value);
        let (set, value, all) = (c.set.clone(), o.value.clone(), c.value.clone());
        let switch = Switch::new(SharedString::from(format!("choice-{}-{}-on", c.id, o.value)), on).held(o.held.is_some()).on_change(
            move |next, _, cx| {
                let picked: Vec<String> = if *next {
                    all.iter().cloned().chain([value.clone()]).collect()
                } else {
                    all.iter().filter(|v| **v != value).cloned().collect()
                };
                set(picked, cx)
            },
        );
        let mut row = Row::new(SharedString::from(format!("choice-{}-{}", c.id, o.value)), o.label.clone())
            .note(o.held.clone().unwrap_or_default())
            .control(switch);
        if let Some(agent) = &o.agent {
            row = row.lead(lead(&Lead::Agent(agent.clone()), &o.label, cx));
        }
        row
    });
    Card::new(SharedString::from(format!("kind-choice-{}", c.id))).head(c.label).items(rows).into_any_element()
}

/// What a level under a detail lists (a server's tools), with when they were read and the refresh.
fn under_card(u: &Under, cx: &App) -> AnyElement {
    let rows = u.rows.as_ref();
    let when = u.read_at.as_deref().and_then(parse_time).map(|at| w::checked_now(&read_when(at)));
    let head = on_head("kind-under", u.title.clone().into_any_element(), when, u.reading, u.refresh.clone(), "Read again", cx);
    let mut card = Card::new("kind-under");
    match rows {
        Some(rows) if !rows.is_empty() => {
            for r in rows {
                let mut row =
                    Row::new(SharedString::from(format!("under-{}", r.key)), r.title.clone()).note(r.subtext.clone().unwrap_or_default());
                if let Some(count) = &r.count {
                    row = row.clip().fact(count.clone());
                }
                card = card.item(row);
            }
        }
        _ if u.reading => card = card.body(RowSkeleton::new("under-reading")),
        _ => {
            let said = u.refused.clone().or_else(|| u.empty.clone()).unwrap_or_else(|| w::NOT_LISTED.into());
            card = card.item(Line::new("under-none", said).empty());
        }
    }
    headed(head, card)
}

/// How long ago an RFC 3339 time was, in ms; none where it does not read.
fn parse_time(at: &str) -> Option<u64> {
    let at = chrono::DateTime::parse_from_rfc3339(at).ok()?;
    let ms = chrono::Utc::now().signed_duration_since(at).num_milliseconds();
    Some(ms.max(0) as u64)
}

/// `skillBody`: the SKILL.md with its frontmatter taken off.
fn skill_body(text: &str) -> &str {
    let rest = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n"));
    let Some(rest) = rest else { return text };
    match rest.find("\n---") {
        Some(end) => {
            let after = &rest[end + 4..];
            let line_end = after.find('\n').map_or(after.len(), |i| i + 1);
            if after[..line_end].trim().is_empty() {
                &after[line_end..]
            } else {
                text
            }
        }
        None => text,
    }
}

/// A skill's SKILL.md drawn whole under its facts, its frontmatter off; its bars while it is read.
fn skill_preview(key: &str, doc: &Doc, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.palette().clone();
    let boxed = || {
        div()
            .id("skill-preview-body")
            .max_h(px(480.))
            .min_h(px(168.))
            .overflow_y_scroll()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.card)
            .px_4()
            .py_3()
    };
    let mut col = v_flex().gap(px(6.));
    if doc.text.is_some() && doc.size > PREVIEW_BYTES {
        col = col.child(crate::ui::word(
            format!("shows the first {} KB of {} KB", PREVIEW_BYTES / 1024, doc.size.div_ceil(1024)),
            WordClass::Fact,
            cx,
        ));
    }
    let body = match (&doc.text, &doc.error) {
        (Some(text), _) => {
            let body = skill_body(text).to_owned();
            let state = window.use_keyed_state(SharedString::from(format!("skill-md-{key}-{}", doc.size)), cx, move |_, cx| {
                gpui_kit::component::text::TextViewState::markdown(&body, cx)
            });
            boxed().text_size(text::VALUE).child(gpui_kit::component::text::TextView::new(&state)).into_any_element()
        }
        (None, Some(error)) => refusal(Some(error)),
        _ => boxed()
            .child(
                v_flex()
                    .gap_2()
                    .child(Skeleton::new(px(640.), px(14.)))
                    .child(Skeleton::new(px(640.), px(14.)))
                    .child(Skeleton::new(px(420.), px(14.))),
            )
            .into_any_element(),
    };
    col.child(body).into_any_element()
}

/// Add a skill: skills.sh searched by the host, each result by name and repo with its installs, or `installed`.
fn add_skill(ctx: &Ctx, k: &K, window: &mut Window, cx: &mut App) -> AnyElement {
    let store = k.store.clone();
    let input = keyed::typed(
        "add-search",
        w::SEARCH_SKILLS_SH,
        "",
        false,
        {
            let store = store.clone();
            move |input, _, cx| {
                let q = keyed::words(input, cx).trim().to_owned();
                store.update(cx, |s, cx| ask_skills(s, q, cx));
            }
        },
        window,
        cx,
    );
    let typed = keyed::words(&input, cx).trim().to_owned();
    let asked = store.read(cx).add_asked.clone();
    if typed != asked && store.read(cx).add_pending.as_deref() != Some(typed.as_str()) {
        let pending = typed.clone();
        store.update(cx, |s, cx| {
            s.add_pending = Some(pending.clone());
            cx.spawn(async move |me, cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
                _ = me.update(cx, |s, cx| {
                    if s.add_pending.as_deref() == Some(pending.as_str()) {
                        s.add_pending = None;
                        ask_skills(s, pending, cx);
                    }
                });
            })
            .detach();
        });
    }
    let link =
        Button::ghost("add-link", w::SKILLS_SH).xs().glyph(Glyph::ExternalLink).on_click(|_, _, cx| cx.open_url("https://skills.sh"));
    let toolbar = h_flex().justify_between().gap_3().child(search_field(&input, cx)).child(link);
    let search = store.read(cx).searches.get(&asked).cloned();
    let (rows, empty, reading): (Vec<(SkillHit, bool)>, Option<String>, bool) = if asked.is_empty() {
        (Vec::new(), Some(w::TYPE_TO_SEARCH.into()), false)
    } else {
        let hits: Vec<(SkillHit, bool)> = search
            .as_ref()
            .and_then(|s| s.hits.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|h| (h.clone(), installed_as(&h, k.report, None).is_some()))
            .collect();
        match &search {
            Some(s) if s.error.is_some() => (Vec::new(), s.error.clone(), false),
            _ => {
                let empty = (search.as_ref().is_some_and(|s| s.hits.is_some()) && hits.is_empty()).then(|| w::no_hits(&asked));
                (hits, empty, search.as_ref().is_none_or(|s| s.reading))
            }
        }
    };
    let mut out = v_flex().gap_3().child(toolbar);
    if (reading || typed != asked) && rows.is_empty() {
        out = out.child(RowSkeleton::new("add-reading"));
    }
    let mut card = Card::new("kind-found");
    for (hit, there) in &rows {
        let level = Level {
            key: format!("found:{}\n{}", hit.id, asked).into(),
            name: hit.skill_id.clone().into(),
            up: Some(Box::new(Level { key: "add".into(), name: w::ADD_SKILL.into(), up: None })),
        };
        let acts = ctx.acts.clone();
        card = card.item(
            Row::new(SharedString::from(hit.id.clone()), hit.skill_id.clone())
                .note(hit.source.clone())
                .mono()
                .clip()
                .fact(if *there { w::INSTALLED.to_owned() } else { compact_count(hit.installs) })
                .on_open(move |_, _, cx| acts.open_level(Some(level.clone()), cx)),
        );
    }
    if rows.is_empty() && !reading {
        if let Some(empty) = empty {
            card = card.item(Line::new("add-none", empty).empty());
        }
    }
    out.child(card).into_any_element()
}

/// `skills.search` once per query; a refused one is asked again when typed again.
fn ask_skills(s: &mut Store, q: String, cx: &mut gpui_kit::Context<Store>) {
    s.add_asked = q.clone();
    if q.is_empty() || s.searches.get(&q).is_some_and(|x| x.error.is_none()) {
        cx.notify();
        return;
    }
    s.searches.insert(q.clone(), super::store::Search { reading: true, ..Default::default() });
    s.call(req::<SkillsSearchRequest>(json!({ "q": q })), cx, move |s, answer: Result<SkillHits, String>, _| {
        let search = s.searches.entry(q).or_default();
        search.reading = false;
        match answer {
            Ok(hits) => search.hits = Some(hits.skills.iter().filter_map(reshape::<SkillHit>).collect()),
            Err(said) => search.error = Some(said),
        }
    });
    cx.notify();
}

/// The file an add for this agent lands in: the one the report already read servers from, else the first it reads.
fn file_of(agent: &str, project: Option<&Project>, report: &Report) -> Option<String> {
    let mcp = entry(agent)?.mcp.as_ref()?;
    match project {
        Some(p) => {
            let files: Vec<String> = mcp.project_files.iter().map(|f| format!("{}/{f}", p.path)).collect();
            let own = report.servers.iter().find(|r| {
                r.agent == agent
                    && r.scope == "project"
                    && r.project.as_ref().is_some_and(|x| x.id == p.id)
                    && r.file.as_ref().is_some_and(|f| files.contains(f))
            });
            own.and_then(|r| r.file.clone()).or_else(|| files.first().cloned())
        }
        None => report
            .servers
            .iter()
            .find(|r| r.agent == agent && r.scope == "user" && r.file.is_some())
            .and_then(|r| r.file.clone())
            .or_else(|| mcp.files.first().map(|f| f.to_string())),
    }
}

/// Add a tool server, in the page's rows: the agent whose config takes it, its name, a command or an address, where
/// it goes, then its variables or headers, each value held here alone until the host takes it once.
fn add_server(k: &K, window: &mut Window, cx: &mut App) -> AnyElement {
    let agents: Vec<String> = super::catalog::AGENTS
        .iter()
        .filter(|a| a.mcp.is_some() && k.report.agents.iter().any(|r| r.id == a.id && r.installed))
        .map(|a| a.id.to_owned())
        .collect();
    if agents.is_empty() {
        return Card::new("add-server").item(Line::new("add-server-none", w::no_server_agents(&k.rows.name)).empty()).into_any_element();
    }
    let form = window.use_keyed_state("add-server-form", cx, |_, _| ServerForm::default());
    let f = form.read(cx).clone();
    let agent = f.agent.clone().filter(|a| agents.contains(a)).unwrap_or_else(|| agents[0].clone());
    let command_road = f.road != "address";
    let place = where_now(k.report, f.picked.as_ref(), &k.rows.name);
    let set = |change: Box<dyn Fn(&mut ServerForm)>| {
        let form = form.clone();
        move |cx: &mut App| {
            form.update(cx, |f, cx| {
                change(f);
                cx.notify();
            })
        }
    };
    let agent_items = agents
        .iter()
        .map(|a| {
            let id = a.clone();
            let mark: keyed::Mark = Rc::new(move |cx: &App| super::marks::harness_mark(&id, &agent_name(&id), px(13.), cx));
            (a.clone(), SharedString::from(agent_name(a)), Some(mark))
        })
        .collect();
    let agent_select = keyed::pick("add-server-agent", agent_items, Some(agent.clone()), window, cx);
    let agent_select = {
        let form = form.clone();
        Select::new(&agent_select).on_change(move |a: &String, _, cx| {
            let a = Some(a.clone());
            form.update(cx, |f, cx| {
                f.agent = a;
                cx.notify();
            })
        })
    };

    let name = keyed::typed("add-server-name", "notion", "", false, |_, _, _| {}, window, cx);
    let reach = if command_road {
        keyed::typed("add-server-command", "npx -y @notionhq/notion-mcp-server", "", false, |_, _, _| {}, window, cx)
    } else {
        keyed::typed("add-server-url", "https://mcp.notion.com/mcp", "", false, |_, _, _| {}, window, cx)
    };
    let road = {
        let set = set(Box::new(|_| {}));
        let form = form.clone();
        SegmentedControl::new(
            "add-server-road",
            if command_road { "command" } else { "address" },
            vec![Segment::new("command", w::BY_COMMAND), Segment::new("address", w::BY_ADDRESS)],
        )
        .on_change(move |to, _, cx| {
            let to = to.to_string();
            form.update(cx, |f, _| {
                if f.road != to {
                    f.pairs.clear();
                }
                f.road = to;
            });
            set(cx);
        })
    };
    let mut card = Card::new("add-server")
        .item(Row::new("add-server-agent", w::AGENT).control(agent_select))
        .item(Row::new("add-server-name", w::SERVER_NAME).control(field(&name, Some(px(176.)), cx)))
        .item(Row::new("add-server-road", w::REACHED_BY).control(road))
        .item(Row::new("add-server-reach", if command_road { w::COMMAND } else { w::URL }).control(field(&reach, Some(px(320.)), cx)));
    if !place.options.is_empty() {
        let items = place.options.iter().map(|o| (o.value.clone(), SharedString::from(o.label.clone()), None)).collect();
        let select = keyed::pick("add-server-where", items, Some(place.value.clone()), window, cx);
        let (form2, projects) = (form.clone(), k.report.projects.clone().unwrap_or_default());
        let select = Select::new(&select).on_change(move |v: &String, _, cx| {
            let picked = projects.iter().find(|p| p.id == *v).map(|p| (p.id.clone(), p.name.clone()));
            form2.update(cx, |f, cx| {
                f.picked = picked;
                cx.notify();
            })
        });
        card = card.item(Row::new("add-server-where", w::WHERE).note(place.lost.clone().unwrap_or_default()).control(select));
    }
    let pair_word = if command_road { w::VARIABLES } else { w::HEADERS };
    let mut pairs_card = Card::new("add-server-pairs").head(pair_word);
    let mut typed_pairs: Vec<(String, String)> = Vec::new();
    for id in &f.pairs {
        let pname = keyed::typed(
            format!("add-server-pair-{id}-name"),
            if command_road { "API_KEY" } else { "Authorization" },
            "",
            false,
            |_, _, _| {},
            window,
            cx,
        );
        let pvalue = keyed::typed(format!("add-server-pair-{id}-value"), w::VALUE, "", true, |_, _, _| {}, window, cx);
        let (n, v) = (keyed::words(&pname, cx), keyed::words(&pvalue, cx));
        typed_pairs.push((n.clone(), v));
        let (form, id) = (form.clone(), *id);
        let row = h_flex().gap_2().child(field(&pname, None, cx)).child(field(&pvalue, None, cx)).child(
            Button::icon_xs(SharedString::from(format!("add-server-pair-{id}-drop")), Glyph::Close)
                .tooltip(w::remove_pair(n.trim()))
                .on_click(move |_, _, cx| {
                    form.update(cx, |f, cx| {
                        f.pairs.retain(|p| *p != id);
                        cx.notify();
                    })
                }),
        );
        pairs_card = pairs_card.item(CardItem::placed(move |corners| {
            corners.apply(v_flex()).px(corners.inset).py_3().min_h(size::ROW_FLOOR).justify_center().child(row).into_any_element()
        }));
    }
    let add_pair = {
        let form = form.clone();
        Button::add("add-server-pair-add", if command_road { w::ADD_VARIABLE } else { w::ADD_HEADER }).xs().on_click(move |_, _, cx| {
            form.update(cx, |f, cx| {
                f.pairs.push(f.next);
                f.next += 1;
                cx.notify();
            })
        })
    };
    pairs_card = pairs_card.under(add_pair);
    let named: Vec<(String, String)> = typed_pairs.into_iter().filter(|(n, v)| !n.trim().is_empty() || !v.is_empty()).collect();
    let name_typed = keyed::words(&name, cx).trim().to_owned();
    let reach_typed = keyed::words(&reach, cx).trim().to_owned();
    let ready = place.lost.is_none()
        && !name_typed.is_empty()
        && !reach_typed.is_empty()
        && named.iter().all(|(n, _)| !n.trim().is_empty())
        && !f.adding;
    let file = if place.lost.is_none() { file_of(&agent, place.project.as_ref(), k.report) } else { None };
    let submit = {
        let (form, store, place_id, project, agent) =
            (form.clone(), k.store.clone(), k.rows.place.clone(), place.project.clone(), agent.clone());
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
            let words = if command_road { command_words(&reach_typed) } else { Some(Vec::new()) };
            let Some(words) = words else {
                form.update(cx, |f, cx| {
                    f.refused = Some(w::UNCLOSED_QUOTE.into());
                    cx.notify();
                });
                return;
            };
            let mut seen: Vec<String> = Vec::new();
            for (n, _) in &named {
                if seen.contains(&n.trim().to_owned()) {
                    form.update(cx, |f, cx| {
                        f.refused = Some(w::two_pairs_one_name(command_road).into());
                        cx.notify();
                    });
                    return;
                }
                seen.push(n.trim().to_owned());
            }
            let values: serde_json::Map<String, Value> = named.iter().map(|(n, v)| (n.trim().to_owned(), json!(v))).collect();
            let mut params = json!({ "target": row_target(&place_id, project.as_ref()), "agent": agent, "name": name_typed });
            if project.is_some() {
                params["project"] = json!(true);
            }
            if command_road {
                params["command"] = json!(words.first().cloned().unwrap_or_default());
                params["args"] = json!(words.iter().skip(1).collect::<Vec<_>>());
                if !values.is_empty() {
                    params["env"] = Value::Object(values);
                }
            } else {
                params["url"] = json!(reach_typed);
                if !values.is_empty() {
                    params["headers"] = Value::Object(values);
                }
            }
            form.update(cx, |f, cx| {
                f.adding = true;
                f.refused = None;
                cx.notify();
            });
            let (form, place_id) = (form.clone(), place_id.clone());
            store.update(cx, |s, cx| {
                s.call(req::<ServersAddRequest>(params), cx, move |s, answer, cx| {
                    match answer {
                        Ok(_) => {
                            s.acts().open_level(None, cx);
                            form.update(cx, |f, _| *f = ServerForm::default());
                        }
                        Err(said) => form.update(cx, |f, cx| {
                            f.adding = false;
                            f.refused = Some(said);
                            cx.notify();
                        }),
                    }
                    s.refresh(&place_id, cx);
                })
            });
        }
    };
    let mut go = Button::primary("add-server-go", if f.adding { w::ADDING } else { w::ADD_SERVER_GO }).glyph(Glyph::Plus).held(!ready);
    if f.adding {
        go = go.glyph(Glyph::LoaderCircle);
    }
    if ready {
        go = go.on_click(submit);
    }
    let footer = v_flex()
        .gap_1()
        .child(
            h_flex()
                .gap_3()
                .child(div().flex_1().min_w_0().truncate().child(crate::ui::word(file.unwrap_or_default(), WordClass::Fact, cx)))
                .child(go),
        )
        .child(refusal(f.refused.as_deref()));
    v_flex().gap(px(30.)).child(card).child(pairs_card).child(footer).into_any_element()
}

/// What the add server form holds besides its fields' words, kept while it is drawn.
#[derive(Clone, Default)]
struct ServerForm {
    agent: Option<String>,
    road: String,
    pairs: Vec<u32>,
    next: u32,
    picked: Option<(String, String)>,
    adding: bool,
    refused: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mcp(agent: &str, name: &str, line: &str) -> McpRow {
        serde_json::from_value(json!({
            "agent": agent, "name": name, "scope": "user", "transport": { "kind": "stdio", "line": line },
            "envNames": [], "auth": "open", "enabled": true
        }))
        .unwrap()
    }

    #[test]
    fn one_server_set_up_for_two_agents_is_one_entry() {
        let rows = vec![mcp("claude", "docs", "npx docs"), mcp("codex", "docs", "npx docs"), mcp("codex", "other", "npx other")];
        let folded = fold_servers(&rows);
        assert_eq!(folded.len(), 2);
        assert_eq!(folded[0].rows.len(), 2);
        assert_eq!(folded[0].key, "server-global-docs-stdio-npx docs");
    }

    #[test]
    fn a_skill_file_drops_its_frontmatter() {
        assert_eq!(skill_body("---\nname: x\n---\n# Body\n"), "# Body\n");
        assert_eq!(skill_body("# No front\n"), "# No front\n");
        assert_eq!(source_page(Some("owner/name")).as_deref(), Some("https://github.com/owner/name"));
        assert_eq!(source_page(Some("a/b/c")), None);
    }
}
