// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/provision-mcp.ts at a4033dc34. Spec: spec/provision_mcp.md.
//!
//! The MCP servers the recipe names, on a computer somebody owns. The file an agent keeps its servers in is that
//! agent's own from its first turn there, so nothing here writes it as a file: the copy that travelled is read out of
//! the job's folder, the agent's own file off the computer, and the recipe's servers are merged in key by key. What
//! wsp wrote it writes down one line per key beside the job, and a name it planned and wrote nowhere as a tombstone.
use std::collections::BTreeSet;
use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};

use wsp_catalog::catalog::mcp_agents;
use wsp_catalog::mcp::{McpMergeResult, McpMergeScope, McpServerScope, MergeOutcome};
use wsp_catalog::mcp_launch::{launch_misses, own_server_config, unset_toml_keys, OwnConfig};
use wsp_catalog::mcp_travelled::FromTo;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::place::{PlaceProvisionKind, PlaceProvisionRow, PlaceProvisionRowOutcome};
use wsp_words::agents_report::config_unread_line;
use wsp_words::project_path::place_provision_paths;

use crate::exec_detached::INLINE_EXEC_MS;
use crate::golden_seam::{
    absent_commands, land_configs, mcp_opening, mcp_rows, parse_configs, parse_mcp_id, read_configs_cmd, read_failed, refused,
    rewrite_string, scope_row_id, without_absent, DropName, EditedName, EditedOutcome, GoldenStage, McpAgentPlan, McpPlan, McpResult,
    McpRowsOptions, McpScope, ProjectMove, ReadAsk, ScopeOutcome, ScopeRead, StageListener, ToolResult, READ_MS,
};
use crate::machine::{ExecOptions, Machine, RunOptions};
use crate::provision_files::{append_landing, landed_servers, server_digest, FilesSay, OwnedHow, OwnedPaths, NO_DIGEST};

/// provision-mcp.ts:45 `atHome`: the plan as it reads where the login's home is `home`. The plan itself (same
/// value) where `home` is its `guest_home`; else every path equal to the old home or under it moves under the new
/// one: each rewrite's target (a trailing `/` kept), every scope's files and its project's `to`.
pub fn at_home(plan: &McpPlan, home: &str) -> McpPlan {
    if home == plan.guest_home {
        return plan.clone();
    }
    let guest = plan.guest_home.as_str();
    let under = format!("{guest}/");
    let moved = |path: &str| -> String {
        if path == guest {
            home.to_string()
        } else if let Some(rest) = path.strip_prefix(under.as_str()) {
            format!("{home}/{rest}")
        } else {
            path.to_string()
        }
    };
    McpPlan {
        agents: plan
            .agents
            .iter()
            .map(|agent| McpAgentPlan {
                scopes: agent
                    .scopes
                    .iter()
                    .map(|scope| McpScope {
                        files: scope.files.iter().map(|f| moved(f)).collect(),
                        project: scope.project.as_ref().map(|p| ProjectMove { from: p.from.clone(), to: moved(&p.to) }),
                        ..scope.clone()
                    })
                    .collect(),
                ..agent.clone()
            })
            .collect(),
        guest_home: home.to_string(),
        rewrites: plan
            .rewrites
            .iter()
            .map(|(from, to)| {
                let trimmed = to.strip_suffix('/').unwrap_or(to);
                let slash = if to.ends_with('/') { "/" } else { "" };
                (from.clone(), format!("{}{slash}", moved(trimmed)))
            })
            .collect(),
        bin_dirs: plan.bin_dirs.clone(),
        tools: plan.tools.clone(),
    }
}

/// provision-mcp.ts:61 `theirServerLine`.
pub fn their_server_line(agent: &str, name: &str, path: &str) -> String {
    format!("{name} in {path} is {agent}'s own under that name; wsp does not write over it")
}

/// provision-mcp.ts:65 `keyUnreachedLine`: the names joined by `, `.
pub fn key_unreached_line(agent: &str, names: &[String]) -> String {
    format!("its key in {} does not reach {agent}'s threads on that computer yet", names.join(", "))
}

