// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/brew-bottles.test.ts.
use std::collections::HashSet;

use serde_json::{json, Value};
use wsp_collect::brew_bottles::{classify_formulae, linux_support, render_snapshot, BottleSnapshot, Classified, LINUX_BOTTLES};
use wsp_collect::manifest::Linux;

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn bottled(files: &[&str]) -> Value {
    let files: serde_json::Map<String, Value> = files.iter().map(|f| (f.to_string(), json!({ "url": "x" }))).collect();
    json!({ "bottle": { "stable": { "files": files } } })
}

fn named(name: &str, extra: Value) -> Value {
    let mut formula = json!({ "name": name });
    if let Value::Object(more) = extra {
        formula.as_object_mut().unwrap().extend(more);
    }
    formula
}

fn formula_json() -> Value {
    json!([
        named("jq", bottled(&["arm64_sequoia", "x86_64_linux"])),
        named("gh", bottled(&["arm64_sequoia", "arm64_linux"])),
        named("ca-certificates", bottled(&["all"])),
        named("mas", bottled(&["arm64_sequoia", "sonoma"])),
        { "name": "afsctool", "bottle": {} },
        { "name": "brightness" },
        named("python@3.12", bottled(&["x86_64_linux"])),
    ])
}

fn snapshot() -> BottleSnapshot {
    BottleSnapshot {
        fetched: "2026-09-03".to_string(),
        source: "https://example.test/formula.json".to_string(),
        linux: strs(&["gh", "jq"]),
        no_linux: strs(&["mas"]),
    }
}

mod brew_bottles {
    use super::*;

    #[test]
    fn splits_formulae_by_whether_any_bottle_runs_on_linux_sorted_by_name() {
        assert_eq!(
            classify_formulae(&formula_json()),
            Classified { linux: strs(&["ca-certificates", "gh", "jq", "python@3.12"]), no_linux: strs(&["afsctool", "brightness", "mas"]) }
        );
    }

    #[test]
    fn ignores_anything_that_is_not_a_formula_list() {
        assert_eq!(classify_formulae(&json!("nope")), Classified { linux: vec![], no_linux: vec![] });
        assert_eq!(classify_formulae(&json!([{ "nope": 1 }, null, 4])), Classified { linux: vec![], no_linux: vec![] });
    }

    // The TS checks the generated .ts source; the Rust `render_snapshot` writes the JSON file the snapshot is
    // embedded from, so this asserts that text: keys fetched, source, linux, noLinux, one name per line.
    #[test]
    fn renders_a_snapshot_as_a_generated_source_file_carrying_the_fetch_date_one_name_p() {
        let want = "{\n  \"fetched\": \"2026-09-03\",\n  \"source\": \"https://example.test/formula.json\",\n  \"linux\": [\n    \"gh\",\n    \"jq\"\n  ],\n  \"noLinux\": [\n    \"mas\"\n  ]\n}\n";
        assert_eq!(render_snapshot(&snapshot()), want);
    }

    #[test]
    fn a_name_maps_to_its_linux_support() {
        let rows = [("jq", Linux::Yes), ("mas", Linux::No), ("oven-sh/bun/bun", Linux::Unknown), ("brand-new-formula", Linux::Unknown)];
        let snapshot = snapshot();
        for (name, want) in rows {
            assert_eq!(linux_support(name, &snapshot), want, "{name} -> {want:?}");
        }
    }

    #[test]
    fn the_checked_in_snapshot_is_dated_disjoint_and_knows_the_everyday_formulae() {
        let fetched = LINUX_BOTTLES.fetched.as_bytes();
        assert!(
            fetched.len() == 10 && fetched.iter().enumerate().all(|(i, b)| if i == 4 || i == 7 { *b == b'-' } else { b.is_ascii_digit() }),
            "fetched is YYYY-MM-DD: {:?}",
            LINUX_BOTTLES.fetched
        );
        assert_eq!(LINUX_BOTTLES.source, "https://formulae.brew.sh/api/formula.json");
        assert!(LINUX_BOTTLES.linux.len() > 5000);
        let linux: HashSet<&String> = LINUX_BOTTLES.linux.iter().collect();
        assert!(!LINUX_BOTTLES.no_linux.iter().any(|n| linux.contains(n)));
        assert_eq!(linux_support("gh", &LINUX_BOTTLES), Linux::Yes);
        assert_eq!(linux_support("jq", &LINUX_BOTTLES), Linux::Yes);
        assert_eq!(linux_support("mas", &LINUX_BOTTLES), Linux::No);
        assert_eq!(linux_support("pinentry-mac", &LINUX_BOTTLES), Linux::No);
    }
}

