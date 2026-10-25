// SPDX-License-Identifier: AGPL-3.0-only
//! Unit import_roads's hard tests: every value here is what golden-import.ts printed under oracle/golden.oracle.ts
//! (tests/fixtures/oracle/golden_import_roads.json). spec/golden-import_roads.md names the rule each pins.
mod golden_common;

use golden_common::{cases, fixture, s, strings};
use serde_json::{json, Map, Value};
use wsp_catalog::roads::{InstallRoad, RoadName};
use wsp_engine::golden_seam::{
    apt_index_step, asks_version, base_row_for, brew_housekeeping, brew_tap_check, brewfile_for, catalog_tool_of, custom_installs_for,
    custom_prelude, manager_formula, path_line, pin_read_of, road_reads, road_step_reads, row_road, shown_of, via_road, BaseRow, BrewTable,
    PlannedRow, RecipeEntry, RoadReads, ToolInstall, ViaRoad, EXEC_ENV, TOOLS_PATH,
};
use wsp_wire::views::golden_image::RecipeCustomRow;

fn table(v: &Value) -> BrewTable {
    v.as_array().unwrap().iter().map(|p| (s(&p[0]), serde_json::from_value(p[1].clone()).unwrap())).collect()
}

fn reads_json(r: &RoadReads) -> Value {
    let mut m = Map::new();
    if let Some(p) = &r.present {
        m.insert("present".into(), json!(p));
    }
    if let Some(b) = &r.bins {
        m.insert("bins".into(), json!(b));
    }
    if let Some(c) = &r.check {
        m.insert("check".into(), json!(c));
    }
    Value::Object(m)
}

fn via_json(v: &ViaRoad) -> Value {
    match v {
        ViaRoad::Step { cmd, shown } => json!({ "cmd": cmd, "shown": shown }),
        ViaRoad::Note(note) => json!({ "note": note }),
    }
}

fn base_json(b: &BaseRow) -> Value {
    let mut m = Map::new();
    m.insert("id".into(), json!(b.id));
    m.insert("name".into(), json!(b.name));
    m.insert("entry".into(), json!(b.entry.id));
    if let Some(v) = &b.version {
        m.insert("version".into(), json!(v));
    }
    Value::Object(m)
}

/// The TS row's `BaseRow` with its entry read as the entry's id, as `base_json` writes the Rust one.
fn base_want(v: &Value) -> Value {
    let mut v = v.clone();
    v["entry"] = v["entry"]["id"].clone();
    v
}

fn planned_json(p: &PlannedRow) -> Value {
    let mut m = Map::new();
    m.insert("road".into(), serde_json::to_value(&p.road).unwrap());
    if let Some(b) = &p.bin {
        m.insert("bin".into(), json!(b));
    }
    if let Some(a) = &p.after {
        m.insert("after".into(), json!(a));
    }
    if let Some(n) = &p.note {
        m.insert("note".into(), json!(n));
    }
    if let Some(pin) = &p.pinned {
        m.insert("pinned".into(), json!(pin));
    }
    if let Some(b) = &p.below {
        m.insert("below".into(), json!({ "node": b.node, "road": serde_json::to_value(&b.road).unwrap() }));
    }
    Value::Object(m)
}

#[test]
fn the_path_line_and_the_lines_beside_it_are_the_ts_byte_for_byte() {
    let f = fixture("import_roads");
    assert_eq!(TOOLS_PATH, s(&f["TOOLS_PATH"]));
    assert_eq!(EXEC_ENV, s(&f["EXEC_ENV"]));
    assert_eq!(path_line(TOOLS_PATH, None), s(&f["PATH_LINE"]));
    for c in cases(&f, "pathLine") {
        assert_eq!(path_line(&s(&c["path"]), c["prefix"].as_str()), s(&c["out"]), "{c}");
    }
    for c in cases(&f, "customPrelude") {
        assert_eq!(custom_prelude(&s(&c["path"]), c["prefix"].as_str()), s(&c["out"]), "{c}");
    }
    for c in cases(&f, "brewHousekeeping") {
        assert_eq!(brew_housekeeping(&s(&c["path"])), strings(&c["out"]), "{c}");
    }
    for c in cases(&f, "brewTapCheck") {
        assert_eq!(brew_tap_check(&s(&c["tap"])), s(&c["out"]), "{c}");
    }
    for c in cases(&f, "aptIndexStep") {
        assert_eq!(serde_json::to_value(apt_index_step(&s(&c["id"]), &s(&c["cmd"]))).unwrap(), c["out"]);
    }
    for c in cases(&f, "shownOf") {
        let lines = strings(&c["lines"]);
        assert_eq!(shown_of(&lines.iter().map(String::as_str).collect::<Vec<_>>()), s(&c["out"]));
    }
}

