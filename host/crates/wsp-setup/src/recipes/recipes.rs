// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `recipes.ts`: the recipes this host keeps, one TOML file each under the state folder.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use serde_json::Value;
use wsp_collect::host::Host;
use wsp_collect::manifest::ManifestEntry;
use wsp_core::types::wiring::RecipeShelf;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_words::exit::HostError;
use wsp_words::recipe_file::{RecipeOptions, ResolvedRecipe};

use crate::recipes::recipe_options::NamedFolder;

/// `recipeSecretRefusal` (`recipes.ts:34`).
pub fn recipe_secret_refusal(_file: &Value, _at: &str) -> Option<String> {
    todo!()
}

/// `readRecipeText` (`recipes.ts:58`).
pub async fn read_recipe_text(_text: &str, _path: &str) -> Result<RecipeFile, HostError> {
    todo!()
}

/// `SavedRecipe` (`recipes.ts:72`).
#[derive(Clone, Debug, PartialEq)]
pub struct SavedRecipe {
    pub slug: String,
    pub file: RecipeFile,
    pub saved_at: Option<String>,
}

/// `readRecipes` (`recipes.ts:86`).
pub async fn read_recipes(_state_path: &str) -> Vec<SavedRecipe> {
    todo!()
}

/// `readRecipe` (`recipes.ts:99`).
pub async fn read_recipe(_state_path: &str, _word: &str) -> Result<SavedRecipe, HostError> {
    todo!()
}

/// `writeRecipe` (`recipes.ts:111`).
pub async fn write_recipe(_state_path: &str, _given: &Value) -> Result<SavedRecipe, HostError> {
    todo!()
}

/// `deleteRecipe` (`recipes.ts:125`).
pub async fn delete_recipe(_state_path: &str, _word: &str) -> Result<SavedRecipe, HostError> {
    todo!()
}

/// `RecipeReading` (`recipes.ts:132`).
#[derive(Clone)]
pub struct RecipeReading {
    pub home: String,
    pub tools: Rc<dyn Fn() -> LocalBoxFuture<'static, Vec<ManifestEntry>>>,
}

/// `agentOwnPaths` (`recipes.ts:159`).
pub fn agent_own_paths(_id: &str) -> Vec<String> {
    todo!()
}

/// The `{ key, paths, digest }` items of `itemSources` (`recipes.ts:178`).
#[derive(Clone)]
pub struct ItemSource {
    pub key: String,
    pub paths: Vec<String>,
    pub digest: Rc<dyn Fn() -> String>,
}

/// `itemSources` (`recipes.ts:178`).
pub fn item_sources(_file: &RecipeFile, _home: &str) -> Vec<ItemSource> {
    todo!()
}

/// `resolveRecipe` (`recipes.ts:224`).
pub async fn resolve_recipe(_file: &RecipeFile, _reading: &RecipeReading) -> ResolvedRecipe {
    todo!()
}

/// `recipeHash` (`recipes.ts:236`).
pub fn recipe_hash(_resolved: &ResolvedRecipe) -> String {
    todo!()
}

/// `TOOLS_KEPT_MS` (`recipes.ts:243`).
pub const TOOLS_KEPT_MS: u64 = 24 * 60 * 60_000;

/// The object `keptTools` returns (`recipes.ts:246`).
pub struct KeptTools;

impl KeptTools {
    /// `tools` (`recipes.ts:254`).
    pub async fn tools(&self) -> Vec<ManifestEntry> {
        todo!()
    }

    /// `refresh` (`recipes.ts:248`).
    pub async fn refresh(&self) -> Vec<ManifestEntry> {
        todo!()
    }
}

/// `keptTools` (`recipes.ts:246`).
pub fn kept_tools(_read: Rc<dyn Fn() -> LocalBoxFuture<'static, Vec<ManifestEntry>>>, _now: Rc<dyn Fn() -> u64>) -> KeptTools {
    todo!()
}

/// The `folders` argument of `keptOptions` (`recipes.ts:257`), not exported in the TS.
pub type Folders = Option<Vec<NamedFolder>>;

/// `keptOptions` (`recipes.ts:262`).
pub fn kept_options(
    _read: Rc<dyn Fn(Folders) -> LocalBoxFuture<'static, RecipeOptions>>,
) -> Rc<dyn Fn(Folders) -> LocalBoxFuture<'static, RecipeOptions>> {
    todo!()
}

/// The `o` of `recipeShelf` (`recipes.ts:292`).
#[derive(Clone)]
pub struct RecipeShelfOptions {
    pub state_path: String,
    pub home: String,
    pub options: Option<Rc<dyn Fn() -> LocalBoxFuture<'static, RecipeOptions>>>,
    pub tools: Option<Rc<dyn Fn() -> LocalBoxFuture<'static, Vec<ManifestEntry>>>>,
    pub here: Option<Host>,
}

/// `recipeShelf` (`recipes.ts:292`).
pub fn recipe_shelf(_o: RecipeShelfOptions) -> Rc<dyn RecipeShelf> {
    todo!()
}

/// `recipesDir` (`recipes.ts:27`).
pub fn recipes_dir(_state_path: &str) -> String {
    todo!()
}
