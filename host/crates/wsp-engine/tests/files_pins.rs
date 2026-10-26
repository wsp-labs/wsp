// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests for files_land, files_ledger and files_outside: each script and parser against what the TS printed
//! (tests/fixtures/oracle/provision.json, written by oracle/provision.oracle.ts), and the empty inputs.
mod common;

use indexmap::IndexMap;
use serde_json::Value;

use wsp_engine::provision_files::*;

fn s(v: &Value) -> String {
    v.as_str().unwrap().to_string()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(s).collect()
}

fn roots() -> StoreRoots {
    vec![(".codex".into(), "/var/lib/wsp/logins/codex".into()), (".odd'dir".into(), "/srv/store with space".into())]
}

fn once() -> Vec<String> {
    vec![".claude-cfg/.claude.json".into(), ".codex/config.toml".into(), "odd*glob\\path".into()]
}

// --- files_land ---------------------------------------------------------------------------------------------------

#[test]
fn the_standing_read_is_the_ts_text_byte_for_byte_with_a_quote_in_the_home() {
    let o = common::oracle("provision.json");
    assert_eq!(standing_script("/root"), s(&o["standingScript"]["/root"]));
    assert_eq!(standing_script("/home/dev o'q"), s(&o["standingScript"]["/home/dev o'q"]));
}

#[test]
fn the_landing_is_the_ts_text_byte_for_byte_with_and_without_once_paths_and_stores() {
    let o = common::oracle("provision.json");
    assert_eq!(land_files_script("/root", &[], &vec![]), s(&o["landFilesScript"]["plain"]));
    // A once path holding a glob character and a backslash, and a store folder holding a quote and a space.
    assert_eq!(land_files_script("/root", &once(), &roots()), s(&o["landFilesScript"]["onceAndRoots"]));
}

#[test]
fn the_standing_records_keep_tabs_in_a_path_skip_short_records_and_let_a_later_record_win() {
    let o = common::oracle("provision.json");
    let got = parse_standing(&s(&o["parseStanding"]["input"]));
    let pairs = |v: &Value| v.as_array().unwrap().iter().map(|p| (s(&p[0]), s(&p[1]))).collect::<Vec<_>>();
    assert_eq!(got.at.into_iter().collect::<Vec<_>>(), pairs(&o["parseStanding"]["at"]));
    assert_eq!(got.listed.into_iter().collect::<Vec<_>>(), pairs(&o["parseStanding"]["listed"]));
}

#[test]
fn nothing_printed_is_nothing_standing() {
    assert_eq!(parse_standing(""), Standing::default());
    assert_eq!(parse_standing("\0\0"), Standing::default());
}

#[test]
fn the_landing_lines_need_the_mark_four_fields_and_a_known_outcome() {
    let o = common::oracle("provision.json");
    let got: Vec<(String, LandOutcome)> = parse_landed(&s(&o["parseLanded"]["input"])).into_iter().map(|l| (l.rel, l.outcome)).collect();
    let want: Vec<(String, LandOutcome)> = o["parseLanded"]["out"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            let outcome = match l["outcome"].as_str().unwrap() {
                "installed" => LandOutcome::Installed,
                "present" => LandOutcome::Present,
                "kept" => LandOutcome::Kept,
                _ => LandOutcome::Failed,
            };
            (s(&l["rel"]), outcome)
        })
        .collect();
    assert_eq!(got, want);
    // The TS keeps a line whose path is empty: `wsp-land\tfailed\td\t` is a failed row at "".
    assert!(got.iter().any(|(rel, o)| rel.is_empty() && *o == LandOutcome::Failed));
    assert!(parse_landed("").is_empty());
}

