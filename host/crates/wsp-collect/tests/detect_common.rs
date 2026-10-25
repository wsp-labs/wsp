// SPDX-License-Identifier: AGPL-3.0-only
//! The row builders every detector uses (spec/common.md). common.ts has no tests of its own in TS; these pin each rule
//! of its spec, with values read off the TS code at a3b870a6e.
mod common;

use common::fake_host::*;
use serde_json::{json, Value};
use wsp_collect::detect::common::{entry, exists, first_line, found, item, present, row, EntrySpec, Found, RowOptions, RowSpec};
use wsp_collect::manifest::{Linux, ManifestEntry, RowDefault, Rung};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn as_json(e: &ManifestEntry) -> Value {
    serde_json::to_value(e).unwrap()
}

fn row_of(v: Value) -> ManifestEntry {
    serde_json::from_value(v).unwrap()
}

#[test]
fn found_keeps_input_order_and_spelling_and_sums_bytes() {
    let host = fake_host(FakeLaptop::new().file("~/b", 20).file("~/.config/x/", 300).file("/etc/abs", 4));
    let got = block_on(found(&host, &s(&["~/b", "~/missing", "~/.config/x", "/etc/abs", "~/b"])));
    // A repeated candidate is stat'ed and counted again, as the TS loop does.
    assert_eq!(got, Found { paths: s(&["~/b", "~/.config/x", "/etc/abs", "~/b"]), bytes: 20 + 300 + 4 + 20 });
}

#[test]
fn found_of_nothing_stats_nothing() {
    let host = fake_host(FakeLaptop::new().file("~/b", 20));
    assert_eq!(block_on(found(&host, &[])), Found { paths: vec![], bytes: 0 });
    assert_eq!(host.calls(), Vec::<String>::new());
}

#[test]
fn exists_expands_a_tilde_path() {
    let host = fake_host(FakeLaptop::new().file("~/.zshrc", 10).file("/opt/x/", 1));
    assert!(block_on(exists(&host, "~/.zshrc")));
    assert!(block_on(exists(&host, "/opt/x")));
    assert!(!block_on(exists(&host, "~/.bashrc")));
    assert!(!block_on(exists(&host, ".zshrc")));
}

#[test]
fn row_is_none_when_nothing_exists_unless_always() {
    let host = fake_host(FakeLaptop::new().file("~/.vimrc", 7));
    let spec = |paths: &[&str], always: bool| RowSpec {
        rung: Rung::Shell,
        id: "shell/vim".into(),
        label: "vim".into(),
        paths: s(paths),
        always,
        opt: RowOptions { group: Some("Editors".into()), ..Default::default() },
    };
    assert!(block_on(row(&host, spec(&["~/.nvimrc"], false))).is_none());
    let always = block_on(row(&host, spec(&["~/.nvimrc"], true))).expect("an always row");
    assert_eq!(
        as_json(&always),
        json!({"rung": "shell", "id": "shell/vim", "label": "vim", "group": "Editors", "paths": [], "bytes": 0, "default": "bring"})
    );
    let some = block_on(row(&host, spec(&["~/.nvimrc", "~/.vimrc"], false))).expect("a found row");
    assert_eq!(
        as_json(&some),
        json!({"rung": "shell", "id": "shell/vim", "label": "vim", "group": "Editors", "paths": ["~/.vimrc"], "bytes": 7, "default": "bring"})
    );
}

#[test]
fn entry_defaults_to_bring_and_leaves_unset_fields_absent() {
    let bare = entry(EntrySpec {
        rung: Rung::Tools,
        id: "tools/x".into(),
        label: "x".into(),
        paths: vec![],
        bytes: 0,
        opt: RowOptions::default(),
    });
    assert_eq!(as_json(&bare), json!({"rung": "tools", "id": "tools/x", "label": "x", "paths": [], "bytes": 0, "default": "bring"}));
    let full = entry(EntrySpec {
        rung: Rung::Tools,
        id: "tools/y".into(),
        label: "y".into(),
        paths: s(&["~/y"]),
        bytes: 9,
        opt: RowOptions {
            default: Some(RowDefault::Skip),
            reason: Some("why".into()),
            group: Some("G".into()),
            required: Some(false),
            linux: Some(Linux::Unknown),
            version: Some("1.2".into()),
            arch: Some("arm64".into()),
            volatile: None,
            detail: Some("d".into()),
        },
    });
    // required: false is kept: the TS tests `!== undefined`, not truthiness.
    assert_eq!(
        as_json(&full),
        json!({"rung": "tools", "id": "tools/y", "label": "y", "group": "G", "paths": ["~/y"], "bytes": 9, "default": "skip", "reason": "why", "required": false, "linux": "unknown", "version": "1.2", "arch": "arm64", "detail": "d"})
    );
}

#[test]
fn entry_keeps_only_volatile_paths_that_are_on_the_row() {
    let make = |volatile: Option<&[&str]>| {
        entry(EntrySpec {
            rung: Rung::Agents,
            id: "agents/a".into(),
            label: "a".into(),
            paths: s(&["~/.a/state.json", "~/.a/config", "~/.a/log"]),
            bytes: 3,
            opt: RowOptions { volatile: volatile.map(s), ..Default::default() },
        })
    };
    let kept = make(Some(&["~/.a/log", "~/.a/gone", "~/.a/state.json"]));
    assert_eq!(as_json(&kept)["volatile"], json!(["~/.a/log", "~/.a/state.json"]));
    assert!(as_json(&make(Some(&["~/.a/gone"]))).get("volatile").is_none());
    assert!(as_json(&make(Some(&[]))).get("volatile").is_none());
    assert!(as_json(&make(None)).get("volatile").is_none());
}

