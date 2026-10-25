// SPDX-License-Identifier: AGPL-3.0-only
//! Unit import_agents's hard tests: every value here is what golden-import.ts printed under oracle/golden.oracle.ts
//! (tests/fixtures/oracle/golden_import_agents.json). spec/golden-import_agents.md names the rule each pins.
mod golden_common;

use golden_common::{cases, fixture, s};
use serde_json::{json, Map, Value};
use wsp_catalog::catalog::{catalog_agents, AgentEntry};
use wsp_engine::golden_seam::{
    agent_installers, agent_installs_for, agent_installs_for_rows, agent_steps, agent_steps_with, node_floor_check, node_major_for,
    AgentsPlan, RecipeEntry, ToolInstall, TOOLS_PATH,
};

fn ms(iso: &str) -> i64 {
    iso.parse::<jiff::Timestamp>().unwrap().as_millisecond()
}

fn agents() -> Vec<AgentEntry> {
    catalog_agents().into_iter().cloned().collect()
}

#[test]
fn every_installer_is_the_ts_line_check_road_floor_and_pin_read_with_aider_last() {
    let f = fixture("import_agents");
    let got = agent_installers(&agents());
    let want = cases(&f, "installers");
    assert_eq!(got.keys().cloned().collect::<Vec<_>>(), want.iter().map(|w| s(&w["id"])).collect::<Vec<_>>());
    for w in want {
        let a = &got[w["id"].as_str().unwrap()];
        let mut m = Map::new();
        m.insert("id".into(), w["id"].clone());
        m.insert("name".into(), json!(a.name));
        m.insert("install".into(), json!(a.install));
        m.insert("smoke".into(), json!(a.smoke));
        m.insert("road".into(), json!(a.road));
        if let Some(n) = a.node {
            m.insert("node".into(), json!(n));
        }
        if let Some(p) = &a.pin {
            m.insert("pin".into(), serde_json::to_value(p).unwrap());
        }
        assert_eq!(Value::Object(m), *w, "{}", w["id"]);
    }
}

#[test]
fn node_major_for_picks_the_lowest_supported_pinned_major_else_the_current_lts() {
    for c in cases(&fixture("import_agents"), "nodeMajorFor") {
        let got = node_major_for(c["floor"].as_u64().unwrap() as u32, ms(c["now"].as_str().unwrap()));
        assert_eq!(got.map(Value::from).unwrap_or(Value::Null), c["out"], "{c}");
    }
}

#[test]
fn node_floor_check_reads_the_major_the_way_the_install_script_does() {
    for c in cases(&fixture("import_agents"), "nodeFloorCheck") {
        assert_eq!(node_floor_check(c["floor"].as_u64().unwrap() as u32), s(&c["out"]));
    }
}

#[test]
fn agent_installs_for_plans_the_ts_installs_skips_and_node_step_for_every_recipe_and_date() {
    let f = fixture("import_agents");
    for c in cases(&f, "agentInstallsFor") {
        let rows: Vec<RecipeEntry> = serde_json::from_value(c["rows"].clone()).unwrap();
        let plan = agent_installs_for_rows(&rows, &agents(), ms(c["now"].as_str().unwrap()));
        assert_eq!(serde_json::to_value(&plan).unwrap(), c["out"], "{}", c["name"]);
    }
}

#[test]
fn agent_steps_are_the_ts_steps_for_every_plan_on_both_paths() {
    let f = fixture("import_agents");
    for c in cases(&f, "agentSteps") {
        let planned = cases(&f, "agentInstallsFor").iter().find(|p| p["name"] == c["plan"]).unwrap();
        let plan: AgentsPlan = serde_json::from_value(planned["out"].clone()).unwrap();
        let got = agent_steps_with(&plan, &agents(), &s(&c["path"]), c["prefix"].as_str());
        let want: Vec<ToolInstall> = serde_json::from_value(c["out"].clone()).unwrap();
        assert_eq!(
            got.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            want.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            "{} on {}",
            c["plan"],
            c["path"]
        );
        for (g, w) in got.iter().zip(want.iter()) {
            assert_eq!(g, w, "{} on {}: {}", c["plan"], c["path"], w.id);
        }
        if c["path"] == TOOLS_PATH && c["prefix"].is_null() {
            assert_eq!(agent_steps(&plan, TOOLS_PATH, None), want, "{}: the catalog's agents by default", c["plan"]);
        }
    }
}

#[test]
fn the_wrapper_provision_calls_plans_provisions_own_rows_as_the_ts() {
    let f = fixture("import_agents");
    let c =
        cases(&f, "agentInstallsFor").iter().find(|p| p["name"] == "provision's own rows: claude and codex, the id as the label").unwrap();
    // Node 20 left support before either date and 22 is the current LTS after its own, so the clock does not move this.
    assert_eq!(serde_json::to_value(agent_installs_for(&["agents/claude", "agents/codex"])).unwrap(), c["out"]);
}
