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

use wsp_wire::views::place::PlaceProvisionRow;

use crate::golden_seam::{McpPlan, StageListener, ToolResult};
use crate::machine::Machine;
use crate::provision_files::OwnedPaths;

/// provision-mcp.ts:45 `atHome`: the plan as it reads where the login's home is `home`. The plan itself (same
/// value) where `home` is its `guest_home`; else every path equal to the old home or under it moves under the new
/// one: each rewrite's target (a trailing `/` kept), every scope's files and its project's `to`.
pub fn at_home(plan: &McpPlan, home: &str) -> McpPlan {
    todo!()
}

/// provision-mcp.ts:61 `theirServerLine`.
pub fn their_server_line(agent: &str, name: &str, path: &str) -> String {
    todo!()
}

/// provision-mcp.ts:65 `keyUnreachedLine`: the names joined by `, `.
pub fn key_unreached_line(agent: &str, names: &[String]) -> String {
    todo!()
}

/// provision-mcp.ts:69 `untrustedLine`.
pub fn untrusted_line(agent: &str, folder: &str, why: &str) -> String {
    todo!()
}

/// provision-mcp.ts:74 `noCopyLine`.
pub fn no_copy_line(path: &str) -> String {
    todo!()
}

/// provision-mcp.ts:78 `travelledPath`: `<staging>/<path below home>` for a file under `home/`; `None` otherwise.
pub fn travelled_path(home: &str, file: &str) -> Option<String> {
    todo!()
}

/// provision-mcp.ts:95 `serverLine`: `<id>\t<digest>\t<digest>`.
pub fn server_line(id: &str, digest: &str) -> String {
    todo!()
}

/// provision-mcp.ts:100 `tombstoneLine`: `<id>\t-\t-`.
pub fn tombstone_line(id: &str) -> String {
    todo!()
}

/// provision-mcp.ts:127 `headlessKeys(machine, home, stores, agents)`: the words of each config that did not take
/// its agent's headless keys, one per agent, in catalog (MCP_AGENTS) order.
pub async fn headless_keys(machine: &dyn Machine, home: &str, stores: &IndexMap<String, String>, agents: &BTreeSet<String>) -> Vec<String> {
    todo!()
}

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
    pub into: Option<Arc<dyn Fn(&str, &[String])>>,
}

/// provision-mcp.ts:158 `provisionMcp`: one row per server of the plan, in mcpRows' order. Never fails: a read or a
/// write that did not happen is a reason on each skipped row.
pub async fn provision_mcp(machine: &dyn Machine, planned: &McpPlan, o: ProvisionMcpOptions) -> Vec<PlaceProvisionRow> {
    todo!()
}
