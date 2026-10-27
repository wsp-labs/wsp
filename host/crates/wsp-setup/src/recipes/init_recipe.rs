// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `init-recipe.ts`: what wsp init does with the collector's manifest, and the golden recipe the ticks add up to.
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;
use wsp_collect::manifest::{Manifest, ManifestEntry};
use wsp_engine::golden_seam::BrewTable;
use wsp_wire::types::init_job::LoginChoice;
use wsp_wire::views::golden_image::Recipe;

/// `refusedNote` (`init-recipe.ts:14`).
pub fn refused_note(_e: &ManifestEntry, _is_dir: Rc<dyn Fn(&str) -> Option<bool>>) -> Option<String> {
    todo!()
}

/// `lockRefused` (`init-recipe.ts:22`).
pub fn lock_refused(_manifest: &Manifest, _is_dir: Rc<dyn Fn(&str) -> Option<bool>>) -> Manifest {
    todo!()
}

/// `withoutAgentTools` (`init-recipe.ts:33`).
pub fn without_agent_tools(_entries: &[ManifestEntry]) -> Vec<ManifestEntry> {
    todo!()
}

/// `LoginTool` (`init-recipe.ts:61`).
#[derive(Clone, Debug, PartialEq)]
pub struct LoginTool {
    pub bin: String,
    pub row: Option<ManifestEntry>,
    pub coming: bool,
    pub why: Option<String>,
}

/// `loginTool` (`init-recipe.ts:73`).
pub fn login_tool(_e: &ManifestEntry, _manifest: &Manifest, _coming: &IndexSet<String>, _brew: &BrewTable) -> Option<LoginTool> {
    todo!()
}

/// `tickLoginTools` (`init-recipe.ts:85`).
pub fn tick_login_tools(
    _manifest: &Manifest,
    _choices: &IndexMap<String, String>,
    _ticks: &mut IndexSet<String>,
    _brew: &BrewTable,
) -> Vec<String> {
    todo!()
}

/// `isTickable` (`init-recipe.ts:98`).
pub fn is_tickable(_e: &ManifestEntry) -> bool {
    todo!()
}

/// `hasChoices` (`init-recipe.ts:103`).
pub fn has_choices(_e: &ManifestEntry) -> bool {
    todo!()
}

/// `initialTicks` (`init-recipe.ts:107`).
pub fn initial_ticks(_e: &ManifestEntry) -> bool {
    todo!()
}

/// `WITHOUT_A_COPY` (`init-recipe.ts:116`).
pub const WITHOUT_A_COPY: LoginChoice = LoginChoice::Later;

/// `initialChoice` (`init-recipe.ts:120`).
pub fn initial_choice(_e: &ManifestEntry) -> LoginChoice {
    todo!()
}

/// `loginEntryId` (`init-recipe.ts:134`).
pub fn login_entry_id(_e: &ManifestEntry) -> String {
    todo!()
}

/// `loginShown` (`init-recipe.ts:139`).
pub fn login_shown(_e: &ManifestEntry, _manifest: &Manifest, _ticks: &IndexSet<String>) -> bool {
    todo!()
}

/// `comingRows` (`init-recipe.ts:146`).
pub fn coming_rows(_manifest: &Manifest) -> IndexSet<String> {
    todo!()
}

/// `Answers` (`init-recipe.ts:150`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answers {
    pub ticks: IndexSet<String>,
    pub choices: IndexMap<String, String>,
}

/// `defaultAnswers` (`init-recipe.ts:156`).
pub fn default_answers(_manifest: &Manifest, _brew: &BrewTable, _coming: &IndexSet<String>) -> Answers {
    todo!()
}

/// `withCatalogAgents` (`init-recipe.ts:171`).
pub fn with_catalog_agents(_manifest: &Manifest) -> Manifest {
    todo!()
}

/// `isLoginChoice` (`init-recipe.ts:181`).
pub fn is_login_choice(_v: &Value) -> bool {
    todo!()
}

/// `applyRecipe` (`init-recipe.ts:191`).
pub fn apply_recipe(_manifest: &Manifest, _recipe: &Recipe) -> Manifest {
    todo!()
}

/// The inline `reading` of `manifestFor` (`init-recipe.ts:222`).
#[derive(Clone, Debug, PartialEq)]
pub struct ManifestReading {
    pub manifest: Manifest,
}

/// `manifestFor` (`init-recipe.ts:222`).
pub fn manifest_for(_reading: &ManifestReading, _recipe: &Recipe) -> Manifest {
    todo!()
}

/// `answeredRows` (`init-recipe.ts:228`).
pub fn answered_rows(_manifest: &Manifest, _ticks: &IndexSet<String>, _choices: &IndexMap<String, String>) -> Vec<ManifestEntry> {
    todo!()
}

/// `agentName` (`init-recipe.ts:235`).
pub fn agent_name(_e: &ManifestEntry) -> String {
    todo!()
}
