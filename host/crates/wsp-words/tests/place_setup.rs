// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/places.test.ts:170, :177, :232, :247, :252, :257, :367-392 (workspacePlaceId),
//! :448, :474-562 (the setup on a computer you own), machine-link.test.ts:152, sign-in-ways.test.ts:1-40 and
//! place-state.test.ts:175-271 (the word a place's row says). The WorkspaceKind.safeParse assertion of :170 is a zod
//! parse, wsp-wire's, and is left out with a comment.

mod support;

use serde_json::{json, Value};
use support::{over, place_view, workspace_view};
use wsp_wire::views::place::{PendingComputer, PlaceApplied, PlaceProvisionRow, PlaceSetup, PlaceSetupStep, PlaceView};
use wsp_wire::views::workspace::WorkspaceView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_wire::wire::machine_link::PLACE_BLOCKED_WORD;
use wsp_words::app_ports::join_address_of;
use wsp_words::place_state::{place_spend_limit, workspace_place_id};
use wsp_words::project_path::{place_daemon_paths, place_provision_paths, work_folder_in, PlaceProvisionPaths};
use wsp_words::projects::copies_folder;
use wsp_words::views::place::provision_count_word;
use wsp_words::views::place_setup_words::{
    pending_word, place_no_home_line, place_no_picks_line, place_provisioning_line, place_state_of, place_word, setup_lines, setup_width, setup_word, PlaceState,
    PlaceTone, SignInWay, SignInWayFacts, sign_in_there_fix, sign_in_way_label, sign_in_ways_fix,
};
use wsp_words::words::computer::{spend_cap_refusal, SPEND_LIMIT_LINE};
use wsp_words::workspace_state::{absent_computer, kind_words};

fn wire<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn place(fields: Value) -> PlaceView {
    wire(over(place_view(), fields))
}

fn workspace(fields: Value) -> WorkspaceView {
    wire(over(workspace_view(), fields))
}

fn said(word: &str, sentence: Option<&str>) -> PlaceState {
    PlaceState { word: word.to_string(), tone: None, sentence: sentence.map(str::to_string) }
}

mod places {
    use super::*;

    mod the_kinds_a_workspace_can_be {
        use super::*;

        #[test]
        fn holds_a_kind_for_a_folder_on_the_computer_a_place_is_whose_threads_run_in_it_as() {
            // WorkspaceKind.safeParse("place").success is a zod parse: wsp-wire's.
            let place_kind: WorkspaceKind = wire(json!("place"));
            let words = kind_words(&place_kind);
            assert!(words.in_folder);
            assert!(!words.copies_folder);
            assert!(!words.driven);
            assert!(!copies_folder(&place_kind));
        }
    }

    mod where_a_daemon_on_somebody_s_own_computer_keeps_things {
        use super::*;

        #[test]
        fn names_the_work_folder_by_the_one_rule_this_computer_s_own_workspace_reads() {
            assert_eq!(work_folder_in("/Users/maya/"), "/Users/maya/wsp-work");
            assert_eq!(place_daemon_paths("/Users/maya").token_path, "/Users/maya/.wsp/daemon-token");
        }
    }

    mod where_a_computer_joined_as_a_place_keeps_its_own_two_files {
        use super::*;

        #[test]
        fn puts_them_in_the_folder_the_daemon_s_own_files_are_in_so_one_sweep_takes_the_lot() {
            let at = place_daemon_paths("/home/maya");
            assert_eq!(at.place_file, "/home/maya/.wsp/place.json");
            assert_eq!(at.place_key, "/home/maya/.wsp/place-key.pem");
            assert_eq!(at.place_log, "/home/maya/.wsp/place.log");
            for path in [&at.place_file, &at.place_key, &at.place_log] {
                assert!(path.starts_with(&format!("{}/", at.wsp)), "{path}");
            }
        }
    }

    mod the_address_a_person_types_on_the_join_screen {
        use super::*;

        #[test]
        fn makes_a_bare_host_and_port_into_the_http_address_the_join_road_dials() {
            assert_eq!(join_address_of("192.168.1.20:4420"), Some("http://192.168.1.20:4420".to_string()));
            assert_eq!(join_address_of("  old-macbook.local:4420 "), Some("http://old-macbook.local:4420".to_string()));
        }

