// SPDX-License-Identifier: AGPL-3.0-only
//! `agents-here.ts`: the agents the desktop's first run lists, read off this computer, with whether their MCP config names wsp.
use wsp_catalog::catalog::AgentEntry;
use wsp_collect::host::Host;
use wsp_words::exit::HostError;

/// `AgentHere` (`agents-here.ts:10`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentHere {
    pub id: String,
    pub name: String,
    pub found: bool,
    pub configured: bool,
    pub version: Option<String>,
    pub installs: Option<bool>,
}

/// `ownServerFiles` (`agents-here.ts:25`), where the TS `McpAgent` is the catalog's `AgentEntry` with an MCP config.
pub fn own_server_files(_host: &Host, _agent: &AgentEntry) -> Vec<String> {
    todo!()
}

/// `serverListedHere` (`agents-here.ts:49`).
pub async fn server_listed_here(_host: &Host, _name: &str) -> bool {
    todo!()
}

/// `AgentsHereOptions` (`agents-here.ts:61`), the inline `opts` of `agentsHere`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentsHereOptions {
    pub versions: Option<bool>,
}

/// `agentsHere` (`agents-here.ts:61`).
pub async fn agents_here(_host: &Host, _opts: AgentsHereOptions) -> Result<Vec<AgentHere>, HostError> {
    todo!()
}
