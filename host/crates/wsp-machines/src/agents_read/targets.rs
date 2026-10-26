// SPDX-License-Identifier: AGPL-3.0-only
//! agents-read.ts:1-311, 351-485, 597-647 at 65f6b89c7: where a read runs, the report, and every write there. Spec:
//! `spec/agents_read_targets.md`.
use std::future::Future;
use std::rc::Rc;

use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_catalog::catalog::catalog_agents;
use wsp_wire::types::agents_report::{
    AgentRow, AgentsProject, AgentsReport, AgentsTarget, AgentsTargetV0, NappingLit, PageReach,
    PluginAsk, PluginRow, ServerAdd, ServerAsk, ServerToolsAnswer, SkillAdded, SkillHit,
    SkillPreview,
};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::thread_defaults::AgentSetupSet;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::wire::capabilities::PauseMode;
use wsp_wire::wire::machine_link::{is_joined_computer, no_such_place_refusal};
use wsp_words::agents_report::{
    napping_agents_refusal, napping_plugins_refusal, napping_servers_refusal,
    napping_sign_in_refusal, napping_skills_refusal, napping_tools_refusal,
    no_such_agents_project_refusal, provider_agents_refusal, shared_agents_project_refusal,
    without_control_chars,
};
use wsp_words::exit::{HostError, Kind};
use wsp_words::js_date::iso_string;
use wsp_words::place_name::table_name;
use wsp_words::place_word::PlaceRef;
use wsp_words::words::computer::THIS_COMPUTER;

use super::{
    AgentsOn, AgentsPlaces, AgentsReads, BoxExec, BoxLogin, CallbackForwards, Napping, OnOrNapping,
    ResolvedServer, ServerToolsAsk, SkillAddAsk, SkillAsk, SKILLS_SEARCH_LIMIT,
};
use crate::places::types::NO_PLACE_DOOR;

/// `usage(sentence)`: a refusal of kind `usage`.
fn usage(sentence: String) -> HostError {
    HostError::with_kind(sentence, Kind::Usage)
}

/// `JSON.stringify(target)`: the key a target's tools answer is kept under.
fn key_of(target: &AgentsTarget) -> String {
    wsp_wire::text::json_stringify(&serde_json::to_value(target).unwrap_or(Value::Null))
}

fn computer(place_id: &str) -> AgentsTarget {
    AgentsTarget::V0(AgentsTargetV0 {
        place_id: place_id.to_string(),
        project: None,
    })
}

/// `storesOn`: a store map left off where it is absent or empty.
fn stores_on(stores: Option<IndexMap<String, String>>) -> Option<IndexMap<String, String>> {
    stores.filter(|s| !s.is_empty())
}

fn kind_word(kind: PlaceKind) -> &'static str {
    match kind {
        PlaceKind::Computer => "computer",
        PlaceKind::Provider => "provider",
    }
}

fn table_name_of(place: &PlaceView) -> String {
    table_name(&PlaceRef {
        id: place.id.clone(),
        name: place.name.clone(),
        kind: Some(kind_word(place.kind).to_string()),
    })
}

impl AgentsReads {
    pub(crate) fn reader(&self) -> Result<Rc<dyn super::AgentsReader>, HostError> {
        self.o
            .reader
            .clone()
            .ok_or_else(|| HostError::new(super::NO_AGENTS_READER))
    }

    pub(crate) fn acts(&self) -> Result<Rc<dyn super::AgentsActs>, HostError> {
        self.o
            .acts
            .clone()
            .ok_or_else(|| HostError::new(super::NO_AGENTS_READER))
    }

    pub(crate) fn skills(&self) -> Result<Rc<dyn super::SkillsActs>, HostError> {
        self.o
            .skills
            .clone()
            .ok_or_else(|| HostError::new(super::NO_AGENTS_READER))
    }

    pub(crate) fn servers(&self) -> Result<Rc<dyn super::ServersActs>, HostError> {
        self.o
            .servers
            .clone()
            .ok_or_else(|| HostError::new(super::NO_AGENTS_READER))
    }

    /// A write there, after which the report there reads again, whether the write held or stopped halfway.
    pub(crate) async fn written<T>(
        &self,
        target: &AgentsTarget,
        run: impl Future<Output = Result<T, HostError>>,
    ) -> Result<T, HostError> {
        let out = run.await;
        self.changed(Some(target));
        out
    }

