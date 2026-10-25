// SPDX-License-Identifier: AGPL-3.0-only
//! The small views functions held to the TS answers for the same inputs, edge cases among them: golden-image rows,
//! a place's add job and rows, sessions, sub-thread rows, a workspace's agents, the host env a turn reads, and the
//! shell quoting every command line goes through. spec/views-small.md lists the rule each test pins.

#[path = "rules/oracle.rs"]
mod oracle;

use oracle::{cases, of, opt_str, text};
use serde_json::{json, Value};
use std::collections::HashMap;
use wsp_wire::types::shell_quote::{in_folder, shell_line, shell_quote};
use wsp_wire::views::golden_image::*;
use wsp_wire::views::place::*;
use wsp_wire::views::session::*;
use wsp_wire::views::session_events::*;
use wsp_wire::views::sub_row::*;
use wsp_wire::views::workspace::*;

fn env_of(v: &Value) -> HashMap<String, String> {
    v.as_object().expect("env object").iter().map(|(k, v)| (k.clone(), text(v))).collect()
}

mod golden_image_rows {
    use super::*;

    #[test]
    fn an_npm_bin_goes_between_the_agents_bin_and_the_path_and_none_leaves_the_path() {
        for (a, want) in cases("on_npm_bin") {
            assert_eq!(on_npm_bin(a[0].as_str(), &text(&a[1])), text(&want), "{a:?}");
        }
    }

    #[test]
    fn an_agents_row_names_its_agent_and_an_mcp_row_or_any_other_names_none() {
        for (a, want) in cases("agent_of_row") {
            assert_eq!(agent_of_row(&text(&a[0])), opt_str(&want), "{a:?}");
        }
    }

    #[test]
    fn a_tools_row_s_package_is_everything_after_its_manager_slashes_kept() {
        for (a, want) in cases("package_of") {
            assert_eq!(package_of(&text(&a[0])), text(&want), "{a:?}");
        }
        for (a, want) in cases("tool_row_id") {
            assert_eq!(tool_row_id(&text(&a[0]), &text(&a[1])), text(&want), "{a:?}");
            assert_eq!(tool_row_prefix(&text(&a[0])), format!("tools/{}/", text(&a[0])));
        }
    }

    #[test]
    fn a_command_check_quotes_the_binary_for_sh() {
        for (a, want) in cases("command_check") {
            assert_eq!(command_check(&text(&a[0])), text(&want), "{a:?}");
        }
    }

    #[test]
    fn pins_are_one_per_tick_that_has_one_sorted_by_id_as_locale_compare_orders_them() {
        // The ids mix case, an accent, an underscore and digits, where localeCompare and a byte sort disagree.
        for (a, want) in cases("recipe_pins") {
            let digest: RecipeDigest = of(&a[0]);
            let pins = if a[1].is_null() {
                recipe_pins(&digest, None)
            } else {
                let key = |id: &str| format!("p/{id}");
                recipe_pins(&digest, Some(&key))
            };
            assert_eq!(serde_json::to_value(&pins).unwrap(), want, "{a:?}");
            let ids: Vec<&str> = pins.iter().map(|p| p.id.as_str()).collect();
            let want_ids: Vec<&str> = want.as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap()).collect();
            assert_eq!(ids, want_ids, "pin order");
        }
    }

    #[test]
    fn the_head_is_the_version_it_names_and_nothing_without_a_manifest_or_that_version() {
        for (a, want) in cases("golden_head") {
            let manifest: Option<GoldenManifest> = if a[0].is_null() { None } else { Some(of(&a[0])) };
            let head = golden_head(manifest.as_ref());
            assert_eq!(head.map(|v| serde_json::to_value(v).unwrap()).unwrap_or(Value::Null), want, "{a:?}");
        }
    }

    #[test]
    fn a_version_boots_from_its_template_with_no_mark_else_from_its_snapshot_marked_volatile() {
        for (a, want) in cases("golden_image") {
            let v: GoldenVersion = of(&a[0]);
            let image = golden_image(&v);
            match &want["spec"] {
                s if s.get("template").is_some() => {
                    assert_eq!(image.spec, ImageSpec::Template(text(&s["template"])));
                    assert!(image.marks.is_empty());
                }
                s => {
                    assert_eq!(image.spec, ImageSpec::FromSnapshot(text(&s["fromSnapshot"])));
                    assert_eq!(image.marks, vec!["volatile"]);
                }
            }
        }
    }

    #[test]
    fn holds_nothing_only_with_no_head_and_no_workspaces() {
        let manifest: GoldenManifest = of(
            &json!({ "head": 1, "versions": [{ "version": 1, "snapshotId": "s", "baseTemplate": "b", "setupSha": "x", "createdAt": "t", "smoke": { "cmd": "true", "exitCode": 0 } }] }),
        );
        assert!(holds_nothing(None, 0));
        assert!(!holds_nothing(None, 1));
        assert!(!holds_nothing(Some(&manifest), 0));
    }

    #[test]
    fn custom_rows_are_the_recipe_s_own_or_none() {
        let without: Recipe = of(&json!({ "version": 1, "at": "t", "histories": [], "rows": [] }));
        assert!(custom_rows(&without).is_empty());
    }
}

