// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/recipe-sync.ts at 65f6b89c7: what moved between the recipe a computer last applied and
//! the recipe it follows now, row by row, and which steps of the setup carry each move. Spec: spec/setup_steps.md.
use indexmap::{IndexMap, IndexSet};
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::PlaceSetupStep;
use wsp_words::recipe_file::RecipeKind;

/// How one row moved (recipe-sync.ts:13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipeChangeHow {
    Added,
    Changed,
    Edited,
    Removed,
}

/// One row that moved, keyed `<kind>/<row>` as a resolved recipe keys it (recipe-sync.ts:10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeChange {
    pub key: String,
    pub kind: RecipeKind,
    pub name: String,
    pub how: RecipeChangeHow,
}

/// Every row added, changed, edited or taken out, kind by kind in RECIPE_KINDS order: within a kind the rows of
/// `after` in its order, then the rows only `before` has. recipe-sync.ts:36.
pub fn recipe_changes(
    before: Option<&RecipeFile>,
    before_items: Option<&IndexMap<String, String>>,
    after: &RecipeFile,
    after_items: &IndexMap<String, String>,
) -> Vec<RecipeChange> {
    todo!()
}

/// The steps that carry a set of changes, in the order first added. recipe-sync.ts:56.
pub fn steps_for(changes: &[RecipeChange], after: &RecipeFile) -> IndexSet<PlaceSetupStep> {
    todo!()
}
