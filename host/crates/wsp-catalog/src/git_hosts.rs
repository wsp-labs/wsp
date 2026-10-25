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

impl GitHostId {
    fn word(self) -> &'static str {
        match self {
            GitHostId::Github => "github",
            GitHostId::Gitlab => "gitlab",
        }
    }
}

impl GitHost {
    /// `https://<host>/<ownerRepo>.git`.
    pub fn https_url(&self, owner_repo: &str) -> String {
        let host = match self.id {
            GitHostId::Github => "github.com",
            GitHostId::Gitlab => "gitlab.com",
        };
        format!("https://{host}/{owner_repo}.git")
    }

    /// `[<cli>, "repo", "clone", ownerRepo, dest]`.
    pub fn clone_argv(&self, owner_repo: &str, dest: &str) -> Vec<String> {
        vec![self.cli.to_string(), "repo".to_string(), "clone".to_string(), owner_repo.to_string(), dest.to_string()]
    }
}

static GIT_HOSTS: [GitHost; 2] = [
    GitHost {
        id: GitHostId::Github,
        cli: "gh",
        ssh_hosts: &["github.com", "ssh.github.com"],
        setup_git: "gh auth setup-git",
        status: "gh auth status",
    },
    GitHost {
        id: GitHostId::Gitlab,
        cli: "glab",
        ssh_hosts: &["gitlab.com", "altssh.gitlab.com"],
        setup_git: "glab auth git-credential",
        status: "glab auth status",
    },
];

/// github, gitlab (git-hosts.ts:24).
pub fn git_hosts() -> &'static [GitHost] {
    &GIT_HOSTS
}

/// git-hosts.ts:46.
pub fn git_host(id: &str) -> Option<&'static GitHost> {
    git_hosts().iter().find(|host| host.id.word() == id)
}

/// git-hosts.ts:51.
pub fn git_host_of(remote: &str) -> Option<&'static GitHost> {
    let host = remote_host(remote)?;
    git_hosts().iter().find(|row| row.ssh_hosts.contains(&host.as_str()))
}

/// The lowercase host a remote names; nothing for none (git-hosts.ts:57).
pub fn remote_host(remote: &str) -> Option<String> {
    let mut rest = remote;
    if let Some(at) = rest.find("://") {
        if at > 0 && rest[..at].chars().all(|c| c.is_ascii_lowercase() || c == '+') {
            rest = &rest[at + 3..];
        }
    }
    if let Some(at) = rest.find('@') {
        if at > 0 && !rest[..at].contains('/') {
            rest = &rest[at + 1..];
        }
    }
    let host = rest.split(['/', ':']).next().unwrap_or_default().to_lowercase();
    (!host.is_empty()).then_some(host)
}

/// git-hosts.ts:64.
pub fn no_git_cli_line(host: &GitHost, computer: &str) -> String {
    format!(
        "{computer} has no {cli} signed in, and a {id} repo is cloned there through it; sign {cli} in on that computer's image, or give the repo's https url",
        cli = host.cli,
        id = host.id.word()
    )
}

/// thread-agents.ts:9.
pub const THREAD_AGENTS: [&str; 4] = ["claude", "codex", "opencode", "cursor"];

/// thread-agents.ts:13.
pub fn runs_threads(id: &str) -> bool {
    THREAD_AGENTS.contains(&id)
}

/// What a new worktree in one ecosystem gets (ecosystems/module.ts:10).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EcosystemModule {
    #[serde(flatten)]
    pub carry: CarryModule,
    pub rebuild: String,
}

pub const NODE_BUILD_OUTPUTS: [&str; 3] = [".next", ".turbo", ".cache"];

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

/// npm, pnpm, yarn, bun, composer, uv, poetry, cargo, go (ecosystems/index.ts:17).
pub static ECOSYSTEM_MODULES: LazyLock<Vec<EcosystemModule>> = LazyLock::new(|| {
    vec![
        EcosystemModule {
            carry: CarryModule {
                id: "npm".into(),
                lockfiles: strs(&["package-lock.json", "npm-shrinkwrap.json"]),
                carry: strs(&["node_modules"]),
                never: strs(&NODE_BUILD_OUTPUTS),
                installed: None,
            },
            rebuild: "npm install --no-save --prefer-offline --no-audit --no-fund".into(),
        },
        EcosystemModule {
            carry: CarryModule {
                id: "pnpm".into(),
                lockfiles: strs(&["pnpm-lock.yaml"]),
                carry: strs(&["node_modules"]),
                never: strs(&NODE_BUILD_OUTPUTS),
                installed: Some("node_modules/.pnpm/lock.yaml".into()),
            },
            rebuild: "pnpm install --frozen-lockfile --prefer-offline".into(),
        },
        EcosystemModule {
            carry: CarryModule {
                id: "yarn".into(),
                lockfiles: strs(&["yarn.lock"]),
                carry: strs(&["node_modules", ".yarn/cache", ".yarn/unplugged"]),
                never: strs(&NODE_BUILD_OUTPUTS),
                installed: None,
            },
            rebuild: "yarn install --frozen-lockfile".into(),
        },
        EcosystemModule {
            carry: CarryModule {
                id: "bun".into(),
                lockfiles: strs(&["bun.lock", "bun.lockb"]),
                carry: strs(&["node_modules"]),
                never: strs(&NODE_BUILD_OUTPUTS),
                installed: None,
            },
            rebuild: "bun install --frozen-lockfile".into(),
        },
        EcosystemModule {
            carry: CarryModule {
                id: "composer".into(),
                lockfiles: strs(&["composer.lock"]),
                carry: strs(&["vendor"]),
                never: strs(&[]),
                installed: None,
            },
            rebuild: "composer install --no-interaction".into(),
        },
        EcosystemModule {
            carry: CarryModule {
                id: "uv".into(),
                lockfiles: strs(&["uv.lock"]),
                carry: strs(&[]),
                never: strs(&[".venv"]),
                installed: None,
            },
            rebuild: "uv sync --locked".into(),
        },
        EcosystemModule {
            carry: CarryModule {
                id: "poetry".into(),
                lockfiles: strs(&["poetry.lock"]),
                carry: strs(&[]),
                never: strs(&[".venv"]),
                installed: None,
            },
            rebuild: "poetry install --no-interaction".into(),
        },
        EcosystemModule {
            carry: CarryModule {
                id: "cargo".into(),
                lockfiles: strs(&["Cargo.lock"]),
                carry: strs(&[]),
                never: strs(&["target"]),
                installed: None,
            },
            rebuild: "cargo fetch --locked".into(),
        },
        EcosystemModule {
            carry: CarryModule { id: "go".into(), lockfiles: strs(&["go.sum"]), carry: strs(&[]), never: strs(&[]), installed: None },
            rebuild: "go list -mod=readonly -deps -test ./... > /dev/null".into(),
        },
    ]
});
