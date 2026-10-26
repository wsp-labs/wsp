// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/golden-image.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::views::golden_image::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_golden_image.json");

/// 3 cases: what zod's parse of GoldenBaseTool wrote, or the first issue it raised.
#[test]
fn golden_base_tool_reads_and_writes_as_zod_does() {
    cases::<GoldenBaseTool>(FIXTURES, "GoldenBaseTool");
}

/// 7 cases: what zod's parse of GoldenBuilderView wrote, or the first issue it raised.
#[test]
fn golden_builder_view_reads_and_writes_as_zod_does() {
    cases::<GoldenBuilderView>(FIXTURES, "GoldenBuilderView");
}

/// 3 cases: what zod's parse of GoldenLeftBehind wrote, or the first issue it raised.
#[test]
fn golden_left_behind_reads_and_writes_as_zod_does() {
    cases::<GoldenLeftBehind>(FIXTURES, "GoldenLeftBehind");
}

/// 4 cases: what zod's parse of GoldenLogin wrote, or the first issue it raised.
#[test]
fn golden_login_reads_and_writes_as_zod_does() {
    cases::<GoldenLogin>(FIXTURES, "GoldenLogin");
}

/// 3 cases: what zod's parse of GoldenManifest wrote, or the first issue it raised.
#[test]
fn golden_manifest_reads_and_writes_as_zod_does() {
    cases::<GoldenManifest>(FIXTURES, "GoldenManifest");
}

/// 4 cases: what zod's parse of GoldenMissingTool wrote, or the first issue it raised.
#[test]
fn golden_missing_tool_reads_and_writes_as_zod_does() {
    cases::<GoldenMissingTool>(FIXTURES, "GoldenMissingTool");
}

/// 3 cases: what zod's parse of GoldenRetired wrote, or the first issue it raised.
#[test]
fn golden_retired_reads_and_writes_as_zod_does() {
    cases::<GoldenRetired>(FIXTURES, "GoldenRetired");
}

/// 14 cases: what zod's parse of GoldenStage wrote, or the first issue it raised.
#[test]
fn golden_stage_reads_and_writes_as_zod_does() {
    cases::<GoldenStage>(FIXTURES, "GoldenStage");
}

/// 5 cases: what zod's parse of GoldenStageEvent wrote, or the first issue it raised.
#[test]
fn golden_stage_event_reads_and_writes_as_zod_does() {
    cases_tagged::<GoldenStageEvent>(FIXTURES, "GoldenStageEvent", "type", GoldenStageEvent::TYPE);
}

/// 3 cases: what zod's parse of GoldenStep wrote, or the first issue it raised.
#[test]
fn golden_step_reads_and_writes_as_zod_does() {
    cases::<GoldenStep>(FIXTURES, "GoldenStep");
}

/// 10 cases: what zod's parse of GoldenVersion wrote, or the first issue it raised.
#[test]
fn golden_version_reads_and_writes_as_zod_does() {
    cases::<GoldenVersion>(FIXTURES, "GoldenVersion");
}

/// 3 cases: what zod's parse of MachineKind wrote, or the first issue it raised.
#[test]
fn machine_kind_reads_and_writes_as_zod_does() {
    cases::<MachineKind>(FIXTURES, "MachineKind");
}

/// 7 cases: what zod's parse of Recipe wrote, or the first issue it raised.
#[test]
fn recipe_reads_and_writes_as_zod_does() {
    cases::<Recipe>(FIXTURES, "Recipe");
}

/// 12 cases: what zod's parse of RecipeCustomRow wrote, or the first issue it raised.
#[test]
fn recipe_custom_row_reads_and_writes_as_zod_does() {
    cases::<RecipeCustomRow>(FIXTURES, "RecipeCustomRow");
}

/// 4 cases: what zod's parse of RecipeDigest wrote, or the first issue it raised.
#[test]
fn recipe_digest_reads_and_writes_as_zod_does() {
    cases::<RecipeDigest>(FIXTURES, "RecipeDigest");
}

/// 9 cases: what zod's parse of RecipeHistory wrote, or the first issue it raised.
#[test]
fn recipe_history_reads_and_writes_as_zod_does() {
    cases::<RecipeHistory>(FIXTURES, "RecipeHistory");
}

/// 10 cases: what zod's parse of RecipeRow wrote, or the first issue it raised.
#[test]
fn recipe_row_reads_and_writes_as_zod_does() {
    cases::<RecipeRow>(FIXTURES, "RecipeRow");
}

/// 5 cases: what zod's parse of RecipeSource wrote, or the first issue it raised.
#[test]
fn recipe_source_reads_and_writes_as_zod_does() {
    cases::<RecipeSource>(FIXTURES, "RecipeSource");
}

/// 10 cases: what zod's parse of SealedImage wrote, or the first issue it raised.
#[test]
fn sealed_image_reads_and_writes_as_zod_does() {
    cases::<SealedImage>(FIXTURES, "SealedImage");
}

/// 4 cases: what zod's parse of SealedImageBuilt wrote, or the first issue it raised.
#[test]
fn sealed_image_built_reads_and_writes_as_zod_does() {
    cases::<SealedImageBuilt>(FIXTURES, "SealedImageBuilt");
}

/// 9 cases: what zod's parse of SealedImageCopy wrote, or the first issue it raised.
#[test]
fn sealed_image_copy_reads_and_writes_as_zod_does() {
    cases::<SealedImageCopy>(FIXTURES, "SealedImageCopy");
}

/// 9 cases: what zod's parse of SealedVault wrote, or the first issue it raised.
#[test]
fn sealed_vault_reads_and_writes_as_zod_does() {
    cases::<SealedVault>(FIXTURES, "SealedVault");
}

/// 5 cases: what zod's parse of SealedVaultHeader wrote, or the first issue it raised.
#[test]
fn sealed_vault_header_reads_and_writes_as_zod_does() {
    cases::<SealedVaultHeader>(FIXTURES, "SealedVaultHeader");
}

/// 7 cases: what zod's parse of ToolPin wrote, or the first issue it raised.
#[test]
fn tool_pin_reads_and_writes_as_zod_does() {
    cases::<ToolPin>(FIXTURES, "ToolPin");
}