    /// Where a skill or a server there is read or written; a napping workspace is never woken for one.
    pub(crate) async fn awake_on(
        &self,
        target: &AgentsTarget,
        napping: fn(&str, Option<&PauseMode>) -> String,
        origin: Option<&Caller>,
    ) -> Result<AgentsOn, HostError> {
        match self.on_of(target, origin, false).await? {
            OnOrNapping::On(on) => Ok(on),
            OnOrNapping::Napping(n) => Err(usage(napping(&n.napping, n.pause_mode.as_ref()))),
        }
    }

    /// agents-read.ts:286 (A1).
    pub fn changed(&self, target: Option<&AgentsTarget>) {
        let mapped = target.map(|t| match t {
            AgentsTarget::V0(place) => computer(&place.place_id),
            other => other.clone(),
        });
        (self.o.changed)(mapped);
    }

    /// agents-read.ts:291: whether a registered relay reaches the target.
    pub fn reached(&self, target: &AgentsTarget) -> bool {
        let relays = self.relays.borrow().clone();
        relays.iter().any(|relay| relay.reaches(target))
    }

    /// agents-read.ts:377-386 (A4): `None` is the TS `{}` (no projects key).
    pub async fn projects_at(
        &self,
        place_id: &str,
        project: Option<&str>,
        computer: &str,
        read: bool,
    ) -> Result<Option<Vec<AgentsProject>>, HostError> {
        if project.is_none() && !read {
            return Ok(None);
        }
        let held = match &self.o.projects {
            Some(projects) => projects(place_id.to_string()).await?,
            None => Vec::new(),
        };
        let Some(wanted) = project else {
            return Ok(Some(held));
        };
        let by_name: Vec<&AgentsProject> = held.iter().filter(|p| p.name == wanted).collect();
        let named = held
            .iter()
            .find(|p| p.id == wanted)
            .or(if by_name.len() == 1 {
                Some(by_name[0])
            } else {
                None
            });
        let said = without_control_chars(wanted);
        match named {
            Some(named) => Ok(Some(vec![named.clone()])),
            None if by_name.len() > 1 => Err(usage(shared_agents_project_refusal(&said, computer))),
            None => Err(usage(no_such_agents_project_refusal(&said, computer))),
        }
    }

    /// agents-read.ts:388-425 (A3).
    pub async fn on_of(
        &self,
        target: &AgentsTarget,
        origin: Option<&Caller>,
        read: bool,
    ) -> Result<OnOrNapping, HostError> {
        match target {
            AgentsTarget::V0(place) => self.on_of_place(target, place, read).await,
            AgentsTarget::V1(ws) => self.on_of_workspace(&ws.workspace_id, origin).await,
        }
    }

    async fn on_of_place(
        &self,
        target: &AgentsTarget,
        place: &AgentsTargetV0,
        read: bool,
    ) -> Result<OnOrNapping, HostError> {
        if place.place_id == HERE_PLACE_ID {
            let stores = match &self.o.stores_here {
                Some(stores_here) => stores_here().await.ok(),
                None => None,
            };
            let projects = self
                .projects_at(
                    &place.place_id,
                    place.project.as_deref(),
                    THIS_COMPUTER,
                    read,
                )
                .await?;
            return Ok(OnOrNapping::On(AgentsOn::Here {
                projects,
                stores: stores_on(stores),
            }));
        }
        let door = (self.o.places)().ok_or_else(|| HostError::new(NO_PLACE_DOOR))?;
        let rows = door.list(self.o.clock.now()).await?;
        let row = match rows.iter().find(|p| p.id == place.place_id) {
            Some(row) => row.clone(),
            None => {
                let held: Vec<String> = rows.iter().map(table_name_of).collect();
                return Err(usage(no_such_place_refusal(&place.place_id, &held)));
            }
        };
        if !is_joined_computer(&row.id, kind_word(row.kind)) {
            return Err(usage(provider_agents_refusal(&table_name_of(&row))));
        }
        let report = door.report_of(&row.id).await?;
        if read {
            door.logins_again(&row.id).await?;
        }
        let agent_ids: Vec<String> = catalog_agents().iter().map(|a| a.id.clone()).collect();
        let sign_ins = door.sign_ins_at(&row.id, &agent_ids);
        let exec: BoxExec = {
            let door: Rc<dyn AgentsPlaces> = door.clone();
            let id = row.id.clone();
            Rc::new(
                move |cmd: String, timeout: Option<u64>, stdin: Option<Vec<u8>>| {
                    let (door, id) = (door.clone(), id.clone());
                    async move { door.exec(&id, &cmd, timeout, stdin).await }.boxed_local()
                },
            )
        };
        let login = BoxLogin {
            home: report.as_ref().and_then(|r| r.login.get("HOME").cloned()),
            path: report.as_ref().and_then(|r| r.login.get("PATH").cloned()),
        };
        let stores = match (&login.home, &self.o.place_stores) {
            (Some(home), Some(place_stores)) => Some(place_stores(&row.id, home)),
            _ => None,
        };
        let relayed = self.reached(target).then_some(true);
        let projects = self
            .projects_at(&place.place_id, place.project.as_deref(), &row.name, read)
            .await?;
        Ok(OnOrNapping::On(AgentsOn::Box {
            name: Some(row.name.clone()),
            exec,
            login,
            sign_ins,
            versions: report.as_ref().and_then(|r| r.agent_versions.clone()),
            setup_rows: row.applied.as_ref().map(|applied| applied.rows.clone()),
            logins: row.logins.clone(),
            relayed,
            projects,
            stores,
        }))
    }

