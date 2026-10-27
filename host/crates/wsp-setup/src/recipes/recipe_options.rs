// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `recipe-options.ts`: what a recipe can pick from, read off this computer when the modal or a recipe opens.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::{IndexMap, IndexSet};
use wsp_collect::history::cache::HistoryCache;
use wsp_collect::history::AgentHistory;
use wsp_collect::host::{Host, HostExec};
use wsp_collect::manifest::Manifest;
use wsp_wire::types::agents_report::SkillRow;
use wsp_wire::types::recipe_file::RecipeSignIn;
use wsp_words::exit::HostError;
use wsp_words::recipe_file::{FolderOption, GhStatus, RecipeOptions};

use crate::recipes::recipe_configs::ConfigFiles;

/// The `billed` of `signinsOf` (`recipe-options.ts:35`), `"key"` or `"login"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BilledAs {
    Key,
    Login,
}

/// `signinsOf` (`recipe-options.ts:35`).
pub fn signins_of(_agent_id: &str, _billed: Option<BilledAs>) -> Vec<RecipeSignIn> {
    todo!()
}

/// `userPlugins` (`recipe-options.ts:48`).
pub fn user_plugins(_text: Option<&str>) -> Vec<String> {
    todo!()
}

/// `CommandCalls` (`recipe-options.ts:62`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CommandCalls {
    pub by_tool: IndexMap<String, u64>,
    pub by_name: IndexMap<String, u64>,
}

/// `commandCalls` (`recipe-options.ts:68`).
pub fn command_calls(_histories: &[AgentHistory]) -> CommandCalls {
    todo!()
}

/// `Bundled` (`recipe-options.ts:82`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bundled {
    pub dir: String,
    pub names: IndexSet<String>,
}

/// The `o` of `recipeOptions` (`recipe-options.ts:92`), an options object.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeOptionsOptions {
    pub skills: Vec<SkillRow>,
    pub bundled: Option<Vec<Bundled>>,
    pub wsp: Option<IndexSet<String>>,
    pub plugins: Vec<String>,
    pub enabled: Option<Vec<String>>,
    pub configs: Vec<ConfigFiles>,
    pub github: bool,
    pub gh: Option<GhStatus>,
    pub calls: Option<CommandCalls>,
    pub billed: Option<IndexMap<String, BilledAs>>,
}

/// `recipeOptions` (`recipe-options.ts:92`).
pub fn recipe_options(_manifest: &Manifest, _o: &RecipeOptionsOptions) -> RecipeOptions {
    todo!()
}

/// `readsAnonymously` (`recipe-options.ts:149`).
pub async fn reads_anonymously(_url: &str) -> bool {
    todo!()
}

/// The inline `{ name, path }` of `folderOptions` (`recipe-options.ts:156`).
#[derive(Clone, Debug, PartialEq)]
pub struct NamedFolder {
    pub name: String,
    pub path: String,
}

/// `folderOptions` (`recipe-options.ts:156`).
pub async fn folder_options(_folders: &[NamedFolder], _anonymous: Rc<dyn Fn(&str) -> LocalBoxFuture<'static, bool>>) -> Vec<FolderOption> {
    todo!()
}

/// `githubHere` (`recipe-options.ts:178`).
pub async fn github_here(_exec: &dyn HostExec) -> GhStatus {
    todo!()
}

/// `bundledSkills` (`recipe-options.ts:185`).
pub async fn bundled_skills(_host: &Host) -> Vec<Bundled> {
    todo!()
}

/// `readRecipeOptions` (`recipe-options.ts:197`).
pub async fn read_recipe_options(_host: &Host, _cache: Option<&dyn HistoryCache>) -> Result<RecipeOptions, HostError> {
    todo!()
}
