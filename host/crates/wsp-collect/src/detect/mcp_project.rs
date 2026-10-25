// SPDX-License-Identifier: AGPL-3.0-only
//! The servers a project carries to another computer: of those a turn in its folder gets, the ones its checkout does
//! not bring along. Port of packages/collect/src/detect/mcp-project.ts at a3b870a6e; the spec is
//! spec/detect_turn_servers.md.
use std::collections::HashSet;

use wsp_catalog::catalog::AgentEntry;
use wsp_catalog::mcp::{McpServer, McpServerScope, McpTransport};
use wsp_catalog::mcp_launch::{launch_server_road, Checkout, TurnScope, TurnServer};

use crate::detect::mcp::{mcp_remote_tokens, mcp_server_row};
use crate::detect::mcp_args::home_paths;
use crate::detect::turn_servers::{read_checkout, read_turn_servers, TurnRead};
use crate::host::{tilde, Host, RunOptions};
use crate::manifest::{ManifestEntry, RowDefault};

/// mcp-project.ts:14.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectServer {
    pub agent: &'static AgentEntry,
    pub turn: TurnServer,
    pub row: ManifestEntry,
}

/// Why a server of the folder stays here though a turn there reads it.
pub const SWITCHED_OFF_HERE: &str = "switched off for this folder here";

/// Node's `path.posix.normalize` for an absolute path: empty and `.` segments go, `..` steps back.
fn normalize(path: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts
}

/// `path.posix.join(a, b)` for absolute paths.
fn join(a: &str, b: &str) -> String {
    format!("/{}", normalize(&format!("{a}/{b}")).join("/"))
}

/// `path.posix.relative(from, to)` for absolute paths.
fn relative(from: &str, to: &str) -> String {
    let a = normalize(from);
    let b = normalize(to);
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<&str> = vec![".."; a.len() - common];
    parts.extend(&b[common..]);
    parts.join("/")
}

fn inside(path: &str, folder: &str) -> bool {
    path.starts_with(&format!("{}/", folder.trim_end_matches('/')))
}

/// Why a project's server stays here: its command or a path it runs against sits in the home outside the project, and
/// nothing carries it, so it is not on that computer.
pub fn outside_project_line(paths: &[String]) -> String {
    format!("runs against {}, outside the project, which nothing carries to that computer", paths.join(", "))
}

/// The TS options object `{ agents, keep? }`.
#[derive(Debug, Clone, Default)]
pub struct ProjectMcpOptions {
    /// TS `McpAgent`s: catalog entries with `mcp` set.
    pub agents: Vec<&'static AgentEntry>,
    /// Untracked files the project keeps, relative to the folder.
    pub keep: Vec<String>,
}

/// The TS answer `{ servers, read, checkout }`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectMcp {
    pub servers: Vec<ProjectServer>,
    /// Each agent that reads a project file in the folder at all.
    pub read: Vec<String>,
    pub checkout: Checkout,
}

/// The home paths a server runs against, its command among them, that sit outside the checkout `top`: what would not
/// be on the computer the project goes to. ~/.local/bin is left out, since a command there is found there by name.
fn outside_paths(server: &McpServer, home: &str, top: &str) -> Vec<String> {
    let McpTransport::Stdio { command, .. } = &server.transport else {
        return Vec::new();
    };
    let at = |p: &str| match p.strip_prefix("~/") {
        Some(rest) => format!("{home}/{rest}"),
        None => p.to_string(),
    };
    let command = at(command.as_str());
    let mut candidates: Vec<String> = Vec::new();
    if command.starts_with(&format!("{home}/")) && !command.starts_with(&format!("{home}/.local/bin/")) {
        candidates.push(tilde(home, &command));
    }
    for p in home_paths(server, home) {
        if !candidates.contains(&p) {
            candidates.push(p);
        }
    }
    candidates.into_iter().filter(|p| !inside(&at(p.as_str()), top) && at(p.as_str()) != top).collect()
}

/// The files of `files` inside the checkout `top` that it brings along: tracked by git, or among the untracked files
/// the project keeps, which `keep` names from `folder`.
async fn travelling(host: &Host, folder: &str, top: &str, files: &[String], keep: &[String]) -> HashSet<String> {
    let rel: Vec<String> = files.iter().filter(|f| inside(f, top)).map(|f| relative(top, f)).collect();
    if rel.is_empty() {
        return HashSet::new();
    }
    let mut args: Vec<String> = vec!["-C".into(), top.into(), "ls-files".into(), "--".into()];
    args.extend(rel.iter().cloned());
    let tracked = host.exec.run("git", &args, RunOptions::default()).await.unwrap_or_default();
    let mut kept: Vec<String> = tracked.split('\n').filter(|l| !l.is_empty()).map(String::from).collect();
    kept.extend(rel.iter().filter(|r| keep.contains(&relative(folder, &join(top, r)))).cloned());
    kept.iter().map(|r| join(top, r)).collect()
}

/// The servers a turn of each agent in `folder` gets that the folder's checkout does not bring along, one per name as
/// the agent reads them, each with the scan's row: only an agent whose servers wsp knows where to carry. A server the
/// folder switches off, or one that runs against a home path outside the project, is a row that stays here. `read` names each agent that reads a project file in the folder at
/// all, carried or not, which for Codex is one whose own file trusts it; `checkout` is the folder's.
pub async fn detect_project_mcp(host: &Host, folder: &str, o: &ProjectMcpOptions) -> ProjectMcp {
    let tokens = mcp_remote_tokens(host).await;
    let checkout = read_checkout(host, folder).await;
    let top = checkout.top.clone().unwrap_or_else(|| folder.to_string());
    let mut servers = Vec::new();
    let mut read = Vec::new();
    for agent in &o.agents {
        if launch_server_road(&agent.id).is_none() {
            continue;
        }
        let turns: Vec<TurnServer> = read_turn_servers(host, agent, folder, TurnRead { checkout: Some(checkout.clone()), own: false })
            .await
            .into_iter()
            .filter(|s| s.scope != TurnScope::User)
            .collect();
        if turns.iter().any(|s| s.scope == TurnScope::Project) {
            read.push(agent.id.clone());
        }
        let mut project_files: Vec<String> = Vec::new();
        for s in turns.iter().filter(|s| s.scope == TurnScope::Project) {
            if !project_files.contains(&s.file) {
                project_files.push(s.file.clone());
            }
        }
        let brought = travelling(host, folder, &top, &project_files, &o.keep).await;
        let mut seen: HashSet<String> = HashSet::new();
        for turn in turns {
            if !seen.insert(turn.server.name.clone()) {
                continue;
            }
            if turn.scope == TurnScope::Project && brought.contains(&turn.file) {
                continue;
            }
            let mut as_user = turn.server.clone();
            as_user.scope = McpServerScope::User;
            let row = mcp_server_row(host, agent, &as_user, &tokens).await;
            let outside = outside_paths(&turn.server, &host.home, &top);
            let reason = if turn.server.disabled == Some(true) {
                Some(SWITCHED_OFF_HERE.to_string())
            } else if !outside.is_empty() {
                Some(outside_project_line(&outside))
            } else {
                None
            };
            let row = match reason {
                Some(reason) => ManifestEntry { default: RowDefault::Skip, reason: Some(reason), ..row },
                None => row,
            };
            servers.push(ProjectServer { agent, turn, row });
        }
    }
    ProjectMcp { servers, read, checkout }
}
