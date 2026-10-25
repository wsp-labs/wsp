// SPDX-License-Identifier: AGPL-3.0-only
//! Ports the project path rules, rewriteJsonl and moveProjectState blocks of packages/engine/test/project-state.test.ts.
//! The gemini write-killed-by-SIGXFSZ test and project-state-import.test.ts are not ported (see the report).
mod ps_fixtures;

use std::os::unix::fs::symlink;
use std::path::Path;

use indexmap::IndexMap;
use serde_json::Value;

use wsp_catalog::catalog::{catalog_agents, StateStatus};
use wsp_engine::errors::EngineError;
use wsp_engine::project_state::resolver::{inside_folder, resolve_project_path, rewrite_jsonl, under_home};
use wsp_engine::project_state::{
    move_project_state, project_state_resolvers, under_project, AgentMoveReport, AgentOutcome, ProjectStateMove,
};
use wsp_wire::views::project_bundle::ProjectCarry;

use ps_fixtures::*;

fn homes(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(a, h)| (a.to_string(), h.to_string())).collect()
}

fn label(o: &AgentOutcome) -> &'static str {
    match o {
        AgentOutcome::Moved(_) => "moved",
        AgentOutcome::Nothing => "nothing",
        AgentOutcome::TranscriptOnly => "transcript-only",
        AgentOutcome::Failed(_) => "failed",
    }
}

fn catalog_ids() -> Vec<String> {
    catalog_agents().iter().map(|a| a.id.clone()).collect()
}

/// node's own words for a missing file, as spec/project_state_files.md pins them.
fn enoent(file: &str) -> EngineError {
    EngineError::Other { message: format!("ENOENT: no such file or directory, open '{file}'"), code: Some("ENOENT".into()) }
}

// --- the core ---------------------------------------------------------------------------------------------------------

// TS: packages/engine/test/project-state.test.ts:502
#[test]
fn a_path_is_under_the_project_at_the_root_or_inside_never_a_sibling_trailing_slash_is_not_part_of_key() {
    assert_eq!(resolve_project_path("/a/proj/"), "/a/proj");
    assert!(under_project("/a/proj", "/a/proj"));
    assert!(under_project("/a/proj/src/deep", "/a/proj"));
    assert!(!under_project("/a/proj2", "/a/proj"));
    assert!(!under_project("/a", "/a/proj"));
}

// TS: packages/engine/test/project-state.test.ts:510
#[test]
fn a_name_an_archive_carried_stays_inside_the_home_and_a_name_starting_with_two_dots_is_a_folder() {
    assert!(inside_folder("sessions/a.jsonl"));
    assert!(inside_folder("..cache"));
    assert!(inside_folder("a/..cache/b"));
    assert!(!inside_folder(".."));
    assert!(!inside_folder(&j("..", &["out"])));
    assert!(!inside_folder(""));
    assert!(!inside_folder("/etc/passwd"));

    assert_eq!(under_home("/root/.codex", &["sessions", "a.jsonl"]), Some("/root/.codex/sessions/a.jsonl".to_string()));
    assert_eq!(under_home("/root/.codex", &["sessions", "../../../../etc/passwd"]), None);
    assert_eq!(under_home("/root/.gemini", &["..cache"]), Some("/root/.gemini/..cache".to_string()));
    assert_eq!(under_home("/root/.gemini", &[]), None);
}

// --- rewriteJsonl -----------------------------------------------------------------------------------------------------

