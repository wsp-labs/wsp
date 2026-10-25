// SPDX-License-Identifier: AGPL-3.0-only
//! Unit base's hard tests: every value here is what golden-base.ts printed under oracle/golden.oracle.ts
//! (tests/fixtures/oracle/golden_base.json). spec/golden-base.md names the rule each pins.
mod golden_common;

use std::collections::BTreeSet;

use golden_common::{assert_calls, cases, fixture, s, strings, timeless_all, tool_result, Scripted, Stages};
use serde_json::{json, Value};
use wsp_engine::golden_seam::{
    base_installs, base_versions_cmd, carried_by_image, install_base, install_base_with, parse_versions, profile_path_line, versions_line,
    GoldenBaseTool, ImportResult, InstallBaseOptions, ToolInstall, ToolResult, BASE_VERSION_LINES, TOOLS_PATH,
};

fn versions(v: &Value) -> Vec<GoldenBaseTool> {
    serde_json::from_value(v.clone()).unwrap()
}

fn set(v: &Value) -> BTreeSet<String> {
    strings(v).into_iter().collect()
}

#[test]
fn the_version_lines_and_the_profile_line_are_the_ts_byte_for_byte() {
    let f = fixture("base");
    assert_eq!(BASE_VERSION_LINES, s(&f["BASE_VERSION_LINES"]));
    assert_eq!(profile_path_line(TOOLS_PATH), s(&f["PROFILE_PATH_LINE"]));
    for c in cases(&f, "profilePathLine") {
        assert_eq!(profile_path_line(&s(&c["path"])), s(&c["out"]), "{c}");
    }
    for c in cases(&f, "baseVersionsCmd") {
        assert_eq!(base_versions_cmd(&s(&c["path"])), s(&c["out"]), "{c}");
    }
}

#[test]
fn base_installs_is_every_floor_step_in_order_with_its_script_byte_for_byte() {
    for c in cases(&fixture("base"), "baseInstalls") {
        let got = base_installs(&set(&c["carried"]), c["path"].as_str().unwrap_or(TOOLS_PATH), c["prefix"].as_str(), &set(&c["left"]));
        let want: Vec<ToolInstall> = serde_json::from_value(c["out"].clone()).unwrap();
        assert_eq!(
            got.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            want.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            "{}: the steps in order",
            c["name"]
        );
        for (g, w) in got.iter().zip(want.iter()) {
            assert_eq!(g, w, "{}: {}", c["name"], w.id);
        }
    }
}

#[test]
fn a_carried_row_leaves_the_plan_and_what_waited_on_it_waits_on_nothing() {
    let f = fixture("base");
    let c = cases(&f, "baseInstalls").iter().find(|c| c["name"] == "the image carries git, jq and the ssh server").unwrap();
    let got = base_installs(&set(&c["carried"]), TOOLS_PATH, None, &BTreeSet::new());
    let ids: Vec<&str> = got.iter().map(|t| t.id.as_str()).collect();
    assert!(!ids.contains(&"base/git") && !ids.contains(&"base/jq") && !ids.contains(&"base/ssh-server"), "{ids:?}");
    assert_eq!(serde_json::to_value(&got).unwrap(), c["out"]);
}

#[test]
fn a_row_left_off_and_carried_is_off_the_plan_either_way() {
    let f = fixture("base");
    let c = cases(&f, "baseInstalls").iter().find(|c| c["name"] == "a row left and carried").unwrap();
    let got = base_installs(&set(&c["carried"]), TOOLS_PATH, None, &set(&c["left"]));
    assert!(got.iter().all(|t| t.id != "base/build-essential"));
    assert_eq!(serde_json::to_value(&got).unwrap(), c["out"]);
}

#[test]
fn parse_versions_keeps_the_number_alone_and_drops_a_line_with_none() {
    for c in cases(&fixture("base"), "parseVersions") {
        assert_eq!(serde_json::to_value(parse_versions(&s(&c["stdout"]))).unwrap(), c["out"], "{}", c["stdout"]);
    }
}

#[test]
fn carried_by_image_names_the_rows_the_image_satisfies_in_catalog_order_then_the_ssh_server() {
    for c in cases(&fixture("base"), "carriedByImage") {
        let got: Vec<String> = carried_by_image(&versions(&c["versions"])).into_iter().collect();
        assert_eq!(got, strings(&c["out"]), "{}", c["versions"]);
    }
}

