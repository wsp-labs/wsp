// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/protocol.test.ts:113-190 (the copy of the image beside the record: copyIsCurrent,
//! copyStanding, sealedCopyLine, sealedBuiltLine, copyBuildingLine, copyStoppedLine, placeWorkspacesParts, copyBuildOf),
//! :237-269 (agentVersionWord, strictVersion, agentsCell, agentSignInWord, codexKeyRefusedLine), :367-373 (lookWord),
//! :1322-1392 (placeDaemonPaths, rootsPathIn, the machine's refusals), :1530-1867 (foldThreads and the thread words),
//! :2076-2084 (releaseAbove) and :2189-2225 (the spawn and across refusals). Assertions whose subject is a zod schema
//! (`X.parse`, `X.safeParse`) are wsp-wire's and stay out. Left out for want of a stub: needsYouCount, in :1789 and :1798.

mod support;

use serde_json::{json, Value};
use support::over;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::{SessionView, ThreadView};

fn sealed_image(extra: Value) -> SealedImage {
    let image = json!({ "name": "default", "version": 2, "hash": "a".repeat(64), "recipeHash": "rh", "logins": [], "sealedAt": "t", "sealedFrom": "h1" });
    serde_json::from_value(over(image, extra)).unwrap()
}

fn image_copy(extra: Value) -> SealedImageCopy {
    let copy = json!({ "place": "solari", "version": 1, "snapshotId": "s", "builtAt": "t" });
    serde_json::from_value(over(copy, extra)).unwrap()
}

fn vault() -> Value {
    json!({ "vault": { "sha256": "c".repeat(64), "bytes": 10, "paths": 2, "takenAt": "t" } })
}

use wsp_wire::views::golden_image::{GoldenStage, SealedImage, SealedImageCopy};

fn place(v: Value) -> wsp_wire::views::place::PlaceView {
    serde_json::from_value(over(support::place_view(), v)).unwrap()
}

mod a_copy_of_the_image_beside_the_record {
    use super::*;
    use wsp_words::words::image::*;
    use wsp_words::words::init::NETWORK_LOST_LINE;
    use wsp_words::words::places::{place_workspaces_parts, WorkspacesParts};

    #[test]
    fn is_current_on_the_hash_alone_each_place_numbers_its_own_manifest_so_the_version() {
        let image = sealed_image(json!({}));
        // A second place's first copy is its v1 and was built from the record's v2; the hash is what they share.
        assert_eq!(copy_is_current(&image, &image_copy(json!({ "version": 1, "hash": image.hash }))), true);
        assert_eq!(copy_is_current(&image, &image_copy(json!({ "version": 9, "hash": image.hash }))), true);
        assert_eq!(copy_is_current(&image, &image_copy(json!({ "version": 2, "hash": "b".repeat(64) }))), false);
        assert_eq!(copy_is_current(&image, &image_copy(json!({ "version": 2 }))), false);
    }

    #[test]
    fn says_how_it_stands_in_one_word_and_says_nothing_where_the_record_holds_no_vault() {
        let image = sealed_image(json!({}));
        let sealed = sealed_image(vault());
        assert_eq!(copy_standing(&sealed, &image_copy(json!({ "hash": image.hash }))).as_deref(), Some(COPY_CURRENT));
        assert_eq!(copy_standing(&sealed, &image_copy(json!({ "hash": "b".repeat(64) }))).as_deref(), Some(COPY_STALE));
        assert_eq!(copy_standing(&sealed, &image_copy(json!({}))).as_deref(), Some(COPY_STALE));
        assert_eq!(copy_standing(&image, &image_copy(json!({ "hash": image.hash }))), None);
        // The line a person reads on the command line is the same rule spelled once: what it says is what the word says.
        assert_eq!(sealed_copy_line(&sealed, &image_copy(json!({ "hash": image.hash }))), format!("solari  v1  {COPY_CURRENT}"));
        assert_eq!(sealed_copy_line(&image, &image_copy(json!({ "hash": image.hash }))), "solari  v1");
    }

    #[test]
    fn a_build_s_own_line_is_the_copy_s_and_says_when_nothing_was_built() {
        let image = sealed_image(json!({}));
        let sealed = sealed_image(vault());
        let built = image_copy(json!({ "hash": image.hash }));
        assert_eq!(sealed_built_line(&sealed, &built, true), sealed_copy_line(&sealed, &built));
        assert_eq!(
            sealed_built_line(&sealed, &built, false),
            format!("{}  already built from this image; nothing was built", sealed_copy_line(&sealed, &built))
        );
    }

    #[test]
    fn a_place_s_row_says_the_stage_a_copy_build_there_is_at_in_the_seal_s_own_words_or() {
        assert_eq!(copy_building_line(GoldenStage::Creating), "copying your image: creating the machine");
        assert_eq!(copy_building_line(GoldenStage::UploadingFiles), "copying your image: copying your files");
        assert_eq!(copy_stopped_line(Some("no room today")), "the build stopped: no room today");
        assert_eq!(copy_stopped_line(None), "the build stopped");
        assert_eq!(copy_stopped_line(Some("fetch failed")), format!("the build stopped: {NETWORK_LOST_LINE}"));
        let boxed = place(json!({ "id": "p_1", "kind": "computer", "name": "box", "default": false, "takesForks": true, "build": copy_building_line(GoldenStage::Creating) }));
        assert_eq!(
            place_workspaces_parts(&boxed, 2.0, None),
            WorkspacesParts { count: "2".to_string(), note: Some(copy_building_line(GoldenStage::Creating)) }
        );
        let provider = json!({ "id": "solari", "kind": "provider", "name": "solari", "default": false, "takesForks": true, "build": copy_stopped_line(Some("no room")) });
        assert_eq!(
            place_workspaces_parts(&place(provider.clone()), 1.0, Some(0.41)),
            WorkspacesParts { count: "1".to_string(), note: Some(copy_stopped_line(Some("no room"))) }
        );
        let mut unbuilt = provider;
        unbuilt.as_object_mut().unwrap().remove("build");
        assert_eq!(
            place_workspaces_parts(&place(unbuilt), 1.0, Some(0.41)),
            WorkspacesParts { count: "1".to_string(), note: Some("$0.41 this month".to_string()) }
        );
    }

