// SPDX-License-Identifier: AGPL-3.0-only
//! What golden.ts hands the join that no TS test reads by name: the recipe an import carries (the engine's
//! `GoldenImport` gains `recipe_hash` and `recipe`, which `GoldenCtx::import_mark` reads) and the key order of the
//! result JSON the TS assigns (golden.ts:613, :664, :696, :760, :789, :801-802).
#![allow(clippy::type_complexity)]
mod golden_roads_common;

use std::sync::Arc;

use golden_roads_common::*;
use serde_json::json;
use wsp_engine::golden::{apply_golden_import, ApplyImportOptions, ImportResult};
use wsp_engine::golden_seam::{GoldenImport, HomebrewCheckout, ToolResult};
use wsp_engine::machine::{Machine, MachineBackend, MachineSpec};
use wsp_wire::views::golden_image::RecipeDigest;

fn digest() -> RecipeDigest {
    serde_json::from_value(json!({ "ticks": [], "files": [] })).expect("a recipe digest")
}

// golden.ts:475 `recipeHash` and `recipe`, read by runtime.ts:474's `GoldenCtx.import_mark`
#[test]
fn an_import_carries_its_recipe_hash_and_recipe_into_the_ledger() {
    let b = backend_default();
    let clock = Manual::new();
    let spec: MachineSpec = serde_json::from_value(json!({ "kind": "sandbox", "template": "base" })).unwrap();
    let machine: Arc<dyn Machine> = run(b.rec.backend.create(spec, None, None)).unwrap();
    let imp = GoldenImport { recipe_hash: "h9".into(), recipe: Some(digest()), ..GoldenImport::default() };
    let applied = run(apply_golden_import(
        &machine,
        ApplyImportOptions { import: Some(imp), setup: "true".into(), fetch: Some(b.dyn_fetch()), ..ApplyImportOptions::default() },
        &*clock,
    ))
    .unwrap();
    assert_eq!(applied.ledger.recipe_hash, "h9");
    assert_eq!(applied.ledger.recipe, Some(digest()));
    assert_eq!(applied.result.recipe_hash, "h9");
}

// golden.ts:613 and the assignments after it: the keys write in the order the TS assigns them
#[test]
fn the_result_json_writes_its_keys_in_the_order_the_ts_assigns_them() {
    let result = ImportResult {
        recipe_hash: "h1".into(),
        tools: vec![ToolResult { id: "tools/npm/bun".into(), label: "bun".into(), outcome: "installed".into(), ..ToolResult::default() }],
        homebrew: Some(HomebrewCheckout { tag: "6.0.21".into(), commit: "abc".into() }),
        context_failure: Some("refused".into()),
        base: Some(vec![]),
        ..ImportResult::default()
    };
    assert_eq!(
        result.to_json().to_string(),
        r#"{"recipeHash":"h1","tools":[{"id":"tools/npm/bun","label":"bun","outcome":"installed"}],"agents":[],"base":[],"homebrew":{"tag":"6.0.21","commit":"abc"},"contextFailure":"refused"}"#
    );
}
