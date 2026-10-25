// SPDX-License-Identifier: AGPL-3.0-only
//! The manifest schema's refusals and key order, against what zod and the TS printed for the same inputs
//! (tests/fixtures/oracle/manifest.json, written by oracle/rest.oracle.ts). spec/manifest.md rules 3 to 9.
use serde_json::{json, Value};
use wsp_collect::manifest::{parse_entry, parse_manifest, safe_parse_manifest, ManifestEntry, ManifestIssue};

fn oracle() -> Value {
    serde_json::from_str(include_str!("fixtures/oracle/manifest.json")).unwrap()
}

fn issues_json(issues: &[ManifestIssue]) -> Value {
    Value::Array(issues.iter().map(|i| json!({ "path": i.path, "message": i.message })).collect())
}

/// `{...ROW, rung: undefined}` in the TS drops the key; the fixture writes it as a missing key, so nothing to undo.
#[test]
fn every_row_refusal_is_zods_issues_in_zods_order_and_a_valid_row_has_none() {
    for case in oracle()["entries"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let got = match parse_entry(&case["data"]) {
            Ok(_) => Value::Null,
            Err(issues) => issues_json(&issues),
        };
        assert_eq!(got, case["issues"], "{name}");
    }
}

#[test]
fn a_type_error_in_a_row_stops_its_rung_checks_but_a_short_string_does_not() {
    // rung 3 is an invalid_type: zod aborts the row, so the id-prefix refine never runs.
    let typed = parse_entry(&json!({"rung": 3, "id": "shell/x", "label": "", "paths": [], "bytes": 0, "default": "bring"})).unwrap_err();
    assert_eq!(typed.len(), 1);
    assert_eq!(typed[0].path, vec![json!("rung")]);
    // An empty id fails min(1), which leaves the row dirty, not aborted: the refine runs and adds its own issue.
    let short = parse_entry(&json!({"rung": "identity", "id": "", "label": "", "paths": [], "bytes": 0, "default": "bring"})).unwrap_err();
    let messages: Vec<&str> = short.iter().map(|i| i.message.as_str()).collect();
    assert_eq!(messages, ["String must contain at least 1 character(s)", "id must start with identity/"]);
}

#[test]
fn every_envelope_refusal_is_zods_issues_and_parse_manifest_says_them_in_one_line() {
    for case in oracle()["manifests"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let got = match safe_parse_manifest(&case["data"]) {
            Ok(_) => Value::Null,
            Err(issues) => issues_json(&issues),
        };
        assert_eq!(got, case["issues"], "{name}");
        let said = match parse_manifest(&case["data"]) {
            Ok(_) => Value::Null,
            Err(e) => Value::String(e.message.clone()),
        };
        assert_eq!(said, case["error"], "{name}");
    }
}

#[test]
fn the_duplicate_check_runs_only_when_no_row_was_refused_by_type() {
    let row = json!({"rung": "identity", "id": "identity/a", "label": "", "paths": [], "bytes": 0, "default": "bring"});
    let mut typed = row.clone();
    typed["bytes"] = json!("0");
    let err = parse_manifest(&json!({"entries": [row, typed, row]})).unwrap_err();
    assert_eq!(err.message, "invalid manifest: entries.1.bytes: Expected number, received string");
    let mut skipped = row.clone();
    skipped["required"] = json!(true);
    skipped["default"] = json!("skip");
    let err = parse_manifest(&json!({"entries": [row, skipped]})).unwrap_err();
    assert_eq!(
        err.message,
        "invalid manifest: entries.1.required: a required row cannot default to skip; entries.1.id: duplicate id identity/a"
    );
}

#[test]
fn a_parsed_row_writes_its_keys_in_the_schemas_order_and_drops_arch_and_unknown_keys() {
    for case in oracle()["keyOrder"].as_array().unwrap() {
        let parsed = parse_manifest(&json!({"entries": [case["data"]]})).unwrap();
        let row = &parsed.entries[0];
        assert_eq!(row.arch, None, "{}", case["parsed"]);
        assert_eq!(serde_json::to_string(row).unwrap(), case["parsed"].as_str().unwrap());
    }
}

#[test]
fn a_detectors_row_keeps_arch_until_it_is_parsed() {
    let mut row = ManifestEntry::new(wsp_collect::manifest::Rung::Tools, "tools/x", "x", vec![], 0);
    row.arch = Some("arm64".to_string());
    assert_eq!(
        serde_json::to_string(&row).unwrap(),
        r#"{"rung":"tools","id":"tools/x","label":"x","paths":[],"bytes":0,"default":"bring","arch":"arm64"}"#
    );
    let parsed = parse_manifest(&json!({"entries": [serde_json::to_value(&row).unwrap()]})).unwrap();
    assert_eq!(parsed.entries[0].arch, None);
}

#[test]
fn an_empty_list_of_entries_is_a_manifest_and_writes_back_as_one() {
    let m = parse_manifest(&json!({"entries": []})).unwrap();
    assert!(m.entries.is_empty());
    assert_eq!(serde_json::to_string(&m).unwrap(), r#"{"entries":[]}"#);
}

/// Every case of tests/fixtures/oracle/w2/manifest.json's parse keys (oracle/w2-manifest.oracle.ts): spec manifest's
/// wave-2 rules. `bytes` past 2^53 is an accepted difference (spec manifest Decisions), so its two cases are skipped.
mod w2_oracle {
    use super::*;

    fn fixture() -> Value {
        let path = format!("{}/tests/fixtures/oracle/w2/manifest.json", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn issues(v: &[ManifestIssue]) -> Value {
        Value::Array(v.iter().map(|i| json!({ "path": i.path, "message": i.message })).collect())
    }

    #[test]
    fn every_row_parses_or_is_refused_with_zod_s_issues_in_zod_s_order() {
        for case in fixture()["parse_entry"].as_array().unwrap() {
            let what = case["name"].as_str().unwrap();
            if what.contains("1e20") || what.contains("above 2^53") {
                continue;
            }
            let data: Value = match case.get("args_json") {
                Some(text) => serde_json::from_str(text.as_str().unwrap()).unwrap(),
                None => case["args"][0].clone(),
            };
            match parse_entry(&data) {
                Ok(row) => {
                    assert!(case.get("issues").is_none(), "{what}: the TS refused it");
                    if let Some(text) = case.get("text") {
                        assert_eq!(serde_json::to_string(&row).unwrap(), text.as_str().unwrap(), "{what}");
                    }
                }
                Err(got) => assert_eq!(issues(&got), case["issues"], "{what}"),
            }
        }
    }

    #[test]
    fn every_envelope_parses_or_is_refused_as_the_ts_did() {
        let f = fixture();
        for case in f["safe_parse_manifest"].as_array().unwrap() {
            match safe_parse_manifest(&case["args"][0]) {
                Ok(m) => assert_eq!(serde_json::to_string(&m).unwrap(), case["text"].as_str().unwrap(), "{}", case["name"]),
                Err(got) => assert_eq!(issues(&got), case["issues"], "{}", case["name"]),
            }
        }
        for case in f["parse_manifest"].as_array().unwrap() {
            match parse_manifest(&case["args"][0]) {
                Ok(m) => {
                    assert_eq!(serde_json::to_value(&m).unwrap(), case["out"]["ok"], "{}", case["name"]);
                    assert_eq!(serde_json::to_string(&m).unwrap(), case["out"]["text"].as_str().unwrap(), "{}", case["name"]);
                }
                Err(e) => assert_eq!(json!(e.message), case["out"]["error"], "{}", case["name"]),
            }
        }
    }
}