    #[test]
    fn one_rule_reads_a_build_frame_as_the_copy_building_stopped_or_cleared_in_the_row() {
        assert_eq!(
            copy_build_of(GoldenStage::InstallingTools, None),
            Some(CopyBuild { line: copy_building_line(GoldenStage::InstallingTools), stopped: false })
        );
        assert_eq!(
            copy_build_of(GoldenStage::Failed, Some("no room today")),
            Some(CopyBuild { line: copy_stopped_line(Some("no room today")), stopped: true })
        );
        assert_eq!(copy_build_of(GoldenStage::Sealed, None), None);
    }
}

mod what_a_computer_says_about_its_agents_and_what_a_person_reads_off_it {
    use super::*;
    use wsp_wire::types::agents_report::AgentSignInState;
    use wsp_words::words::places::*;

    #[test]
    fn the_version_is_the_number_an_agent_printed_whichever_way_it_worded_the_line() {
        assert_eq!(agent_version_word("codex-cli 0.153.0"), "0.153.0");
        assert_eq!(agent_version_word("2.1.270 (Claude Code)"), "2.1.270");
        // Nothing of that shape in the line: the computer's own words stand rather than a guess at a number.
        assert_eq!(agent_version_word(" nightly "), "nightly");
    }

    #[test]
    fn a_version_read_off_a_vendor_is_a_whole_strict_semver_under_64_characters_or_noth() {
        assert_eq!(strict_version(" 2.1.283\n").as_deref(), Some("2.1.283"));
        assert_eq!(strict_version("v0.96.1").as_deref(), Some("0.96.1"));
        assert_eq!(strict_version("0.0.1790352060-g26b83c").as_deref(), Some("0.0.1790352060-g26b83c"));
        let long_pre = format!("1.0.0-{}", "a".repeat(60));
        let long_major = format!("{}.0.0", "9".repeat(100));
        for bad in ["nginx/1.18.0", "\x1b[31m2.1.283\x07", "<html>moved</html>", "01.2.3", "1.2", long_pre.as_str(), long_major.as_str()] {
            assert_eq!(strict_version(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_agents_cell_names_each_agent_its_version_and_its_sign_in_and_says_nothing_fo() {
        let row = place(json!({ "agents": ["claude", "codex"], "agentVersions": { "claude": "2.1.270 (Claude Code)", "codex": "codex-cli 0.153.0" }, "signIns": { "claude": "vault-key", "codex": "none" } }));
        assert_eq!(agents_cell(&row), "claude 2.1.270 signed in with a token, codex 0.153.0 not signed in");
        assert_eq!(agents_cell(&place(json!({ "agents": ["codex"], "signIns": { "codex": "signed-in" } }))), "codex signed in");
        assert_eq!(agents_cell(&place(json!({}))), "");
        assert_eq!(agent_sign_in_word(AgentSignInState::SignedIn), "signed in");
    }

    #[test]
    fn a_key_the_provider_turned_down_is_said_as_a_refused_key_with_what_the_provider_s() {
        let login = "codex login --device-auth";
        assert_eq!(
            codex_key_refused_line("OPENAI_API_KEY", "invalid_api_key", login),
            format!("OpenAI refused your OPENAI_API_KEY: invalid_api_key. Put a working key in ~/.wsp/.env, or sign Codex in where this thread runs with {login}.")
        );
        // A refusal the provider said nothing after drops the clause rather than reading as a colon with nothing behind it.
        assert_eq!(
            codex_key_refused_line("OPENAI_API_KEY", "", login),
            format!("OpenAI refused your OPENAI_API_KEY. Put a working key in ~/.wsp/.env, or sign Codex in where this thread runs with {login}.")
        );
        // The provider's reason is a clause in somebody else's sentence, so it is cut before it becomes a paragraph.
        let said = codex_key_refused_line("OPENAI_API_KEY", &"x".repeat(200), login);
        assert!(said.contains(&format!(": {}.", "x".repeat(80))), "{said}");
    }
}

mod a_workspace_s_look {
    use super::*;
    use regex::Regex;
    use std::collections::HashSet;
    use wsp_wire::types::workspace_look::WORKSPACE_GLYPHS;
    use wsp_words::words::places::look_word;

    #[test]
    fn about_two_dozen_glyphs_each_one_word_so_its_name_needs_no_second_table_and_none() {
        assert!(WORKSPACE_GLYPHS.len() >= 20);
        assert_eq!(WORKSPACE_GLYPHS.iter().collect::<HashSet<_>>().len(), WORKSPACE_GLYPHS.len());
        let word = Regex::new("^[a-z]+$").unwrap();
        for glyph in WORKSPACE_GLYPHS {
            assert!(word.is_match(glyph), "{glyph}");
        }
        assert_eq!(look_word("terminal"), "Terminal");
    }
}

mod daemon_files_and_diff_ops {
    use super::*;
    use wsp_words::exit::HostError;
    use wsp_words::project_path::{place_daemon_paths, roots_path_in, PlaceDaemonPaths};
    use wsp_words::words::computer::*;
    use wsp_words::words::computer::{machine_lacking, machine_lacks_line, machine_lacks_short, no_import_road_line};

    fn paths(home: &str) -> PlaceDaemonPaths {
        let at = |rest: &str| format!("{home}{rest}");
        PlaceDaemonPaths {
            wsp: at("/.wsp"),
            dir: at("/.wsp/daemon"),
            bundle: at("/.wsp/daemon.tgz"),
            inbox: at("/.wsp/inbox"),
            token_path: at("/.wsp/daemon-token"),
            port_file: at("/.wsp/daemon.port"),
            run_dir: at("/.wsp/run"),
            put_dir: at("/.wsp/put"),
            open_socket: at("/.wsp/open.sock"),
            guest_socket: at("/.wsp/daemon.sock"),
            guest_bin: at("/.wsp/place-bin"),
            docker_notes: at("/.wsp/docker-notes"),
            manifest_path: at("/.wsp/manifest.json"),
            profile_file: at("/.wsp/profile.sh"),
            unit_dir: at("/.config/systemd/user"),
            bin_dir: at("/.local/bin"),
            open_shim: at("/.local/bin/wsp-open"),
            roots_path: at("/.wsp/roots"),
            // A computer joined as a place keeps these four beside the daemon's own, so one sweep takes the lot.
            place_file: at("/.wsp/place.json"),
            place_key: at("/.wsp/place-key.pem"),
            place_log: at("/.wsp/place.log"),
            place_found: at("/.wsp/place-found"),
            // A leave takes ~/.wsp only once it is empty, so every write under it has its own name here.
            place_found_part: at("/.wsp/place-found.part"),
            join_code: at("/.wsp/join-code"),
            install_logs: at("/.wsp/install-logs"),
            readings: at("/.wsp/readings"),
            ssh_dir: at("/.wsp/ssh"),
        }
    }

    #[test]
    fn names_everywhere_a_daemon_on_a_machine_reached_over_ssh_keeps_something_all_unde() {
        let at = place_daemon_paths("/home/maya");
        assert_eq!(at, paths("/home/maya"));
        // The deploy on the host writes these and the runtime reads the token and the port back off them, which is
        // why the rule sits here and in neither of them.
        assert_eq!(at.roots_path, roots_path_in("/home/maya"));
        assert_eq!(place_daemon_paths("/home/maya/"), at);
        // Nothing here is root's: the daemon on a machine somebody owns is installed under their own login.
        let at_json = serde_json::to_value(&at).unwrap();
        let outside: Vec<&str> = at_json.as_object().unwrap().values().map(|p| p.as_str().unwrap()).filter(|path| !path.starts_with("/home/maya/")).collect();
        assert_eq!(outside, Vec::<&str>::new());
    }

    #[test]
    fn refuses_a_login_whose_services_stop_with_it() {
        // A login whose services stop with it would lose the daemon the moment the connection closed, so it is
        // refused with the one command that turns that off.
        assert!(NO_LINGER_LINE.contains("loginctl enable-linger"), "{NO_LINGER_LINE}");
        // The daemon is one static binary; the wsp command beside it still runs on node, so that is what is asked for.
        assert!(NO_NODE_LINE.contains("no Node 22"), "{NO_NODE_LINE}");
        assert!(NO_NODE_LINE.contains("install Node 22 or newer"), "{NO_NODE_LINE}");
        // A refusal the machine itself raised is marked where it is thrown, so a row can show those words and show a
        // deploy that failed further in the general line instead of the deploy's tail.
        assert_eq!(machine_lacks_line(&machine_lacking(NO_NODE_LINE)).as_deref(), Some(NO_NODE_LINE));
        assert_eq!(machine_lacks_line(&HostError::new(NO_NODE_LINE)), None);
        // The TS passes a bare string here; the stub takes a HostError, so it is an unmarked one.
        assert_eq!(machine_lacks_line(&HostError::new("daemon deploy failed: DAEMON_DOWN")), None);
        // Every refusal is written as what is wrong, then why, then what to do, so its first clause is the row's half
        // and fits the row, and the whole sentence keeps the command a person types.
        assert_eq!(machine_lacks_short(NO_NODE_LINE), "this machine has no Node 22");
        assert_eq!(machine_lacks_short(NO_LINGER_LINE), "this login does not linger");
        assert_eq!(machine_lacks_short(NO_SYSTEMD_LINE), "this machine runs no systemd");
        // Off the one list beside the sentences, not a copy of it here: a refusal added to a place's preflight takes
        // this rule by being listed once, rather than by somebody remembering that this loop exists.
        assert!(MACHINE_LACKS_LINES.contains(&NO_NODE_LINE));
        assert!(MACHINE_LACKS_LINES.contains(&NO_LINGER_LINE));
        assert!(MACHINE_LACKS_LINES.contains(&NO_SYSTEMD_LINE));
        for line in MACHINE_LACKS_LINES {
            let short = machine_lacks_short(line);
            assert!(short.encode_utf16().count() <= 30, "{short}");
            assert!(line.encode_utf16().count() > short.encode_utf16().count(), "{line}");
        }
        assert_eq!(
            no_import_road_line("box", "a plan9 box"),
            "box is a plan9 box, which lands no folder yet; import to a fork, or register the folder on this computer"
        );
    }
}

mod thread_provenance {
    use super::*;
    use regex::Regex;
    use wsp_wire::views::session::{ThreadPlacement, ThreadSection};
    use wsp_words::thread_state::*;
    use wsp_words::views::session::fold_threads;
    use wsp_words::words::base::{cut_line, ELLIPSIS};
    use wsp_words::words::turn::*;
    use wsp_wire::views::session_events::TurnResult;

    fn row() -> Value {
        json!({ "id": "s1", "workspaceId": "ws_1", "harness": "claude", "status": "completed" })
    }

    fn s(v: Value) -> SessionView {
        serde_json::from_value(over(row(), v)).unwrap()
    }

    fn thread(v: Value) -> ThreadView {
        serde_json::from_value(over(support::thread_view(), v)).unwrap()
    }

    fn sub(v: Value) -> Value {
        over(json!({ "state": "napping", "on": "box", "machine": "fix", "size": { "cpu": 2, "memMb": 4096 }, "branch": "wsp/fix" }), v)
    }

    fn result(v: Value) -> TurnResult {
        serde_json::from_value(v).unwrap()
    }

    fn at(n: f64) -> Option<JsNumber> {
        Some(JsNumber(n))
    }

    fn words(v: &str) -> usize {
        v.encode_utf16().count()
    }

    #[test]
    fn foldthreads_groups_turns_by_threadid_titles_by_the_opening_turn_reads_state_row() {
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "startedBy": "cli", "prompt": "make a server", "claudeSessionId": "c1", "startedAt": 1_000, "endedAt": 2_000 })),
            s(json!({ "id": "s2", "threadId": "thr_b", "prompt": "unrelated", "startedAt": 3_000, "endedAt": 4_000 })),
            s(json!({ "id": "s3", "threadId": "thr_a", "status": "running", "startedBy": "person", "prompt": "and tests", "claudeSessionId": "c2", "startedAt": 5_000 })),
            s(json!({ "id": "s4", "status": "failed", "prompt": "before threads", "startedAt": 6_000, "endedAt": 7_000 })),
        ]);
        assert_eq!(
            threads,
            vec![
                thread(json!({ "id": "thr_a", "threadId": "thr_a", "workspaceId": "ws_1", "harness": "claude", "startedBy": "cli", "status": "running", "title": "make a server", "sessionId": "s3", "claudeSessionId": "c2", "startedAt": 5_000, "turns": 2, "ran": true })),
                thread(json!({ "id": "thr_b", "threadId": "thr_b", "workspaceId": "ws_1", "harness": "claude", "startedBy": "person", "status": "completed", "title": "unrelated", "sessionId": "s2", "startedAt": 3_000, "endedAt": 4_000, "turns": 1, "ran": false })),
                thread(json!({ "id": "s4", "workspaceId": "ws_1", "harness": "claude", "startedBy": "person", "status": "failed", "title": "before threads", "sessionId": "s4", "startedAt": 6_000, "endedAt": 7_000, "turns": 1, "ran": false })),
            ]
        );
        // The TS also parses each folded thread back through ThreadView, which is wsp-wire's.
    }

    #[test]
    fn foldthreads_adds_up_what_a_thread_s_turns_cost_and_says_nothing_where_no_turn_of() {
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "claudeSessionId": "c1", "costUsd": 0.75, "startedAt": 1_000, "endedAt": 2_000 })),
            s(json!({ "id": "s2", "threadId": "thr_a", "claudeSessionId": "c1", "costUsd": 0.39, "startedAt": 3_000, "endedAt": 4_000 })),
            s(json!({ "id": "s3", "threadId": "thr_b", "claudeSessionId": "c2", "startedAt": 5_000, "endedAt": 6_000 })),
        ]);
        let (spent, said) = (&threads[0], &threads[1]);
        let cost = spent.cost_usd.unwrap().0;
        assert!((cost - 1.14).abs() < 5e-11, "{cost}");
        assert_eq!(said.cost_usd, None);
    }

    #[test]
    fn foldthreads_carries_the_access_and_the_fast_mode_the_latest_turn_ran_at_so_a_lis() {
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "permissionMode": "default" })),
            s(json!({ "id": "s2", "threadId": "thr_a", "permissionMode": "acceptEdits", "fast": true })),
            s(json!({ "id": "s3", "threadId": "thr_b" })),
        ]);
        let (thread, plain) = (&threads[0], &threads[1]);
        assert_eq!((thread.permission_mode.as_deref(), thread.fast), (Some("acceptEdits"), Some(true)));
        assert_eq!((plain.permission_mode.as_deref(), plain.fast), (None, None));
        // The TS parses the thread back through ThreadView, which is wsp-wire's.
    }

    #[test]
    fn a_thread_reads_as_run_once_a_turn_of_it_did_work_announcing_a_session_is_not_eno() {
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "status": "failed", "claudeSessionId": "c1" })),
            s(json!({ "id": "s2", "threadId": "thr_b", "status": "failed" })),
            s(json!({ "id": "s3", "threadId": "thr_c", "status": "running" })),
            // The launch got as far as a session id and the agent then refused the whole task: no work behind it.
            s(json!({ "id": "s4", "threadId": "thr_d", "status": "failed", "claudeSessionId": "c2", "refusal": "sign-in" })),
        ]);
        let (worked, never_announced, working, refused) = (&threads[0], &threads[1], &threads[2], &threads[3]);
        // One turn of the thread did work, so the thread did, whatever a later turn came to.
        let later = fold_threads(&[
            s(json!({ "id": "s5", "threadId": "thr_e", "claudeSessionId": "c3", "status": "failed", "refusal": "sign-in" })),
            s(json!({ "id": "s6", "threadId": "thr_e", "claudeSessionId": "c3", "status": "completed" })),
        ]);
        let signed_in_later = &later[0];
        assert_eq!(
            [worked.ran, never_announced.ran, working.ran, refused.ran, signed_in_later.ran],
            [true, false, true, false, true]
        );
        // threadRan is wsp-wire's own function (views::session), todo!() there as well.
        assert_eq!(wsp_wire::views::session::thread_ran(&[]), false);
    }

    #[test]
    fn foldthreads_carries_the_latest_turn_s_process_on_this_computer_so_the_pane_heads() {
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "status": "running", "prompt": "read the docs", "pid": 4_242 })),
            s(json!({ "id": "s2", "threadId": "thr_b", "status": "running", "prompt": "on a machine" })),
        ]);
        let (here, elsewhere) = (&threads[0], &threads[1]);
        assert_eq!(here.pid, Some(4_242));
        assert_eq!(elsewhere.pid, None);
        // The TS reads the pid back through ThreadView.parse, which is wsp-wire's.
        // The latest turn is the one running: a thread whose earlier turn had a process carries none now.
        let threads = fold_threads(&[
            s(json!({ "id": "s3", "threadId": "thr_c", "status": "completed", "pid": 99 })),
            s(json!({ "id": "s4", "threadId": "thr_c", "status": "completed" })),
        ]);
        assert_eq!(threads[0].pid, None);
    }

    #[test]
    fn foldthreads_carries_the_latest_turn_s_open_prompt_so_the_word_every_row_reads_co() {
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "status": "running", "prompt": "write it", "asking": "Permission for Write: out.txt" })),
            s(json!({ "id": "s2", "threadId": "thr_b", "status": "running", "prompt": "nothing to ask" })),
        ]);
        let (asked, quiet) = (&threads[0], &threads[1]);
        assert_eq!(asked.asking.as_deref(), Some("Permission for Write: out.txt"));
        assert_eq!(quiet.asking, None);
        // The TS reads the prompt back through ThreadView.parse, which is wsp-wire's.
        assert_eq!([thread_state(asked), thread_state(quiet)], [ThreadState::Waiting, ThreadState::Running]);
        assert_eq!([thread_word_of(asked), thread_word_of(quiet)], ["Needs you", "Working"]);
        // A prompt raised on a turn that has since settled says nothing: the fold reads the latest turn alone.
        let threads = fold_threads(&[
            s(json!({ "id": "s3", "threadId": "thr_c", "status": "running", "asking": "Permission for Bash: ls" })),
            s(json!({ "id": "s4", "threadId": "thr_c", "status": "completed" })),
        ]);
        let settled = &threads[0];
        assert_eq!(settled.asking, None);
        assert_eq!(thread_word_of(settled), "Idle");
    }

    #[test]
    fn a_thread_whose_own_call_is_behind_another_thread_s_question_is_waiting_too_and_i() {
        let behind = json!({
            "threadId": "thr_b",
            "workspaceId": "ws_1",
            "sessionId": "s2",
            "title": "read the file",
            "prompt": { "askId": "ask_1", "toolName": "Write", "input": r#"{"file_path":"/root/hello.txt","content":"hi"}"#, "options": [] },
        });
        let caller = json!({ "status": "running", "waitingOn": behind });
        assert_eq!(thread_state(&thread(caller.clone())), ThreadState::Waiting);
        assert_eq!(thread_word_of(&thread(caller.clone())), "Needs you");
        assert_eq!(waiting_line(&thread(caller.clone())).as_deref(), Some("Write hello.txt in root (2 B) needs an answer"));
        // Its own prompt leads: a thread asked something itself says that, whatever it is also behind.
        assert_eq!(waiting_line(&thread(over(caller, json!({ "asking": "Permission for Bash: ls" })))).as_deref(), Some("Permission for Bash: ls"));
        // The TS passes a bare {}, which has no status; the base thread here has a completed one and no prompt.
        assert_eq!(waiting_line(&thread(json!({}))), None);
        assert_eq!(thread_state(&thread(json!({ "status": "running" }))), ThreadState::Running);
    }

    #[test]
    fn reads_the_wsp_calls_that_wait_for_another_thread_off_the_call_alone_and_nothing() {
        use wsp_words::wsp_tools::{threads_followed, Followed};
        assert_eq!(threads_followed("mcp__wsp__send", r#"{"thread":"thr_b","message":"go"}"#), Some(Followed::Named(vec!["thr_b".to_string()])));
        assert_eq!(
            threads_followed("mcp__wsp__threads_wait", r#"{"threads":["thr_b","thr_c"],"timeout":60}"#),
            Some(Followed::Named(vec!["thr_b".to_string(), "thr_c".to_string()]))
        );
        // run opens the thread it follows, so the call names none and the caller is behind what it started.
        assert_eq!(threads_followed("mcp__wsp__run", r#"{"workspace":"api","task":"build it"}"#), Some(Followed::Opened));
        // Detached is the whole point of detach: the call answers at once and waits for nobody.
        assert_eq!(threads_followed("mcp__wsp__run", r#"{"workspace":"api","task":"build it","detach":true}"#), None);
        assert_eq!(threads_followed("mcp__wsp__send", r#"{"thread":"thr_b","message":"go","detach":true}"#), None);
        // A wsp verb that answers out of the host alone, another server's tool, the agent's own tools, and junk input.
        assert_eq!(threads_followed("mcp__wsp__threads", "{}"), None);
        assert_eq!(threads_followed("mcp__other__send", r#"{"thread":"thr_b"}"#), None);
        assert_eq!(threads_followed("Read", r#"{"file_path":"/root/hello.txt"}"#), None);
        assert_eq!(threads_followed("mcp__wsp__send", "not json"), None);
        assert_eq!(threads_followed("mcp__wsp__send", "{}"), None);
    }

    #[test]
    fn every_thread_state_has_one_word_and_a_turn_that_failed_says_so_rather_than_readi() {
        // The design spec's four words for a thread: Working, Idle, Failed and the prompt's own Needs you. A launch that
        // never ran is the row this separates from a thread that did its work and stopped.
        assert_eq!([thread_state_word(ThreadState::Running), thread_state_word(ThreadState::Completed)], ["Working", "Idle"]);
        assert_eq!(thread_state_word(ThreadState::Failed), "Failed");
        assert_eq!(thread_state_word(ThreadState::Interrupted), "Idle");
        assert_eq!(thread_state_word(ThreadState::Waiting), "Needs you");
        assert_eq!(thread_word_of(&thread(json!({ "status": "failed" }))), "Failed");
        // Only a running turn is ever waiting on a person: the runtime clears the prompt however the turn ends, on the
        // harness's own exit and on the roads that cut it, so a settled row carrying one is a row nothing can answer.
        assert_eq!(thread_word_of(&thread(json!({ "status": "running", "asking": "Permission for Write: out.txt" }))), "Needs you");
    }

    #[test]
    fn a_turn_that_ended_and_that_no_window_has_shown_since_reads_done_until_a_read_sta() {
        let ended = json!({ "status": "completed", "endedAt": 2_000 });
        assert_eq!(thread_unread(&thread(ended.clone())), true);
        assert_eq!(thread_word_of(&thread(over(ended.clone(), json!({ "readAt": 1_999 })))), "Done");
        assert_eq!(thread_state(&thread(over(ended.clone(), json!({ "readAt": 1_999 })))), ThreadState::Done);
        assert_eq!(thread_word_of(&thread(over(ended.clone(), json!({ "readAt": 2_000 })))), "Idle");
        assert_eq!(thread_word_of(&thread(json!({ "status": "interrupted", "endedAt": 2_000, "readAt": 10 }))), "Done");
        // A failed turn says so whether anybody saw it, a running one works, and a question outranks the finish.
        assert_eq!(thread_word_of(&thread(json!({ "status": "failed", "endedAt": 2_000 }))), "Failed");
        assert_eq!(thread_word_of(&thread(json!({ "status": "running" }))), "Working");
        assert_eq!(thread_word_of(&thread(over(ended, json!({ "asking": "Permission for Bash: ls" })))), "Needs you");
        // The stamps ride the latest row into the fold, as every other fact of the thread does.
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "status": "completed", "endedAt": 1_000, "readAt": 500 })),
            s(json!({ "id": "s2", "threadId": "thr_a", "status": "completed", "endedAt": 2_000, "readAt": 500, "settledAt": 400 })),
        ]);
        let thread = &threads[0];
        assert_eq!((thread.read_at, thread.settled_at), (at(500.0), at(400.0)));
        // The TS reads the same two stamps back through ThreadView.parse, which is wsp-wire's.
        assert_eq!(thread_word_of(thread), "Done");
    }

    #[test]
    fn a_snooze_that_passed_reads_done_until_a_read_at_or_after_its_end_and_the_thread() {
        let read = json!({ "status": "completed", "endedAt": 2_000, "readAt": 3_000 });
        assert_eq!(thread_word_of(&thread(read.clone())), "Idle");
        assert_eq!(thread_word_of(&thread(over(read.clone(), json!({ "wokeAt": 4_000 })))), "Done");
        assert_eq!(thread_word_of(&thread(over(read.clone(), json!({ "readAt": 4_000, "wokeAt": 4_000 })))), "Idle");
        // A snooze still standing reads as the read stamp says, and a failure says so whatever the snooze.
        let snoozed = over(read.clone(), json!({ "snoozedUntil": 9_000 }));
        assert_eq!(thread_word_of(&thread(snoozed)), "Idle");
        assert_eq!(thread_word_of(&thread(over(read, json!({ "status": "failed", "wokeAt": 4_000 })))), "Failed");
        let placed = json!({ "name": "done", "whileState": "completed s2" });
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "status": "completed", "endedAt": 1_000 })),
            s(json!({ "id": "s2", "threadId": "thr_a", "status": "completed", "endedAt": 2_000, "pinnedAt": 10, "snoozedUntil": 20, "wokeAt": 15, "section": placed })),
        ]);
        // The TS reads these marks back through ThreadView.parse (wsp-wire's); they are read off the fold's own value here.
        let thread = &threads[0];
        assert_eq!((thread.pinned_at, thread.snoozed_until, thread.woke_at), (at(10.0), at(20.0), at(15.0)));
        assert_eq!(thread.section, Some(ThreadPlacement { name: ThreadSection::Done, while_state: "completed s2".to_string() }));
    }

    #[test]
    fn foldthreads_carries_the_latest_turn_s_last_line_its_failure_and_the_fold_mark_so() {
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "status": "failed", "endedAt": 1_000, "failure": "the model refused" })),
            s(json!({ "id": "s2", "threadId": "thr_a", "status": "completed", "endedAt": 2_000, "lastLine": "Pushed the branch.", "foldedAt": 30 })),
        ]);
        let thread = &threads[0];
        // The TS reads these two back through ThreadView.parse (wsp-wire's); they are read off the fold's own value here.
        assert_eq!((thread.last_line.as_deref(), thread.folded_at), (Some("Pushed the branch."), at(30.0)));
        assert_eq!(thread.failure, None);
        let threads = fold_threads(&[s(json!({ "id": "s3", "threadId": "thr_b", "status": "failed", "failure": "the model refused" }))]);
        let failed = &threads[0];
        assert_eq!(failed.failure.as_deref(), Some("the model refused"));
        assert_eq!(failed.last_line, None);
    }

    #[test]
    fn a_turn_s_end_leaves_its_reply_s_last_line_cut_to_200_characters_and_a_failure_s() {
        let long = "word ".repeat(80).trim().to_string();
        let done = turn_lines(&result(json!({ "status": "completed", "text": format!("Working.\n\n{long}\n") })), None);
        assert!(words(done.last_line.as_deref().unwrap()) <= LISTED_LINE_CHARS);
        assert!(Regex::new("^word word .*\u{2026}$").unwrap().is_match(done.last_line.as_deref().unwrap()), "{:?}", done.last_line);
        assert_eq!(done.failure, None);
        assert_eq!(turn_lines(&result(json!({ "status": "completed", "text": "  " })), None), TurnLines::default());
        assert_eq!(
            turn_lines(&result(json!({ "status": "failed", "error": "\nthe model refused\nmore of it" })), None),
            TurnLines { failure: Some("the model refused".to_string()), ..TurnLines::default() }
        );
        assert_eq!(
            turn_lines(&result(json!({ "status": "failed" })), Some("host restarted")),
            TurnLines { failure: Some("host restarted".to_string()), ..TurnLines::default() }
        );
        assert_eq!(
            turn_lines(&result(json!({ "status": "failed" })), None),
            TurnLines { failure: Some(NO_RESULT_LINE.to_string()), ..TurnLines::default() }
        );
        assert_eq!(
            turn_lines(&result(json!({ "status": "interrupted", "text": "half" })), None),
            TurnLines { last_line: Some("half".to_string()), ..TurnLines::default() }
        );
        assert_eq!(words(&listed_failure(Some(&"x".repeat(500)))), LISTED_LINE_CHARS);
        assert!(words(&subagent_asked(&format!("  {}", "brief ".repeat(100)))) <= SUBAGENT_ASKED_CHARS);
        assert_eq!(subagent_asked("Count to three."), "Count to three.");
    }

    #[test]
    fn a_listed_last_line_drops_the_markdown_list_marker_it_was_written_under_and_keeps() {
        assert_eq!(
            listed_last_line(Some("Found three gaps:\n- No test counts the case where the fold is empty.")).as_deref(),
            Some("No test counts the case where the fold is empty.")
        );
        assert_eq!(listed_last_line(Some("* Main is green.")).as_deref(), Some("Main is green."));
        assert_eq!(listed_last_line(Some("+ Main is green.")).as_deref(), Some("Main is green."));
        assert_eq!(listed_last_line(Some("12. Main is green.")).as_deref(), Some("Main is green."));
        assert_eq!(listed_last_line(Some("3) Main is green.")).as_deref(), Some("Main is green."));
        assert_eq!(listed_last_line(Some("  -   Main is green.")).as_deref(), Some("Main is green."));
        assert_eq!(listed_last_line(Some("3")).as_deref(), Some("3"));
        assert_eq!(listed_last_line(Some("-1 below zero")).as_deref(), Some("-1 below zero"));
        assert_eq!(listed_last_line(Some("**Main** is green.")).as_deref(), Some("**Main** is green."));
        assert_eq!(listed_last_line(Some("-")).as_deref(), Some("-"));
    }

    #[test]
    fn a_cut_through_a_line_with_no_space_cjk_with_an_emoji_at_the_edge_ends_on_a_whole() {
        let line = format!("{}\u{1f389}{}", "完成".repeat(99), "了".repeat(60));
        for cut in [
            listed_last_line(Some(&line)).unwrap(),
            listed_failure(Some(&line)),
            subagent_asked(&format!("{}\u{1f389}{}", "完成".repeat(139), "了".repeat(60))),
            cut_line(&line, 200),
        ] {
            // The TS also checks encodeURIComponent does not throw; a Rust string cannot hold half of a pair.
            assert!(cut.ends_with(ELLIPSIS), "{cut}");
        }
        assert_eq!(listed_last_line(Some(&line)), Some(format!("{}{ELLIPSIS}", "完成".repeat(99))));
        assert_eq!(cut_line("ab\u{1f389}cd", 4), format!("ab{ELLIPSIS}"));
        assert_eq!(cut_line("a\u{1f389}cd", 4), format!("a\u{1f389}{ELLIPSIS}"));
    }

    #[test]
    fn a_thread_needs_the_person_while_it_asks_while_a_finish_or_a_failure_sits_unseen() {
        assert_eq!(thread_needs_you(&thread(json!({ "status": "running", "asking": "Permission for Bash: ls" }))), true);
        let behind = json!({ "threadId": "thr_b", "workspaceId": "ws_a", "sessionId": "s_b", "title": "child", "prompt": { "askId": "ask_1", "toolName": "Bash", "input": "{}", "options": [] } });
        assert_eq!(thread_needs_you(&thread(json!({ "status": "running", "waitingOn": behind }))), true);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "running" }))), false);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "completed", "endedAt": 2_000, "readAt": 1_000 }))), true);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "failed", "endedAt": 2_000, "readAt": 1_000 }))), true);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "failed", "endedAt": 2_000, "readAt": 2_000 }))), false);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "completed", "endedAt": 2_000, "readAt": 2_000 }))), false);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "completed", "endedAt": 2_000, "readAt": 3_000, "wokeAt": 4_000 }))), true);
    }

    #[test]
    fn a_sub_thread_whose_work_not_pushed_holds_its_machine_needs_the_person_seen_or_no() {
        let held = json!({ "held": { "files": 3, "commits": 2 } });
        assert_eq!(thread_needs_you(&thread(json!({ "status": "completed", "endedAt": 2_000, "readAt": 3_000, "sub": sub(held.clone()) }))), true);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "running", "sub": sub(held) }))), false);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "completed", "endedAt": 2_000, "readAt": 3_000, "sub": sub(json!({})) }))), false);
        // The TS then counts needsYouCount over one such session (1); needsYouCount is not in the stubs.
    }

    #[test]
    fn a_turn_the_person_stopped_asks_nothing_of_them_seen_or_not_a_snooze_ending_on_it() {
        assert_eq!(thread_needs_you(&thread(json!({ "status": "interrupted", "endedAt": 2_000, "readAt": 1_000 }))), false);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "interrupted", "endedAt": 2_000 }))), false);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "interrupted", "endedAt": 2_000, "readAt": 3_000, "wokeAt": 4_000 }))), true);
        assert_eq!(thread_needs_you(&thread(json!({ "status": "interrupted", "endedAt": 2_000, "asking": "Permission for Bash: ls" }))), true);
        // The TS then counts needsYouCount over one interrupted session (0); needsYouCount is not in the stubs.
    }

    #[test]
    fn foldthreads_carries_the_latest_turn_s_folder_so_every_director_shows_where_the_t() {
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "prompt": "make a server", "cwd": "/root/work/proj" })),
            s(json!({ "id": "s2", "threadId": "thr_a", "prompt": "and tests", "cwd": "/root/work/proj/packages" })),
            s(json!({ "id": "s3", "threadId": "thr_b", "prompt": "no folder" })),
        ]);
        let (worked, bare) = (&threads[0], &threads[1]);
        assert_eq!(worked.cwd.as_deref(), Some("/root/work/proj/packages"));
        assert_eq!(bare.cwd, None);
        // The TS reads the folder back through ThreadView.parse, which is wsp-wire's.
    }

    #[test]
    fn foldthreads_titles_a_thread_by_its_opening_prompt_s_first_line_so_the_cli_s_tabl() {
        let threads = fold_threads(&[s(json!({ "prompt": "You are a builder.\n\nTicket: wsp-labs/wsp-map#292.\nBuild: the fix." }))]);
        assert_eq!(threads[0].title, "You are a builder.");
    }

    #[test]
    fn foldthreads_titles_a_thread_with_no_harness_title_by_its_opening_turn_s_first_se() {
        let brief = "You are a builder for the wsp repo, which is at /Users/dev/wsp on this Mac: read the ticket, then run `pnpm test` and report.\n\nTicket: wsp-labs/wsp-map#408.";
        let threads = fold_threads(&[s(json!({ "prompt": brief }))]);
        assert_eq!(threads[0].title, "You are a builder for the wsp repo, which is at\u{2026}");
        let two = fold_threads(&[s(json!({ "prompt": "Bump the lockfile. Then run the gate." }))]);
        assert_eq!(two[0].title, "Bump the lockfile.");
    }

    #[test]
    fn foldthreads_titles_a_thread_by_what_the_harness_calls_the_session_the_next_send() {
        let threads = fold_threads(&[
            s(json!({ "id": "s1", "threadId": "thr_a", "prompt": "make a server", "claudeSessionId": "c1", "harnessTitle": "Building the server", "startedAt": 1_000 })),
            s(json!({ "id": "s2", "threadId": "thr_a", "prompt": "and tests", "claudeSessionId": "c1", "harnessTitle": "the sidebar's own name", "startedAt": 2_000 })),
        ]);
        assert_eq!(threads[0].title, "the sidebar's own name");
    }

    #[test]
    fn foldthreads_keeps_the_opening_turn_s_words_while_the_harness_has_no_title_of_its() {
        let none = fold_threads(&[s(json!({ "id": "s1", "threadId": "thr_a", "prompt": "make a server" }))]);
        assert_eq!(none[0].title, "make a server");
        let wrapped = fold_threads(&[s(json!({ "id": "s2", "threadId": "thr_b", "prompt": "make a server", "harnessTitle": "  Building  the server \nand its tests" }))]);
        assert_eq!(wrapped[0].title, "Building the server");
        let named = fold_threads(&[s(json!({ "id": "s3", "threadId": "thr_c", "claudeSessionId": "c3", "harnessTitle": "Building the server" }))]);
        assert_eq!(named[0].title, "Building the server");
    }

    #[test]
    fn a_thread_always_says_who_opened_it_the_fold_reads_a_row_from_before_provenance_a() {
        let threads = fold_threads(&[s(json!({ "prompt": "old" }))]);
        assert_eq!(serde_json::to_value(threads[0].started_by).unwrap(), json!("person"));
        // The TS then checks ThreadView.parse refuses a row with no startedBy, which is wsp-wire's.
    }
}