        #[test]
        fn leaves_an_address_that_already_carries_a_scheme_alone() {
            assert_eq!(join_address_of("https://p_x.usewsp.com"), Some("https://p_x.usewsp.com".to_string()));
            assert_eq!(join_address_of("http://192.168.1.20:4420"), Some("http://192.168.1.20:4420".to_string()));
        }

        #[test]
        fn is_nothing_for_a_word_that_names_no_port_and_for_a_scheme_this_road_cannot_dial() {
            assert_eq!(join_address_of("box"), None);
            assert_eq!(join_address_of(""), None);
            assert_eq!(join_address_of("ws://x"), None);
        }
    }

    mod which_row_of_the_places_list_a_workspace_stands_on {
        use super::*;

        fn here() -> PlaceView {
            place(json!({ "id": "here", "kind": "computer" }))
        }
        fn laptop() -> PlaceView {
            place(json!({ "id": "p_1", "kind": "computer" }))
        }
        fn ascii() -> PlaceView {
            place(json!({ "id": "box", "kind": "provider" }))
        }
        fn solari() -> PlaceView {
            place(json!({ "id": "solari", "kind": "provider" }))
        }
        fn places() -> Vec<PlaceView> {
            vec![here(), laptop(), ascii(), solari()]
        }

        #[test]
        fn takes_the_computer_a_fork_s_record_names() {
            let w = workspace(json!({ "kind": "cloud", "machineId": "m1", "place": "p_1" }));
            assert_eq!(workspace_place_id(&w, &places()), Some("p_1".to_string()));
        }

        #[test]
        fn puts_this_computer_s_own_workspace_on_the_first_row_which_is_the_computer_the_ho() {
            let w = workspace(json!({ "kind": "local", "machineId": "local" }));
            assert_eq!(workspace_place_id(&w, &places()), Some("here".to_string()));
        }

        #[test]
        fn puts_a_fork_on_the_provider_its_record_was_stamped_with_so_two_providers_in_one() {
            let w1 = workspace(json!({ "kind": "cloud", "machineId": "fk_1", "provider": "solari" }));
            assert_eq!(workspace_place_id(&w1, &places()), Some("solari".to_string()));
            let w2 = workspace(json!({ "kind": "cloud", "machineId": "fk_2", "provider": "box" }));
            assert_eq!(workspace_place_id(&w2, &places()), Some("box".to_string()));
        }

        #[test]
        fn stands_a_fork_written_before_records_carried_that_word_at_the_first_provider_whe() {
            let w1 = workspace(json!({ "kind": "cloud", "machineId": "fk_1" }));
            assert_eq!(workspace_place_id(&w1, &places()), Some("box".to_string()));
            let w2 = workspace(json!({ "machineId": "fk_1" }));
            assert_eq!(workspace_place_id(&w2, &places()), Some("box".to_string()));
        }

        #[test]
        fn places_a_fork_stamped_with_a_provider_this_list_does_not_hold_nowhere_so_a_remov() {
            // Removing a provider deletes its workspaces where they stand and keeps their series for the month. Falling
            // back to the first provider would add every one of them to the provider that is left.
            let hetzner = workspace(json!({ "kind": "cloud", "machineId": "fk_1", "provider": "hetzner" }));
            assert_eq!(workspace_place_id(&hetzner, &places()), None);
            let solari = workspace(json!({ "kind": "cloud", "machineId": "fk_1", "provider": "solari" }));
            assert_eq!(workspace_place_id(&solari, &[here(), ascii()]), None);
        }

        #[test]
        fn places_nothing_it_cannot_a_computer_that_has_been_removed_and_a_fork_on_a_list_w() {
            let gone = workspace(json!({ "kind": "cloud", "machineId": "m1", "place": "p_gone" }));
            assert_eq!(workspace_place_id(&gone, &places()), None);
            let none = workspace(json!({ "kind": "cloud", "machineId": "fk_1" }));
            assert_eq!(workspace_place_id(&none, &[here()]), None);
        }
    }

    mod the_setup_on_a_computer_you_own {
        use super::*;

