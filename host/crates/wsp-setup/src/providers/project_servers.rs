// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `project-servers.ts`: the plan that carries a project's own MCP servers and switches to a computer somebody owns.
use serde_json::Value;
use wsp_collect::host::Host;
use wsp_engine::golden_seam::McpPlan;
use wsp_engine::provision::ProvisionOn;
use wsp_wire::types::recipe_file::{RecipeFile, RecipeFileFoldersValue};
use wsp_words::exit::HostError;

/// The `o` of `projectServersPlan` (`project-servers.ts:70`).
#[derive(Clone)]
pub struct ProjectServersPlanOptions {
    pub here: Host,
    pub picks: RecipeFile,
    pub folder: RecipeFileFoldersValue,
    pub path: String,
    pub on: ProvisionOn,
    pub recipe: Option<String>, // on.recipe in the TS, which is ProvisionOn & { recipe?: string }
    pub vault: Value,           // ServerVault: env-keys.ts's type, no Rust form yet
}

/// `projectServersPlan` (`project-servers.ts:70`).
pub async fn project_servers_plan(_o: &ProjectServersPlanOptions) -> Result<Option<McpPlan>, HostError> {
    todo!()
}