#[test]
fn versions_line_names_each_version_with_its_cost_then_what_was_there_then_what_missed() {
    for c in cases(&fixture("base"), "versionsLine") {
        let results: Vec<ToolResult> = c["results"].as_array().unwrap().iter().map(tool_result).collect();
        assert_eq!(versions_line(&versions(&c["versions"]), &results), s(&c["out"]), "{c}");
    }
}

async fn replay(c: &Value) -> (Result<Value, String>, Value, Value) {
    let m = Scripted::new(&c["rules"], false);
    let stages = Stages::default();
    let opts = InstallBaseOptions {
        keep_caches: c.get("caches") == Some(&json!("keep")),
        path: c["path"].as_str().map(str::to_string),
        prefix: c["prefix"].as_str().map(str::to_string),
        left: set(&c["left"]),
    };
    let out = install_base_with(&m, stages.tool_stage(), opts)
        .await
        .map(|o| json!({ "tools": timeless_all(&o.tools), "versions": serde_json::to_value(&o.versions).unwrap(), "line": o.line }))
        .map_err(|e| e.to_string());
    (out, stages.value(), m.calls())
}

#[tokio::test]
async fn install_base_runs_the_read_the_loop_and_the_read_after_as_the_ts_did_in_every_scenario() {
    let f = fixture("base");
    let all = cases(&f, "installBase");
    assert_eq!(all.len(), 6);
    for c in all {
        let name = s(&c["name"]);
        let (out, stages, calls) = replay(c).await;
        let out = out.unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(out["tools"], c["out"]["tools"], "{name}: the rows, floor rows in catalog order");
        assert_eq!(out["versions"], c["out"]["versions"], "{name}: the versions read after");
        assert_eq!(out["line"], c["out"]["line"], "{name}: the closing line");
        assert_eq!(stages, c["stages"], "{name}: every stage line under deploying-daemon");
        assert_calls(&name, &calls, &c["calls"]);
    }
}

#[tokio::test]
async fn install_base_as_provision_runs_it_keeps_the_caches() {
    let f = fixture("base");
    let c = cases(&f, "installBase")
        .iter()
        .find(|c| c["name"] == "a computer somebody owns: the probe PATH, the prefix, the C toolchain left off")
        .unwrap();
    let m = Scripted::new(&c["rules"], false);
    let out = install_base(&m, Stages::default().tool_stage(), &s(&c["path"]), c["prefix"].as_str(), &set(&c["left"])).await.unwrap();
    assert_eq!(timeless_all(&out.tools), c["out"]["tools"]);
    assert_eq!(out.line, s(&c["out"]["line"]));
    assert_calls("install_base", &m.calls(), &c["calls"]);
    assert!(
        m.calls().as_array().unwrap().iter().all(|call| !call["text"].as_str().unwrap().contains("go clean -cache")),
        "a computer somebody owns keeps its caches"
    );
}

#[test]
fn a_saved_import_result_reads_its_fields_and_ignores_the_rest() {
    // golden.ts:547 as the TS writes it, with keys this read does not keep.
    let saved = json!({
        "recipeHash": "h1",
        "files": { "bytes": 12, "skipped": [{ "id": "dotfiles/zshrc", "path": "~/.zshrc", "note": "gone" }], "cut": [] },
        "homebrew": { "tag": "4.6.0", "commit": "abc" },
        "tools": [{ "id": "tools/brew/gh", "label": "gh", "outcome": "installed", "ms": 12, "bytes": 3 }],
        "agents": [{ "id": "agents/claude", "name": "Claude Code", "outcome": "failed", "note": "npm broke", "ms": 9 }],
        "mcp": [],
        "base": [{ "id": "base/git", "label": "git", "outcome": "installed", "note": "already on the machine" }],
        "context": []
    });
    let read: ImportResult = serde_json::from_value(saved).unwrap();
    assert_eq!(read.recipe_hash, "h1");
    assert_eq!(
        serde_json::to_value(&read).unwrap(),
        json!({
            "recipeHash": "h1",
            "files": { "bytes": 12, "skipped": [{ "id": "dotfiles/zshrc", "path": "~/.zshrc", "note": "gone" }] },
            "tools": [{ "id": "tools/brew/gh", "label": "gh", "outcome": "installed" }],
            "agents": [{ "id": "agents/claude", "name": "Claude Code", "outcome": "failed", "note": "npm broke" }],
            "base": [{ "id": "base/git", "label": "git", "outcome": "installed", "note": "already on the machine" }]
        })
    );
}
