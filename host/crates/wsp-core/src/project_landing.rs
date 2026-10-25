// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/project-landing.ts at 65f6b89c7: the landing roads as modules, one per kind of
//! computer a project can be added on, and the scripts every road runs there. Specs: spec/landing_scripts.md (the
//! scripts, :119-352 and :483-504) and spec/landing_roads.md (the roads, :354-481 and :506-639).
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde::Serialize;
use wsp_engine::machine::Machine;
use wsp_wire::views::place::MachineBind;
use wsp_wire::views::workspace::{
    BranchOfRecord, ProjectAddStage, ProjectSource, ProjectView, ProjectViewGit, ProjectViewInstalled, ProjectViewSeeded, SeedChoice, SeedPlan,
};
use wsp_wire::views::workspace_more::ProjectBranch;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use crate::project_sources::ProjectSourceModule;

/// The folder a fork of an image holds its checkout under. project-landing.ts:107.
pub const FORK_CHECKOUT_HOME: &str = "/root";
/// How long the clone gets on the computer. project-landing.ts:115.
pub const CLONE_MS: u64 = 600_000;
/// How long the small steps get. project-landing.ts:117.
pub const STEP_MS: u64 = 120_000;
/// What the memory step prints where the computer already keeps memory at the agent's path. project-landing.ts:251.
pub const MEMORY_KEPT_MARK: &str = "wsp-memory-kept";
/// What the unsaved read prints for a folder that holds no git checkout. project-landing.ts:254.
pub const NO_CHECKOUT_MARK: &str = "wsp-no-checkout";
/// How many names the add tries in a home. project-landing.ts:495.
pub const NAMES_TRIED: u32 = 99;

/// How far the add has got. project-landing.ts:14.
pub type LandingReport = Rc<dyn Fn(ProjectAddStage, &str)>;

/// Where one computer keeps a project. project-landing.ts:18.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPlaces {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    pub memory_dir: String,
}

/// The seed archive a folder on this computer packed. project-landing.ts:29.
#[derive(Clone, Debug, PartialEq)]
pub struct LandSeed {
    pub tar: Vec<u8>,
    pub choice: SeedChoice,
    pub plan: SeedPlan,
}

/// What the add hands a landing road. project-landing.ts:26.
#[derive(Clone)]
pub struct LandRequest {
    pub project: ProjectView,
    pub source: Rc<dyn ProjectSourceModule>,
    pub seed: Option<LandSeed>,
    pub report: LandingReport,
}

/// What one landing left behind, which the record keeps. Serialized in the TS's interface order for tests; the
/// record's own keys are the projects area's. project-landing.ts:34.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Landed {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<ProjectViewGit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seeded: Option<ProjectViewSeeded>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<ProjectViewInstalled>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

/// A computer the person joined, as its login runs every command there, and that login's home.
#[derive(Clone)]
pub struct LandingComputer {
    pub machine: Arc<dyn Machine>,
    pub home: String,
}

/// What a landing road may ask of the runtime. project-landing.ts:52.
#[derive(Clone)]
pub struct LandingDeps {
    pub land: Rc<dyn Fn(Arc<dyn Machine>, String, Vec<u8>) -> LocalBoxFuture<'static, Result<(), HostError>>>,
    /// Where wsp writes its own working files on that machine.
    pub scratch: Rc<dyn Fn(&dyn Machine) -> String>,
    /// Absent on a provider and on this computer.
    pub computer: Option<LandingComputer>,
    /// Where Claude Code keeps its projects on this Mac.
    pub mac_state_home: String,
    /// The branch a folder on this computer has checked out; `None` on a detached head or where git answers nothing.
    pub branch_here: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, Option<String>>>,
    pub computer_name: String,
    /// What a clone on a computer the person owns reads on its own input.
    pub clone_env: Option<Rc<dyn Fn() -> IndexMap<String, String>>>,
    pub now: Rc<dyn Fn() -> i64>,
}

/// One landing road. The TS's optional members answer `None` where the road has none. project-landing.ts:71.
#[async_trait(?Send)]
pub trait ProjectLanding {
    /// `"box"`, `"provider"` or `"mac"`.
    fn kind(&self) -> &'static str;
    fn path(&self, name: &str, source: &ProjectSource, deps: &LandingDeps) -> Result<String, HostError>;
    fn places(&self, project: &ProjectView, memory_key: &str, deps: &LandingDeps) -> Result<ProjectPlaces, HostError>;
    fn lands_at_add(&self, seeding: bool) -> bool;
    async fn land(&self, o: LandRequest, deps: &LandingDeps) -> Result<Landed, HostError>;
    fn refusal(&self, project: &ProjectView, deps: &LandingDeps) -> Option<String>;
    async fn remove(&self, project: &ProjectView, deps: &LandingDeps) -> Result<String, HostError>;
    async fn folder_stands(&self, project: &ProjectView, deps: &LandingDeps) -> Option<Result<bool, HostError>> {
        None
    }
    async fn seed_into(&self, project: &ProjectView, tar: Vec<u8>, deps: &LandingDeps) -> Option<Result<(), HostError>> {
        None
    }
    async fn unsaved(&self, project: &ProjectView, deps: &LandingDeps) -> Option<Option<String>> {
        None
    }
    fn workspace_binds(&self, project: &ProjectView) -> Vec<MachineBind>;
    async fn branch(&self, project: &ProjectView, deps: &LandingDeps) -> ProjectBranch;
}