/// provision-mcp.ts:69 `untrustedLine`.
pub fn untrusted_line(agent: &str, folder: &str, why: &str) -> String {
    format!("{agent} reads them only in a folder it trusts, and {folder} could not be marked trusted ({why})")
}

/// provision-mcp.ts:74 `noCopyLine`.
pub fn no_copy_line(path: &str) -> String {
    format!("nothing of this computer's copy of {path} arrived this run")
}

/// provision-mcp.ts:78 `travelledPath`: `<staging>/<path below home>` for a file under `home/`; `None` otherwise.
pub fn travelled_path(home: &str, file: &str) -> Option<String> {
    let rest = file.strip_prefix(format!("{home}/").as_str())?;
    Some(format!("{}/{rest}", place_provision_paths(home).staging))
}

/// provision-mcp.ts:95 `serverLine`: `<id>\t<digest>\t<digest>`.
pub fn server_line(id: &str, digest: &str) -> String {
    format!("{id}\t{digest}\t{digest}")
}

/// provision-mcp.ts:100 `tombstoneLine`: `<id>\t-\t-`.
pub fn tombstone_line(id: &str) -> String {
    format!("{id}\t{NO_DIGEST}\t{NO_DIGEST}")
}

/// provision-mcp.ts:127 `headlessKeys(machine, home, stores, agents)`: the words of each config that did not take
/// its agent's headless keys, one per agent, in catalog (MCP_AGENTS) order.
pub async fn headless_keys(machine: &dyn Machine, home: &str, stores: &IndexMap<String, String>, agents: &BTreeSet<String>) -> Vec<String> {
    let mut failed = Vec::new();
    for agent in mcp_agents() {
        let Some(mcp) = &agent.mcp else {
            continue;
        };
        let (Some(keys), Some(store)) = (&mcp.headless, stores.get(&agent.id)) else {
            continue;
        };
        let make =
            if agents.contains(&agent.id) { format!("mkdir -p {}", shell_quote(store)) } else { format!("[ -d {} ]", shell_quote(store)) };
        let script = format!("[ \"$(uname -s)\" = Linux ] && {make}");
        let there = machine.exec(&script, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await;
        if !matches!(&there, Ok(res) if res.exit_code.get() == 0) {
            continue;
        }
        let config = own_server_config(&agent.id, &mcp.files, home, Some(store));
        let file = read_config(machine, &config.files, &config.base).await;
        if let Some(ScopeRead::Unread { path, unread }) = &file {
            failed.push(format!("{}: {}", agent.name, config_unread_line(path, unread)));
            continue;
        }
        let text = match &file {
            Some(ScopeRead::File(f)) => f.text.clone(),
            _ => String::new(),
        };
        let missing = unset_toml_keys(&text, keys);
        if missing.is_empty() {
            continue;
        }
        let next = format!("{}{text}", missing.iter().map(|k| format!("{} = {}\n", k.key, k.value)).collect::<String>());
        let path = match &file {
            Some(ScopeRead::File(f)) => f.path.clone(),
            _ => config.files.first().cloned().unwrap_or_default(),
        };
        let texts = IndexMap::from([(path, next)]);
        let read: Vec<Option<ScopeRead>> = file.clone().into_iter().map(Some).collect();
        if let Some(why) = land_configs(machine, &|_: &str| config.base.clone(), &read, &texts).await {
            failed.push(format!("{}: {why}", agent.name));
        }
    }
    failed
}

/// Called once per file the merge read, with the names of this computer's servers standing in it.
pub type IntoReport = Arc<dyn Fn(&str, &[String])>;

/// provision-mcp.ts:161 `provisionMcp`'s options.
#[derive(Clone)]
pub struct ProvisionMcpOptions {
    pub home: String,
    pub landed: OwnedPaths,
    pub tools: Vec<ToolResult>,
    pub stage: StageListener,
    pub path: String,
    /// `None` is the TS's absent `held`.
    pub held: Option<IndexSet<String>>,
    /// `None` is the TS's absent `stores`.
    pub stores: Option<IndexMap<String, String>>,
    /// Called once per file the merge read, with the names of this computer's servers standing in it.
    pub into: Option<IntoReport>,
}

/// provision-mcp.ts:158 `provisionMcp`: one row per server of the plan, in mcpRows' order. Never fails: a read or a
/// write that did not happen is a reason on each skipped row.
pub async fn provision_mcp(machine: &dyn Machine, planned: &McpPlan, o: ProvisionMcpOptions) -> Vec<PlaceProvisionRow> {
    let plan = at_home(planned, &o.home);
    (o.stage)(GoldenStage::InstallingMcp, Some(mcp_opening(&plan.agents).as_str()));
    let scopes: Vec<&McpScope> = plan.agents.iter().flat_map(|a| a.scopes.iter()).collect();
    let n = scopes.len();
    let configs: Vec<Option<OwnConfig>> = plan
        .agents
        .iter()
        .flat_map(|a| {
            a.scopes.iter().map(|s| {
                s.own
                    .as_ref()
                    .map(|own| OwnConfig { files: own.files.clone(), base: own.base.clone() })
                    .or_else(|| store_config(&a.id, &o.home, o.stores.as_ref()))
            })
        })
        .collect();
    let mut bases: IndexMap<String, String> = IndexMap::new();
    for config in configs.iter().flatten() {
        for file in &config.files {
            bases.insert(file.clone(), config.base.clone());
        }
    }
    let candidates: Vec<Vec<Candidate>> = scopes
        .iter()
        .enumerate()
        .map(|(at, s)| {
            if s.travelled.is_some() {
                let files = configs[at].as_ref().map_or_else(|| s.files.clone(), |c| c.files.clone());
                files.into_iter().map(|own| Candidate { own, travelled: None }).collect()
            } else {
                s.files
                    .iter()
                    .filter_map(|file| {
                        let travelled = travelled_path(&o.home, file)?;
                        let own = configs[at].as_ref().and_then(|c| c.files.first().cloned()).unwrap_or_else(|| file.clone());
                        Some(Candidate { own, travelled: Some(travelled) })
                    })
                    .collect()
            }
        })
        .collect();
    let mut asks: Vec<ReadAsk> = candidates
        .iter()
        .map(|c| ReadAsk {
            files: c.iter().map(|x| x.own.clone()).collect(),
            base: c.first().and_then(|x| bases.get(&x.own).cloned()).unwrap_or_else(|| o.home.clone()),
        })
        .collect();
    asks.extend(candidates.iter().map(|c| ReadAsk { files: c.iter().filter_map(|x| x.travelled.clone()).collect(), base: o.home.clone() }));
    let res = match machine.run(&read_configs_cmd(&asks), RunOptions { deadline_ms: READ_MS, unlogged: true, ..Default::default() }).await {
        Ok(res) => res,
        Err(e) => refused(&e.to_string()),
    };
    let read: Option<Vec<Option<ScopeRead>>> = if res.exit_code.get() == 0 { parse_configs(&res.stdout, &asks) } else { None };
    let slot = |i: usize| read.as_ref().and_then(|r| r.get(i).cloned().flatten());
    let own: Vec<Option<ScopeRead>> = (0..n).map(slot).collect();
    let copies: Vec<Option<ScopeRead>> = (0..n).map(|at| slot(n + at)).collect();
    let arrived: Vec<Option<Arrived>> = (0..n)
        .map(|at| match &scopes[at].travelled {
            Some(text) => Some(Arrived { path: String::new(), text: text.clone() }),
            None => match &copies[at] {
                Some(ScopeRead::File(f)) => Some(Arrived { path: f.path.clone(), text: f.text.clone() }),
                _ => None,
            },
        })
        .collect();
    let unread: Vec<Option<(String, String)>> = (0..n)
        .map(|at| {
            [&own[at], &copies[at]].into_iter().find_map(|f| match f {
                Some(ScopeRead::Unread { path, unread }) => Some((path.clone(), unread.clone())),
                _ => None,
            })
        })
        .collect();
    let target: Vec<Option<String>> = (0..n)
        .map(|at| match &own[at] {
            Some(f) => Some(path_of(f).to_string()),
            None if scopes[at].travelled.is_some() => candidates[at].first().map(|c| c.own.clone()),
            None => {
                let travelled = arrived[at].as_ref().map(|a| a.path.as_str());
                candidates[at].iter().find(|c| c.travelled.as_deref() == travelled).map(|c| c.own.clone())
            }
        })
        .collect();
    let mut stood: IndexMap<String, String> = IndexMap::new();
    for at in 0..n {
        if let (Some(path), Some(ScopeRead::File(f))) = (&target[at], &own[at]) {
            stood.insert(path.clone(), f.text.clone());
        }
    }
    let say: FilesSay = {
        let stage = o.stage.clone();
        Arc::new(move |line: &str| stage(GoldenStage::InstallingMcp, Some(line)))
    };
    let owned = landed_servers(machine, &o.home, say).await;
    let round = Round { plan: &plan, landed: &o.landed, target, arrived, unread, stood, owned };

    let mut missing: IndexSet<String> = IndexSet::new();
    let mut agents = plan.agents.clone();
    let mut merged: Option<Merged> = None;
    let mut failure: Option<String> = if read.is_none() { Some(read_failed(&res)) } else { None };
    if read.is_some() {
        let first = merge_all(&round, &agents);
        agents = set_aside(&agents, &first);
        for command in absent_commands(machine, &plan, &first.outcomes, &o.path).await {
            missing.insert(command);
        }
        agents = without_absent(&plan, &agents, &first.outcomes, &missing, &o.tools);
        let second = merge_all(&round, &agents);
        let base_of = |path: &str| bases.get(path).cloned().unwrap_or_else(|| o.home.clone());
        failure = land_configs(machine, &base_of, &own, &second.texts).await;
        merged = Some(second);
    }

    let present: IndexSet<String> = match (&failure, &merged) {
        (None, Some(m)) => {
            m.same.iter().filter(|id| !round.arrived_whole(m.file_of.get(id.as_str()).map(String::as_str))).cloned().collect()
        }
        _ => IndexSet::new(),
    };
    let results: Vec<McpResult> = mcp_rows(
        machine,
        &plan,
        McpRowsOptions {
            agents: agents.clone(),
            report: merged.as_ref().map(|m| m.outcomes.clone()),
            failure: failure.clone(),
            missing: missing.clone(),
            present: present.clone(),
            stage: o.stage.clone(),
        },
    )
    .await;

    let mut unreached: IndexMap<String, String> = IndexMap::new();
    if let (None, Some(m), Some(held)) = (&failure, &merged, &o.held) {
        if !held.is_empty() {
            let mut at = 0;
            for agent in &agents {
                for scope in &agent.scopes {
                    let path = round.target[at].clone();
                    at += 1;
                    let Some(text) = path.and_then(|p| m.texts.get(&p).or_else(|| round.stood.get(&p)).cloned()) else {
                        continue;
                    };
                    let project_to = scope.project.as_ref().map_or(o.home.as_str(), |p| p.to.as_str());
                    let servers = scope.format.read(&text, project_to);
                    for name in &scope.keep {
                        let server =
                            servers.iter().find(|s| s.name == *name && matches!(s.scope, McpServerScope::Home) == scope.project.is_some());
                        let misses = match server {
                            Some(server) if server.disabled != Some(true) => launch_misses(&agent.id, scope.format, server, held),
                            _ => Vec::new(),
                        };
                        if !misses.is_empty() {
                            unreached.insert(
                                scope_row_id(&agent.id, scope.project.as_ref(), scope.folder.as_deref(), name),
                                key_unreached_line(&agent.label, &misses),
                            );
                        }
                    }
                }
            }
        }
    }

    if let (Some(m), Some(into)) = (&merged, &o.into) {
        let mut per_file: IndexMap<String, IndexSet<String>> = m.file_of.values().map(|f| (f.clone(), IndexSet::new())).collect();
        if failure.is_none() {
            for id in m.records.keys() {
                if let (Some(name), Some(file)) = (parse_mcp_id(id).ok().flatten().map(|p| p.name), m.file_of.get(id)) {
                    if let Some(names) = per_file.get_mut(file) {
                        names.insert(name);
                    }
                }
            }
        }
        for (file, names) in &per_file {
            let names: Vec<String> = names.iter().cloned().collect();
            into(file.as_str(), names.as_slice());
        }
    }

    if let (None, Some(m)) = (&failure, &merged) {
        let lines: Vec<String> =
            m.records.iter().map(|(id, digest)| server_line(id, digest)).chain(m.tombstones.iter().map(|id| tombstone_line(id))).collect();
        append_landing(machine, &o.home, &lines).await;
    }

    results
        .into_iter()
        .map(|r| {
            let outcome = if r.outcome == "skipped" {
                PlaceProvisionRowOutcome::Skipped
            } else if present.contains(&r.id) {
                PlaceProvisionRowOutcome::Present
            } else {
                PlaceProvisionRowOutcome::Installed
            };
            let own_note = (outcome != PlaceProvisionRowOutcome::Present).then(|| r.note.clone()).flatten();
            let unreached_note = (outcome != PlaceProvisionRowOutcome::Skipped).then(|| unreached.get(&r.id).cloned()).flatten();
            let note = [own_note, unreached_note].into_iter().flatten().collect::<Vec<String>>().join("; ");
            PlaceProvisionRow {
                label: format!("{} {}", r.agent, r.name),
                id: r.id,
                outcome,
                kind: Some(PlaceProvisionKind::Server),
                step: None,
                note: (!note.is_empty()).then_some(note),
                fix: None,
                keys: r.keys,
                ms: None,
                earlier: None,
                project: None,
            }
        })
        .collect()
}

struct Candidate {
    own: String,
    travelled: Option<String>,
}

struct Arrived {
    path: String,
    text: String,
}

struct Round<'a> {
    plan: &'a McpPlan,
    landed: &'a OwnedPaths,
    target: Vec<Option<String>>,
    arrived: Vec<Option<Arrived>>,
    unread: Vec<Option<(String, String)>>,
    stood: IndexMap<String, String>,
    owned: IndexMap<String, String>,
}