    async fn on_of_workspace(
        &self,
        workspace_id: &str,
        origin: Option<&Caller>,
    ) -> Result<OnOrNapping, HostError> {
        let ws = (self.o.workspace)(workspace_id.to_string(), origin.cloned()).await?;
        if ws.phase == WorkspacePhase::Napping {
            return Ok(OnOrNapping::Napping(Napping {
                napping: ws.name,
                pause_mode: ws.pause_mode,
            }));
        }
        if ws.local {
            let here = match &self.o.stores_here {
                Some(stores_here) => stores_here().await.ok(),
                None => None,
            };
            return Ok(OnOrNapping::On(AgentsOn::Here {
                projects: Some(vec![ws.project]),
                stores: stores_on(here),
            }));
        }
        let relayed = (self.o.relayed.as_ref().map(|f| f()) == Some(true)).then_some(true);
        Ok(OnOrNapping::On(AgentsOn::Machine {
            machine: ws.machine,
            projects: Some(vec![ws.project]),
            relayed,
            brokered: (ws.brokered == Some(true)).then_some(true),
            stores: ws.stores,
            seeds: ws.seeds,
        }))
    }

    /// agents-read.ts:441-457 (A5).
    pub async fn read(
        &self,
        target: &AgentsTarget,
        origin: Option<&Caller>,
    ) -> Result<AgentsReport, HostError> {
        let reader = self.reader()?;
        let on = self.on_of(target, origin, true).await?;
        let on = match on {
            OnOrNapping::Napping(n) => {
                let held = match target {
                    AgentsTarget::V1(ws) => self.last.borrow().get(&ws.workspace_id).cloned(),
                    AgentsTarget::V0(_) => None,
                };
                let Some(held) = held else {
                    return Err(usage(napping_agents_refusal(
                        &n.napping,
                        n.pause_mode.as_ref(),
                    )));
                };
                return Ok(AgentsReport {
                    stale: Some(NappingLit),
                    pause_mode: n.pause_mode,
                    ..held
                });
            }
            OnOrNapping::On(on) => on,
        };
        let latest = match &self.o.latest_on {
            Some(latest_on) => latest_on().await,
            None => true,
        };
        let read = reader.read(&on, latest).await?;
        let reach: PageReach = super::page_reach_of(&on, read.run_as.as_deref());
        let agents: Vec<AgentRow> = read
            .agents
            .into_iter()
            .map(|row| self.with_setup(target, row))
            .collect();
        let report = AgentsReport {
            target: target.clone(),
            home: read.home,
            user: read.user,
            read_at: iso_string(self.o.clock.now() as f64).unwrap_or_default(),
            stale: None,
            pause_mode: None,
            agents,
            skills: read.skills,
            servers: read.servers,
            refused: read.refused,
            plugins: read.plugins,
            projects: read.projects,
            reach: Some(reach),
        };
        if let AgentsTarget::V1(ws) = target {
            self.last
                .borrow_mut()
                .insert(ws.workspace_id.clone(), report.clone());
        }
        Ok(report)
    }

    /// agents-read.ts:449-455: a computer's agent row carries the setup the person set there.
    fn with_setup(&self, target: &AgentsTarget, mut row: AgentRow) -> AgentRow {
        if let AgentsTarget::V0(place) = target {
            if let Some(setup) = self
                .o
                .setup_of
                .as_ref()
                .and_then(|f| f(&place.place_id, &row.id))
            {
                row.setup = Some(setup);
            }
        }
        row
    }

