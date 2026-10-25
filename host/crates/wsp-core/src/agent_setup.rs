// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/agent-setup.ts at 65f6b89c7: how each agent runs on each computer the person owns,
//! one document per computer and agent, held in memory. Spec: spec/agent_setups.md.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_adapters::port::AgentLaunch;
use wsp_store::store::Store;
use wsp_wire::types::thread_defaults::{AgentSetup, AgentSetupSet, ResolvedFolder};
use wsp_words::exit::HostError;

/// The store collection. agent-setup.ts:14.
pub const AGENT_SETUPS: &str = "agentSetups";
/// How many links a path is followed through before it is a loop. agent-setup.ts:21.
pub const LINK_HOPS: u32 = 40;

/// `<path> goes round a loop of links`. agent-setup.ts:22.
pub fn loop_line(path: &str) -> String {
    todo!()
}

/// The path with every link on its way followed, on this computer: the part that exists resolved, a link to a place
/// not made yet read for where it points, the rest kept as named. A usage refusal (`loop_line`) after more than
/// LINK_HOPS links. agent-setup.ts:27.
pub async fn real_folder_here(path: &str) -> Result<String, HostError> {
    todo!()
}

/// The same reading on another computer, as one shell line: the resolved folder, then that computer's home resolved,
/// one a line; exit 3 with `loop_line` on stderr after LINK_HOPS links. agent-setup.ts:48.
pub fn real_folder_script(path: &str) -> String {
    todo!()
}

/// `<place_id>:<agent>`, the document's id. agent-setup.ts:57.
pub fn key_of(place_id: &str, agent: &str) -> String {
    todo!()
}

/// What a launch of one agent on one computer takes from its setup. agent-setup.ts:60.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetupLaunch {
    pub config_dir: Option<String>,
    /// The variables the launch carries, the config folder's own last for an agent that reads one.
    pub env: IndexMap<String, String>,
    pub launch: Option<AgentLaunch>,
}

/// What `set` asks of its caller: the config folder resolved on the computer it names, and the agent's name for a
/// refusal. agent-setup.ts:75.
pub struct SetupSetOptions {
    pub folder: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<ResolvedFolder, HostError>>>,
    pub agent_name: String,
}

/// Every setup, held in memory and written through to the store. Nothing is read until `load`. agent-setup.ts:78.
pub struct AgentSetups {
    store: Rc<dyn Store>,
    held: RefCell<IndexMap<String, AgentSetup>>,
}

impl AgentSetups {
    /// Reads every document of AGENT_SETUPS into memory. agent-setup.ts:82.
    pub async fn load(&self) -> Result<(), HostError> {
        todo!()
    }

    pub fn get(&self, place_id: &str, agent: &str) -> Option<AgentSetup> {
        todo!()
    }

    /// Whether the person turned the agent off on that computer.
    pub fn off(&self, place_id: &str, agent: &str) -> bool {
        todo!()
    }

    /// agent-setup.ts:87.
    pub fn launch_of(&self, place_id: &str, agent: &str) -> SetupLaunch {
        todo!()
    }

    /// The change over what stands, checked first and refused with a usage error; the store written before memory.
    /// agent-setup.ts:101.
    pub async fn set(&self, place_id: &str, agent: &str, change: AgentSetupSet, o: SetupSetOptions) -> Result<Option<AgentSetup>, HostError> {
        todo!()
    }
}

/// agent-setup.ts:78.
pub fn agent_setups(store: Rc<dyn Store>) -> AgentSetups {
    todo!()
}
