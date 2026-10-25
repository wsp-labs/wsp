// SPDX-License-Identifier: AGPL-3.0-only
//! One row per git host whose signed-in command line the image carries, the agents wsp opens threads on, and the
//! ecosystems a new worktree is read for. Port of packages/catalog/src/git-hosts.ts, thread-agents.ts and
//! ecosystems/*.ts at a3b870a6e; the spec is spec/seed.md.
use std::sync::LazyLock;

use serde::Serialize;

use wsp_wire::views::workspace_more::CarryModule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GitHostId {
    Github,
    Gitlab,
}

/// git-hosts.ts:8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHost {
    pub id: GitHostId,
    pub cli: &'static str,
    pub ssh_hosts: &'static [&'static str],
    pub setup_git: &'static str,
    pub status: &'static str,
}

impl GitHost {
    /// `https://<host>/<ownerRepo>.git`.
    pub fn https_url(&self, owner_repo: &str) -> String {
        todo!()
    }

    /// `[<cli>, "repo", "clone", ownerRepo, dest]`.
    pub fn clone_argv(&self, owner_repo: &str, dest: &str) -> Vec<String> {
        todo!()
    }
}

/// github, gitlab (git-hosts.ts:24).
pub fn git_hosts() -> &'static [GitHost] {
    todo!()
}

/// git-hosts.ts:46.
pub fn git_host(id: &str) -> Option<&'static GitHost> {
    todo!()
}

/// git-hosts.ts:51.
pub fn git_host_of(remote: &str) -> Option<&'static GitHost> {
    todo!()
}

/// The lowercase host a remote names; nothing for none (git-hosts.ts:57).
pub fn remote_host(remote: &str) -> Option<String> {
    todo!()
}

/// git-hosts.ts:64.
pub fn no_git_cli_line(host: &GitHost, computer: &str) -> String {
    todo!()
}

/// thread-agents.ts:9.
pub const THREAD_AGENTS: [&str; 4] = ["claude", "codex", "opencode", "cursor"];

/// thread-agents.ts:13.
pub fn runs_threads(id: &str) -> bool {
    todo!()
}

/// What a new worktree in one ecosystem gets (ecosystems/module.ts:10).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EcosystemModule {
    #[serde(flatten)]
    pub carry: CarryModule,
    pub rebuild: String,
}

pub const NODE_BUILD_OUTPUTS: [&str; 3] = [".next", ".turbo", ".cache"];

/// npm, pnpm, yarn, bun, composer, uv, poetry, cargo, go (ecosystems/index.ts:17).
pub static ECOSYSTEM_MODULES: LazyLock<Vec<EcosystemModule>> = LazyLock::new(|| todo!());