    /// agents-read.ts:461-470 (A7).
    pub async fn setup(
        &self,
        place_id: &str,
        agent: &str,
        change: AgentSetupSet,
        origin: Option<&Caller>,
    ) -> Result<AgentRow, HostError> {
        let write = self
            .o
            .setup_write
            .clone()
            .ok_or_else(|| HostError::new(super::NO_AGENTS_READER))?;
        let target = computer(place_id);
        if let OnOrNapping::Napping(n) = self.on_of(&target, origin, false).await? {
            return Err(usage(napping_agents_refusal(
                &n.napping,
                n.pause_mode.as_ref(),
            )));
        }
        self.written(
            &target,
            write(place_id.to_string(), agent.to_string(), change),
        )
        .await?;
        let report = self.read(&target, origin).await?;
        report
            .agents
            .into_iter()
            .find(|a| a.id == agent)
            .ok_or_else(|| usage(format!("there is no agent {agent} in the catalog")))
    }

    /// agents-read.ts:471-476 (A8).
    pub async fn tools(
        &self,
        target: &AgentsTarget,
        agent: &str,
        name: &str,
        refresh: Option<bool>,
        origin: Option<&Caller>,
    ) -> Result<ServerToolsAnswer, HostError> {
        let reader = self.reader()?;
        let on = match self.on_of(target, origin, false).await? {
            OnOrNapping::On(on) => on,
            OnOrNapping::Napping(n) => {
                return Err(usage(napping_tools_refusal(
                    &n.napping,
                    n.pause_mode.as_ref(),
                )))
            }
        };
        reader
            .tools(
                &on,
                ServerToolsAsk {
                    key: key_of(target),
                    agent: agent.to_string(),
                    name: name.to_string(),
                    refresh,
                },
            )
            .await
    }

    /// agents-read.ts:477-483 (A8).
    pub async fn server(
        &self,
        target: &AgentsTarget,
        agent: &str,
        name: &str,
        origin: Option<&Caller>,
    ) -> Result<ResolvedServer, HostError> {
        let reader = self.reader()?;
        if !reader.has_server() {
            return Err(HostError::new(super::NO_AGENTS_READER));
        }
        let on = match self.on_of(target, origin, false).await? {
            OnOrNapping::On(on) => on,
            OnOrNapping::Napping(n) => {
                return Err(usage(napping_tools_refusal(
                    &n.napping,
                    n.pause_mode.as_ref(),
                )))
            }
        };
        reader.server(&on, agent, name).await
    }

    /// agents-read.ts:485 (A9).
    pub fn forget(&self, workspace_id: &str) {
        self.last.borrow_mut().shift_remove(workspace_id);
    }

    /// agents-read.ts:606-610 (A14).
    pub async fn key(&self, agent: &str, key: &str, way: Option<&str>) -> Result<(), HostError> {
        self.acts()?.key(agent, key, way).await?;
        self.changed(None);
        if let Some(door) = (self.o.places)() {
            door.key_landed(agent).await?;
        }
        Ok(())
    }

    /// agents-read.ts:635-642 (A13): answers the config file written.
    pub async fn add_tools(
        &self,
        target: &AgentsTarget,
        agent: &str,
        origin: Option<&Caller>,
    ) -> Result<String, HostError> {
        let acts = self.acts()?;
        let on = self
            .awake_on(target, napping_sign_in_refusal, origin)
            .await?;
        let added = acts.add_tools(&on, agent).await?;
        self.changed(Some(target));
        Ok(added)
    }

    /// agents-read.ts:643-646 (A15): answers the removal.
    pub fn forwards(self: &Rc<Self>, relay: Rc<dyn CallbackForwards>) -> Box<dyn FnOnce()> {
        {
            let mut relays = self.relays.borrow_mut();
            if !relays.iter().any(|held| Rc::ptr_eq(held, &relay)) {
                relays.push(relay.clone());
            }
        }
        let me = self.clone();
        Box::new(move || {
            me.relays
                .borrow_mut()
                .retain(|held| !Rc::ptr_eq(held, &relay))
        })
    }

