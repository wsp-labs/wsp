// SPDX-License-Identifier: AGPL-3.0-only
use serde::Deserialize;

use crate::{protocol, Done};

ops! {
    "agents.read" => AgentsReadRequest => Report,
    "agents.setup" => AgentsSetupRequest => AgentSetUp,
    "agents.signIn" => AgentsSignInRequest => SignInStarted,
    "agents.signInCode" => AgentsSignInCodeRequest => Done,
    "agents.signInStop" => AgentsSignInStopRequest => Done,
    "agents.signIns" => AgentsSignInsRequest => SignIns,
    "agents.cloudSignIn" => AgentsCloudSignInRequest => SignInStarted,
    "agents.key" => AgentsKeyRequest => Done,
    "agents.addTools" => AgentsAddToolsRequest => Written,
    "broker.status" => BrokerStatusRequest => protocol::BrokerStatus,
    "providers.checkKey" => ProvidersCheckKeyRequest => protocol::ProviderKeyWorks,
    "servers.add" => ServersAddRequest => Written,
    "servers.icon" => ServersIconRequest => Icon,
    "servers.remove" => ServersRemoveRequest => Written,
    "servers.signIn" => ServersSignInRequest => SignInStarted,
    "servers.toggle" => ServersToggleRequest => Written,
    "servers.tools" => ServersToolsRequest => Tools,
    "skills.add" => SkillsAddRequest => SkillInstalled,
    "skills.get" => SkillsGetRequest => Preview,
    "skills.preview" => SkillsPreviewRequest => Preview,
    "skills.remove" => SkillsRemoveRequest => SkillRemoved,
    "skills.search" => SkillsSearchRequest => SkillHits,
    "skills.toggle" => SkillsToggleRequest => SkillSwitched,
    "plugins.toggle" => PluginsToggleRequest => PluginSwitched,
}

#[derive(Debug, Deserialize)]
pub struct Report {
    pub report: protocol::AgentsReport,
}

#[derive(Debug, Deserialize)]
pub struct AgentSetUp {
    pub agent: protocol::AgentRow,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInStarted {
    pub sign_in_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SignIns {
    pub runs: Vec<protocol::AgentsSignInRun>,
}

/// The config file an act wrote, `~`-relative under the home.
#[derive(Debug, Deserialize)]
pub struct Written {
    pub file: String,
}

/// The icon as a data url, none where the host has no icon for that name or may not ask for one.
#[derive(Debug, Deserialize)]
pub struct Icon {
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Tools {
    pub answer: protocol::ServerToolsAnswer,
}

#[derive(Debug, Deserialize)]
pub struct SkillInstalled {
    pub added: protocol::SkillAdded,
}

#[derive(Debug, Deserialize)]
pub struct Preview {
    pub preview: protocol::SkillPreview,
}

#[derive(Debug, Deserialize)]
pub struct SkillRemoved {
    pub removed: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SkillHits {
    pub skills: Vec<protocol::SkillHit>,
}

#[derive(Debug, Deserialize)]
pub struct SkillSwitched {
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PluginSwitched {
    pub plugin: protocol::PluginRow,
}