mod a_place_s_add_and_rows {
    use super::*;

    #[test]
    fn a_stage_line_replaces_its_step_in_place_or_lands_last_and_ends_the_job_on_join_or_failure() {
        for (a, want) in cases("with_place_stage") {
            let job: PlaceAddJob = of(&a[0]);
            let e: PlaceStageEvent = of(&a[1]);
            let next = with_place_stage(&job, &e);
            assert_eq!(serde_json::to_string(&next).unwrap(), serde_json::to_string(&want).unwrap(), "{a:?}");
        }
    }

    #[test]
    fn a_sign_in_row_names_its_agent_and_the_github_row_names_gh() {
        for (a, want) in cases("sign_in_of_row") {
            assert_eq!(sign_in_of_row(&text(&a[0])), opt_str(&want), "{a:?}");
        }
        assert_eq!(sign_in_row_id("codex"), "signins/codex");
    }

    #[test]
    fn rows_count_by_kind_in_the_order_each_kind_is_first_met_an_unkinded_row_a_tool() {
        for (a, want) in cases("provision_counts") {
            let rows: Vec<PlaceProvisionRow> = of(&a[0]);
            let counts = provision_counts(&rows);
            assert_eq!(serde_json::to_string(&counts).unwrap(), serde_json::to_string(&want).unwrap(), "{a:?}");
        }
    }

    #[test]
    fn a_mac_s_kind_reads_off_its_model_name_spaces_and_case_aside() {
        for (a, want) in cases("mac_kind_of") {
            let got = mac_kind_of(&text(&a[0])).map(|k| serde_json::to_value(k).unwrap()).unwrap_or(Value::Null);
            assert_eq!(got, want, "{a:?}");
        }
    }

    #[test]
    fn a_place_away_with_forks_or_projects_is_only_forgotten() {
        for (a, want) in cases("place_forgets_only") {
            let holds: PlaceHolds = of(&a[0]);
            assert_eq!(place_forgets_only(&holds), want.as_bool().unwrap(), "{a:?}");
        }
    }
}

mod sessions {
    use super::*;

    #[test]
    fn a_turn_s_thread_key_is_its_thread_id_else_its_own_id() {
        for (a, want) in cases("thread_key_of") {
            let s: SessionView = of(&a[0]);
            assert_eq!(thread_key_of(&s), text(&want), "{a:?}");
        }
    }

    #[test]
    fn a_thread_ran_once_a_turn_runs_or_kept_a_session_without_a_refusal() {
        for (a, want) in cases("thread_ran") {
            let turns: Vec<SessionView> = of(&a[0]);
            assert_eq!(thread_ran(&turns), want.as_bool().unwrap(), "{a:?}");
        }
    }

    #[test]
    fn the_host_a_turn_reaches_is_read_trimmed_as_javascript_trims_and_needs_url_and_token() {
        // One case trims U+FEFF and U+00A0, which JavaScript's trim takes and Rust's str::trim does not all of.
        for (a, want) in cases("host_from_env") {
            let got = host_from_env(&env_of(&a[0]));
            let want = if want.is_null() {
                None
            } else {
                Some(HostFromEnv { url: text(&want["url"]), token: text(&want["token"]), host_key: opt_str(&want["hostKey"]) })
            };
            assert_eq!(got, want, "{a:?}");
        }
    }

    #[test]
    fn the_turn_token_is_read_as_it_is_and_an_empty_one_is_none() {
        for (a, want) in cases("turn_token_of") {
            assert_eq!(turn_token_of(&env_of(&a[0])), opt_str(&want), "{a:?}");
        }
    }

    #[test]
    fn a_session_event_is_one_of_the_listed_types_and_nothing_else() {
        for (a, want) in cases("is_session_event") {
            assert_eq!(is_session_event(&text(&a[0])), want.as_bool().unwrap(), "{a:?}");
        }
    }

