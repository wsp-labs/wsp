// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/runtime/test/recipe-sync.test.ts (5 cases) and pins src/recipe_sync.rs against the TS's own answers
//! in tests/fixtures/core_rest/recipe_sync.json: the changes and the steps of every row, and the steps of the
//! hand-built change lists.
mod rest;

use indexmap::IndexMap;
use rest::*;
use serde_json::{json, Value};
use wsp_core::recipe_sync::{recipe_changes, steps_for, RecipeChange, RecipeChangeHow};
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::PlaceSetupStep;
use wsp_words::recipe_file::RecipeKind;

type Items = IndexMap<String, String>;

fn before_json() -> Value {
    json!({
        "name": "laptop",
        "agents": { "claude": { "signin": "vault" } },
        "clis": { "jq": { "via": "brew" } },
        "skills": { "unslop": { "from": "~/.claude/skills" } },
        "mcp": { "linear": { "agents": ["claude"] } },
        "configs": { "git": {} },
    })
}

fn before() -> RecipeFile {
    wire(&before_json())
}

fn with(over: &[(&str, Value)]) -> RecipeFile {
    let mut v = before_json();
    for (k, val) in over {
        v[*k] = val.clone();
    }
    wire(&v)
}

fn items() -> Items {
    wire(&json!({ "agents/claude": "a1", "clis/jq": "1.7", "skills/unslop": "s1", "mcp/linear/claude": "m1", "configs/git": "g1" }))
}

fn items_with(over: &[(&str, &str)]) -> Items {
    let mut i = items();
    for (k, v) in over {
        i.insert(k.to_string(), v.to_string());
    }
    i
}

fn how_word(how: RecipeChangeHow) -> &'static str {
    match how {
        RecipeChangeHow::Added => "added",
        RecipeChangeHow::Changed => "changed",
        RecipeChangeHow::Edited => "edited",
        RecipeChangeHow::Removed => "removed",
    }
}

fn how_of(word: &str) -> RecipeChangeHow {
    match word {
        "added" => RecipeChangeHow::Added,
        "changed" => RecipeChangeHow::Changed,
        "edited" => RecipeChangeHow::Edited,
        "removed" => RecipeChangeHow::Removed,
        other => panic!("{other} is not a way a row moves"),
    }
}

fn key_how(changes: &[RecipeChange]) -> Vec<(String, &'static str)> {
    changes.iter().map(|c| (c.key.clone(), how_word(c.how))).collect()
}

