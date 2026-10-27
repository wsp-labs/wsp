// SPDX-License-Identifier: AGPL-3.0-only
//! Packet 9's ports: the skills, servers and plugins acts, and the newest-version read the agents reader takes.
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::types::wiring::Local;
use wsp_machines::agents_read::{PluginsActs, ServersActs, SkillsActs};

use crate::context::HostWiring;

/// `skillsActs()` (`cli/wiring.ts:357`).
pub fn skills_acts(_wiring: &HostWiring) -> Option<Rc<dyn SkillsActs>> {
    None
}

/// `serversActs({ vault: serverVault(statePath) })` (`cli/wiring.ts:360`).
pub fn servers_acts(_wiring: &HostWiring) -> Option<Rc<dyn ServersActs>> {
    None
}

/// `pluginsActs()` (`cli/wiring.ts:362`).
pub fn plugins_acts(_wiring: &HostWiring) -> Option<Rc<dyn PluginsActs>> {
    None
}

/// `agentLatest({ statePath, running: VERSION }).read` (`cli/wiring.ts:352`): each agent's newest version by id, which
/// the agents reader's `latest` takes.
pub fn agent_latest(_wiring: &HostWiring) -> Option<Rc<dyn Fn() -> Local<'static, IndexMap<String, String>>>> {
    None
}
