// SPDX-License-Identifier: AGPL-3.0-only
//! agents-read.ts:1-311, 351-485, 597-647 at 65f6b89c7: where a read runs, the report, and every write there. Spec:
//! `spec/agents_read_targets.md`.
use std::rc::Rc;

use wsp_wire::types::agents_report::{
    AgentRow, AgentsProject, AgentsReport, AgentsTarget, PluginAsk, PluginRow, ServerAdd, ServerAsk, ServerToolsAnswer,
    SkillAdded, SkillHit, SkillPreview,
};
use wsp_wire::types::thread_defaults::AgentSetupSet;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::{AgentsReads, CallbackForwards, OnOrNapping, ResolvedServer, SkillAddAsk, SkillAsk};

impl AgentsReads {
    /// agents-read.ts:286 (A1).
    pub fn changed(&self, target: Option<&AgentsTarget>) {
        todo!()
    }

    /// agents-read.ts:291: whether a registered relay reaches the target.
    pub fn reached(&self, target: &AgentsTarget) -> bool {
        todo!()
    }

    /// agents-read.ts:377-386 (A4): `None` is the TS `{}` (no projects key).
    pub async fn projects_at(&self, place_id: &str, project: Option<&str>, computer: &str, read: bool) -> Result<Option<Vec<AgentsProject>>, HostError> {
        todo!()
    }

    /// agents-read.ts:388-425 (A3).
    pub async fn on_of(&self, target: &AgentsTarget, origin: Option<&Caller>, read: bool) -> Result<OnOrNapping, HostError> {
        todo!()
    }

    /// agents-read.ts:441-457 (A5).
    pub async fn read(&self, target: &AgentsTarget, origin: Option<&Caller>) -> Result<AgentsReport, HostError> {
        todo!()
    }

    /// agents-read.ts:461-470 (A7).
    pub async fn setup(&self, place_id: &str, agent: &str, change: AgentSetupSet, origin: Option<&Caller>) -> Result<AgentRow, HostError> {
        todo!()
    }

    /// agents-read.ts:471-476 (A8).
    pub async fn tools(&self, target: &AgentsTarget, agent: &str, name: &str, refresh: Option<bool>, origin: Option<&Caller>) -> Result<ServerToolsAnswer, HostError> {
        todo!()
    }

    /// agents-read.ts:477-483 (A8).
    pub async fn server(&self, target: &AgentsTarget, agent: &str, name: &str, origin: Option<&Caller>) -> Result<ResolvedServer, HostError> {
        todo!()
    }

    /// agents-read.ts:485 (A9).
    pub fn forget(&self, workspace_id: &str) {
        todo!()
    }

    /// agents-read.ts:606-610 (A14).
    pub async fn key(&self, agent: &str, key: &str, way: Option<&str>) -> Result<(), HostError> {
        todo!()
    }

    /// agents-read.ts:635-642 (A13): answers the config file written.
    pub async fn add_tools(&self, target: &AgentsTarget, agent: &str, origin: Option<&Caller>) -> Result<String, HostError> {
        todo!()
    }

    /// agents-read.ts:643-646 (A15): answers the removal.
    pub fn forwards(self: &Rc<Self>, relay: Rc<dyn CallbackForwards>) -> Box<dyn FnOnce()> {
        todo!()
    }

    /// agents-read.ts:611 (A10): `limit` none is `SKILLS_SEARCH_LIMIT`.
    pub async fn skills_search(&self, q: &str, limit: Option<u32>) -> Result<Vec<SkillHit>, HostError> {
        todo!()
    }

    /// agents-read.ts:612 (A10).
    pub async fn skills_get(&self, skill: &str) -> Result<SkillPreview, HostError> {
        todo!()
    }

    /// agents-read.ts:613 (A10).
    pub async fn skills_preview(&self, target: &AgentsTarget, ask: &SkillAsk, origin: Option<&Caller>) -> Result<SkillPreview, HostError> {
        todo!()
    }

    /// agents-read.ts:614-617 (A10).
    pub async fn skills_add(&self, target: &AgentsTarget, ask: &SkillAddAsk, origin: Option<&Caller>) -> Result<SkillAdded, HostError> {
        todo!()
    }

    /// agents-read.ts:618-621 (A10): answers the removed paths.
    pub async fn skills_remove(&self, target: &AgentsTarget, ask: &SkillAsk, origin: Option<&Caller>) -> Result<Vec<String>, HostError> {
        todo!()
    }

    /// agents-read.ts:622-625 (A10): answers the switched paths.
    pub async fn skills_toggle(&self, target: &AgentsTarget, ask: &SkillAsk, to: bool, origin: Option<&Caller>) -> Result<Vec<String>, HostError> {
        todo!()
    }

    /// agents-read.ts:626 (A11): answers the config file written.
    pub async fn servers_add(&self, target: &AgentsTarget, ask: &ServerAdd, origin: Option<&Caller>) -> Result<String, HostError> {
        todo!()
    }

    /// agents-read.ts:627 (A11).
    pub async fn servers_remove(&self, target: &AgentsTarget, ask: &ServerAsk, origin: Option<&Caller>) -> Result<String, HostError> {
        todo!()
    }

    /// agents-read.ts:629 (A11).
    pub async fn servers_toggle(&self, target: &AgentsTarget, ask: &ServerAsk, to: bool, origin: Option<&Caller>) -> Result<String, HostError> {
        todo!()
    }

    /// agents-read.ts:628 (A11).
    pub async fn servers_icon(&self, host: &str, refresh: Option<bool>) -> Result<Option<String>, HostError> {
        todo!()
    }

    /// agents-read.ts:630-634 (A12).
    pub async fn plugins_toggle(&self, target: &AgentsTarget, ask: &PluginAsk, to: bool, origin: Option<&Caller>) -> Result<PluginRow, HostError> {
        todo!()
    }
}
