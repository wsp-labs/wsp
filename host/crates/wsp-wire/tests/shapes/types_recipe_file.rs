// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of recipe-file.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::types::recipe_file::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_recipe_file.json");

/// 4 cases: what zod's parse of GitHubSignIn wrote, or the first issue it raised.
#[test]
fn git_hub_sign_in_reads_and_writes_as_zod_does() {
    cases::<GitHubSignIn>(FIXTURES, "GitHubSignIn");
}

/// 7 cases: what zod's parse of RecipeFile wrote, or the first issue it raised.
#[test]
fn recipe_file_reads_and_writes_as_zod_does() {
    cases::<RecipeFile>(FIXTURES, "RecipeFile");
}

/// 5 cases: what zod's parse of RecipeSignIn wrote, or the first issue it raised.
#[test]
fn recipe_sign_in_reads_and_writes_as_zod_does() {
    cases::<RecipeSignIn>(FIXTURES, "RecipeSignIn");
}

/// 7 cases: what zod's parse of RecipeView wrote, or the first issue it raised.
#[test]
fn recipe_view_reads_and_writes_as_zod_does() {
    cases::<RecipeView>(FIXTURES, "RecipeView");
}

/// 5 cases: what zod's parse of ServerSignIn wrote, or the first issue it raised.
#[test]
fn server_sign_in_reads_and_writes_as_zod_does() {
    cases::<ServerSignIn>(FIXTURES, "ServerSignIn");
}
