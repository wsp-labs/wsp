// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/project-sources.ts at 65f6b89c7: the project sources as modules, one per kind. The add
//! asks a module through `project_sources()`; nothing else decides by a source's kind. Spec: spec/landing_roads.md.
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_catalog::git_hosts::{git_host_of, git_hosts, no_git_cli_line, GitHost, GitHostId};
use wsp_wire::types::shell_quote::shell_line;
use wsp_wire::views::workspace::{ProjectSource, SeedPlan};
use wsp_words::exit::{HostError, Kind};
use wsp_words::project_seed::no_remote_line;
use wsp_words::projects::project_name_of;
use wsp_words::start::owner_repo_of;

/// The default branch a clone takes where nothing could be read. project-sources.ts:54.
pub const DEFAULT_BRANCH: &str = "main";

/// What a source resolved to on this computer. project-sources.ts:13.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedSource {
    pub remote: String,
    pub default_branch: String,
    pub name: String,
    /// Only a folder seeding another computer has one.
    pub seed: Option<SeedPlan>,
}

/// A folder's origin and the branch that remote's HEAD names; both empty where it has none. project-sources.ts:29.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FolderRemote {
    pub remote: String,
    pub default_branch: String,
}

/// What a module may ask of this computer while it resolves. project-sources.ts:24.
#[derive(Clone)]
pub struct SourceDeps {
    /// True where the folder seeds a project on another computer.
    pub seeding: bool,
    pub seed_plan: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<SeedPlan, HostError>>>,
    pub folder_remote: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Result<FolderRemote, HostError>>>,
}

/// The command a clone goes through on the computer, where it goes through one rather than through git: the word the
/// image carries it as, the line pointing git at its login, and its host for the refusal. project-sources.ts:44.
#[derive(Clone, Copy, Debug)]
pub struct SourceCli {
    pub bin: &'static str,
    pub setup_git: &'static str,
    pub host: &'static GitHost,
}

impl SourceCli {
    /// The sentence a computer whose image has no such command is refused with (`noGitCliLine`).
    pub fn missing(&self, computer: &str) -> String {
        no_git_cli_line(self.host, computer)
    }
}

/// One source kind. project-sources.ts:32.
#[async_trait(?Send)]
pub trait ProjectSourceModule {
    /// `"folder"`, `"git"`, `"github"`, `"gitlab"`.
    fn kind(&self) -> &'static str;
    /// The remote this source names on its own, without asking this computer anything.
    fn remote(&self, source: &ProjectSource) -> String;
    async fn resolve(&self, source: &ProjectSource, deps: &SourceDeps) -> Result<ResolvedSource, HostError>;
    /// The line that clones the project onto the computer that holds it, in that computer's own shell.
    fn clone_command(&self, remote: &str, dest: &str, branch: Option<&str>) -> String;
    fn cli(&self) -> Option<SourceCli> {
        None
    }
}

/// The path a folder source names; empty for any other source, which the dispatch by kind never hands it.
fn folder_path(source: &ProjectSource) -> &str {
    match source {
        ProjectSource::Folder(folder) => &folder.path,
        _ => "",
    }
}

/// The url a git source names; empty for any other source.
fn git_url(source: &ProjectSource) -> &str {
    match source {
        ProjectSource::Git(git) => &git.url,
        _ => "",
    }
}

/// The owner/repo a host source names; empty for any other source.
fn repo_of(source: &ProjectSource) -> &str {
    match source {
        ProjectSource::Github(github) => &github.repo,
        ProjectSource::Gitlab(gitlab) => &gitlab.repo,
        _ => "",
    }
}

fn branch_or(read: Option<&str>) -> String {
    read.filter(|b| !b.is_empty()).unwrap_or(DEFAULT_BRANCH).to_string()
}

fn git_clone(remote: &str, dest: &str, branch: Option<&str>) -> String {
    let mut words = vec!["git", "clone"];
    if let Some(branch) = branch {
        words.extend(["--branch", branch]);
    }
    words.extend(["--", remote, dest]);
    shell_line(&words)
}

struct FolderModule;