impl Round<'_> {
    fn arrived_whole(&self, path: Option<&str>) -> bool {
        path.is_some_and(|p| matches!(self.landed.get(p), Some(OwnedHow::Installed)))
    }
}

struct Wrote {
    id: String,
    name: String,
    scope: McpScope,
    path: String,
}

struct Merged {
    outcomes: Vec<ScopeOutcome>,
    texts: IndexMap<String, String>,
    records: IndexMap<String, String>,
    tombstones: Vec<String>,
    same: IndexSet<String>,
    theirs: IndexMap<String, String>,
    no_copy: IndexMap<String, String>,
    file_of: IndexMap<String, String>,
    untrusted: IndexMap<String, String>,
}

fn store_config(agent: &str, home: &str, stores: Option<&IndexMap<String, String>>) -> Option<OwnConfig> {
    let store = stores?.get(agent)?;
    let entry = mcp_agents().into_iter().find(|a| a.id == agent)?;
    Some(own_server_config(agent, &entry.mcp.as_ref()?.files, home, Some(store)))
}

// MachineServerPort takes an Arc of the machine, which a borrowed machine cannot give, so its read is made here.
async fn read_config(machine: &dyn Machine, files: &[String], base: &str) -> Option<ScopeRead> {
    let asked = [ReadAsk { files: files.to_vec(), base: base.to_string() }];
    let options = RunOptions { deadline_ms: READ_MS, unlogged: true, ..Default::default() };
    let res = match machine.run(&read_configs_cmd(&asked), options).await {
        Ok(res) if res.exit_code.get() == 0 => res,
        _ => return None,
    };
    parse_configs(&res.stdout, &asked).and_then(|scopes| scopes.into_iter().next().flatten())
}

