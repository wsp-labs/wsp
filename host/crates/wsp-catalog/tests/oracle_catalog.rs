// SPDX-License-Identifier: AGPL-3.0-only
// The catalog table and its readers against tests/fixtures/catalog.json, which the TS catalog wrote
// (oracle/dump.oracle.ts). Every row, every derived list and every reader's answer on the oracle's inputs.
mod oracle_common;

use indexmap::IndexMap;
use oracle_common::{fixture, json, opt, s, same};
use serde_json::{json, Value};
use wsp_catalog::bwrap_apparmor::BWRAP_APPARMOR;
use wsp_catalog::catalog::*;
use wsp_catalog::pending::{PageReach, Platform};
use wsp_catalog::road_modules::install_homes;
use wsp_catalog::roads::{InstallRoad, RoadName};

fn cat() -> Value {
    fixture("catalog.json")
}

fn ok_or_error<T: serde::Serialize>(r: Result<T, wsp_catalog::pending::HostError>) -> Value {
    match r {
        Ok(v) => json!({ "ok": json(v) }),
        Err(e) => json!({ "error": e.message }),
    }
}

#[test]
fn every_row_is_the_ts_row_in_the_ts_order() {
    let want = cat()["entries"].as_array().unwrap().clone();
    let got: Vec<&str> = catalog().iter().map(|e| e.id()).collect();
    let ids: Vec<&str> = want.iter().map(|e| s(&e["id"])).collect();
    assert_eq!(got, ids);
    for (e, w) in catalog().iter().zip(&want) {
        same(e.id(), json(e), w);
    }
}

#[test]
fn each_rows_install_line_shown_words_smoke_and_bytes_are_the_ts_ones() {
    let per = cat()["perEntry"].clone();
    for e in catalog() {
        let w = &per[e.id()];
        same(&format!("{} installLine", e.id()), ok_or_error(install_line(e)), &w["installLine"]);
        same(&format!("{} installShown", e.id()), ok_or_error(install_shown(e)), &w["installShown"]);
        same(&format!("{} smoke", e.id()), json(smoke_of(e)), &w["smoke"]);
        same(&format!("{} sizeBytes", e.id()), opt(size_bytes(e.size())), &w["sizeBytes"]);
    }
}

#[test]
fn each_tools_install_after_and_base_note_are_the_ts_ones() {
    let per = cat()["perEntry"].clone();
    for t in catalog_tools() {
        let w = &per[t.id.as_str()];
        same(&format!("{} installAfter", t.id), opt(install_after(t)), &w["installAfter"]);
        let note = &w["baseNote"];
        same(&format!("{} baseNote none", t.id), json(base_note(t, None, Platform::Darwin)), &note["none"]);
        same(&format!("{} baseNote v99", t.id), json(base_note(t, Some("v99.1.2"), Platform::Darwin)), &note["v99"]);
        same(&format!("{} baseNote linux99", t.id), json(base_note(t, Some("99.1"), Platform::Linux)), &note["linux99"]);
        let same_major = t.major.as_ref().map(|m| base_note(t, Some(&format!("v{}.4", m.version)), Platform::Darwin));
        same(&format!("{} baseNote same", t.id), opt(same_major), &note["same"]);
    }
}

#[test]
fn each_agents_env_setup_rows_ways_shared_login_name_mark_and_switch_are_the_ts_ones() {
    let per = cat()["perEntry"].clone();
    for a in catalog_agents() {
        let w = &per[a.id.as_str()];
        same(&format!("{} guestEnv", a.id), json(guest_env(a)), &w["guestEnv"]);
        same(&format!("{} setupNeedRows", a.id), json(setup_need_rows(&a.id)), &w["setupNeedRows"]);
        same(&format!("{} aptNeedRows", a.id), json(apt_need_rows(&a.id)), &w["aptNeedRows"]);
        same(&format!("{} signInWays", a.id), json(sign_in_ways_of(&a.id)), &w["signInWays"]);
        same(&format!("{} sharedOn", a.id), opt(shared_on(&a.id)), &w["sharedOn"]);
        same(&format!("{} agentName", a.id), json(agent_name(&a.id)), &w["agentName"]);
        same(&format!("{} mark", a.id), opt(agent_mark(&a.id)), &w["mark"]);
        same(&format!("{} mcpSwitch", a.id), json(mcp_switch(&a.id)), &w["mcpSwitch"]);
    }
}