/// The embedded table is the TS table, name for name, and the file is the one data/linux_bottles.py wrote
/// (spec/brew_bottles.md rules 6 and 7).
mod embedded_table {
    use super::*;

    fn ts_names(src: &str, key: &str) -> Vec<String> {
        let start = src.find(&format!("\n  {key}: [\n")).unwrap() + key.len() + 7;
        let end = start + src[start..].find("\n  ],").unwrap();
        src[start..end].lines().map(|l| l.trim().trim_end_matches(',').trim_matches('"').to_string()).collect()
    }

    #[test]
    fn the_embedded_table_holds_every_name_of_the_ts_table_in_its_order() {
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/collect/src/data/linux-bottles.ts")).unwrap();
        assert_eq!(LINUX_BOTTLES.linux, ts_names(&src, "linux"));
        assert_eq!(LINUX_BOTTLES.no_linux, ts_names(&src, "noLinux"));
        assert_eq!((LINUX_BOTTLES.linux.len(), LINUX_BOTTLES.no_linux.len()), (8428, 158));
        assert_eq!(LINUX_BOTTLES.fetched, "2026-09-02");
        assert_eq!(LINUX_BOTTLES.linux.first().map(String::as_str), Some("a2ps"));
        assert_eq!(LINUX_BOTTLES.no_linux.last().map(String::as_str), Some("zzz"));
    }

    #[test]
    fn rendering_the_embedded_snapshot_gives_back_the_embedded_file_byte_for_byte() {
        use sha2::{Digest, Sha256};
        let text = wsp_collect::data::linux_bottles::LINUX_BOTTLES_JSON;
        let digest: String = Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(digest, "ae9c156dadab5757892c4aed7b5377acc1b59e5a25715e72a33a3102ae41e721");
        assert_eq!(render_snapshot(&LINUX_BOTTLES), text);
    }

    #[test]
    fn an_empty_snapshot_renders_its_lists_as_empty_arrays() {
        let s = BottleSnapshot { fetched: "2026-01-01".into(), source: "s".into(), linux: vec![], no_linux: vec![] };
        assert_eq!(
            render_snapshot(&s),
            "{\n  \"fetched\": \"2026-01-01\",\n  \"source\": \"s\",\n  \"linux\": [],\n  \"noLinux\": []\n}\n"
        );
    }
}

/// Every case of tests/fixtures/oracle/w2/brew_bottles.json (oracle/w2-brew_bottles.oracle.ts): spec brew_bottles's
/// wave-2 rules.
mod w2_oracle {
    use super::*;

    fn fixture() -> Value {
        let path = format!("{}/tests/fixtures/oracle/w2/brew_bottles.json", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn word(l: Linux) -> Value {
        serde_json::to_value(l).unwrap()
    }

    #[test]
    fn classify_support_and_render_answer_what_the_ts_answered() {
        let f = fixture();
        for case in f["classify_formulae"].as_array().unwrap() {
            let c = classify_formulae(&case["args"][0]);
            assert_eq!(json!({ "linux": c.linux, "noLinux": c.no_linux }), case["out"], "{}", case["name"]);
        }
        for case in f["linux_support"].as_array().unwrap() {
            let snapshot: BottleSnapshot = serde_json::from_value(case["args"][1].clone()).unwrap();
            assert_eq!(word(linux_support(case["args"][0].as_str().unwrap(), &snapshot)), case["out"], "{}", case["name"]);
        }
        for case in f["linux_support_embedded"].as_array().unwrap() {
            assert_eq!(word(linux_support(case["args"][0].as_str().unwrap(), &LINUX_BOTTLES)), case["out"], "{}", case["name"]);
        }
        for case in f["render_snapshot"].as_array().unwrap() {
            let snapshot: BottleSnapshot = serde_json::from_value(case["args"][0].clone()).unwrap();
            assert_eq!(json!(render_snapshot(&snapshot)), case["out"], "{}", case["name"]);
        }
        let ends = &f["embedded_table"][0]["out"];
        assert_eq!(json!(LINUX_BOTTLES.linux.len()), ends["linux"]);
        assert_eq!(json!(LINUX_BOTTLES.no_linux.len()), ends["noLinux"]);
        assert_eq!(json!(LINUX_BOTTLES.fetched), ends["fetched"]);
        assert_eq!(json!(LINUX_BOTTLES.linux.first()), ends["firstLinux"]);
        assert_eq!(json!(LINUX_BOTTLES.no_linux.last()), ends["lastNoLinux"]);
    }
}