fn path_of(file: &ScopeRead) -> &str {
    match file {
        ScopeRead::File(f) => f.path.as_str(),
        ScopeRead::Unread { path, .. } => path.as_str(),
    }
}

fn entry_digest(scope: &McpScope, text: Option<&str>, name: &str) -> Option<String> {
    let entry = text.and_then(|t| scope.format.entry_of(t, name, scope.project.as_ref().map(|p| p.to.as_str())));
    server_digest(entry.as_deref())
}

fn edited_outcome(outcome: MergeOutcome) -> EditedOutcome {
    match outcome {
        MergeOutcome::Added => EditedOutcome::Added,
        MergeOutcome::Replaced => EditedOutcome::Replaced,
        MergeOutcome::Same => EditedOutcome::Same,
        MergeOutcome::Theirs => EditedOutcome::Theirs,
        MergeOutcome::Missing => EditedOutcome::Missing,
        MergeOutcome::Dropped => EditedOutcome::Dropped,
        MergeOutcome::Left => EditedOutcome::Left,
    }
}

fn merge_scope(
    plan: &McpPlan,
    scope: &McpScope,
    names: &[String],
    replace: Vec<String>,
    standing: Option<&str>,
    arrived: &str,
) -> Result<(String, Vec<McpMergeResult>), String> {
    let (text, results) = if names.is_empty() {
        (standing.unwrap_or("").to_string(), Vec::new())
    } else {
        let lib = |s: &str, command: bool| rewrite_string(plan, s, command);
        let input = McpMergeScope {
            keep: scope.keep.clone(),
            drop: scope.drop.iter().map(|d| d.name.clone()).collect(),
            project: scope.project.as_ref().map(|p| FromTo { from: p.from.clone(), to: p.to.clone() }),
            replace,
        };
        let merged = scope.format.merge(&lib, &input, standing, arrived).map_err(|e| e.to_string())?;
        (merged.text, merged.results)
    };
    let trusted = match scope.trust.as_deref().and_then(|folder| scope.format.trust(Some(&text), folder)) {
        Some(done) => done.map_err(|e| e.to_string())?,
        None => text,
    };
    let edited = match &scope.edit {
        Some(edit) => edit(if trusted.is_empty() { None } else { Some(trusted.as_str()) }),
        None => trusted,
    };
    Ok((edited, results))
}