#[test]
fn the_rows_name_three_paths_then_how_many_more_put_failed_first_and_read_a_moved_store() {
    let o = common::oracle("provision.json");
    let lands: Vec<ProvisionLanding> = o["filesRows"]["lands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| ProvisionLanding {
            id: s(&l["id"]),
            label: s(&l["label"]),
            dest: s(&l["dest"]),
            once: l["once"].as_bool().unwrap_or(false),
        })
        .collect();
    let landed = parse_landed(
        &o["filesRows"]["landed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| format!("wsp-land\t{}\td\t{}", s(&l["outcome"]), s(&l["rel"])))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let stores: IndexMap<String, String> = serde_json::from_value(o["storeRoots"]["stores"].clone()).unwrap();
    let rows = files_rows(&lands, &landed, "/root", &store_roots("/root", &stores));
    assert_eq!(serde_json::to_value(&rows).unwrap(), o["filesRows"]["rows"]);
}

#[test]
fn no_planned_path_and_nothing_landed_is_no_row() {
    assert!(files_rows(&[], &[], "/root", &vec![]).is_empty());
}

#[test]
fn a_store_moves_only_its_own_folder_and_a_store_at_the_home_folder_moves_nothing() {
    let o = common::oracle("provision.json");
    let stores: IndexMap<String, String> = serde_json::from_value(o["storeRoots"]["stores"].clone()).unwrap();
    let got = store_roots("/root", &stores);
    let want: StoreRoots = o["storeRoots"]["roots"].as_array().unwrap().iter().map(|p| (s(&p[0]), s(&p[1]))).collect();
    assert_eq!(got, want);
    for pair in o["storeRoots"]["landedAt"].as_array().unwrap() {
        assert_eq!(landed_at("/root", &got, &s(&pair[0])), s(&pair[1]), "{}", pair[0]);
    }
    assert!(store_roots("/root", &IndexMap::new()).is_empty());
}

#[test]
fn an_agent_file_is_its_own_rows_path_and_never_a_lookalike_folder() {
    let o = common::oracle("provision.json");
    for case in o["agentStateFile"].as_array().unwrap() {
        assert_eq!(agent_state_file(&s(&case[0]), &s(&case[1]), "/root").unwrap(), case[2].as_bool().unwrap(), "{case}");
    }
}

#[test]
fn the_merged_line_says_none_or_names_each_server() {
    let o = common::oracle("provision.json");
    assert_eq!(merged_into_line(&[]), s(&o["mergedIntoLine"]["none"]));
    assert_eq!(merged_into_line(&["a".into(), "b".into()]), s(&o["mergedIntoLine"]["two"]));
}

#[test]
fn a_path_lands_once_under_a_once_folder_and_not_beside_it() {
    let once = vec![".codex".to_string()];
    assert!(lands_once(".codex", &once));
    assert!(lands_once(".codex/config.toml", &once));
    assert!(!lands_once(".codexx/config.toml", &once));
    assert!(!lands_once(".codex", &[]));
}

// --- files_ledger -------------------------------------------------------------------------------------------------

#[test]
fn the_list_readers_and_the_close_are_the_ts_text_byte_for_byte() {
    let o = common::oracle("provision.json");
    assert_eq!(landed_servers_script("/root"), s(&o["landedServersScript"]));
    assert_eq!(landed_files_script("/root"), s(&o["landedFilesScript"]["/root"]));
    assert_eq!(landed_files_script("{home}"), s(&o["landedFilesScript"]["{home}"]));
    assert_eq!(close_files_script("/root", &[], &vec![]), s(&o["closeFilesScript"]["plain"]));
    assert_eq!(close_files_script("/root", &once(), &roots()), s(&o["closeFilesScript"]["onceAndRoots"]));
}

#[test]
fn the_unland_is_the_ts_text_byte_for_byte_and_never_sweeps_a_store_or_an_agents_own_folder() {
    let o = common::oracle("provision.json");
    assert_eq!(unland_script("/root", &[".claude/skills/a".into()], &vec![]), s(&o["unlandScript"]["oneDest"]));
    let dests: Vec<String> = vec![".claude/skills/a".into(), ".codex/prompts".into(), ".codex".into(), ".odd'dir/x y".into()];
    let got = unland_script("/root", &dests, &roots());
    assert_eq!(got, s(&o["unlandScript"]["withRoots"]));
    // `.codex` itself is the agent's folder moved to a store: no find -delete is written for it.
    assert!(!got.contains("find '/var/lib/wsp/logins/codex' -depth"));
}

#[test]
fn the_key_lines_need_three_fields_keep_tabs_in_an_id_and_let_a_later_line_win_in_the_first_place() {
    let o = common::oracle("provision.json");
    let got: Vec<(String, String)> = parse_landed_servers(&s(&o["parseLandedServers"]["input"])).into_iter().collect();
    let want: Vec<(String, String)> = o["parseLandedServers"]["out"].as_array().unwrap().iter().map(|p| (s(&p[0]), s(&p[1]))).collect();
    assert_eq!(got, want);
    assert!(parse_landed_servers("").is_empty());
}

#[test]
fn the_ownership_marks_drop_empty_absolute_and_climbing_paths() {
    let o = common::oracle("provision.json");
    assert_eq!(own_marks(&s(&o["ownMarks"]["input"])), strings(&o["ownMarks"]["out"]));
    assert!(own_marks("").is_empty());
}

#[test]
fn a_server_digest_is_the_sha256_of_the_entrys_utf8_and_none_for_no_entry() {
    let o = common::oracle("provision.json");
    for (text, want) in o["serverDigest"].as_object().unwrap() {
        assert_eq!(server_digest(Some(text)), Some(s(want)), "{text}");
    }
    assert_eq!(server_digest(None), None);
}

#[test]
fn a_leave_line_names_what_came_out_or_why_the_file_was_left_unread() {
    let o = common::oracle("provision.json");
    assert_eq!(
        servers_out_lines(&ServersOut::Took { path: "/root/.claude.json".into(), names: vec!["a".into(), "b".into()] }),
        strings(&o["serversOutLines"]["took"])
    );
    assert_eq!(
        servers_out_lines(&ServersOut::Unread { path: "/root/.claude.json".into(), unread: "it is a link".into() }),
        strings(&o["serversOutLines"]["unread"])
    );
}

#[tokio::test]
async fn no_dests_asks_the_computer_nothing_and_takes_nothing() {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let machine = common::FakeMachine::recording(seen.clone());
    assert_eq!(unland_files(&machine, "/root", &[], &IndexMap::new()).await, Some(Unlanded::default()));
    append_landing(&machine, "/root", &[]).await;
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_unland_that_did_not_run_answers_nothing_rather_than_an_empty_take() {
    let machine = common::FakeMachine::new(std::sync::Arc::new(|_, _| Box::pin(async { Ok(common::res(1, "wsp-unland\tgone\ta", "")) })));
    assert_eq!(unland_files(&machine, "/root", &["a".into()], &IndexMap::new()).await, None);
    let threw = common::FakeMachine::new(std::sync::Arc::new(|_, _| Box::pin(async { Err(common::other("spoo is not connected")) })));
    assert_eq!(unland_files(&threw, "/root", &["a".into()], &IndexMap::new()).await, None);
}

#[tokio::test]
async fn the_unland_answer_is_sorted_whatever_order_the_file_system_listed() {
    let out = "wsp-unland\tkept\tz/b\nwsp-unland\tgone\tz/a\nwsp-unland\tgone\tB\nnoise\nwsp-unland\tkept\ta\tb\nwsp-unland\tother\tq\n";
    let machine = common::FakeMachine::new(std::sync::Arc::new(move |_, _| Box::pin(async move { Ok(common::res(0, out, "")) })));
    let got = unland_files(&machine, "/root", &["z".into()], &IndexMap::new()).await.unwrap();
    assert_eq!(got, Unlanded { gone: vec!["B".into(), "z/a".into()], kept: vec!["a\tb".into(), "z/b".into()] });
}

// --- files_outside ------------------------------------------------------------------------------------------------

#[test]
fn the_record_and_the_leave_outside_the_home_are_the_ts_text_byte_for_byte() {
    let o = common::oracle("provision.json");
    let walked: Vec<String> = vec!["/usr/local/bin".into(), "/opt".into(), "/usr/local/lib/node_modules".into()];
    let x = &o["outside"];
    assert_eq!(outside_before_script("clis", &walked, ""), s(&x["before"]));
    assert_eq!(outside_before_script("floor", &walked, "/tmp/scratch root"), s(&x["beforeRooted"]));
    assert_eq!(outside_after_script("clis", &walked, ""), s(&x["after"]));
    assert_eq!(outside_after_script("floor", &walked, "/tmp/scratch root"), s(&x["afterRooted"]));
    assert_eq!(outside_sweep_script(""), s(&x["sweep"]));
    assert_eq!(outside_sweep_script("{root}"), s(&x["sweepTemplate"]));
}

#[test]
fn a_step_that_walks_nothing_records_nothing() {
    let o = common::oracle("provision.json");
    assert_eq!(outside_before_script("clis", &[], ""), s(&o["outside"]["beforeNone"]));
    assert_eq!(outside_after_script("clis", &[], ""), s(&o["outside"]["afterNone"]));
    assert_eq!(outside_before_script("clis", &[], ""), "exit 0");
}

#[test]
fn the_outside_marks_are_absolute_paths_and_keep_a_tab_in_one() {
    let o = common::oracle("provision.json");
    assert_eq!(outside_marks(&s(&o["outsideMarks"]["input"])), strings(&o["outsideMarks"]["out"]));
    assert!(outside_marks("").is_empty());
}
