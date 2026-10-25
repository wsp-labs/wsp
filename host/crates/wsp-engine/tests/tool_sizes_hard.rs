// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of tool sizes and the shared brew step against the TS's own output (tests/fixtures/oracle/
//! tool-sizes.json, brew-shared.json). spec/tool_sizes.md.
use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use wsp_engine::brew_shared::{brew_shared_check, brew_shared_deps};
use wsp_engine::golden_seam::{BrewFormula, RecipeEntry, ToolSource};
use wsp_engine::tool_sizes::{assumed_size, brew_table, parse_brew_info, parse_du, source_of};

fn oracle(name: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle").join(name);
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

fn formula_of(v: &Value) -> BrewFormula {
    BrewFormula {
        name: v["name"].as_str().unwrap().into(),
        full_name: v["fullName"].as_str().unwrap().into(),
        deps: v["deps"].as_array().unwrap().iter().map(|d| d.as_str().unwrap().to_string()).collect(),
        bytes: v["bytes"].as_f64(),
        version: v["version"].as_str().map(String::from),
        macos_only: v["macosOnly"].as_bool().unwrap(),
        source: v.get("source").map(|s| ToolSource { repo: s["repo"].as_str().unwrap().into(), tag: s["tag"].as_str().unwrap().into() }),
    }
}

/// The input the oracle parsed, rebuilt here: formula() of the TS test with the edge rows the oracle added.
fn info() -> Value {
    let formula = |over: Value| {
        let mut f = json!({ "tap": "homebrew/core", "dependencies": [], "build_dependencies": [], "requirements": [], "urls": { "stable": { "url": "https://example.org/src.tar.xz", "tag": null } }, "versions": { "stable": "1.0", "bottle": true }, "installed": [{ "version": "1.0", "runtime_dependencies": [] }] });
        for (k, v) in over.as_object().unwrap() {
            if v.is_null() {
                f.as_object_mut().unwrap().remove(k);
            } else {
                f[k] = v.clone();
            }
        }
        f
    };
    json!({ "formulae": [
        formula(json!({ "name": "ffmpeg", "full_name": "ffmpeg", "dependencies": ["x264", "openssl@3", "x264"], "installed": [{ "version": "8.1.2", "runtime_dependencies": [{ "full_name": "x264" }, { "full_name": "openssl@3" }, { "full_name": "ca-certificates" }, { "name": "nofull" }, "str"] }] })),
        formula(json!({ "name": "nofull", "installed": [] })),
        formula(json!({ "full_name": "noname" })),
        formula(json!({ "name": "gitonly", "full_name": "someone/tap/gitonly", "urls": { "stable": { "url": "https://github.com/someone/gitonly.git", "tag": "v3.1.0" } } })),
        formula(json!({ "name": "rel", "full_name": "rel", "urls": { "stable": { "url": "https://github.com/o/r/releases/download/v1/r.tgz" } } })),
        formula(json!({ "name": "macthing", "full_name": "someone/tap/macthing", "requirements": [{ "name": "macos" }] })),
        formula(json!({ "name": "sketchybar", "full_name": "felixkratz/formulae/sketchybar" })),
        "not a record",
        formula(json!({ "name": "nourls", "full_name": "nourls", "urls": null })),
    ] })
}

const DU: &str = "102400\t/opt/homebrew/Cellar/ffmpeg\n  20\t/x/y/  \n5 /opt/rel//\nbad line\n7\t/opt/homebrew/Cellar/ffmpeg\n\n";

// spec/tool_sizes.md Z4
#[test]
fn parse_brew_info_matches_the_ts_oracle() {
    let o = oracle("tool-sizes.json");
    let want: Vec<BrewFormula> = o["parseBrewInfo"].as_array().unwrap().iter().map(formula_of).collect();
    assert_eq!(parse_brew_info(&info()), want);
    for not_shaped in [Value::Null, json!([]), json!({ "formulae": {} })] {
        assert_eq!(parse_brew_info(&not_shaped), vec![], "{not_shaped}");
    }
}

// spec/tool_sizes.md Z5, Z6
#[test]
fn parse_du_matches_the_ts_oracle() {
    let o = oracle("tool-sizes.json");
    let pairs =
        |v: &Value| v.as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_f64().unwrap())).collect::<Vec<_>>();
    assert_eq!(parse_du(DU, None).into_iter().collect::<Vec<_>>(), pairs(&o["parseDu"]));
    let keyed = |p: &str| format!("k:{p}");
    assert_eq!(parse_du(DU, Some(&keyed)).into_iter().collect::<Vec<_>>(), pairs(&o["parseDuKeyed"]));
    let table = brew_table(&info(), &parse_du(DU, None));
    let want: Vec<(String, BrewFormula)> =
        o["brewTable"].as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), formula_of(&p[1]))).collect();
    assert_eq!(table.into_iter().collect::<Vec<_>>(), want);
}

// spec/tool_sizes.md Z3
#[test]
fn source_of_matches_the_ts_oracle() {
    for c in oracle("tool-sizes.json")["sourceOf"].as_array().unwrap() {
        let tag = match &c["tag"] {
            Value::String(t) if t == "undefined" => None,
            Value::String(t) => Some(t.as_str()),
            _ => None,
        };
        let want = if c["source"].is_null() {
            None
        } else {
            Some(ToolSource { repo: c["source"]["repo"].as_str().unwrap().into(), tag: c["source"]["tag"].as_str().unwrap().into() })
        };
        assert_eq!(source_of(c["url"].as_str(), tag), want, "{c}");
    }
}

// spec/tool_sizes.md Z2
#[test]
fn assumed_sizes_match_the_ts_oracle() {
    for c in oracle("tool-sizes.json")["assumed"].as_array().unwrap() {
        let e = RecipeEntry { rung: c["rung"].as_str().unwrap().into(), id: c["id"].as_str().unwrap().into() };
        let got = assumed_size(&e);
        assert_eq!(got.bytes, c["size"]["bytes"].as_u64().unwrap(), "{}", e.id);
        assert_eq!(got.kind.words(), c["size"]["kind"].as_str().unwrap(), "{}", e.id);
    }
}

// spec/tool_sizes.md Z9
#[test]
fn brew_shared_scripts_match_the_ts_byte_for_byte() {
    for c in oracle("brew-shared.json").as_array().unwrap() {
        let formulae: Vec<String> = c["formulae"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect();
        assert_eq!(brew_shared_deps(&formulae), c["deps"].as_str().unwrap(), "{formulae:?}");
        assert_eq!(brew_shared_check(&formulae), c["check"].as_str().unwrap(), "{formulae:?}");
    }
}