        fn row(fields: Value) -> Value {
            over(json!({ "id": "agents/codex", "label": "Codex", "outcome": "installed", "step": "agents" }), fields)
        }
        fn running() -> Value {
            json!({ "state": "running", "addId": "a_1", "startedAt": "2026-10-03T10:01:00.000Z", "steps": [{ "step": "floor", "state": "done", "ms": 41_000 }, { "step": "agents", "state": "running" }], "waiting": [] })
        }
        fn done() -> Value {
            over(running(), json!({ "state": "done", "finishedAt": "2026-10-03T10:05:00.000Z", "steps": [] }))
        }
        fn applied(rows: Vec<Value>) -> Value {
            json!({ "hash": "h", "at": done()["finishedAt"], "rows": rows })
        }
        fn wait() -> Value {
            json!({ "row": "signins/codex", "label": "Codex", "url": "https://auth.openai.com/codex/device", "code": "ABCD-EFGH", "expiresAt": "2026-10-03T10:15:00.000Z", "state": "waiting" })
        }
        fn setup(v: Value) -> PlaceSetup {
            wire(v)
        }
        fn applied_of(rows: Vec<Value>) -> PlaceApplied {
            wire(applied(rows))
        }
        fn base() -> Value {
            over(place_view(), json!({ "id": "p_1", "kind": "computer", "name": "spoo", "default": false, "daemonVersion": DAEMON_VERSION }))
        }
        fn with(v: Value, fields: Value) -> PlaceView {
            wire(over(v, fields))
        }
        // RecipeFile.parse({ name: "spoo" }) with the defaults zod fills in written out, so the value does not depend on wsp-wire's default reader.
        fn pending_choosing() -> Value {
            json!({ "id": "a_1", "address": "root@spoo", "step": "choosing", "choices": { "name": "spoo", "agents": {}, "mcp": {}, "clis": {}, "skills": {}, "plugins": {}, "folders": {}, "configs": {} }, "startedAt": "2026-10-03T10:00:00.000Z", "placeId": "p_1" })
        }

        #[test]
        fn runs_one_step_after_the_base_tools_per_2_gb_of_the_computer_s_memory_at_least_on() {
            let widths: Vec<f64> = [1024.0, 2047.0, 2048.0, 4096.0, 7700.0, 8192.0, 65536.0].into_iter().map(setup_width).collect();
            assert_eq!(widths, vec![1.0, 1.0, 1.0, 2.0, 3.0, 4.0, 4.0]);
        }

        #[test]
        fn reads_one_word_by_one_precedence_setup_failed_needs_you_offline_setting_up_behin() {
            let away = absent_computer("spoo", None);
            let failed = over(done(), json!({ "state": "failed", "said": "the base tools did not install: curl" }));
            // Each row of the table holds everything below it too, so only precedence decides.
            let p = with(base(), json!({ "setup": over(failed.clone(), json!({ "waiting": [wait()] })) }));
            assert_eq!(place_word(&p, Some(&away), None), said("Setup failed", Some("the base tools did not install: curl")));
            let p = with(base(), json!({ "setup": over(running(), json!({ "waiting": [wait()] })) }));
            assert_eq!(
                place_word(&p, Some(&away), None),
                said("Needs you", Some("Codex waits on you to sign in at https://auth.openai.com/codex/device with code ABCD-EFGH"))
            );
            let p = with(
                base(),
                json!({ "setup": done(), "applied": applied(vec![row(json!({ "id": "folders/wsp", "label": "wsp", "step": "folders", "outcome": "failed", "note": "the clone exited 128" }))]) }),
            );
            assert_eq!(place_word(&p, Some(&away), None), said("Needs you", Some("wsp: the clone exited 128")));
            let p = with(base(), json!({ "setup": running() }));
            assert_eq!(place_word(&p, Some(&away), None), said(away.away.as_str(), Some(&away.sentence)));
            assert_eq!(place_word(&p, None, None), said("Setting up", Some("setting up the agents")));
            let p = with(base(), json!({ "setup": done(), "daemonVersion": DAEMON_VERSION - 1 }));
            assert_eq!(place_word(&p, None, None).word, format!("daemon {}, host {}", DAEMON_VERSION - 1, DAEMON_VERSION));
            // A failed row whose step can be fixed later never reads Needs you: it is a row with Retry.
            let p = with(base(), json!({ "setup": done(), "applied": applied(vec![row(json!({ "id": "skills/unslop", "label": "unslop", "step": "skills", "outcome": "failed" }))]) }));
            assert_eq!(place_word(&p, None, None), said("Ready", None));
            let expired = over(wait(), json!({ "state": "expired" }));
            let p = with(base(), json!({ "setup": over(done(), json!({ "waiting": [expired] })) }));
            assert_eq!(place_word(&p, None, None).sentence.as_deref(), Some("Codex's sign-in ran out; a retry asks for a fresh code"));
        }