    #[test]
    fn the_starting_line_and_the_wsp_bundle_paths_read_as_the_ts_writes_them() {
        for (a, want) in cases("agent_starting_line") {
            assert_eq!(agent_starting_line(&text(&a[0]), a[1].as_bool().unwrap()), text(&want), "{a:?}");
        }
        for (a, want) in cases("wsp_package_in") {
            assert_eq!(wsp_package_in(&text(&a[0])), text(&want), "{a:?}");
        }
        for (a, want) in cases("wsp_bin_in") {
            assert_eq!(wsp_bin_in(&text(&a[0])), text(&want), "{a:?}");
        }
    }
}

mod sub_thread_rows {
    use super::*;

    #[test]
    fn a_start_is_under_way_until_it_stands_or_fails() {
        for (a, want) in cases("sub_spawning") {
            let spawn: Option<SubSpawn> = if a[0].is_null() { None } else { Some(of(&a[0])) };
            assert_eq!(sub_spawning(spawn.as_ref()), want.as_bool().unwrap(), "{a:?}");
        }
    }

    #[test]
    fn all_is_pushed_only_with_nothing_unpushed_nothing_changed_and_a_read_that_answered() {
        // An empty readFailed is still a read that failed: only its absence counts as answered.
        for (a, want) in cases("sub_all_pushed") {
            let r: SubResult = of(&a[0]);
            assert_eq!(sub_all_pushed(&r), want.as_bool().unwrap(), "{a:?}");
        }
    }

    #[test]
    fn a_turn_is_gone_with_its_machine_when_it_started_before_the_release_or_its_start_is_unknown() {
        for (a, want) in cases("sub_turn_gone") {
            assert_eq!(sub_turn_gone(a[0].as_f64(), a[1].as_f64()), want.as_bool().unwrap(), "{a:?}");
        }
    }
}

mod a_workspace_s_agents {
    use super::*;

    #[test]
    fn a_patch_lands_over_the_held_setting_or_over_agents_on() {
        for (a, want) in cases("agents_from") {
            let held: Option<WorkspaceAgents> = if a[0].is_null() { None } else { Some(of(&a[0])) };
            let p = &a[1];
            let patch = WorkspaceAgentsPatch {
                spawn: p["spawn"].as_bool(),
                max_machines: p["maxMachines"].as_i64(),
                max_depth: p["maxDepth"].as_i64(),
            };
            let got = agents_from(held.as_ref(), &patch);
            assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(&want).unwrap(), "{a:?}");
        }
    }

    #[test]
    fn a_road_alone_has_no_scope_and_a_scoped_caller_has_both() {
        let scope: ThreadScope = of(&json!({ "kind": "thread", "threadId": "t", "workspaceId": "w", "rootThreadId": "r" }));
        assert_eq!(road_of(None), None);
        assert_eq!(scope_of(None), None);
        assert_eq!(road_of(Some(&Caller::Road(WorkspaceOrigin::Paired))), Some(WorkspaceOrigin::Paired));
        assert_eq!(scope_of(Some(&Caller::Road(WorkspaceOrigin::Paired))), None);
        let scoped = Caller::Scoped { origin: WorkspaceOrigin::Here, by: scope.clone() };
        assert_eq!(road_of(Some(&scoped)), Some(WorkspaceOrigin::Here));
        assert_eq!(scope_of(Some(&scoped)), Some(scope));
    }
}

mod shell_quoting {
    use super::*;

    #[test]
    fn one_word_is_single_quoted_with_each_quote_closed_escaped_and_reopened() {
        for (a, want) in cases("shell_quote") {
            assert_eq!(shell_quote(&text(&a[0])), text(&want), "{a:?}");
        }
    }

    #[test]
    fn a_line_leaves_bare_what_sh_reads_as_itself_and_quotes_the_rest_the_empty_word_too() {
        for (a, want) in cases("shell_line") {
            let words: Vec<String> = of(&a[0]);
            let refs: Vec<&str> = words.iter().map(String::as_str).collect();
            assert_eq!(shell_line(&refs), text(&want), "{a:?}");
        }
    }

    #[test]
    fn a_command_in_a_folder_cds_into_it_quoted_or_into_the_home_when_none_is_named() {
        for (a, want) in cases("in_folder") {
            assert_eq!(in_folder(a[0].as_str(), &text(&a[1])), text(&want), "{a:?}");
        }
    }
}
