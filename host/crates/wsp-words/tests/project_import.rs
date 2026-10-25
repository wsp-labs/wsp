// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/project-import.test.ts lines 1-46 (importConsented, and the request test's value
//! assertions at :27-:45); the second `it` of importRequest (:48-:62) reads the host's sources and is a law test, skipped.

use indexmap::IndexSet;
use serde_json::{json, Value};
use wsp_words::project_import::*;
use wsp_wire::views::project_bundle::ProjectPlan;

fn plan() -> ProjectPlan {
    let env = json!({ "path": ".env", "bytes": 19, "signals": ["name"] });
    let config = json!({ "path": ".git/config", "bytes": 200, "signals": ["url"], "rewrite": { "urls": ["https://github.com/o/r"], "drop": [] } });
    serde_json::from_value(json!({
        "source": "/Users/me/code/proj",
        "repo": true,
        "files": 3,
        "bytes": 300,
        "secrets": [env, config],
        "excluded": ["node_modules"],
        "skipped": [],
        "agents": [
            { "agent": "claude", "name": "Claude Code", "sessions": 2, "bytes": 100, "carry": "moves" },
            { "agent": "codex", "name": "Codex", "sessions": 1, "bytes": 50, "carry": "transcript-only" },
        ],
    }))
    .unwrap()
}

fn set(items: &[&str]) -> IndexSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn request_json(plan: &ProjectPlan, source: &str, ticked: &IndexSet<String>, agents: &IndexSet<String>, replace: Option<bool>) -> Value {
    serde_json::to_value(import_request(plan, source, ticked, agents, replace)).unwrap()
}

fn answers(yes: Option<bool>, keep: &[&str], cut: &[&str]) -> ImportAnswers {
    ImportAnswers { yes, keep: keep.iter().map(|s| s.to_string()).collect(), cut: cut.iter().map(|s| s.to_string()).collect() }
}

mod import_consented {
    use super::*;

    #[test]
    fn is_yes_or_an_answer_to_a_secret_shaped_row_nothing_given_is_the_dry_run() {
        assert!(import_consented(&answers(Some(true), &[], &[])));
        assert!(import_consented(&answers(None, &[".env"], &[])));
        assert!(import_consented(&answers(None, &[], &[".git/config"])));
        assert!(!import_consented(&answers(Some(false), &[], &[])));
        assert!(!import_consented(&answers(None, &[], &[])));
    }
}

mod import_request {
    use super::*;

    #[test]
    fn is_the_one_request_every_client_sends_the_source_as_given_dest_from_the_plan_car() {
        let plan = plan();
        assert_eq!(
            request_json(&plan, "/Users/me/code/proj/", &set(&[".env", ".git/config"]), &set(&["claude"]), Some(true)),
            json!({
                "source": "/Users/me/code/proj/",
                "dest": "/Users/me/code/proj",
                "carry": [".env"],
                "rewrite": [".git/config"],
                "agents": ["claude"],
                "replace": true,
            })
        );
        let bare = request_json(&plan, "/Users/me/code/proj", &set(&[]), &set(&[]), Some(false));
        assert_eq!(bare, json!({ "source": "/Users/me/code/proj", "dest": "/Users/me/code/proj", "carry": [], "rewrite": [] }));
        assert!(bare.get("agents").is_none(), "{bare}");
        assert!(bare.get("replace").is_none(), "{bare}");
        let both = request_json(&plan, "/Users/me/code/proj", &set(&[]), &set(&["codex", "claude"]), None);
        assert_eq!(both["agents"], json!(["claude", "codex"]));
    }
}
