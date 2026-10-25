// SPDX-License-Identifier: AGPL-3.0-only
//! Unit import_plan's hard tests: each recipe in tests/fixtures/oracle/golden_import_plan.json is a run of the TS
//! toolInstallsFor; the Rust plan must be the same steps in the same order, each step's every field byte for byte,
//! with the same skips, base rows and Brewfile. spec/golden-import_plan.md names the rule each recipe pins.
mod golden_common;

use golden_common::{fixture, s};
use serde_json::{json, Map, Value};
use wsp_engine::golden_seam::{tool_installs_for, tool_installs_for_rows, BrewTable, RecipeEntry, ToolInstall};
use wsp_wire::views::golden_image::RecipeCustomRow;

fn recipe(name: &str) -> (Value, Value) {
    let f = fixture("import_plan");
    let r = f["recipes"].as_array().unwrap().iter().find(|r| r["name"] == name).unwrap_or_else(|| panic!("no recipe {name}")).clone();
    (f, r)
}

fn check(name: &str) {
    let (f, r) = recipe(name);
    let rows: Vec<RecipeEntry> = serde_json::from_value(r["rows"].clone()).unwrap();
    let table: BrewTable = if r["table"] == json!(true) {
        f["table"].as_array().unwrap().iter().map(|p| (s(&p[0]), serde_json::from_value(p[1].clone()).unwrap())).collect()
    } else {
        BrewTable::new()
    };
    let custom: Vec<RecipeCustomRow> =
        if r["custom"] == json!(true) { serde_json::from_value(f["custom"].clone()).unwrap() } else { vec![] };
    let plan =
        tool_installs_for_rows(&rows, &table, &custom, &s(&r["path"]), r["prefix"].as_str()).unwrap_or_else(|e| panic!("{name}: {e}"));
    let want: Vec<ToolInstall> = serde_json::from_value(r["out"]["installs"].clone()).unwrap();
    assert_eq!(
        plan.installs.iter().map(|t| (t.id.as_str(), t.after.as_deref())).collect::<Vec<_>>(),
        want.iter().map(|t| (t.id.as_str(), t.after.as_deref())).collect::<Vec<_>>(),
        "{name}: the steps in order with what each waits on"
    );
    for (g, w) in plan.installs.iter().zip(want.iter()) {
        assert_eq!(g, w, "{name}: {}", w.id);
    }
    assert_eq!(serde_json::to_value(&plan.skipped).unwrap(), r["out"]["skipped"], "{name}: the skips");
    let base: Vec<Value> = plan
        .base
        .iter()
        .map(|b| {
            let mut m = Map::new();
            m.insert("id".into(), json!(b.id));
            m.insert("name".into(), json!(b.name));
            m.insert("entry".into(), json!(b.entry.id));
            if let Some(v) = &b.version {
                m.insert("version".into(), json!(v));
            }
            Value::Object(m)
        })
        .collect();
    let want_base: Vec<Value> = r["out"]["base"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            let mut b = b.clone();
            b["entry"] = b["entry"]["id"].clone();
            b
        })
        .collect();
    assert_eq!(base, want_base, "{name}: the base rows");
    assert_eq!(plan.brewfile, s(&r["out"]["brewfile"]), "{name}: the Brewfile");
}

#[test]
fn homebrew_first_then_taps_formulae_and_each_manager_after_its_own_install() {
    check("the TS test's recipe: Homebrew first, taps, formulae, each manager after its own install");
}

#[test]
fn gh_and_turbo_on_the_probe_path() {
    check("the provision tests' gh and turbo on the probe PATH");
}

#[test]
fn ruff_and_tokei_under_a_prefix_name_the_prefix_homes() {
    check("ruff and tokei under a prefix");
}

#[test]
fn rows_the_floor_covers_are_the_bases_with_the_version_read_from_the_brew_table() {
    check("rows the base floor covers are the base's");
}

#[test]
fn one_formula_has_no_shared_step() {
    check("one formula: no shared step");
}

#[test]
fn two_formulae_get_one_shared_step_between_the_taps_and_the_first_formula() {
    check("two formulae: one shared step");
}

#[test]
fn eight_formulae_share_one_step() {
    check("eight formulae");
}

#[test]
fn a_manager_needed_only_for_its_rows_brings_homebrew_when_a_formula_is_its_road() {
    check("a manager needed only for its rows brings Homebrew when a formula is its road");
}

#[test]
fn cargos_rows_bring_rustup_and_wait_on_it() {
    check("cargo's rows bring rustup");
}

#[test]
fn a_macs_rust_formula_is_the_catalogs_rust_row_and_cargo_rows_wait_on_it() {
    check("a Mac's rust formula is the catalog's rust row");
}

#[test]
fn a_macs_rustup_formula_is_the_catalogs_rust_row() {
    check("a Mac's rustup formula");
}

#[test]
fn buns_rows_wait_on_an_npm_install_of_bun() {
    check("bun's rows wait on an npm install of bun");
}

#[test]
fn the_node_step_runs_once_ahead_of_every_row_on_node() {
    check("the node step runs once ahead of every row on node");
}

#[test]
fn the_recipes_own_node_row_is_the_step() {
    check("the recipe's own node row is the step");
}

#[test]
fn a_macs_node_formula_is_the_step() {
    check("a Mac's node formula");
}

#[test]
fn unticked_rows_plan_nothing() {
    check("no Homebrew for unticked rows");
}

#[test]
fn an_empty_recipe_plans_nothing_and_writes_an_empty_brewfile() {
    check("nothing");
}

#[test]
fn catalog_rows_on_every_road_wait_on_what_their_road_needs_and_an_unknown_one_is_skipped() {
    check("catalog rows on every road");
}

#[test]
fn a_catalog_go_row_beside_go_rows_is_the_managers_step() {
    check("a catalog go row beside go rows");
}

#[test]
fn tap_formulae_with_a_known_release_go_by_it_with_their_pin_and_the_rest_are_skipped() {
    check("tap formulae on the release road");
}

#[test]
fn rows_outside_the_catalog_come_last_each_after_its_manager() {
    check("custom rows after everything, each after its manager");
}

#[test]
fn rows_outside_the_catalog_alone_bring_their_managers() {
    check("custom rows alone bring their managers");
}

#[test]
fn agent_and_apt_rows_plan_nothing_and_a_tap_alone_brings_homebrew() {
    check("an agent row and an apt row install nothing here");
}

#[test]
fn provisions_own_rows_plan_as_the_ts_with_the_id_as_the_label() {
    check("provision's own rows: gh and turbo, the id as the label");
    check("provision's own rows: ruff and tokei under a prefix");
}

#[test]
fn the_wrapper_provision_calls_is_the_plan_of_provisions_own_rows() {
    for (name, ids) in [
        ("provision's own rows: gh and turbo, the id as the label", vec!["tools/brew/gh", "tools/npm/turbo"]),
        ("provision's own rows: ruff and tokei under a prefix", vec!["tools/uv/ruff", "tools/cargo/tokei"]),
    ] {
        let (_, r) = recipe(name);
        let want: Vec<ToolInstall> = serde_json::from_value(r["out"]["installs"].clone()).unwrap();
        assert_eq!(tool_installs_for(&ids, &s(&r["path"]), r["prefix"].as_str()), want, "{name}");
    }
}
