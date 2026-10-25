// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/recipe-sync.ts: what moved between the recipe a computer last applied and the recipe
//! it follows now. Only the row type is here; `recipe_changes` and `steps_for` have no interface yet.

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