mod the_newest_release_as_the_host_read_it {
    use super::*;
    use wsp_wire::types::release::ReleaseView;
    use wsp_words::release::release_above;

    #[test]
    fn reads_the_release_as_ahead_only_when_it_is_above_a_version_that_runs() {
        let view: ReleaseView = serde_json::from_value(json!({ "state": "read", "latest": { "version": "0.3.0", "tag": "v0.3.0", "url": "u", "publishedAt": "p" } })).unwrap();
        assert_eq!(release_above(&view, &["0.2.0"]), true);
        assert_eq!(release_above(&view, &["0.3.0", "0.2.0"]), true);
        assert_eq!(release_above(&view, &["0.3.0"]), false);
        // A host built ahead of the newest release, a prerelease or a checkout's, reads level.
        assert_eq!(release_above(&view, &["0.4.0-rc.1"]), false);
        let checking: ReleaseView = serde_json::from_value(json!({ "state": "checking" })).unwrap();
        assert_eq!(release_above(&checking, &["0.2.0"]), false);
    }
}

mod bringing_work_back {
    use super::*;
    use regex::Regex;
    use wsp_wire::views::workspace_kind::WorkspaceKind;
    use wsp_words::projects::child_on_another_computer_line;
    use wsp_words::words::computer::*;
    use wsp_words::words::thread::refusal_line;
    use wsp_words::wire::runtime::thread_op_refusal;
    use wsp_words::workspace_state::thread_place;