// TS: packages/engine/test/project-state.test.ts:530
#[tokio::test]
async fn a_rewrite_that_fails_leaves_the_file_as_it_was_and_no_wsp_move_beside_it() {
    let s = scratch();
    let missing = j(&s.root, &["missing.jsonl"]);
    for _ in 0..300 {
        let err = rewrite_jsonl(&missing, &mut |_| Ok(true)).await.unwrap_err();
        assert_eq!(err, enoent(&missing));
        assert!(!Path::new(&format!("{missing}.wsp-move")).exists());
    }
    let file = j(&s.root, &["s.jsonl"]);
    let text = lines(&[&serde_json::json!({ "cwd": FROM }).to_string(), &serde_json::json!({ "cwd": OTHER }).to_string()]);
    write(&file, &text);
    let err = rewrite_jsonl(&file, &mut |_| Err(EngineError::Other { message: "bad line".into(), code: None })).await.unwrap_err();
    assert_eq!(err, EngineError::Other { message: "bad line".into(), code: None });
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    assert!(!Path::new(&format!("{file}.wsp-move")).exists());
}

// --- moveProjectState -------------------------------------------------------------------------------------------------

// TS: packages/engine/test/project-state.test.ts:548
#[test]
fn registers_each_module_under_its_own_catalog_agent_naming_only_measured_rows_of_that_entry() {
    assert!(!project_state_resolvers().is_empty());
    for (id, r) in project_state_resolvers().iter() {
        assert_eq!(r.agent(), *id);
        let agents = catalog_agents();
        let entry = agents.iter().find(|a| a.id == *id).unwrap_or_else(|| panic!("{id} is not a catalog agent"));
        let measured: Vec<&str> =
            entry.project_state.iter().filter(|s| s.status == StateStatus::Measured).map(|s| s.state.as_str()).collect();
        let states = r.states();
        assert!(!states.is_empty(), "{id}");
        for s in states {
            assert!(measured.contains(&s), "{id} {s}");
        }
        assert!(matches!(r.carry(), ProjectCarry::Moves | ProjectCarry::TranscriptOnly), "{id}");
    }
}

// TS: packages/engine/test/project-state.test.ts:561
#[tokio::test]
async fn walks_every_catalog_agent_two_present_move_the_four_absent_report_nothing() {
    let s = scratch();
    let claude = claude_home(&s.root);
    let pi = pi_home(&s.root);
    let gemini = j(&s.root, &["gemini-empty"]);
    std::fs::create_dir(&gemini).unwrap();
    let codex = j(&s.root, &["no-such-home"]);
    let report = move_project_state(
        &ProjectStateMove {
            from: FROM.into(),
            to: TO.into(),
            homes: homes(&[("claude", &claude), ("pi", &pi), ("gemini", &gemini), ("codex", &codex)]),
        },
        None,
    )
    .await;
    let got: Vec<(&str, &str)> = report.iter().map(|r| (r.agent.as_str(), label(&r.outcome))).collect();
    assert_eq!(
        got,
        vec![
            ("claude", "moved"),
            ("codex", "nothing"),
            ("gemini", "nothing"),
            ("opencode", "nothing"),
            ("pi", "moved"),
            ("hermes", "nothing"),
            ("crush", "transcript-only"),
            ("qwen", "transcript-only"),
            ("goose", "transcript-only"),
            ("amp", "transcript-only"),
            ("cursor", "transcript-only"),
        ]
    );
    assert_eq!(tree(&claude), claude_after("-root-work-b-2-x"));
    assert!(tree(&pi).contains_key("sessions/--root-work-b_2.x--/2026-09-05T21-56-00-000Z_s1.jsonl"));
    match &report[0].outcome {
        AgentOutcome::Moved(m) => {
            assert_eq!(m.iter().map(|x| x.state.as_str()).collect::<Vec<_>>(), vec!["session transcripts", "auto memory"])
        }
        other => panic!("claude: {other:?}"),
    }
}

