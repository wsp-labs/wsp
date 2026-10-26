// SPDX-License-Identifier: AGPL-3.0-only
//! Recording, reading and dropping a project, and moving folders onto and off a workspace's machine. Port of
//! runtime/src/projects/projects.ts at 65f6b89c7; spec/projects_add.md, spec/projects_door.md, spec/projects_move.md.
pub mod add;
pub mod door;
pub mod moves;

use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_core::types::wiring::{ProjectExportOptions, ProjectImportOptions, SeedWiring};
use wsp_engine::machine::MachineBackend;
use wsp_wire::types::thread_defaults::ThreadDefaults;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::project_bundle::{ProjectExportResult, ProjectImportResult};
use wsp_wire::views::workspace::{Caller, ProjectSeed, ProjectSource, ProjectView, SeedPlan};
use wsp_wire::views::workspace_more::ProjectBranch;
use wsp_words::exit::HostError;

use crate::projects::workspaces::seam::Base;
use add::{ComputerRow, JoinedComputer, LandingRoad, ProjectSourceModule};
use moves::MoveRoads;

/// What the projects area reads off the runtime context, past `Base`.
#[async_trait(?Send)]
pub trait ProjectsCtx: Base + MoveRoads {
    fn state_folder(&self) -> String;
    fn has_state_path(&self) -> bool;
    /// `ctx.opts.seed`.
    fn seed_wiring(&self) -> Option<Rc<dyn SeedWiring>>;
    /// `ctx.opts.cloudProjects`: `Some(false)` refuses projects on a provider.
    fn cloud_projects(&self) -> Option<bool>;
    /// The vault's `GITHUB_TOKEN`, where the vault holds one (`opts.vault?.()[GITHUB_TOKEN_ENV]`).
    fn vault_github_token(&self) -> Option<String>;
    /// `local?.home("claude")`.
    fn local_claude_home(&self) -> Option<String>;
    /// `ctx.homesHere()`.
    async fn homes_here(&self) -> Result<IndexMap<String, String>, HostError>;
    /// `ctx.refuseRecording(word, origin)`.
    fn refuse_recording(&self, word: &str, origin: Option<&Caller>) -> Result<(), HostError>;
    /// `ctx.computerRows()`.
    async fn computer_rows(&self) -> Result<Vec<ComputerRow>, HostError>;
    /// `ctx.nameOfComputer(id, rows)`.
    fn name_of_computer(&self, id: &str, rows: &[ComputerRow]) -> String;
    /// `ctx.cloneHere(word, source, into)`: the folder the repo was cloned into.
    async fn clone_here(&self, word: &str, source: &ProjectSource, into: &str) -> Result<String, HostError>;
    /// `ctx.gitTopOf(path)`.
    async fn git_top_of(&self, path: &str) -> Result<Option<String>, HostError>;
    /// `ctx.remoteHere(folder)`.
    async fn remote_here(&self, folder: &str) -> Result<String, HostError>;
    /// `ctx.landingPlace(computer).placeId`.
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError>;
    /// `ctx.forkingAt(placeId)`.
    async fn forking_at(&self, place_id: Option<&str>) -> Result<Arc<dyn MachineBackend>, HostError>;
    /// `ctx.landingBackend(placeId)`: refuses with the computer's absent sentence.
    async fn landing_backend(&self, place_id: &str) -> Result<Arc<dyn MachineBackend>, HostError>;
    /// `placeDoor?.folderComputer(placeId)`.
    fn folder_computer(&self, place_id: &str) -> Option<JoinedComputer>;
    /// `placeDoor?.githubFromVault(placeId) === true`.
    fn github_from_vault(&self, place_id: &str) -> bool;
    /// `projectLanding(kind)`.
    fn landing_road(&self, kind: add::LandingKind) -> Rc<dyn LandingRoad>;
    /// `projectSource(kind)`.
    fn project_source(&self, kind: &ProjectSource) -> Rc<dyn ProjectSourceModule>;
    /// `ctx.rememberProject(project)`.
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError>;
    /// `ctx.projectReached(origin, id)`.
    fn project_reached(&self, origin: Option<&Caller>, project_id: &str) -> bool;
    /// `ctx.projectOfScope(scope)`.
    fn project_of_scope(&self, origin: Option<&Caller>) -> Option<String>;
    /// `ctx.elsewhereRefusal(origin, project, word)`.
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &ProjectView, word: &str) -> Option<HostError>;
    /// `ctx.ofThreadsRepository(origin, id)`.
    fn of_threads_repository(&self, origin: Option<&Caller>, project_id: &str) -> bool;
    /// `ctx.holdsThread(workspaceId)` (folders').
    fn holds_thread(&self, workspace_id: &str) -> bool;
    /// `ctx.workspaces.delete(id, origin)`.
    async fn delete_workspace(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError>;
    /// `local.copier.worktreeForget({from, home, project})`; none where no copier is wired.
    async fn copier_worktree_forget(&self, from: &str, home: &str, project: &str) -> Option<Result<(), HostError>>;
    /// `ctx.projectIcons.forget(id)`.
    async fn forget_icon(&self, project_id: &str) -> Result<(), HostError>;
    /// `ctx.projectIcons.set(id, png)`.
    async fn set_icon(&self, project_id: &str, png: Option<String>) -> Result<Option<String>, HostError>;
    /// `ctx.projectIcons.read(hashes)`.
    async fn read_icons(&self, hashes: Vec<String>) -> Result<IndexMap<String, Option<String>>, HostError>;
    /// Agent ids an adapter is wired for (`adapters[id] !== undefined`).
    fn runs_agent(&self, agent: &str) -> bool;
    /// `ctx.computerHome(computer, origin)` then `ctx.agentLists(home)`.
    async fn agent_lists(&self, computer: &str, origin: Option<&Caller>) -> Result<Vec<HarnessCatalog>, HostError>;
    /// `resolveThreadDefaults({firstAgent, catalogOf, runs, prefs, project?})` over the lists read for its computer.
    fn thread_defaults(&self, project_id: &str, first_agent: &str, lists: &[HarnessCatalog]) -> ThreadDefaults;
    /// The first catalog agent an adapter runs, else the default agent.
    fn first_agent(&self) -> String;
}