    /// agents-read.ts:611 (A10): `limit` none is `SKILLS_SEARCH_LIMIT`.
    pub async fn skills_search(
        &self,
        q: &str,
        limit: Option<u32>,
    ) -> Result<Vec<SkillHit>, HostError> {
        self.skills()?
            .search(q, limit.unwrap_or(SKILLS_SEARCH_LIMIT))
            .await
    }

    /// agents-read.ts:612 (A10).
    pub async fn skills_get(&self, skill: &str) -> Result<SkillPreview, HostError> {
        self.skills()?.get(skill).await
    }

    /// agents-read.ts:613 (A10).
    pub async fn skills_preview(
        &self,
        target: &AgentsTarget,
        ask: &SkillAsk,
        origin: Option<&Caller>,
    ) -> Result<SkillPreview, HostError> {
        let skills = self.skills()?;
        let on = self
            .awake_on(target, napping_skills_refusal, origin)
            .await?;
        skills.preview(&on, ask).await
    }

    /// agents-read.ts:614-617 (A10).
    pub async fn skills_add(
        &self,
        target: &AgentsTarget,
        ask: &SkillAddAsk,
        origin: Option<&Caller>,
    ) -> Result<SkillAdded, HostError> {
        let skills = self.skills()?;
        let on = self
            .awake_on(target, napping_skills_refusal, origin)
            .await?;
        self.written(target, skills.add(&on, ask)).await
    }

    /// agents-read.ts:618-621 (A10): answers the removed paths.
    pub async fn skills_remove(
        &self,
        target: &AgentsTarget,
        ask: &SkillAsk,
        origin: Option<&Caller>,
    ) -> Result<Vec<String>, HostError> {
        let skills = self.skills()?;
        let on = self
            .awake_on(target, napping_skills_refusal, origin)
            .await?;
        self.written(target, skills.remove(&on, ask)).await
    }

    /// agents-read.ts:622-625 (A10): answers the switched paths.
    pub async fn skills_toggle(
        &self,
        target: &AgentsTarget,
        ask: &SkillAsk,
        to: bool,
        origin: Option<&Caller>,
    ) -> Result<Vec<String>, HostError> {
        let skills = self.skills()?;
        let on = self
            .awake_on(target, napping_skills_refusal, origin)
            .await?;
        self.written(target, skills.toggle(&on, ask, to)).await
    }

    /// agents-read.ts:626 (A11): answers the config file written.
    pub async fn servers_add(
        &self,
        target: &AgentsTarget,
        ask: &ServerAdd,
        origin: Option<&Caller>,
    ) -> Result<String, HostError> {
        let servers = self.servers()?;
        let on = self
            .awake_on(target, napping_servers_refusal, origin)
            .await?;
        self.written(target, servers.add(&on, ask)).await
    }

    /// agents-read.ts:627 (A11).
    pub async fn servers_remove(
        &self,
        target: &AgentsTarget,
        ask: &ServerAsk,
        origin: Option<&Caller>,
    ) -> Result<String, HostError> {
        let servers = self.servers()?;
        let on = self
            .awake_on(target, napping_servers_refusal, origin)
            .await?;
        self.written(target, servers.remove(&on, ask)).await
    }

    /// agents-read.ts:629 (A11).
    pub async fn servers_toggle(
        &self,
        target: &AgentsTarget,
        ask: &ServerAsk,
        to: bool,
        origin: Option<&Caller>,
    ) -> Result<String, HostError> {
        let servers = self.servers()?;
        let on = self
            .awake_on(target, napping_servers_refusal, origin)
            .await?;
        self.written(target, servers.toggle(&on, ask, to)).await
    }

    /// agents-read.ts:628 (A11).
    pub async fn servers_icon(
        &self,
        host: &str,
        refresh: Option<bool>,
    ) -> Result<Option<String>, HostError> {
        match &self.o.icons {
            Some(icons) => icons.icon(host, refresh).await,
            None => Ok(None),
        }
    }

    /// agents-read.ts:630-634 (A12).
    pub async fn plugins_toggle(
        &self,
        target: &AgentsTarget,
        ask: &PluginAsk,
        to: bool,
        origin: Option<&Caller>,
    ) -> Result<PluginRow, HostError> {
        let plugins = self
            .o
            .plugins
            .clone()
            .ok_or_else(|| HostError::new(super::NO_AGENTS_READER))?;
        let on = self
            .awake_on(target, napping_plugins_refusal, origin)
            .await?;
        self.written(target, plugins.toggle(&on, ask, to)).await
    }
}