        #[test]
        fn reads_a_pending_add_as_pending_with_how_far_it_got_or_setup_failed_with_why() {
            let pending: PendingComputer = wire(pending_choosing());
            assert_eq!(pending_word(&pending), said("Pending", Some("joined; waiting on what goes on it")));
            let failed: PendingComputer = wire(over(pending_choosing(), json!({ "step": "check", "failed": { "said": "spoo runs no systemd" } })));
            assert_eq!(pending_word(&failed), said("Setup failed", Some("spoo runs no systemd")));
        }

        #[test]
        fn reads_a_joined_computer_still_choosing_as_pending_after_needs_you_and_before_off() {
            let choosing_json = over(pending_choosing(), json!({ "startedAt": "x" }));
            let choosing: PendingComputer = wire(choosing_json.clone());
            let p = with(base(), json!({}));
            assert_eq!(place_word(&p, None, Some(&choosing)), said("Pending", Some("joined; waiting on what goes on it")));
            assert_eq!(place_word(&p, Some(&absent_computer("spoo", None)), Some(&choosing)).word, "Pending");
            let waiting = with(base(), json!({ "setup": over(running(), json!({ "waiting": [wait()] })) }));
            assert_eq!(place_word(&waiting, None, Some(&choosing)).word, "Needs you");
            let failed: PendingComputer = wire(over(choosing_json, json!({ "failed": { "said": "the base tools did not install: curl" } })));
            assert_eq!(place_word(&p, None, Some(&failed)), said("Setup failed", Some("the base tools did not install: curl")));
        }

        #[test]
        fn says_on_the_tools_cell_what_is_under_way_where_it_stopped_or_what_stands() {
            assert_eq!(setup_word(None, None), "");
            assert_eq!(setup_word(Some(&setup(running())), None), "setting up the agents");
            assert_eq!(setup_word(Some(&setup(over(done(), json!({ "state": "failed", "said": "spoo is not connected" })))), None), "stopped: spoo is not connected");
            let ready = applied_of(vec![
                row(json!({})),
                row(json!({ "id": "files/.claude/skills/x", "label": "x", "kind": "file" })),
                row(json!({ "id": "agents/mcp/claude/github", "label": "Claude Code github", "kind": "server", "outcome": "present" })),
            ]);
            assert_eq!(setup_word(Some(&setup(done())), Some(&ready)), "1 tool, 1 file, 1 MCP server ready");
            let failed = applied_of(vec![row(json!({})), row(json!({ "id": "tools/uv/uv", "label": "uv", "outcome": "failed" }))]);
            assert_eq!(setup_word(Some(&setup(done())), Some(&failed)), "1 of 2 failed: uv");
            assert_eq!(provision_count_word(&indexmap::IndexMap::new()), "0 tools");
        }

        #[test]
        fn prints_what_installed_by_name_what_was_already_there_every_row_that_did_not_land() {
            let lines = setup_lines(
                "spoo",
                &setup(over(done(), json!({ "waiting": [wait()] }))),
                Some(&applied_of(vec![
                    row(json!({})),
                    row(json!({ "id": "tools/uv/ruff", "label": "ruff", "outcome": "present" })),
                    row(json!({ "id": "tools/release/gh", "label": "GitHub CLI", "outcome": "failed", "note": "no Linux build" })),
                    row(json!({ "id": "plugins/x@y", "label": "x@y", "outcome": "skipped", "note": "this computer names no marketplace for it" })),
                ])),
                None,
            );
            assert_eq!(
                lines,
                vec![
                    "spoo: 1 installed: Codex, 1 already there",
                    "  x GitHub CLI: no Linux build",
                    "  - x@y: this computer names no marketplace for it",
                    "  ? Codex waits on you to sign in at https://auth.openai.com/codex/device with code ABCD-EFGH",
                ]
            );
            let stopped = setup_lines("spoo", &setup(over(done(), json!({ "state": "failed", "said": "spoo is not connected" }))), Some(&applied_of(vec![])), None);
            assert_eq!(stopped.last().map(String::as_str), Some("spoo: spoo is not connected"));
        }