#[test]
fn the_derived_lists_keep_catalog_order() {
    let c = cat();
    same("catalogAgents", json(catalog_agents().iter().map(|a| a.id.clone()).collect::<Vec<_>>()), &c["catalogAgents"]);
    same("catalogTools", json(catalog_tools().iter().map(|t| t.id.clone()).collect::<Vec<_>>()), &c["catalogTools"]);
    same("defaultAgent", json(&default_agent().id), &c["defaultAgent"]);
    same("mcpAgents", json(mcp_agents().iter().map(|a| a.id.clone()).collect::<Vec<_>>()), &c["mcpAgents"]);
    same("mcpAgentIds", json(mcp_agent_ids()), &c["mcpAgentIds"]);
    same("baseFloor", json(base_floor().iter().map(|t| t.id.clone()).collect::<Vec<_>>()), &c["baseFloor"]);
    same("floorBytes compiler", json(floor_bytes(true)), &c["floorBytes"]["compiler"]);
    same("floorBytes none", json(floor_bytes(false)), &c["floorBytes"]["none"]);
    same("unmarked", json(UNMARKED_AGENTS), &c["unmarked"]);
    same("historyFormats", json(HISTORY_FORMATS), &c["historyFormats"]);
}

#[test]
fn the_login_rows_are_the_ts_rows_keys_rows_beside_their_logins() {
    let got: Vec<Value> = login_rows()
        .iter()
        .map(|r| {
            let mut v = json!({ "id": r.id, "entry": r.entry.id(), "signIn": json(&r.sign_in) });
            if let Some(k) = &r.keys {
                v["keys"] = json(k);
            }
            v
        })
        .collect();
    same("loginRows", json(got), &cat()["loginRows"]);
    for r in login_rows() {
        assert_eq!(login_row(&r.id).map(|x| x.id.clone()), Some(r.id.clone()));
    }
    assert!(login_row("nothing").is_none());
}

#[test]
fn a_package_name_finds_the_ts_tool_by_every_road_it_is_known_by() {
    let c = cat();
    for (pkg, want) in c["toolFor"].as_object().unwrap() {
        same(&format!("toolFor {pkg:?}"), opt(catalog_tool_for(pkg).map(|t| t.id.clone())), want);
    }
    for (pkg, want) in c["toolForDependency"].as_object().unwrap() {
        same(&format!("toolForDependency npm {pkg}"), opt(catalog_tool_for_dependency(RoadName::Npm, pkg).map(|t| t.id.clone())), &want["npm"]);
        same(&format!("toolForDependency pnpm {pkg}"), opt(catalog_tool_for_dependency(RoadName::Pnpm, pkg).map(|t| t.id.clone())), &want["pnpm"]);
    }
    for (key, want) in c["toolByRoad"].as_object().unwrap() {
        let (road, pkg) = key.split_once(' ').unwrap();
        let road: RoadName = serde_json::from_value(json!(road)).unwrap();
        same(&format!("toolByRoad {key}"), opt(catalog_tool_by_road(road, pkg).map(|t| t.id.clone())), want);
    }
}

#[test]
fn a_recipe_rows_catalog_id_and_a_login_rows_sign_in_are_the_ts_ones() {
    let c = cat();
    for (id, want) in c["idOfRow"].as_object().unwrap() {
        same(&format!("idOfRow {id}"), opt(catalog_id_of_row(id)), want);
    }
    for (row, want) in c["loginSignIn"].as_object().unwrap() {
        same(&format!("loginSignIn {row}"), opt(login_sign_in(row)), want);
    }
    let agents: Vec<String> = ["claude", "codex", "gemini", "codex", "nothing"].iter().map(|x| x.to_string()).collect();
    same("sharedAgentsOn", json(shared_agents_on(&agents)), &c["sharedAgentsOn"]);
    for (id, want) in c["agentName"].as_object().unwrap() {
        same(&format!("agentName {id}"), json(agent_name(id)), want);
    }
}