#[test]
fn item_has_no_path_no_bytes_and_no_volatile() {
    let got = item(
        Rung::Tools,
        "tools/brew/gh",
        "gh",
        RowOptions { group: Some("Homebrew".into()), linux: Some(Linux::Yes), volatile: Some(s(&["~/x"])), ..Default::default() },
    );
    assert_eq!(
        as_json(&got),
        json!({"rung": "tools", "id": "tools/brew/gh", "label": "gh", "group": "Homebrew", "paths": [], "bytes": 0, "default": "bring", "linux": "yes"})
    );
}

#[test]
fn present_drops_absent_rows_in_order() {
    let a = row_of(json!({"rung": "tools", "id": "tools/a", "label": "a", "paths": [], "bytes": 0, "default": "bring"}));
    let b = row_of(json!({"rung": "tools", "id": "tools/b", "label": "b", "paths": [], "bytes": 0, "default": "skip"}));
    let got = present(vec![None, Some(b.clone()), None, Some(a.clone()), None]);
    assert_eq!(got.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), s(&["tools/b", "tools/a"]));
    assert_eq!(got, vec![b, a]);
    assert!(present(vec![None, None]).is_empty());
}

#[test]
fn first_line_reads_only_the_first_line_trimmed_as_javascript_trims() {
    assert_eq!(first_line(None), None);
    assert_eq!(first_line(Some("")), None);
    assert_eq!(first_line(Some("   ")), None);
    assert_eq!(first_line(Some("\u{feff}")), None);
    assert_eq!(first_line(Some("\nv1")), None);
    assert_eq!(first_line(Some("  /opt/homebrew  \nsecond")), Some("/opt/homebrew".into()));
    assert_eq!(first_line(Some("v1\r\n")), Some("v1".into()));
    // U+0085 is not JavaScript whitespace, so it stays (rule 8.6).
    assert_eq!(first_line(Some("\u{85}v1")), Some("\u{85}v1".into()));
}

/// Every case of tests/fixtures/oracle/w2/common.json (oracle/w2-common.oracle.ts): spec common's wave-2 rules.
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, host, strs, w2};
    use serde_json::{json, Value};
    use wsp_collect::detect::common::{entry, exists, first_line, found, item, present, row, EntrySpec, RowOptions, RowSpec};
    use wsp_collect::manifest::{ManifestEntry, RUNGS};

    fn opt(v: &Value) -> RowOptions {
        RowOptions {
            default: v.get("default").map(|x| serde_json::from_value(x.clone()).unwrap()),
            reason: v["reason"].as_str().map(str::to_string),
            group: v["group"].as_str().map(str::to_string),
            required: v["required"].as_bool(),
            linux: v.get("linux").map(|x| serde_json::from_value(x.clone()).unwrap()),
            version: v["version"].as_str().map(str::to_string),
            arch: v["arch"].as_str().map(str::to_string),
            volatile: v.get("volatile").map(strs),
            detail: v["detail"].as_str().map(str::to_string),
        }
    }

    fn rung(v: &Value) -> wsp_collect::manifest::Rung {
        *RUNGS.iter().find(|r| r.as_str() == v.as_str().unwrap()).unwrap()
    }

    fn row_json(r: Option<ManifestEntry>) -> Value {
        r.map(|r| serde_json::to_value(r).unwrap()).unwrap_or(Value::Null)
    }

    #[test]
    fn found_exists_and_row_answer_what_the_ts_answered() {
        let f = w2("common");
        for case in cases(&f, "found") {
            let h = host(&case["laptop"]);
            let got = block_on(found(&h, &strs(&case["args"][0])));
            assert_eq!(json!({ "paths": got.paths, "bytes": got.bytes }), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "exists") {
            let h = host(&case["laptop"]);
            assert_eq!(json!(block_on(exists(&h, case["args"][0].as_str().unwrap()))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "row") {
            let h = host(&case["laptop"]);
            let a = &case["args"][0];
            let spec = RowSpec {
                rung: rung(&a["rung"]),
                id: a["id"].as_str().unwrap().into(),
                label: a["label"].as_str().unwrap().into(),
                paths: strs(&a["paths"]),
                always: a["always"] == true,
                opt: opt(a),
            };
            assert_eq!(row_json(block_on(row(&h, spec))), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }

    #[test]
    fn entry_item_present_and_first_line_build_what_the_ts_built() {
        let f = w2("common");
        for case in cases(&f, "entry") {
            let a = &case["args"][0];
            let spec = EntrySpec {
                rung: rung(&a["rung"]),
                id: a["id"].as_str().unwrap().into(),
                label: a["label"].as_str().unwrap().into(),
                paths: strs(&a["paths"]),
                bytes: a["bytes"].as_u64().unwrap(),
                opt: opt(a),
            };
            assert_eq!(serde_json::to_value(entry(spec)).unwrap(), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "item") {
            let a = &case["args"][0];
            let got = item(rung(&a["rung"]), a["id"].as_str().unwrap(), a["label"].as_str().unwrap(), opt(a));
            assert_eq!(serde_json::to_value(got).unwrap(), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "present") {
            let rows: Vec<Option<ManifestEntry>> = serde_json::from_value(case["args"][0].clone()).unwrap();
            assert_eq!(serde_json::to_value(present(rows)).unwrap(), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "first_line") {
            assert_eq!(json!(first_line(case["args"].get(0).and_then(Value::as_str))), case["out"], "{}", case["name"]);
        }
    }
}