        #[test]
        fn prints_under_a_row_that_did_not_land_what_to_do_about_it_then_the_command_line_s() {
            let failed = row(json!({ "id": "signins/codex", "label": "Codex", "outcome": "failed", "note": "zingzy's MacBook Pro has no Codex sign-in to copy", "fix": "Sign in on spoo instead." }));
            let skipped = row(json!({ "id": "github", "label": "GitHub", "outcome": "skipped", "note": "skipped; gh is not signed in there" }));
            let command = |r: &PlaceProvisionRow| -> Option<String> {
                if r.id == "signins/codex" {
                    Some("wsp add spoo --sign-in codex".to_string())
                } else if r.id == "github" {
                    Some("wsp add spoo --sign-in gh".to_string())
                } else {
                    None
                }
            };
            let lines = setup_lines("spoo", &setup(done()), Some(&applied_of(vec![failed, skipped])), Some(&command));
            assert_eq!(
                lines[1..].to_vec(),
                vec![
                    "  x Codex: zingzy's MacBook Pro has no Codex sign-in to copy",
                    "    Sign in on spoo instead.",
                    "    wsp add spoo --sign-in codex",
                    "  - GitHub: skipped; gh is not signed in there",
                    "    wsp add spoo --sign-in gh",
                ]
            );
        }

        #[test]
        fn says_in_one_sentence_why_a_workspace_cannot_be_made_there_yet_naming_the_step_it() {
            assert_eq!(
                place_provisioning_line("spoo", Some(&PlaceSetupStep::Clis)),
                "spoo is still being set up (the CLIs); wsp computers shows it, and a workspace there can be made once it is done"
            );
            assert_eq!(
                place_provisioning_line("spoo", None),
                "spoo is still being set up; wsp computers shows it, and a workspace there can be made once it is done"
            );
        }

        #[test]
        fn says_a_computer_that_reported_no_home_folder_or_holds_no_picks_got_nothing_and_w() {
            assert_eq!(place_no_home_line("spoo"), "spoo got no agents or tools: it reported no home folder for its login, so nothing on it could be reached");
            assert_eq!(
                place_no_picks_line("spoo"),
                "spoo has nothing picked to go on it: choose in the app's Add a computer, or run wsp add spoo --resume --recipe <name>"
            );
        }

        #[test]
        fn keeps_its_log_and_its_outcome_in_the_folder_wsp_already_owns_on_that_computer_ne() {
            let at = place_provision_paths("/root");
            assert_eq!(
                at,
                PlaceProvisionPaths {
                    dir: "/root/.wsp/provision".to_string(),
                    run_dir: "/root/.wsp/provision/run".to_string(),
                    log: "/root/.wsp/provision/log".to_string(),
                    result: "/root/.wsp/provision/result.json".to_string(),
                    staging: "/root/.wsp/provision/files".to_string(),
                    landed: "/root/.wsp/provision/landed".to_string(),
                    landing: "/root/.wsp/provision/landing".to_string(),
                    asked: "/root/.wsp/provision/asked".to_string(),
                }
            );
            let wsp = place_daemon_paths("/root").wsp;
            let values = serde_json::to_value(&at).unwrap();
            for path in values.as_object().unwrap().values() {
                let path = path.as_str().unwrap();
                assert!(path.starts_with(&format!("{wsp}/")), "{path}");
            }
        }
    }
}

mod machine_link {
    use super::*;

    mod the_machine_ops_on_a_place_link {
        use super::*;

        #[test]
        fn keeps_the_parts_of_an_upload_in_wsp_s_own_folder_under_the_login_s_home() {
            assert_eq!(place_daemon_paths("/Users/maya").put_dir, "/Users/maya/.wsp/put");
        }
    }
}

mod sign_in_ways {
    use super::*;

    fn facts() -> SignInWayFacts {
        SignInWayFacts {
            name: "Claude Code".to_string(),
            here: "zingzy's MacBook Pro".to_string(),
            r#box: "hetzner".to_string(),
            mint: Some("claude setup-token".to_string()),
            key_env: Some("ANTHROPIC_API_KEY".to_string()),
            field: None,
        }
    }
    const WAYS: [SignInWay; 3] = [SignInWay::Token, SignInWay::Key, SignInWay::Machine];