#[test]
fn a_harness_line_cds_into_the_folder_and_points_at_the_store_as_the_ts_does() {
    let w = cat()["harnessLine"].clone();
    let stores: IndexMap<String, String> = IndexMap::from([("claude".into(), "/w/logins/claude".into()), ("codex".into(), "/w/c".into())]);
    let codex: IndexMap<String, String> = IndexMap::from([("codex".into(), "/w/logins/codex".into())]);
    let gemini: IndexMap<String, String> = IndexMap::from([("gemini".into(), "/w/g".into())]);
    same("plain", json(harness_line("claude", "claude mcp list", None, None)), &w["plain"]);
    same("folder", json(harness_line("claude", "claude mcp list", None, Some("/home/me/it's"))), &w["folder"]);
    same("store", json(harness_line("claude", "claude mcp list", Some(&stores), None)), &w["store"]);
    same("both", json(harness_line("codex", "codex mcp list", Some(&codex), Some("/p"))), &w["both"]);
    same("noVariable", json(harness_line("gemini", "gemini", Some(&gemini), None)), &w["noVariable"]);
}

#[test]
fn the_node_floor_test_and_the_pinned_note_are_the_ts_text() {
    let c = cat();
    let got: Vec<String> = ["22.13", "20", "18.0", "x.y"].iter().map(|f| node_at_least(f)).collect();
    same("nodeAtLeast", json(got), &c["nodeAtLeast"]);
    same("pinnedNote", json(pinned_note("10.1.0", "11.9.0")), &c["pinnedNote"]);
}

#[test]
fn a_road_at_the_catalogs_pin_is_the_ts_answer_for_every_ask() {
    for case in cat()["atCatalogPin"].as_array().unwrap() {
        let road: InstallRoad = serde_json::from_value(case["road"].clone()).unwrap();
        let asked = case["asked"].as_str();
        same(&format!("{} asked {:?}", case["road"], asked), json(at_catalog_pin(&road, asked)), &case["out"]);
    }
}

#[test]
fn the_below_line_installs_the_pin_on_a_new_node_and_the_ask_on_an_old_one() {
    let at = at_catalog_pin(&serde_json::from_value(json!({ "road": "npm", "package": "pnpm" })).unwrap(), Some("10.1.0"));
    let below = at.below.clone().expect("pnpm's pin names a Node floor");
    same("belowLine", json(below_line(&at.road, &below, "pnpm", &install_homes(None)).unwrap()), &cat()["belowLine"]);
}

#[test]
fn a_servers_sign_in_road_is_the_ts_one_for_each_agent_and_reach() {
    for (key, want) in cat()["serverSignInRoad"].as_object().unwrap() {
        let (agent, reach) = key.split_once(' ').unwrap();
        let reach: PageReach = serde_json::from_value(json!(reach)).unwrap();
        let got = server_sign_in_road(agent, "linear", reach, false).map(|r| r.unwrap());
        same(key, opt(got), want);
    }
}

#[test]
fn a_brokered_machine_refuses_every_servers_sign_in_as_a_usage_refusal() {
    let err = server_sign_in_road("claude", "linear", PageReach::Here, true).unwrap().unwrap_err();
    assert_eq!(err.message, "A cloud machine holds no sign-in, so linear cannot be signed in on it. Cloud threads run without its tools; sign it in on your own computer for the threads there.");
    assert_eq!(err.fix.as_deref(), Some("Cloud threads run without its tools; sign it in on your own computer for the threads there."));
    assert_eq!(err.kind, Some(wsp_catalog::pending::Kind::Usage));
}

#[test]
fn the_size_methods_say_how_each_was_read_and_the_bwrap_row_is_the_ts_row() {
    let c = cat();
    for (method, words) in c["sizeMethods"].as_object().unwrap() {
        let m: SizeMethod = serde_json::from_value(json!(method)).unwrap();
        same(method, json(size_method_words(m)), words);
    }
    same("bwrapApparmor", json(&*BWRAP_APPARMOR), &c["bwrapApparmor"]);
}

#[test]
fn an_unknown_id_finds_no_entry_name_or_mark() {
    assert!(catalog_entry("nothing").is_none());
    assert_eq!(agent_name("nothing"), "nothing");
    assert!(agent_mark("gh").is_none());
    assert!(agent_mark("crush").is_none());
    assert_eq!(catalog_entry("gh").map(|e| e.name().to_string()), Some("GitHub CLI".to_string()));
}
