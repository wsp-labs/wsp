// SPDX-License-Identifier: AGPL-3.0-only
//! agents_launch: config folders, files a turn carries, environments, servers, the adapter. agents.ts:325-681.
//! Spec: spec/agents_launch.md.
use std::sync::Arc;

use indexmap::IndexMap;
use wsp_adapters::port::{AttachmentRoad, Env, TurnImage};
use wsp_catalog::mcp_launch::{LaunchConfigs, LaunchServerRoad};
use wsp_core::machine_exec::TurnWaiting;
use wsp_core::types::harness::HarnessAdapter;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::thread_defaults::{AccessChoice, ProjectOverrides, ResolvedFolder};
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::super::threads::Entry;
use super::AgentsArea;

pub const INLINE_EXEC_MS: u64 = 20_000;

/// `{ images, dir? }` (agents.ts:448).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LandedImages {
    pub images: Vec<TurnImage>,
    pub dir: Option<String>,
}

/// `{ road, configs }` (agents.ts:534).
pub struct TurnConfigs {
    pub road: LaunchServerRoad,
    pub configs: LaunchConfigs,
}

/// `{ catalog, open }` (agents.ts:421).
#[derive(Clone, Debug, PartialEq)]
pub struct DefaultsOn {
    pub catalog: HarnessCatalog,
    pub open_model: Option<String>,
    pub open_effort: Option<String>,
}

/// `Pick<HarnessStartOptions, "serverValues" | "offServers">` (agents.ts:615).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchServers {
    pub server_values: Option<IndexMap<String, Env>>,
    pub off_servers: Option<Vec<String>>,
}

/// What `adapter_for` takes beside the entry (agents.ts:632).
#[derive(Clone, Default)]
pub struct AdapterAsk {
    pub named: Option<String>,
    pub turn_env: Option<Env>,
    pub waiting: Option<TurnWaiting>,
    pub servers: Env,
    pub thread: Option<String>,
    pub asks_until_stopped: bool,
}

impl AgentsArea {
    /// agents.ts:327: A1.
    pub fn setup_place(entry: &Entry) -> Option<String> {
        todo!()
    }
    /// agents.ts:329: A1.
    pub fn agent_off(&self, entry: &Entry, agent: &str) -> bool {
        todo!()
    }
    /// agents.ts:333: A1.
    pub fn agent_label(agent: &str) -> String {
        todo!()
    }
    /// agents.ts:337: A2.
    pub async fn config_folder_on(&self, place: &str, path: &str) -> Result<ResolvedFolder, HostError> {
        todo!()
    }
    /// agents.ts:362: A3.
    pub async fn setup_refusal(&self, entry: &Entry, named: Option<&str>) -> Result<Option<String>, HostError> {
        todo!()
    }
    /// agents.ts:376: A4.
    pub async fn homes_here(&self) -> Result<IndexMap<String, String>, HostError> {
        todo!()
    }
    /// agents.ts:390: A4.
    pub async fn stores_here(&self) -> Result<IndexMap<String, String>, HostError> {
        todo!()
    }
    /// agents.ts:396: A5.
    pub async fn confine_setup(&self, entry: &Entry, named: Option<&str>) -> Result<(), HostError> {
        todo!()
    }
    /// agents.ts:403: A5. The harness and its adapter.
    pub async fn launch_adapter_for(&self, entry: &Entry, ask: AdapterAsk) -> Result<(String, Arc<dyn HarnessAdapter>), HostError> {
        todo!()
    }
    /// agents.ts:412: A6.
    pub fn default_agent_of(&self, prefs: &Preferences, entry: Option<&Entry>) -> String {
        todo!()
    }
    /// agents.ts:421: A7.
    pub fn defaults_on(catalog: &HarnessCatalog, prefs: &Preferences, project: Option<&ProjectOverrides>) -> DefaultsOn {
        todo!()
    }
    /// agents.ts:428: A8.
    pub fn named_mode(catalog: Option<&HarnessCatalog>, harness: &str, word: AccessChoice) -> Result<String, HostError> {
        todo!()
    }
    /// agents.ts:442: A9.
    pub async fn land_images(&self, entry: &Entry, road: Option<AttachmentRoad>, folder: &str, dir: &str, images: &[Attachment]) -> Result<LandedImages, HostError> {
        todo!()
    }
    /// agents.ts:464: A10.
    pub async fn land_files(&self, entry: &Entry, folder: &str, dir: &str, files: &[Attachment]) -> Result<Vec<String>, HostError> {
        todo!()
    }
    /// agents.ts:481: A11.
    pub async fn drop_thread_files(&self, entry: &Entry, threads: &[String]) {
        todo!()
    }
    /// agents.ts:498: A12.
    pub fn drop_images(&self, entry: &Entry, dir: &str) {
        todo!()
    }
    /// agents.ts:506: A13.
    pub fn agent_home(&self, entry: &Entry, agent: &str) -> String {
        todo!()
    }
    /// agents.ts:514: A13.
    pub fn thread_env(&self, entry: &Entry, harness: Option<&str>) -> Env {
        todo!()
    }
    /// agents.ts:524: A13.
    pub fn place_stores(&self, place: &str, home: &str) -> IndexMap<String, String> {
        todo!()
    }
    /// agents.ts:534: A14.
    pub async fn turn_configs_on(&self, entry: &Entry, harness: &str, folder: &str) -> Result<Option<TurnConfigs>, HostError> {
        todo!()
    }
    /// agents.ts:561: A15.
    pub async fn server_values_for(&self, entry: &Entry, harness: &str, folder: &str) -> Result<Option<IndexMap<String, Env>>, HostError> {
        todo!()
    }
    /// agents.ts:592: A16.
    pub async fn off_servers_for(&self, entry: &Entry, harness: &str, folder: &str) -> Option<Vec<String>> {
        todo!()
    }
    /// agents.ts:615: A17.
    pub async fn launch_servers_for(&self, entry: &Entry, harness: &str, adapter: &dyn HarnessAdapter, folder: &str) -> Result<LaunchServers, HostError> {
        todo!()
    }
    /// agents.ts:632: A18. The harness and its adapter.
    pub fn adapter_for(&self, entry: &Entry, ask: AdapterAsk) -> Result<(String, Arc<dyn HarnessAdapter>), HostError> {
        todo!()
    }
    /// agents.ts:671: A19.
    pub async fn project_home(&self, reference: &str, origin: Option<Caller>) -> Result<Option<Entry>, HostError> {
        todo!()
    }
    /// agents.ts:678: A19.
    pub async fn computer_home(&self, computer: &str, origin: Option<Caller>) -> Result<Option<Entry>, HostError> {
        todo!()
    }
}