fn merge_all(round: &Round<'_>, agents: &[McpAgentPlan]) -> Merged {
    let mut texts: IndexMap<String, String> = IndexMap::new();
    let mut outcomes: Vec<ScopeOutcome> = Vec::new();
    let mut same: IndexSet<String> = IndexSet::new();
    let mut theirs: IndexMap<String, String> = IndexMap::new();
    let mut no_copy: IndexMap<String, String> = IndexMap::new();
    let mut file_of: IndexMap<String, String> = IndexMap::new();
    let mut untrusted: IndexMap<String, String> = IndexMap::new();
    let mut wrote: Vec<Wrote> = Vec::new();
    let mut planned: Vec<String> = Vec::new();
    let mut at = 0;
    for agent in agents {
        for scope in &agent.scopes {
            let here = at;
            at += 1;
            let id = |name: &str| scope_row_id(&agent.id, scope.project.as_ref(), scope.folder.as_deref(), name);
            let Some(path) = round.target[here].clone() else {
                outcomes.push(ScopeOutcome::default());
                continue;
            };
            if let Some((file, why)) = &round.unread[here] {
                outcomes.push(ScopeOutcome { file: Some(file.clone()), unread: Some(why.clone()), ..Default::default() });
                continue;
            }
            let names: Vec<String> = scope.keep.iter().cloned().chain(scope.drop.iter().map(|d| d.name.clone())).collect();
            for name in &names {
                file_of.insert(id(name.as_str()), path.clone());
            }
            let standing: Option<String> = texts.get(&path).or_else(|| round.stood.get(&path)).cloned();
            let standing_text = standing.as_deref();
            match round.arrived[here].as_ref() {
                None => {
                    let mut results = Vec::new();
                    for name in &scope.keep {
                        if entry_digest(scope, standing_text, name).is_none() {
                            no_copy.insert(id(name.as_str()), path.clone());
                            results.push(EditedName { name: name.clone(), outcome: EditedOutcome::Missing, command: None });
                        } else {
                            same.insert(id(name.as_str()));
                            results.push(EditedName { name: name.clone(), outcome: EditedOutcome::Same, command: None });
                        }
                    }
                    for d in &scope.drop {
                        results.push(EditedName { name: d.name.clone(), outcome: EditedOutcome::Left, command: None });
                    }
                    outcomes.push(ScopeOutcome { file: Some(path), results, ..Default::default() });
                }
                Some(arrived) => {
                    let replace: Vec<String> = if round.arrived_whole(Some(path.as_str())) {
                        names.clone()
                    } else {
                        names
                            .iter()
                            .filter(|name| {
                                round
                                    .owned
                                    .get(&id(name.as_str()))
                                    .is_some_and(|left| entry_digest(scope, standing_text, name).as_deref() == Some(left.as_str()))
                            })
                            .cloned()
                            .collect()
                    };
                    match merge_scope(round.plan, scope, &names, replace, standing_text, &arrived.text) {
                        Ok((edited, results)) => {
                            if edited.as_str() != standing_text.unwrap_or("") {
                                texts.insert(path.clone(), edited);
                            }
                            planned.extend(names.iter().map(|name| id(name.as_str())));
                            for r in &results {
                                let rid = id(r.name.as_str());
                                if r.outcome == MergeOutcome::Same {
                                    same.insert(rid.clone());
                                }
                                if r.outcome == MergeOutcome::Theirs {
                                    theirs.insert(rid.clone(), path.clone());
                                }
                                if matches!(r.outcome, MergeOutcome::Added | MergeOutcome::Replaced | MergeOutcome::Same) {
                                    wrote.push(Wrote { id: rid, name: r.name.clone(), scope: scope.clone(), path: path.clone() });
                                }
                            }
                            let results = results
                                .into_iter()
                                .map(|r| EditedName { name: r.name, outcome: edited_outcome(r.outcome), command: r.command })
                                .collect();
                            outcomes.push(ScopeOutcome { file: Some(path), results, ..Default::default() });
                        }
                        Err(said) => {
                            if let Some(folder) = &scope.trust {
                                untrusted.insert(format!("{}\0{folder}", agent.id), said.clone());
                            }
                            outcomes.push(ScopeOutcome { file: Some(path), error: Some(said), ..Default::default() });
                        }
                    }
                }
            }
        }
    }
    let mut records: IndexMap<String, String> = IndexMap::new();
    for w in &wrote {
        let text = texts.get(&w.path).or_else(|| round.stood.get(&w.path)).map(String::as_str);
        if let Some(digest) = entry_digest(&w.scope, text, &w.name) {
            records.insert(w.id.clone(), digest);
        }
    }
    let tombstones: Vec<String> = planned.iter().filter(|id| !records.contains_key(id.as_str())).cloned().collect();
    Merged { outcomes, texts, records, tombstones, same, theirs, no_copy, file_of, untrusted }
}

