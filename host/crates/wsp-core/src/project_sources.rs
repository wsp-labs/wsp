// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/project-sources.ts at 65f6b89c7: the project sources as modules, one per kind. The add
//! asks a module through `project_sources()`; nothing else decides by a source's kind. Spec: spec/landing_roads.md.
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_catalog::git_hosts::GitHost;
use wsp_wire::views::workspace::{ProjectSource, SeedPlan};
use wsp_words::exit::HostError;

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
        todo!()
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

/// folder, git, then each of GIT_HOSTS (github, gitlab), in that order. project-sources.ts:109.
pub fn project_sources() -> IndexMap<&'static str, Rc<dyn ProjectSourceModule>> {
    todo!()
}

/// The remote one source names, read through its own module. project-sources.ts:117.
pub fn project_remote(source: &ProjectSource) -> String {
    todo!()
}

/// The module for one kind, or `no project source module for <kind>`. project-sources.ts:120.
pub fn project_source(kind: &str) -> Result<Rc<dyn ProjectSourceModule>, HostError> {
    todo!()
}