// TS: packages/engine/test/project-state.test.ts:586
#[tokio::test]
async fn reports_a_catalog_agent_without_a_module_as_transcript_only_and_never_touches_its_home() {
    let s = scratch();
    let home = hermes_home(&s.root);
    let before = tree(&home);
    let mut agents = catalog_ids();
    agents.push("newagent".into());
    let report =
        move_project_state(&ProjectStateMove { from: FROM.into(), to: TO.into(), homes: homes(&[("newagent", &home)]) }, Some(&agents))
            .await;
    let found = report.iter().find(|r| r.agent == "newagent");
    assert_eq!(found, Some(&AgentMoveReport { agent: "newagent".into(), outcome: AgentOutcome::TranscriptOnly }));
    let mut want: Vec<String> = catalog_ids().into_iter().filter(|id| !project_state_resolvers().contains_key(id.as_str())).collect();
    want.push("newagent".into());
    let got: Vec<String> = report.iter().filter(|r| r.outcome == AgentOutcome::TranscriptOnly).map(|r| r.agent.clone()).collect();
    assert_eq!(got, want);
    assert_eq!(tree(&home), before);
}

// TS: packages/engine/test/project-state.test.ts:596
#[tokio::test]
async fn keys_on_the_real_path_a_symlinked_destination_resolves_before_the_dashed_key_is_built() {
    let s = scratch();
    let claude = claude_home(&s.root);
    std::fs::create_dir_all(j(&s.root, &["real", "work", "b_2.x"])).unwrap();
    symlink(j(&s.root, &["real"]), j(&s.root, &["link"])).unwrap();
    let real = std::fs::canonicalize(j(&s.root, &["real", "work", "b_2.x"])).unwrap().to_str().unwrap().to_string();
    let report = move_project_state(
        &ProjectStateMove { from: format!("{FROM}/"), to: j(&s.root, &["link", "work", "b_2.x"]), homes: homes(&[("claude", &claude)]) },
        None,
    )
    .await;
    assert_eq!((report[0].agent.as_str(), label(&report[0].outcome)), ("claude", "moved"));
    let key: String = real.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let after = tree(&claude);
    let first = after
        .get(&format!("projects/{key}/S1.jsonl"))
        .unwrap_or_else(|| panic!("no projects/{key}/S1.jsonl in {:?}", after.keys().collect::<Vec<_>>()));
    let head: Value = serde_json::from_str(first.as_str().unwrap().split('\n').next().unwrap()).unwrap();
    assert_eq!(head["cwd"], Value::String(real));
}

// TS: packages/engine/test/project-state.test.ts:609
#[tokio::test]
async fn does_nothing_when_the_machine_mirrors_the_old_path() {
    let s = scratch();
    let claude = claude_home(&s.root);
    let before = tree(&claude);
    std::fs::create_dir_all(j(&s.root, &["real", "proj"])).unwrap();
    symlink(j(&s.root, &["real"]), j(&s.root, &["link"])).unwrap();
    let report = move_project_state(
        &ProjectStateMove { from: j(&s.root, &["link", "proj"]), to: j(&s.root, &["real", "proj"]), homes: homes(&[("claude", &claude)]) },
        None,
    )
    .await;
    assert!(!report.is_empty());
    assert!(report.iter().all(|r| r.outcome == AgentOutcome::Nothing), "{report:?}");
    assert_eq!(tree(&claude), before);
}

// TS: packages/engine/test/project-state.test.ts:620
#[tokio::test]
async fn reports_one_agents_failure_and_still_moves_the_others() {
    let s = scratch();
    let claude = claude_home(&s.root);
    let hermes = j(&s.root, &["hermes"]);
    write(&j(&hermes, &["state.db"]), "not a database\n");
    let report = move_project_state(
        &ProjectStateMove { from: FROM.into(), to: TO.into(), homes: homes(&[("claude", &claude), ("hermes", &hermes)]) },
        None,
    )
    .await;
    match &report.iter().find(|r| r.agent == "hermes").unwrap().outcome {
        AgentOutcome::Failed(error) => assert!(error.contains("not a database"), "{error}"),
        other => panic!("hermes: {other:?}"),
    }
    assert_eq!(label(&report.iter().find(|r| r.agent == "claude").unwrap().outcome), "moved");
}