    #[test]
    fn a_thread_works_on_its_own_repository_s_projects_alone_and_the_refusal_names_both() {
        let id = "thread_a1b2c3d4";
        let word = thread_word(id);
        assert_eq!(
            refusal_line(&spawn_repository_refusal(id, "landing", "docs"), SPAWN_REPOSITORY_FIX),
            format!("thread {word} works on landing, and docs is a project of another repository; a thread starts children on its own repository's projects alone. Name a project of the same repository, or ask the person to start this one.")
        );
        // The road out of a workspace outside the tree is what a run does on that project's kind of computer.
        assert_eq!(spawn_reach_fix("my lab", true), r#"Start a child on my lab instead with wsp run 'my lab' "<message>", which forks a machine in your own tree."#);
        assert_eq!(spawn_reach_fix("my lab", false), r#"Start a child in my lab's folder instead with wsp run 'my lab' "<message>", which opens a thread there in your own tree."#);
        assert_eq!(
            refusal_line(&spawn_folder_refusal(id, "lab-two"), SPAWN_FOLDER_FIX),
            format!("thread {word} may not start children in lab-two: it is a folder the person keeps of this repository, and a thread started there would stand outside your tree. Name a project wsp projects lists, or ask the person to start this one.")
        );
        assert_eq!(
            refusal_line(&spawn_repository_workspace_refusal(id, "landing", "nightly"), SPAWN_REPOSITORY_WORKSPACE_FIX),
            format!("thread {word} works on landing, and the workspace nightly holds another repository; a thread reaches its own repository's workspaces alone. Name a workspace of your own tree or a project of your own repository, or ask the person.")
        );
        // A thread on a computer the person joined reaches the lead of its tree on another computer by message alone, and
        // where no lead of its reaches the project, the person is the one who starts it there.
        assert_eq!(
            child_to_leads_computer_line("hetzner", "my mac", id),
            format!(r#"a thread on hetzner cannot start one on my mac yet; ask thread {word}, your lead, with wsp send {word} "<message>", or name a project on hetzner"#)
        );
        assert_eq!(
            child_on_another_computer_line("hetzner", "default"),
            "a thread on hetzner cannot start one on default yet; the person can start it there, or name a project on hetzner"
        );
        // A workspace of another computer is refused in the words of the act asked for there.
        assert_eq!(
            elsewhere_workspace_line("lab-one", "default", "hetzner", AcrossAct::Exec, Some(id)),
            format!(r#"lab-one is on default, and a thread on hetzner cannot run commands there yet; ask thread {word}, your lead, with wsp send {word} "<message>""#)
        );
        assert_eq!(
            elsewhere_workspace_line("theirs", "default", "hetzner", AcrossAct::Work, None),
            "theirs is on default, and a thread on hetzner cannot work there yet; ask the person"
        );
        // A switch that is off on a computer, over a folder there holding none of its own, is turned on where it is held.
        assert_eq!(
            agents_off_computer_refusal("t_1234567890", "my box", SpawnAct::ThreadNew),
            format!("thread t_123456 may not {}, since agents on my box are off; turn them on with wsp computers set 'my box' --spawn on", SpawnAct::ThreadNew.words())
        );
        // Bringing work back is one of the acts a thread may ask for, and it is named in the table like the rest.
        assert!(SPAWN_ACTS_ALLOWED.contains(&SpawnAct::BringBack));
        assert_eq!(SpawnAct::BringBack.words(), "bring its work back");
        let refused = spawn_act_refusal(id, SpawnAct::Delete, Some("hetzner"));
        assert!(refused.contains("bring its work back"), "{refused}");
        // Every refusal to a thread's own token names where that thread runs, read off the kind table, and claims nothing
        // about what a thread there may start.
        let sized = spawn_act_refusal(id, SpawnAct::Size, Some("hetzner"));
        assert!(Regex::new(r"^this request came out of thread \S+ on hetzner, and .*, never pick a machine's size$").unwrap().is_match(&sized), "{sized}");
        let sized = spawn_act_refusal(id, SpawnAct::Size, None);
        assert!(Regex::new(r"^this request came out of thread \S+, and ").unwrap().is_match(&sized), "{sized}");
        assert_eq!(
            [thread_place(&WorkspaceKind::Local, "mini"), thread_place(&WorkspaceKind::Place, "hetzner"), thread_place(&WorkspaceKind::Cloud, "solari")],
            ["mini", "hetzner", MACHINE_WSP_FORKS]
        );
        let op = thread_op_refusal("pair.issue", id, Some("hetzner"));
        assert!(!op.contains("forks machines"), "{op}");
    }
}