    /// Every order of every non-empty set of ways.
    fn orders(left: &[SignInWay]) -> Vec<Vec<SignInWay>> {
        left.iter()
            .flat_map(|way| {
                let rest: Vec<SignInWay> = left.iter().copied().filter(|w| w != way).collect();
                let mut all = vec![vec![*way]];
                all.extend(orders(&rest).into_iter().map(|tail| std::iter::once(*way).chain(tail).collect()));
                all
            })
            .collect()
    }

    /// `fix.split(/(?<=\.) (?=[A-Z])/)`: a space after a full stop and before a capital.
    fn sentences(fix: &str) -> Vec<&str> {
        let bytes = fix.as_bytes();
        let mut out = Vec::new();
        let mut from = 0;
        for i in 1..bytes.len().saturating_sub(1) {
            if bytes[i] == b' ' && bytes[i - 1] == b'.' && bytes[i + 1].is_ascii_uppercase() {
                out.push(&fix[from..i]);
                from = i + 1;
            }
        }
        out.push(&fix[from..]);
        out
    }

    mod a_failed_sign_in_row_s_line_and_its_buttons {
        use super::*;

        #[test]
        fn opens_one_sentence_per_way_with_that_way_s_button_in_the_order_the_buttons_stand() {
            let all = orders(&WAYS);
            assert_eq!(all.len(), 15);
            for ways in all {
                let fix = sign_in_ways_fix(&ways, &facts());
                let heads: Vec<String> = sentences(&fix).into_iter().map(|s| s[..s.find(':').unwrap()].to_string()).collect();
                let labels: Vec<String> = ways.iter().map(|way| sign_in_way_label(way, &facts().r#box)).collect();
                assert_eq!(heads, labels);
            }
        }

        #[test]
        fn says_what_each_button_does_the_command_that_makes_the_token_here_where_the_key_g() {
            assert_eq!(
                sign_in_ways_fix(&WAYS, &facts()),
                "Make a token: claude setup-token runs on zingzy's MacBook Pro and asks your browser once. Use an API key: put ANTHROPIC_API_KEY in ~/.wsp/.env on zingzy's MacBook Pro first, and wsp reads it from there without showing it. Sign in on hetzner: Claude Code's own sign-in runs there."
            );
        }

        #[test]
        fn names_the_sign_in_on_the_box_that_a_one_way_row_draws() {
            let fix = sign_in_there_fix("hetzner");
            let label = sign_in_way_label(&SignInWay::Machine, "hetzner");
            assert!(fix.contains(&label), "{fix}");
        }
    }
}

mod place_state {
    use super::*;

    fn spoo() -> Value {
        over(place_view(), json!({ "id": "p_1", "kind": "computer", "name": "spoo", "default": false, "shape": { "cpu": 2, "memMb": 7885 }, "daemonVersion": DAEMON_VERSION, "cap": { "threads": 2 }, "running": 0 }))
    }
    fn solari() -> Value {
        over(place_view(), json!({ "id": "solari", "kind": "provider", "name": "solari", "default": false, "cap": { "machines": 3, "spendPerDayUsd": 10 }, "running": 0 }))
    }
    fn view(base: Value, fields: Value) -> PlaceView {
        wire(over(base, fields))
    }
    fn without(mut v: Value, key: &str) -> Value {
        v.as_object_mut().unwrap().remove(key);
        v
    }

    mod the_word_a_place_s_row_says {
        use super::*;

        fn running() -> Value {
            json!({ "state": "running", "addId": "a_1", "startedAt": "2026-09-17T10:01:00.000Z", "steps": [{ "step": "agents", "state": "running" }], "waiting": [] })
        }
        fn warning(word: &str, sentence: &str) -> PlaceState {
            PlaceState { word: word.to_string(), tone: Some(PlaceTone::Warning), sentence: Some(sentence.to_string()) }
        }

        #[test]
        fn reads_ready_when_nothing_stands_in_the_way() {
            assert_eq!(place_state_of(&view(spoo(), json!({})), None, None, None), said("Ready", None));
            assert_eq!(place_state_of(&view(solari(), json!({})), None, None, None), said("Ready", None));
        }

        #[test]
        fn reads_full_once_the_count_meets_the_cap_with_the_count_in_the_sentence() {
            assert_eq!(place_state_of(&view(spoo(), json!({ "running": 2 })), None, None, None), warning("Full", "full: 2 of 2 threads running"));
            assert_eq!(place_state_of(&view(solari(), json!({ "running": 3 })), None, None, None), warning("Full", "full: 3 of 3 machines running"));
            let one = view(spoo(), json!({ "cap": { "threads": 1 }, "running": 1 }));
            assert_eq!(place_state_of(&one, None, None, None).sentence.as_deref(), Some("full: 1 of 1 thread running"));
        }