/// box, provider, mac, in that order. project-landing.ts:613.
pub fn project_landings() -> IndexMap<&'static str, Rc<dyn ProjectLanding>> {
    todo!()
}

/// The road for one kind, or `no project landing road for <kind>`. project-landing.ts:620.
pub fn project_landing(kind: &str) -> Result<Rc<dyn ProjectLanding>, HostError> {
    todo!()
}

/// `claudeMemoryDir(CLAUDE_CONFIG_DIR, memory_key)`. project-landing.ts:628.
pub fn guest_memory_dir(memory_key: &str) -> String {
    todo!()
}

/// A clone where the host keeps the machine's credentials. project-landing.ts:125 `brokered`.
#[derive(Clone, Debug, PartialEq)]
pub struct BrokeredClone {
    pub of_record: BranchOfRecord,
    /// The project's own owner/name.
    pub origin: String,
    pub held: bool,
}

/// What every clone line reads. project-landing.ts:125.
#[derive(Clone)]
pub struct CloneAsk {
    pub source: Rc<dyn ProjectSourceModule>,
    pub remote: String,
    pub checkout: String,
    pub computer: String,
    pub branch: Option<String>,
    /// `Some(false)` drops the setup-git line; `None` and `Some(true)` keep it.
    pub setup_git: Option<bool>,
    pub brokered: Option<BrokeredClone>,
}

/// project-landing.ts:125.
pub fn clone_lines(o: &CloneAsk) -> Vec<String> {
    todo!()
}

/// project-landing.ts:155.
pub fn on_record_lines(checkout: &str, of_record: &BranchOfRecord, origin: &str, held: bool) -> Vec<String> {
    todo!()
}

/// The repo of record as the git environment reads it. project-landing.ts:181 `repo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitEnvRepo {
    pub owner: String,
    pub name: String,
    pub public: bool,
}

/// GIT_CONFIG_COUNT, the numbered pairs and GIT_TERMINAL_PROMPT, in that key order. project-landing.ts:181.
pub fn brokered_git_env(token: &str, repo: Option<&GitEnvRepo>, direct: &[String]) -> IndexMap<String, String> {
    todo!()
}

/// The clone lines with the seed unpacked on top, joined by newlines. `o.brokered` is not read. project-landing.ts:200.
pub fn clone_script(o: &CloneAsk, seed_tar: Option<&str>) -> String {
    todo!()
}

/// project-landing.ts:209.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchStep {
    pub branch: String,
    pub base: String,
    pub back: String,
}

/// project-landing.ts:221.
pub fn patch_script(step: &PatchStep, checkout: &str) -> String {
    todo!()
}

/// project-landing.ts:239.
pub fn patch_cleanup_script(step: &PatchStep, checkout: &str) -> String {
    todo!()
}

/// project-landing.ts:262.
pub fn unsaved_script(checkout: &str) -> String {
    todo!()
}

/// project-landing.ts:301.
pub fn unsaved_of(name: &str, ran: &ExecResult) -> Option<String> {
    todo!()
}

/// One command run where the checkout is, with its timeout in ms.
pub type RunHere = Rc<dyn Fn(String, u64) -> LocalBoxFuture<'static, Result<ExecResult, HostError>>>;

/// project-landing.ts:314.
pub async fn read_unsaved(name: &str, checkout: &str, run: RunHere) -> Option<String> {
    todo!()
}

/// project-landing.ts:336.
pub fn seed_rest_script(checkout: &str, memory_dir: &str, seed_tar: &str, memory: bool) -> String {
    todo!()
}

/// project-landing.ts:485.
pub fn seed_into_script(checkout: &str, seed_tar: &str) -> String {
    todo!()
}

/// project-landing.ts:492.
pub fn under_login_home(home: &str, path: &str) -> String {
    todo!()
}

/// project-landing.ts:500.
pub fn free_folder_script(home: &str, name: &str) -> String {
    todo!()
}

/// The last line of `out` trimmed, `None` where it is empty. project-landing.ts:636.
pub fn last_line(out: &str) -> Option<String> {
    todo!()
}