/// The projects area.
pub struct Projects {
    pub(crate) ctx: Rc<dyn ProjectsCtx>,
}

// export function projectsArea(ctx: RuntimeContext): ProjectsArea {
pub fn projects_area(ctx: Rc<dyn ProjectsCtx>) -> Rc<Projects> {
    Rc::new(Projects { ctx })
}

impl wsp_core::context::ProjectsArea for Projects {}

/// `rt.projects`: each call forwards to the unit that holds it (add, door, moves).
#[async_trait(?Send)]
impl wsp_core::types::api::ProjectsApi for Projects {
    async fn add(&self, ask: wsp_core::types::api::ProjectAddAsk, origin: Option<Caller>) -> Result<wsp_core::types::api::ProjectAdded, HostError> {
        let handle = Rc::new(Projects { ctx: self.ctx.clone() });
        let added = handle
            .add(
                add::AddAsk {
                    source: ask.source,
                    on: ask.on,
                    name: ask.name,
                    base: ask.base,
                    into: ask.into,
                    seed: ask.seed,
                    ..Default::default()
                },
                origin,
            )
            .await?;
        Ok(wsp_core::types::api::ProjectAdded { project: added.project, notice: added.notice })
    }
    async fn seed_plan(&self, source: &str, on: Option<String>) -> Result<SeedPlan, HostError> {
        Projects::seed_plan(self, source, on.as_deref()).await
    }
    async fn seed(&self, reference: &str, ask: wsp_core::types::api::SeedAsk, origin: Option<Caller>) -> Result<ProjectSeed, HostError> {
        let words = add::SeedWords {
            keep: ask.keep,
            cut: ask.cut,
            none: ask.none == Some(true),
            save: ask.save == Some(true),
        };
        Projects::seed(self, reference, words, origin).await
    }
    async fn list(&self, origin: Option<Caller>) -> Result<Vec<ProjectView>, HostError> {
        Projects::list(self, origin).await
    }
    async fn defaults(&self, origin: Option<Caller>) -> Result<IndexMap<String, ThreadDefaults>, HostError> {
        Projects::defaults(self, origin).await
    }
    async fn computers(&self) -> Result<Vec<wsp_core::types::api::ComputerName>, HostError> {
        let rows = Projects::computers(self).await?;
        Ok(rows.into_iter().map(|row| wsp_core::types::api::ComputerName { id: row.id, name: row.name }).collect())
    }
    async fn resolve(&self, reference: &str, origin: Option<Caller>) -> Result<ProjectView, HostError> {
        Projects::resolve(self, reference, origin).await
    }
    async fn branch(&self, reference: &str, origin: Option<Caller>) -> Result<ProjectBranch, HostError> {
        Projects::branch(self, reference, origin).await
    }
    async fn remove(&self, id: &str, origin: Option<Caller>, force: Option<bool>, check: Option<bool>) -> Result<wsp_core::types::api::ProjectRemoved, HostError> {
        let words = door::RemoveWords { force: force == Some(true), check: check == Some(true), keep_look: false };
        let removed = Projects::remove_with(self, id, origin, words).await?;
        Ok(wsp_core::types::api::ProjectRemoved { said: removed.said, unsaved: removed.unsaved })
    }
    async fn icon(&self, project_id: &str, png: Option<String>) -> Result<Option<String>, HostError> {
        self.ctx.set_icon(project_id, png).await
    }
    async fn icons(&self, hashes: Vec<String>) -> Result<IndexMap<String, Option<String>>, HostError> {
        self.ctx.read_icons(hashes).await
    }
    async fn import(&self, opts: ProjectImportOptions, origin: Option<Caller>) -> Result<ProjectImportResult, HostError> {
        Projects::import(self, opts, origin).await
    }
    async fn export(&self, opts: ProjectExportOptions, origin: Option<Caller>) -> Result<ProjectExportResult, HostError> {
        Projects::export(self, opts, origin).await
    }
}