fn set_aside(agents: &[McpAgentPlan], first: &Merged) -> Vec<McpAgentPlan> {
    agents
        .iter()
        .map(|agent| McpAgentPlan {
            scopes: agent
                .scopes
                .iter()
                .map(|scope| {
                    let folder = scope.folder.as_deref();
                    let distrust = folder.and_then(|f| first.untrusted.get(&format!("{}\0{f}", agent.id)));
                    let aside: Vec<DropName> = scope
                        .keep
                        .iter()
                        .filter_map(|name| {
                            if let (Some(f), Some(why)) = (folder, distrust) {
                                return Some(DropName { name: name.clone(), reason: untrusted_line(&agent.label, f, why), keys: None });
                            }
                            let rid = scope_row_id(&agent.id, scope.project.as_ref(), folder, name);
                            if let Some(path) = first.theirs.get(&rid) {
                                return Some(DropName {
                                    name: name.clone(),
                                    reason: their_server_line(&agent.label, name, path),
                                    keys: None,
                                });
                            }
                            first.no_copy.get(&rid).map(|path| DropName { name: name.clone(), reason: no_copy_line(path), keys: None })
                        })
                        .collect();
                    if aside.is_empty() {
                        return scope.clone();
                    }
                    let mut next = scope.clone();
                    next.keep.retain(|name| !aside.iter().any(|a| a.name == *name));
                    next.drop.extend(aside);
                    next
                })
                .collect(),
            ..agent.clone()
        })
        .collect()
}