fn pairs(list: &[(&str, &'static str)]) -> Vec<(String, &'static str)> {
    list.iter().map(|(k, h)| (k.to_string(), *h)).collect()
}

fn step_words(steps: impl IntoIterator<Item = PlaceSetupStep>) -> Vec<String> {
    steps.into_iter().map(|s| json(&s).as_str().unwrap().to_string()).collect()
}

fn sorted_steps(after: &RecipeFile, after_items: &Items) -> Vec<String> {
    let changes = recipe_changes(Some(&before()), Some(&items()), after, after_items);
    let mut words = step_words(steps_for(&changes, after));
    words.sort();
    words
}

fn strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

// TS: recipe-sync.test.ts:19
#[test]
fn what_moved_is_nothing_where_nothing_moved() {
    assert_eq!(recipe_changes(Some(&before()), Some(&items()), &before(), &items()), vec![]);
}

// TS: recipe-sync.test.ts:23
#[test]
fn names_each_row_added_changed_in_the_recipe_edited_on_this_computer_and_taken_out() {
    let after = with(&[
        ("agents", json!({ "claude": { "signin": "machine" } })),
        ("skills", json!({ "why": { "from": "~/.claude/skills" } })),
        ("plugins", json!({ "lint@acme": {} })),
    ]);
    let after_items = items_with(&[("clis/jq", "1.8"), ("skills/why", "w1")]);
    let changes = recipe_changes(Some(&before()), Some(&items()), &after, &after_items);
    assert_eq!(
        key_how(&changes),
        pairs(&[
            ("agents/claude", "changed"),
            ("clis/jq", "edited"),
            ("skills/why", "added"),
            ("skills/unslop", "removed"),
            ("plugins/lint@acme", "added"),
        ])
    );
}

// TS: recipe-sync.test.ts:35
#[test]
fn the_one_answer_about_the_servers_keys_moving_is_every_server_that_stays_changed_carried_by_the_servers_step() {
    let yes = with(&[("copyKeys", json!(true))]);
    let moved = recipe_changes(Some(&before()), Some(&items()), &yes, &items());
    assert_eq!(key_how(&moved), pairs(&[("mcp/linear", "changed")]));
    assert_eq!(step_words(steps_for(&moved, &yes)), strings(&["mcp"]));
    let back = recipe_changes(Some(&yes), Some(&items()), &before(), &items());
    assert_eq!(key_how(&back), pairs(&[("mcp/linear", "changed")]));
    assert_eq!(recipe_changes(Some(&yes), Some(&items()), &yes, &items()), vec![]);
}

// TS: recipe-sync.test.ts:43
#[test]
fn reads_every_row_with_an_item_as_edited_once_on_a_computer_that_kept_no_items() {
    let changes = recipe_changes(Some(&before()), None, &before(), &items());
    let keys: Vec<&str> = changes.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(keys, vec!["agents/claude", "mcp/linear", "clis/jq", "skills/unslop", "configs/git"]);
}

// TS: recipe-sync.test.ts:47
#[test]
fn carries_each_move_by_the_steps_that_put_it_on_and_a_removal_by_none_but_the_servers() {
    let why = json!({ "unslop": { "from": "~/.claude/skills" }, "why": { "from": "~/.claude/skills" } });
    assert_eq!(sorted_steps(&with(&[("skills", why)]), &items_with(&[("skills/why", "w")])), strings(&["skills"]));
    assert_eq!(sorted_steps(&before(), &items_with(&[("agents/claude", "a2")])), strings(&["mcp"]));
    assert_eq!(sorted_steps(&with(&[("agents", json!({ "claude": { "signin": "machine" } }))]), &items()), strings(&["signins"]));
    let codex = json!({ "claude": { "signin": "vault" }, "codex": { "signin": "machine" } });
    assert_eq!(sorted_steps(&with(&[("agents", codex)]), &items()), strings(&["agents", "context", "mcp", "signins", "skills"]));
    let nextest = json!({ "jq": { "via": "brew" }, "nextest": { "via": "cargo", "needs": ["build-essential"] } });
    assert_eq!(sorted_steps(&with(&[("clis", nextest)]), &items()), strings(&["clis", "context", "floor"]));
    assert_eq!(sorted_steps(&with(&[("mcp", json!({}))]), &items()), strings(&["mcp"]));
    assert_eq!(sorted_steps(&with(&[("skills", json!({}))]), &items()), Vec::<String>::new());
    let github = json!({ "git": {}, "github": { "signin": "vault" } });
    assert_eq!(sorted_steps(&with(&[("configs", github)]), &items()), strings(&["github"]));
    let folder = json!({ "app": { "from": "~/code/app", "keep": [] } });
    assert_eq!(sorted_steps(&with(&[("folders", folder)]), &items()), strings(&["folderServers", "folders"]));
}

const FIRST_APPLY: &[&str] = &["nothing moved", "first apply", "first apply with items before", "no items kept", "no items kept, none now"];
const MOVED_IN_THE_RECIPE: &[&str] = &["mixed", "keys yes", "keys no", "keys yes, first"];
const MOVED_IN_THE_ITEMS: &[&str] = &["item prefix only", "item sub key", "item dropped", "agent edited"];
const TAKEN_OUT: &[&str] = &["all removed", "cli removed", "agent removed", "config removed"];
const ADDED_AND_CHANGED: &[&str] = &["agent added", "cli needs", "cli no needs", "cli changed needs", "github config", "folder"];

fn rows_named(names: &[&str]) -> Vec<Value> {
    let all = fixture("recipe_sync");
    let cases = all["cases"].as_array().expect("cases are a list");
    names
        .iter()
        .map(|n| cases.iter().find(|c| c["name"] == *n).unwrap_or_else(|| panic!("no fixture row named {n}")).clone())
        .collect()
}

fn read_opt<T: serde::de::DeserializeOwned>(v: &Value) -> Option<T> {
    if v.is_null() {
        None
    } else {
        Some(wire(v))
    }
}

fn check_row(row: &Value) {
    let name = row["name"].as_str().unwrap();
    let before_file: Option<RecipeFile> = read_opt(&row["before"]);
    let before_items: Option<Items> = read_opt(&row["beforeItems"]);
    let after: RecipeFile = wire(&row["after"]);
    let after_items: Items = wire(&row["afterItems"]);
    let changes = recipe_changes(before_file.as_ref(), before_items.as_ref(), &after, &after_items);
    let got: Vec<Value> = changes.iter().map(|c| json!({ "key": c.key, "kind": json(&c.kind), "name": c.name, "how": how_word(c.how) })).collect();
    assert_eq!(Value::Array(got), row["changes"], "row {name}: changes");
    let steps: Vec<Value> = steps_for(&changes, &after).into_iter().map(|s| json(&s)).collect();
    assert_eq!(Value::Array(steps), row["steps"], "row {name}: steps");
}

// fixture: recipe_sync.json cases
#[test]
fn every_fixture_row_is_in_one_group() {
    let all = fixture("recipe_sync");
    let mut named: Vec<&str> = [FIRST_APPLY, MOVED_IN_THE_RECIPE, MOVED_IN_THE_ITEMS, TAKEN_OUT, ADDED_AND_CHANGED].concat();
    named.sort();
    let mut have: Vec<&str> = all["cases"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    have.sort();
    assert_eq!(named, have);
    assert_eq!(all["kinds"], json!(["agents", "mcp", "clis", "skills", "plugins", "folders", "configs"]));
}

// fixture: recipe_sync.json cases
#[test]
fn first_apply_and_a_computer_that_kept_no_items_match_the_ts_changes_and_steps() {
    for row in rows_named(FIRST_APPLY) {
        check_row(&row);
    }
}

// fixture: recipe_sync.json cases
#[test]
fn rows_moved_in_the_recipe_match_the_ts_changes_and_steps() {
    for row in rows_named(MOVED_IN_THE_RECIPE) {
        check_row(&row);
    }
}

// fixture: recipe_sync.json cases
#[test]
fn rows_moved_only_in_the_items_match_the_ts_changes_and_steps() {
    for row in rows_named(MOVED_IN_THE_ITEMS) {
        check_row(&row);
    }
}

// fixture: recipe_sync.json cases
#[test]
fn rows_taken_out_match_the_ts_changes_and_steps() {
    for row in rows_named(TAKEN_OUT) {
        check_row(&row);
    }
}

// fixture: recipe_sync.json cases
#[test]
fn rows_added_or_changed_match_the_ts_changes_and_steps() {
    for row in rows_named(ADDED_AND_CHANGED) {
        check_row(&row);
    }
}

fn change(kind: &str, name: &str, how: &str) -> RecipeChange {
    RecipeChange { key: format!("{kind}/{name}"), kind: wire::<RecipeKind>(&json!(kind)), name: name.into(), how: how_of(how) }
}

// fixture: recipe_sync.json stepsOnly
#[test]
fn the_steps_of_a_hand_built_list_of_changes_match_the_ts_rows() {
    let all = fixture("recipe_sync");
    let inputs: Vec<(&str, Vec<RecipeChange>)> = vec![
        ("removed mcp and clis", vec![change("mcp", "a", "removed"), change("clis", "b", "removed"), change("skills", "c", "removed")]),
        ("added cli not in after", vec![change("clis", "zz", "added")]),
        ("order", vec![change("folders", "f", "added"), change("plugins", "p", "edited"), change("configs", "x", "changed")]),
        ("none", vec![]),
    ];
    let rows = all["stepsOnly"].as_array().unwrap();
    assert_eq!(rows.len(), inputs.len());
    for (name, changes) in inputs {
        let row = rows.iter().find(|r| r[0] == name).unwrap_or_else(|| panic!("no stepsOnly row named {name}"));
        let got = Value::Array(steps_for(&changes, &before()).into_iter().map(|s| json(&s)).collect());
        assert_eq!(got, row[1], "stepsOnly row {name}");
    }
}
