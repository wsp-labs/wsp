// SPDX-License-Identifier: AGPL-3.0-only
//! The servers a turn of an agent gets in a folder, read off one Host by the catalog's resolver, keyed by the folder's
//! checkout as git answers it on that Host. Port of packages/collect/src/detect/turn-servers.ts at a3b870a6e; the
//! spec is spec/detect_turn_servers.md.
use futures::future::join_all;
use wsp_catalog::catalog::AgentEntry;
use wsp_catalog::mcp_launch::{
    checkout_args, checkout_of, turn_server_files, turn_servers, Checkout, LaunchConfigs, TurnAgent, TurnFile, TurnServer,
};

use crate::host::{Host, RunOptions};

/// The TS options object `{ checkout?, own? }` (turn-servers.ts:15).
#[derive(Debug, Clone, Default)]
pub struct TurnRead {
    /// The folder's checkout, read off the Host where it is not handed in.
    pub checkout: Option<Checkout>,
    /// Read the agent's own file alone, keyed by the folder itself.
    pub own: bool,
}

/// The folder's checkout on that Host: `git <checkout_args(folder)>` read by `checkout_of`.
pub async fn read_checkout(host: &Host, folder: &str) -> Checkout {
    let args = checkout_args(folder);
    let said = host.exec.run("git", &args, RunOptions::default()).await;
    checkout_of(said.as_deref(), folder)
}

/// Every server a turn of the agent in `folder` gets there, by scope, read off the files that Host holds. The agent is
/// TS `McpAgent` (an entry with `mcp` set); the resolver's TurnAgent is its id and its `mcp` format, files and
/// project files. An entry with no `mcp` gets no servers.
pub async fn read_turn_servers(host: &Host, agent: &AgentEntry, folder: &str, o: TurnRead) -> Vec<TurnServer> {
    let Some(mcp) = &agent.mcp else {
        return Vec::new();
    };
    let checkout = match o.checkout {
        Some(checkout) => checkout,
        None if o.own => Checkout { top: None, key: folder.to_string() },
        None => read_checkout(host, folder).await,
    };
    let turn = TurnAgent { id: agent.id.clone(), format: mcp.format, files: mcp.files.clone(), project_files: mcp.project_files.clone() };
    let store = host.stores.as_ref().and_then(|s| s.get(&agent.id)).map(String::as_str);
    let files = turn_server_files(&turn, folder, &host.home, store, checkout.top.as_deref());
    let mut user = None;
    for path in &files.user {
        if let Some(text) = host.fs.read_text(path).await {
            user = Some(TurnFile { path: path.clone(), text });
            break;
        }
    }
    let projects: Vec<TurnFile> = if o.own {
        Vec::new()
    } else {
        let reads = files
            .projects
            .iter()
            .map(|path| async move { host.fs.read_text(path).await.map(|text| TurnFile { path: path.clone(), text }) });
        join_all(reads).await.into_iter().flatten().collect()
    };
    let configs = LaunchConfigs { user, projects: Some(projects), folder: folder.to_string(), key: Some(checkout.key) };
    turn_servers(&turn, &configs)
}