#[async_trait(?Send)]
impl ProjectSourceModule for FolderModule {
    fn kind(&self) -> &'static str {
        "folder"
    }

    fn remote(&self, _source: &ProjectSource) -> String {
        String::new()
    }

    // A folder on this computer is not carried anywhere: the computer clones the folder's own origin and the folder
    // seeds what git ignores on top of that clone. A folder git has no remote for is refused here, before anything
    // is created, since there would be nothing for the other computer to clone.
    async fn resolve(&self, source: &ProjectSource, deps: &SourceDeps) -> Result<ResolvedSource, HostError> {
        let path = folder_path(source);
        if !deps.seeding {
            let read = (deps.folder_remote)(path).await?;
            return Ok(ResolvedSource { remote: read.remote, default_branch: branch_or(Some(read.default_branch.as_str())), name: project_name_of(source), seed: None });
        }
        let seed = (deps.seed_plan)(path).await?;
        let Some(seed_remote) = seed.remote.clone() else {
            return Err(HostError::with_kind(no_remote_line(path), Kind::Invalid));
        };
        // An ssh remote is recorded as its host's https url: no box carries an ssh key of the person's, so the remote
        // as it stands would ask for one, and over https the host's own signed-in command line answers for it.
        let remote = match (git_host_of(&seed_remote), owner_repo_of(&seed_remote)) {
            (Some(host), Some(owner_repo)) if !seed_remote.starts_with("https://") => host.https_url(&owner_repo),
            _ => seed_remote,
        };
        let default = seed.default_branch.clone().unwrap_or_else(|| seed.branch.clone());
        Ok(ResolvedSource { remote, default_branch: branch_or(Some(default.as_str())), name: project_name_of(source), seed: Some(seed) })
    }

    fn clone_command(&self, remote: &str, dest: &str, branch: Option<&str>) -> String {
        git_clone(remote, dest, branch)
    }
}

struct GitModule;

#[async_trait(?Send)]
impl ProjectSourceModule for GitModule {
    fn kind(&self) -> &'static str {
        "git"
    }

    fn remote(&self, source: &ProjectSource) -> String {
        git_url(source).to_string()
    }

    async fn resolve(&self, source: &ProjectSource, _deps: &SourceDeps) -> Result<ResolvedSource, HostError> {
        Ok(ResolvedSource { remote: self.remote(source), default_branch: DEFAULT_BRANCH.to_string(), name: project_name_of(source), seed: None })
    }

    fn clone_command(&self, remote: &str, dest: &str, branch: Option<&str>) -> String {
        git_clone(remote, dest, branch)
    }
}

/// The two hosts whose signed-in command line the image carries, as source modules: the clone goes through that
/// command, which reads the login on the computer rather than asking for a credential no box holds. A computer
/// whose image has no such login is refused by name, so nothing waits inside a clone for a password.
struct HostModule {
    host: &'static GitHost,
}

#[async_trait(?Send)]
impl ProjectSourceModule for HostModule {
    fn kind(&self) -> &'static str {
        match self.host.id {
            GitHostId::Github => "github",
            GitHostId::Gitlab => "gitlab",
        }
    }

    fn remote(&self, source: &ProjectSource) -> String {
        self.host.https_url(repo_of(source))
    }

    async fn resolve(&self, source: &ProjectSource, _deps: &SourceDeps) -> Result<ResolvedSource, HostError> {
        Ok(ResolvedSource { remote: self.remote(source), default_branch: DEFAULT_BRANCH.to_string(), name: project_name_of(source), seed: None })
    }

    // The command line takes owner/repo, which the https url the record keeps is read back to: one word on the
    // record, and the clone the host's own command makes of it.
    fn clone_command(&self, remote: &str, dest: &str, _branch: Option<&str>) -> String {
        let owner_repo = owner_repo_of(remote).unwrap_or_else(|| remote.to_string());
        let argv = self.host.clone_argv(&owner_repo, dest);
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        shell_line(&words)
    }

    fn cli(&self) -> Option<SourceCli> {
        Some(SourceCli { bin: self.host.cli, setup_git: self.host.setup_git, host: self.host })
    }
}

/// folder, git, then each of GIT_HOSTS (github, gitlab), in that order. project-sources.ts:109.
pub fn project_sources() -> IndexMap<&'static str, Rc<dyn ProjectSourceModule>> {
    let mut modules: IndexMap<&'static str, Rc<dyn ProjectSourceModule>> = IndexMap::new();
    modules.insert("folder", Rc::new(FolderModule));
    modules.insert("git", Rc::new(GitModule));
    for host in git_hosts() {
        let module = HostModule { host };
        modules.insert(module.kind(), Rc::new(module));
    }
    modules
}

/// The remote one source names, read through its own module: what a record written before it carried one is
/// filled in with, and what the add records. project-sources.ts:117.
pub fn project_remote(source: &ProjectSource) -> String {
    let kind = match source {
        ProjectSource::Folder(_) => "folder",
        ProjectSource::Git(_) => "git",
        ProjectSource::Github(_) => "github",
        ProjectSource::Gitlab(_) => "gitlab",
    };
    project_source(kind).map(|module| module.remote(source)).unwrap_or_default()
}

/// The module for one kind, or `no project source module for <kind>`. project-sources.ts:120.
pub fn project_source(kind: &str) -> Result<Rc<dyn ProjectSourceModule>, HostError> {
    project_sources().get(kind).cloned().ok_or_else(|| HostError::new(format!("no project source module for {kind}")))
}