#[test]
fn every_road_reads_and_scripts_as_the_ts_with_and_without_a_prefix() {
    let f = fixture("import_roads");
    let all = cases(&f, "roads");
    assert!(all.len() >= 30, "{}", all.len());
    for c in all {
        let road: InstallRoad = serde_json::from_value(c["road"].clone()).unwrap_or_else(|e| panic!("{}: {e}", c["road"]));
        let bin = s(&c["bin"]);
        let prefix = c["prefix"].as_str();
        let what = format!("{} {bin} prefix {prefix:?}", c["road"]);
        assert_eq!(via_json(&via_road(&road, &bin, TOOLS_PATH, prefix)), c["viaRoad"], "viaRoad {what}");
        assert_eq!(reads_json(&road_reads(&road, &bin, prefix)), c["roadReads"], "roadReads {what}");
        assert_eq!(reads_json(&road_step_reads(&road, &bin, prefix)), c["roadStepReads"], "roadStepReads {what}");
        assert_eq!(serde_json::to_value(pin_read_of(&road, &bin, prefix)).unwrap(), c["pinReadOf"], "pinReadOf {what}");
        assert_eq!(asks_version(&road).map(Value::from).unwrap_or(Value::Null), c["asksVersion"], "asksVersion {what}");
    }
}

#[test]
fn every_row_has_the_ts_base_row_road_and_catalog_tool() {
    let f = fixture("import_roads");
    let brew = table(&f["table"]);
    for c in cases(&f, "rows") {
        let e: RecipeEntry = serde_json::from_value(c["row"].clone()).unwrap();
        let what = format!("{} {}", e.id, c["row"]);
        let base = base_row_for(&e, &brew).map(|b| base_json(&b)).unwrap_or(Value::Null);
        let want = if c["baseRowFor"].is_null() { Value::Null } else { base_want(&c["baseRowFor"]) };
        assert_eq!(base, want, "baseRowFor {what}");
        assert_eq!(row_road(&e, &brew).map(|p| planned_json(&p)).unwrap_or(Value::Null), c["rowRoad"], "rowRoad {what}");
        assert_eq!(catalog_tool_of(&e).map(|t| json!(t.id)).unwrap_or(Value::Null), c["catalogToolOf"], "catalogToolOf {what}");
    }
}

#[test]
fn the_brewfile_lists_taps_then_formulae_and_notes_what_does_not_go() {
    let f = fixture("import_roads");
    for (i, c) in cases(&f, "brewfileFor").iter().enumerate() {
        let rows: Vec<RecipeEntry> = serde_json::from_value(c["rows"].clone()).unwrap();
        let brew = if i == 0 { table(&f["table"]) } else { BrewTable::new() };
        let b = brewfile_for(&rows, &brew);
        let o = &c["out"];
        assert_eq!(b.text, s(&o["text"]), "case {i}");
        assert_eq!(b.taps, strings(&o["taps"]), "case {i}");
        assert_eq!(b.formulae, strings(&o["formulae"]), "case {i}");
        assert_eq!(serde_json::to_value(&b.skipped).unwrap(), o["skipped"], "case {i}");
        let roads: Vec<Value> = b
            .roads
            .iter()
            .map(|r| {
                let mut m = Map::new();
                m.insert("id".into(), json!(r.id));
                m.insert("name".into(), json!(r.name));
                m.insert("source".into(), serde_json::to_value(&r.source).unwrap());
                if let Some(p) = &r.pin {
                    m.insert("pin".into(), serde_json::to_value(p).unwrap());
                }
                Value::Object(m)
            })
            .collect();
        assert_eq!(Value::Array(roads), o["roads"], "case {i}");
        assert_eq!(
            Value::Array(b.base.iter().map(base_json).collect()),
            Value::Array(o["base"].as_array().unwrap().iter().map(base_want).collect()),
            "case {i}"
        );
    }
}

#[test]
fn every_manager_comes_by_the_ts_formula_or_none() {
    for c in cases(&fixture("import_roads"), "managerFormula") {
        let m: RoadName = serde_json::from_value(c["manager"].clone()).unwrap();
        assert_eq!(manager_formula(m).map(Value::from).unwrap_or(Value::Null), c["out"], "{c}");
    }
}

#[test]
fn rows_outside_the_catalog_install_by_their_own_lines_with_their_manager_reads_and_wait_on_what_is_named() {
    let f = fixture("import_roads");
    let custom: Vec<RecipeCustomRow> = serde_json::from_value(f["custom"].clone()).unwrap();
    for c in cases(&f, "customInstallsFor") {
        let after = c.get("after").cloned().unwrap_or(json!({}));
        let waits = move |row: &RecipeCustomRow| after.get(&row.id).and_then(Value::as_str).map(str::to_string);
        let got = custom_installs_for(&custom, &waits, &s(&c["path"]), c["prefix"].as_str());
        let want: Vec<ToolInstall> = serde_json::from_value(c["out"].clone()).unwrap();
        assert_eq!(got, want, "{}", c["path"]);
    }
}