        #[test]
        fn reads_full_for_a_computer_with_no_room_under_its_cap_and_keeps_no_room_in_the_se() {
            let p = view(spoo(), json!({ "running": 1, "forks": { "running": 1, "room": 0 } }));
            assert_eq!(place_state_of(&p, None, None, None), warning("Full", "no room on spoo"));
        }

        #[test]
        fn reads_at_limit_only_with_a_spend_figure_and_never_answers_it_without_one() {
            let spent = view(solari(), json!({ "cap": { "machines": 3, "spendPerDayUsd": 10 } }));
            assert_eq!(place_state_of(&spent, None, None, None).word, "Ready");
            assert_eq!(place_state_of(&spent, None, Some(9.99), None).word, "Ready");
            assert_eq!(place_state_of(&spent, None, Some(10.0), None), warning("At limit", "spend limit reached today"));
            // A $0 limit is a cloud that starts nothing today.
            let zero = view(solari(), json!({ "cap": { "machines": 3, "spendPerDayUsd": 0 } }));
            assert_eq!(place_state_of(&zero, None, Some(0.0), None).word, "At limit");
            // A computer has no spend limit whatever figure is handed in.
            assert_eq!(place_state_of(&view(spoo(), json!({})), None, Some(100.0), None).word, "Ready");
        }

        #[test]
        fn reads_a_spend_limit_only_off_a_cloud_s_cap_and_the_refusal_opens_with_the_row_s() {
            assert_eq!(place_spend_limit(&view(solari(), json!({}))), Some(10.0));
            assert_eq!(place_spend_limit(&view(solari(), json!({ "cap": { "machines": 3, "spendPerDayUsd": 0 } }))), Some(0.0));
            assert_eq!(place_spend_limit(&view(spoo(), json!({}))), None);
            assert_eq!(place_spend_limit(&view(without(solari(), "cap"), json!({}))), None);
            assert_eq!(place_state_of(&view(solari(), json!({})), None, Some(10.0), None).sentence.as_deref(), Some(SPEND_LIMIT_LINE));
            assert_eq!(
                spend_cap_refusal("solari", 10.02, 10.0),
                format!("{SPEND_LIMIT_LINE} on solari ($10.02/$10); raise its spend per day or start the machine after midnight")
            );
        }

        #[test]
        fn puts_blocked_then_not_answering_then_at_limit_then_full_then_the_setup_running_t() {
            let all = over(solari(), json!({ "blocked": "no overlay", "running": 3, "setup": running(), "daemonVersion": DAEMON_VERSION - 1 }));
            let away = absent_computer("solari", None);
            assert_eq!(place_state_of(&view(all.clone(), json!({})), Some(&away), Some(20.0), None), said(PLACE_BLOCKED_WORD, Some("no overlay")));
            let unblocked = without(all, "blocked");
            assert_eq!(place_state_of(&view(unblocked.clone(), json!({})), Some(&away), Some(20.0), None), said(away.away.as_str(), Some(&away.sentence)));
            assert_eq!(place_state_of(&view(unblocked.clone(), json!({})), None, Some(20.0), None).word, "At limit");
            assert_eq!(place_state_of(&view(unblocked.clone(), json!({})), None, None, None).word, "Full");
            assert_eq!(place_state_of(&view(unblocked.clone(), json!({ "running": 0 })), None, None, None), said("Setting up", Some("setting up the agents")));
            // A setup that waits on the person, or failed, wins over the spend and the cap: nothing new runs there until it is through.
            let failed = view(unblocked.clone(), json!({ "setup": over(running(), json!({ "state": "failed", "said": "no agent installed" })) }));
            assert_eq!(place_state_of(&failed, None, Some(20.0), None), said("Setup failed", Some("no agent installed")));
            let settled = without(unblocked, "setup");
            assert_eq!(
                place_state_of(&view(settled, json!({ "running": 0 })), None, None, None).word,
                format!("daemon {}, host {}", DAEMON_VERSION - 1, DAEMON_VERSION)
            );
        }
    }
}
