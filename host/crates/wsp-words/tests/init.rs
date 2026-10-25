// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/init-job.test.ts:134-142 (initStepCounter assertions only), :189-232, :242-268,
//! :282-295, :297-303, :311-363, :377-379, :385-449 (whole, with the units functions), :480-496 (less the
//! templateStatusLine and InitRow.parse assertions), :498-506, and format.test.ts:1467-1473 (the disk sync lines).
//! Skipped: init-job.test.ts:107, :124, :144, :156, :163, :365, :370 and :234 (law: no, schema; they test InitJob,
//! InitRow, InitSetup and the event as wire schemas in init-job.ts, with INIT_ROW_STATES.open incidental),
//! :270 (threadWorkingLine, words/tools), :305 (credentialOnBuilderLine, no stub), :381 (SOLARI_CONSOLE, no stub),
//! :451 (already ported in tests/units.rs), outside-line.test.ts:10, :16, :31 (outsideLine, a different module;
//! NEEDS_YOU is incidental). Missing stubs: initShownScreens, templateStatusLine.

use regex::Regex;
use serde_json::{json, Value};
use wsp_words::pending_wire::types::init_job::{InitJob, InitPhase, InitRow, InitRowKind, LoginState, LOGIN_CHOICES};
use wsp_words::pending_wire::wire::capabilities::InitSetup;
use wsp_words::words::computer::Platform;
use wsp_words::words::init::*;
use wsp_words::words::units::*;

const MIB: f64 = 1024.0 * 1024.0;

fn merged(base: Value, over: Value) -> Value {
    let mut base = base;
    for (k, v) in over.as_object().unwrap() {
        base.as_object_mut().unwrap().insert(k.clone(), v.clone());
    }
    base
}

fn row_json(over: Value) -> Value {
    merged(json!({ "id": "stage/creating", "kind": "stage", "label": "Creating the machine", "state": "done" }), over)
}

fn row(over: Value) -> InitRow {
    serde_json::from_value(row_json(over)).unwrap()
}

fn rows_from(values: Vec<Value>) -> Vec<InitRow> {
    values.into_iter().map(|v| serde_json::from_value(v).unwrap()).collect()
}

fn state_row(state: &str, over: Value) -> InitRow {
    row(merged(json!({ "id": "x", "kind": "stage", "label": "x", "state": state }), over))
}

fn screen() -> Value {
    json!({ "id": "agents", "title": "Agents", "top": "Which agents go on the image", "items": [], "ticks": [], "answers": {}, "footer": [] })
}

fn job_rows() -> Vec<Value> {
    vec![
        row_json(json!({})),
        row_json(json!({ "id": "stage/deploying-daemon", "label": "Installing the base tools", "state": "running" })),
        row_json(json!({ "id": "stage/ready", "label": "Waiting for the machine", "state": "waiting" })),
    ]
}

fn job_json(over: Value) -> Value {
    merged(
        json!({
            "id": "init_1",
            "road": "manual",
            "phase": "building",
            "keys": { "solari": true },
            "step": 0,
            "stoppable": true,
            "screens": [],
            "rows": job_rows(),
            "progress": { "done": 1, "total": 3 },
            "log": ["Recipe saved to /tmp/recipe.json"],
        }),
        over,
    )
}

fn job(over: Value) -> InitJob {
    serde_json::from_value(job_json(over)).unwrap()
}

fn github_row() -> Value {
    row_json(json!({ "id": "sign-in/gh", "kind": "sign-in", "label": "GitHub CLI login", "state": init_row_states::OPEN, "page": "https://github.com/login/device", "code": "8F4A-C21B" }))
}

fn waiting_json() -> Value {
    let mut rows = job_rows();
    rows.push(github_row());
    job_json(json!({
        "phase": "signing-in",
        "rows": rows,
        "needsYou": { "what": "sign in to GitHub CLI login", "since": 1_760_000_000_000_i64 },
    }))
}

fn waiting() -> InitJob {
    serde_json::from_value(waiting_json()).unwrap()
}

const ALL_LOGIN_STATES: [LoginState; 6] = [
    LoginState::SignedIn,
    LoginState::NotSignedIn,
    LoginState::NotVerified,
    LoginState::Skipped,
    LoginState::Copied,
    LoginState::Deferred,
];

fn login_name(state: LoginState) -> Value {
    serde_json::to_value(state).unwrap()
}

mod the_init_job_view {
    use super::*;

    // Only the initStepCounter assertions: initShownScreens has no stub.
    #[test]
    fn a_screen_with_no_row_to_pick_is_not_shown_whichever_screen_it_is_and_a_step_s_co() {
        assert_eq!(init_step_counter(1.0, 4.0), "1/4");
        assert_eq!(init_step_counter(4.0, 4.0), "4/4");
    }
}

mod the_words_the_clients_print_for_the_job {
    use super::*;

    #[test]
    fn one_word_per_phase_in_lower_case_since_it_sits_in_a_muted_mono_slot() {
        assert_eq!(init_phase_word(InitPhase::Agent), "agent writing the recipe");
        assert_eq!(init_phase_word(InitPhase::Reading), "reading this computer");
        assert_eq!(init_phase_word(InitPhase::Answering), "waiting for you");
        assert_eq!(init_phase_word(InitPhase::Building), "building");
        assert_eq!(init_phase_word(InitPhase::SigningIn), "signing in");
        assert_eq!(init_phase_word(InitPhase::Sealing), "sealing");
        assert_eq!(init_phase_word(InitPhase::Finishing), "finishing");
        assert_eq!(init_phase_word(InitPhase::Done), "done");
        assert_eq!(init_phase_word(InitPhase::Failed), "failed");
        assert_eq!(init_phase_word(InitPhase::Cancelled), "cancelled");
    }

    #[test]
    fn the_collapsed_row_s_line_is_the_phase_and_the_count_while_it_builds_the_sign_in() {
        assert_eq!(init_progress_line(&job(json!({}))), "building 1/3");
        let mut rows = job_rows();
        rows.push(row_json(json!({ "id": "sign-in/gh", "kind": "sign-in", "label": "GitHub CLI login", "state": init_row_states::OPEN, "page": "https://github.com/login/device", "code": "8F4A-C21B" })));
        let waiting = job(json!({ "phase": "signing-in", "rows": rows }));
        assert_eq!(init_progress_line(&waiting), "sign in to GitHub CLI login");
        assert_eq!(init_progress_line(&job(json!({ "phase": "answering", "rows": [] }))), "waiting for you");
        assert_eq!(init_progress_line(&job(json!({ "phase": "failed", "rows": [] }))), "failed");
    }

    #[test]
    fn only_a_wait_the_person_is_not_looking_at_is_a_need_a_sign_in_s_open_page_never_t() {
        assert_eq!(init_need_what(&job(json!({}))), None);
        assert_eq!(init_need_what(&waiting()), Some("sign in to GitHub CLI login".to_string()));
        // The screens wait on the person, but they are the thing the person is looking at, so they are no one's need.
        // The phase is not even an argument here, which is what keeps any of them from becoming one by accident.
        for phase in ["agent", "reading", "answering"] {
            let j = job(json!({ "phase": phase, "rows": [] }));
            assert_eq!(init_need_what(&j), None, "{phase}");
        }
        // A sign-in that moved on waits on nobody, whatever it once said.
        let moved_on = {
            let mut v = waiting_json();
            let rows: Vec<Value> = v["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    if r["kind"] == "sign-in" {
                        merged(r.clone(), json!({ "state": init_sign_in_word(LoginState::SignedIn, Platform::Darwin), "login": "signed-in" }))
                    } else {
                        r.clone()
                    }
                })
                .collect();
            v["rows"] = Value::Array(rows);
            serde_json::from_value::<InitJob>(v).unwrap()
        };
        assert_eq!(init_need_what(&moved_on), None);
        // The sidebar's line and the need are one spelling, so the toast never says it a second way.
        assert_eq!(Some(init_progress_line(&waiting())), init_need_what(&waiting()));
    }

    #[test]
    fn the_toast_and_the_system_notification_say_the_app_s_name_and_the_need_and_a_titl() {
        assert_eq!(NEEDS_YOU, "wsp needs you");
        assert_eq!(init_needs_you_line("sign in to GitHub CLI login"), "wsp needs you: sign in to GitHub CLI login");
        assert_eq!(title_with_need("wsp", true), "\u{2022} wsp");
        assert_eq!(title_with_need("wsp", false), "wsp");
        assert_eq!(title_with_need(&title_with_need("wsp", true), true), "\u{2022} wsp");
        assert_eq!(title_with_need(&title_with_need("wsp", true), false), "wsp");
    }

    #[test]
    fn the_agent_s_first_message_names_the_two_tools_and_where_the_recipe_goes_asks_the() {
        let prompt = init_agent_prompt("/Users/me/.wsp/recipe.json");
        assert!(prompt.contains("recipe_scan"), "{prompt}");
        assert!(prompt.contains("recipe"), "{prompt}");
        assert!(prompt.contains("/Users/me/.wsp/recipe.json"), "{prompt}");
        assert!(Regex::new(r"(?i)ask (them|the person) nothing").unwrap().is_match(&prompt), "{prompt}");
        // The tools are named as the only road: a thread that reached for the command line instead asked for permission
        // once per call and got nowhere.
        assert!(Regex::new(r"(?i)run no commands").unwrap().is_match(&prompt), "{prompt}");
        assert!(!prompt.contains("wsp init"), "{prompt}");
    }

    #[test]
    fn the_line_when_the_thread_ended_with_no_recipe_names_the_file_waited_on_and_the_t() {
        assert_eq!(init_agent_no_recipe_line("/Users/me/.wsp/recipe.json", None), "the thread ended without writing /Users/me/.wsp/recipe.json");
        assert_eq!(
            init_agent_no_recipe_line("/Users/me/.wsp/recipe.json", Some("the harness died")),
            "the thread ended without writing /Users/me/.wsp/recipe.json: the harness died"
        );
    }

    #[test]
    fn one_predicate_says_the_job_s_step_is_the_agent_s_its_road_writing_the_recipe_and() {
        let agent = |over: Value| job(merged(json!({ "road": "agent", "screens": [] }), over));
        assert!(init_agent_step(&agent(json!({ "phase": "agent" }))));
        assert!(init_agent_step(&agent(json!({ "phase": "failed" }))));
        assert!(init_agent_step(&agent(json!({ "phase": "cancelled" }))));
        // Past the step: the screens exist, so a failure there is the build's own.
        assert!(!init_agent_step(&agent(json!({ "phase": "answering" }))));
        assert!(!init_agent_step(&agent(json!({ "phase": "failed", "screens": [screen()] }))));
        assert!(!init_agent_step(&job(json!({ "road": "manual", "phase": "failed" }))));
    }

    #[test]
    fn wsp_setup_prints_the_keys_as_held_or_not_the_agents_with_their_tools_the_price_a() {
        let setup_json = json!({
            "keys": { "box": false, "solari": true },
            "home": "/Users/me",
            "agents": [
                { "id": "claude", "name": "Claude Code", "configured": true, "takesTools": true },
                { "id": "codex", "name": "Codex", "configured": false, "takesTools": false },
            ],
            "pricing": { "size": { "cpu": 2, "memMb": 4096 }, "rateUsdPerHour": 0.11 },
            "job": null,
        });
        let setup: InitSetup = serde_json::from_value(setup_json.clone()).unwrap();
        // One line per provider this computer can hold a key for, each in that provider's own words for its key.
        assert_eq!(
            init_setup_lines(&setup),
            vec![
                "Boat API key: not set",
                "Solari API key: saved",
                "Agents here: Claude Code (MCP added), Codex",
                "A 2 vCPU, 4 GB workspace costs about $0.11 an hour while it runs and naps when idle",
                "No setup is running; the app's Image section starts one.",
            ]
        );
        let waiting_job = job_json(json!({
            "phase": "signing-in",
            "rows": [row_json(json!({})), row_json(json!({ "id": "sign-in/gh", "kind": "sign-in", "label": "GitHub CLI login", "state": init_row_states::OPEN, "page": "https://github.com/login/device", "code": "8F4A-C21B" }))],
        }));
        let with_job: InitSetup = serde_json::from_value(merged(setup_json, json!({ "job": waiting_job }))).unwrap();
        assert_eq!(
            init_setup_lines(&with_job)[4..].to_vec(),
            vec!["Setup on the manual road: sign in to GitHub CLI login", "  Creating the machine: done", "  GitHub CLI login: waiting for you https://github.com/login/device code 8F4A-C21B"]
        );
        let printed = serde_json::to_string(&init_setup_lines(&setup)).unwrap();
        assert!(!Regex::new(r"slr_live|sk-ant").unwrap().is_match(&printed), "{printed}");
    }

    #[test]
    fn the_sign_in_answers_offer_six_words_each_with_the_words_its_row_and_its_counts_p() {
        let names: Vec<Value> = LOGIN_CHOICES.iter().map(|c| serde_json::to_value(c).unwrap()).collect();
        assert_eq!(names, vec![json!("copy"), json!("machine"), json!("later"), json!("key"), json!("skip"), json!("token")]);
        for value in LOGIN_CHOICES {
            assert!(!sign_in_answer_label(value, Platform::Darwin).is_empty(), "{value:?}");
        }
        assert_eq!(sign_in_choices(Platform::Darwin).iter().map(|c| c.value).collect::<Vec<_>>(), LOGIN_CHOICES.to_vec());
        assert_eq!(sign_in_choice(wsp_words::pending_wire::types::init_job::LoginChoice::Token, Platform::Darwin).label, "token from this computer");
        assert_eq!(sign_in_answer_short(wsp_words::pending_wire::types::init_job::LoginChoice::Token), "token");
    }

    #[test]
    fn a_row_s_state_words_have_one_table_and_one_predicate_says_which_of_them_end_the() {
        for word in ["done", "failed", "stopped", "forked", "imported", "MCP added", "key set", "not made", "gone", "skipped"] {
            assert!(init_row_over(&state_row(word, json!({}))), "{word}");
        }
        for word in ["waiting", "running", "forking", "importing", "waiting for you", "waiting for a machine slot", "machine still running, retrying"] {
            assert!(!init_row_over(&state_row(word, json!({}))), "{word}");
        }
        // A sign-in ends on its outcome, never on the sentence it prints: the word a Linux host drew ends the row the
        // same as a Mac's, and a sentence with no outcome beside it ends nothing.
        for state in ALL_LOGIN_STATES {
            let r = state_row(&init_sign_in_word(state, Platform::Linux), json!({ "login": login_name(state) }));
            assert!(init_row_over(&r), "{state:?}");
        }
        assert!(init_row_over(&state_row("copied from this computer", json!({ "login": "copied" }))));
        assert!(!init_row_over(&state_row("copied from this Mac", json!({}))), "a sentence nothing names");
        // The app's row words for a sign-in's outcome, drawn for the computer the run read: done once the machine has
        // the credential, the copy named with that computer.
        for (state, expected) in [
            (LoginState::SignedIn, "done"),
            (LoginState::NotSignedIn, "not signed in"),
            (LoginState::Copied, "copied from this Mac"),
            (LoginState::NotVerified, "not verified"),
            (LoginState::Skipped, "skipped"),
            (LoginState::Deferred, SIGN_IN_DEFERRED_WORD),
        ] {
            assert_eq!(init_sign_in_word(state, Platform::Darwin), expected, "{state:?}");
        }
        for (state, expected) in [
            (LoginState::SignedIn, "done"),
            (LoginState::NotSignedIn, "not signed in"),
            (LoginState::Copied, "copied from this computer"),
            (LoginState::NotVerified, "not verified"),
            (LoginState::Skipped, "skipped"),
            (LoginState::Deferred, SIGN_IN_DEFERRED_WORD),
        ] {
            assert_eq!(init_sign_in_word(state, Platform::Linux), expected, "{state:?}");
        }
    }

    #[test]
    fn the_sidebar_s_line_says_a_machine_an_earlier_build_left_is_still_going_before_it() {
        let at = |id: &str, state: &str| -> Value { json!({ "id": id, "kind": "stage", "label": id, "state": state }) };
        let building_rows = vec![at("stage/creating", "done"), at("stage/ready", "running")];
        let building = |rows: Vec<Value>| job(json!({ "phase": "building", "rows": rows, "progress": { "done": 1, "total": 2 } }));
        assert_eq!(init_progress_line(&building(building_rows.clone())), "building 1/2");
        let left = |state: &str| -> Value { json!({ "id": "machine/b_1", "kind": "machine", "label": "Builder b_1", "state": state }) };
        let with_left = |state: &str| -> Vec<Value> {
            let mut rows = building_rows.clone();
            rows.push(left(state));
            rows
        };
        assert!(!init_sweeping(&rows_from(building_rows.clone())));
        assert!(init_sweeping(&rows_from(with_left(init_row_states::RETRYING))));
        assert_eq!(init_progress_line(&building(with_left(init_row_states::RETRYING))), MACHINE_SWEEP_LINE);
        // Once the provider took it the line goes back to the job the person is looking at.
        assert_eq!(init_progress_line(&building(with_left(init_row_states::GONE))), "building 1/2");
        // A machine row's name is words, never a bare provider id in a column of sentences.
        // No provider id in a row's name or a sentence: the builder is the builder, and a stopped build's machine is the machine.
        assert_eq!(MACHINE_ROW_LABEL, "The builder");
        assert_eq!(MACHINE_GONE_LINE, "The machine is gone; nothing is billing.");
        let said = format!("{MACHINE_ROW_LABEL} {MACHINE_GONE_LINE} {MACHINE_SWEEP_LINE}");
        assert!(!Regex::new(r"b_[a-z0-9]{6,}").unwrap().is_match(&said), "{said}");
        // A rollback the provider refused is a line on the stage's block with the provider's own words, never the headline.
        assert_eq!(machine_left_line("getaddrinfo ENOTFOUND api.getsolari.com"), "the machine could not be removed and bills on: getaddrinfo ENOTFOUND api.getsolari.com");
    }

    #[test]
    fn a_stopped_build_s_sentence_is_this_computer_s_own_word_for_what_happened_and_a_r() {
        // The provider's client raises one fetch failure per call, so an outage arrives as the same line repeated.
        assert_eq!(init_stopped_line("fetch failed; fetch failed"), NETWORK_LOST_LINE);
        assert_eq!(init_stopped_line("getaddrinfo ENOTFOUND api.getsolari.com"), NETWORK_LOST_LINE);
        assert_eq!(init_stopped_line("connect ECONNREFUSED 127.0.0.1:443; fetch failed"), NETWORK_LOST_LINE);
        // A refusal the provider gave is the provider's word, and it stands as it is.
        assert_eq!(init_stopped_line("Sandbox limit reached"), "Sandbox limit reached");
        assert_eq!(init_stopped_line("Unauthorized; fetch failed"), "Unauthorized; fetch failed");
        assert_eq!(init_stopped_line("the snapshot failed; the snapshot failed"), "the snapshot failed");
        // The stop line the run writes names the stage the same way wherever it is read.
        assert_eq!(init_stage_while("Creating the machine"), "while creating the machine");
    }

    #[test]
    fn one_predicate_says_the_job_is_over_and_one_that_it_is_building_so_no_client_spel() {
        use InitPhase::*;
        assert_eq!([Done, Failed, Cancelled].map(init_job_over), [true, true, true]);
        assert_eq!([Agent, Reading, Answering, Building, SigningIn, Sealing, Finishing].map(init_job_over), [false, false, false, false, false, false, false]);
        assert_eq!([Building, SigningIn, Sealing, Finishing].map(init_job_building), [true, true, true, true]);
        assert_eq!([Agent, Reading, Answering, Done, Failed, Cancelled].map(init_job_building), [false, false, false, false, false, false]);
    }

    // Constants only: passes against the stubs.
    #[test]
    #[ignore = "checks constants alone, which the interface copied by script; no stub to fail on"]
    fn the_first_workspace_s_default_name_lives_here_so_the_terminal_and_the_host_read() {
        assert_eq!(FIRST_WORKSPACE, "first");
    }

    // Constants only: passes against the stubs.
    #[test]
    #[ignore = "checks constants alone, which the interface copied by script; no stub to fail on"]
    fn every_step_has_a_sentence_under_its_title_and_no_title_or_sentence_ends_in_a_per() {
        use wsp_words::words::init::cloud_setup_words::{agent, build, choice, reading};
        assert!(!build::SLIDE_TOP.ends_with('.') && !build::SLIDE_TOP.ends_with(';'));
        for (headline, top) in [(choice::HEADLINE, choice::TOP), (reading::HEADLINE, reading::TOP), (agent::HEADLINE, agent::TOP), (build::HEADLINE, build::TOP)] {
            assert!(top.chars().count() > 20, "{top}");
            assert!(!top.ends_with('.'), "{top}");
            assert!(!headline.ends_with('.'), "{headline}");
        }
    }

    #[test]
    fn the_tally_the_disk_words_and_a_row_s_calls_come_from_one_formatter_each_whole_un() {
        assert_eq!((init_tally_count(3.0, "agents"), fmt_bytes_of_total(1.1 * 1024.0 * MIB, None)), ("3 agents on the image".to_string(), "1.1 GB".to_string()));
        assert_eq!((init_tally_count(1.0, "tools"), fmt_bytes_of_total(60.0 * MIB, None)), ("1 tool on the image".to_string(), "60 MB".to_string()));
        assert_eq!((init_tally_count(0.0, "agents"), fmt_bytes_of_total(0.0, None)), ("0 agents on the image".to_string(), "0 B".to_string()));
        assert_eq!((init_tally_count(3.0, "more"), fmt_bytes_of_total(1.2 * 1024.0 * MIB, None)), ("3 more on the image".to_string(), "1.2 GB".to_string()));
        assert_eq!((init_tally_count(1.0, "more"), fmt_bytes_of_total(MIB, None)), ("1 more on the image".to_string(), "1 MB".to_string()));
        assert_eq!(init_disk_line(1.1 * 1024.0 * MIB, 20.0 * 1024.0 * MIB), "about 1.1 GB of 20 GB on the image");
        // Past the disk the tooltip carries the overshoot too: said there and in Continue's refusal, nowhere else.
        assert_eq!(init_disk_line(21.0 * 1024.0 * MIB, 20.0 * 1024.0 * MIB), "about 21 GB of 20 GB on the image, over by 1 GB");
        // The sign-in stage with a run-out is a failure to draw as one, beside the stage that failed; nothing else is.
        assert_eq!(
            [
                init_row_failed(&state_row(init_row_states::FAILED, json!({}))),
                init_row_failed(&state_row(&init_sign_in_word(LoginState::NotSignedIn, Platform::Linux), json!({ "login": "not-signed-in" }))),
                init_row_failed(&state_row(init_row_states::DONE, json!({}))),
                init_row_failed(&state_row(init_row_states::STOPPED, json!({}))),
            ],
            [true, true, false, false]
        );
        // The sentence alone says nothing: a row that carries no outcome is no failure, whatever its word reads.
        assert!(!init_row_failed(&state_row("not signed in", json!({}))));
        // A sign-in the build never waited out is over and is no failure: the person chose to sign in later, or the
        // build's cap ended a wait they were not going to finish, and neither is the machine's doing.
        let deferred_state = init_sign_in_word(LoginState::Deferred, Platform::Darwin);
        let deferred_row = state_row(&deferred_state, json!({ "login": "deferred" }));
        assert_eq!([init_row_failed(&deferred_row), init_row_over(&deferred_row), init_row_unrun(&deferred_state)], [false, true, false]);
        let rows = rows_from(vec![
            json!({ "id": "agent/claude", "kind": "agent", "label": "Claude Code", "state": "MCP added" }),
            json!({ "id": "stage/creating", "kind": "stage", "label": "Creating the machine", "state": "done" }),
            json!({ "id": "sign-in/gh", "kind": "sign-in", "tool": "gh", "label": "Sign in to GitHub CLI", "state": "waiting for you" }),
            json!({ "id": "sign-in/claude", "kind": "sign-in", "tool": "claude", "label": "Sign in to Claude Code", "state": "key set" }),
            json!({ "id": "stage/snapshotting", "kind": "stage", "label": "Taking the snapshot", "state": "waiting" }),
            json!({ "id": "workspace/first", "kind": "workspace", "label": "first", "state": "waiting" }),
        ]);
        let patch = |is: &dyn Fn(&InitRow) -> bool, over: Value| -> Vec<InitRow> {
            rows.iter().map(|r| if is(r) { serde_json::from_value(merged(serde_json::to_value(r).unwrap(), over.clone())).unwrap() } else { r.clone() }).collect()
        };
        let is_gh = |r: &InitRow| r.id == "sign-in/gh";
        let folded = init_build_rows(&rows);
        assert_eq!(folded.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec!["stage/creating", SIGN_IN_STAGE_ID, "stage/snapshotting", "workspace/first"]);
        assert_eq!(folded.rows[1].kind, InitRowKind::Stage);
        assert_eq!(folded.rows[1].label, "Signing in on the computer");
        assert_eq!(folded.rows[1].state, "waiting for you");
        assert_eq!(folded.sign_ins.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec!["sign-in/gh", "sign-in/claude"]);
        let settled = init_build_rows(&patch(&is_gh, json!({ "state": "not signed in", "login": "not-signed-in" })));
        // a sign-in that ran out keeps the stage from reading done and hands it its own outcome
        assert_eq!(settled.rows[1].state, "not signed in");
        assert_eq!(settled.rows[1].login, Some(LoginState::NotSignedIn));
        // The count reads the same answer the cross does: a fold whose sign-in ran out is over, and it is not done.
        assert_eq!(init_stage_count(&settled.rows), StageCount { done: 1, total: 3 });
        // A sign-in left to first use, or one the cap ended, folds as a stage that is over and done: the count and the
        // glyph read the same answer, and a build that deferred every sign-in still reads complete.
        let deferred = init_build_rows(&patch(&is_gh, json!({ "state": init_sign_in_word(LoginState::Deferred, Platform::Darwin), "login": "deferred" })));
        assert_eq!(deferred.rows[1].state, "done");
        assert_eq!(init_stage_count(&deferred.rows), StageCount { done: 2, total: 3 });
        // The stage folds on the outcomes, so a run on a Linux computer folds where a Mac's does, on the words that computer drew.
        let linux = init_build_rows(&patch(&is_gh, json!({ "state": init_sign_in_word(LoginState::Copied, Platform::Linux), "login": "copied" })));
        assert_eq!(linux.rows[1].state, "done");
        assert_eq!(init_build_rows(&patch(&is_gh, json!({ "state": "done", "login": "signed-in" }))).rows[1].state, "done");
        assert_eq!(init_build_rows(&patch(&|r| r.kind == InitRowKind::SignIn, json!({ "state": "waiting" }))).rows[1].state, "waiting");
        // Stages alone: the bar measures the build, so the first workspace beside them moves neither end of it.
        assert_eq!(init_stage_count(&folded.rows), StageCount { done: 1, total: 3 });
        assert_eq!(init_stage_count(&[row(json!({ "state": "done" })), row(json!({ "id": "stage/ready", "state": "failed" }))]), StageCount { done: 1, total: 2 }, "a failed row is not done");
        assert_eq!(init_stage_count(&[row(json!({ "state": "done" })), row(json!({ "id": "stage/ready", "state": "stopped" }))]), StageCount { done: 1, total: 2 }, "nor is a stopped one");
        // A machine an earlier build left behind is not a stage, so it moves neither end of the count either.
        assert_eq!(
            init_stage_count(&[row(json!({ "state": "done" })), row(json!({ "id": "machine/b_1", "kind": "machine", "label": "Builder b_1", "state": "machine still running, retrying" }))]),
            StageCount { done: 1, total: 1 }
        );
        assert_eq!(init_stage_count_line(&StageCount { done: 3, total: 12 }), "3 of 12");
        assert_eq!(
            [size_tone(1024.0 * MIB), size_tone(1024.0 * MIB - 1.0), size_tone(200.0 * MIB), size_tone(200.0 * MIB - 1.0), size_tone(50.0 * MIB), size_tone(50.0 * MIB - 1.0), size_tone(0.0)],
            [SizeTone::Danger, SizeTone::Warning, SizeTone::Warning, SizeTone::Yellow, SizeTone::Yellow, SizeTone::Muted, SizeTone::Muted]
        );
        assert_eq!(
            [disk_tone(13.0, 20.0), disk_tone(14.0, 20.0), disk_tone(18.0, 20.0), disk_tone(21.0, 20.0), disk_tone(1.0, 0.0)],
            [SizeTone::Muted, SizeTone::Warning, SizeTone::Danger, SizeTone::Danger, SizeTone::Danger]
        );
        assert_eq!(init_disk_over_line(300.0 * MIB), "over by 300 MB");
        assert_eq!(fmt_calls(29_623.0), "29,623 calls");
        assert_eq!(fmt_calls(1.0), "1 call");
    }

    // Skipped here: the templateStatusLine assertions (no stub) and the InitRow.parse assertion on `since` (schema).
    #[test]
    fn a_sign_in_s_sentence_names_what_the_machine_waits_on_and_never_a_command_the_sea() {
        assert_eq!(init_sign_in_line(&state_row(init_row_states::OPEN, json!({ "code": "8F4A-C21B" }))), Some("waiting for the code".to_string()));
        assert_eq!(init_sign_in_line(&state_row(init_row_states::OPEN, json!({ "finish": "code" }))), Some("waiting for the code".to_string()));
        assert_eq!(init_sign_in_line(&state_row(init_row_states::OPEN, json!({}))), Some("the page is open on this computer".to_string()));
        assert_eq!(init_sign_in_line(&state_row(&init_sign_in_word(LoginState::Copied, Platform::Darwin), json!({}))), None);
        assert_eq!(init_sign_in_line(&state_row(init_row_states::DONE, json!({}))), None);
        assert_eq!(init_sign_in_line(&state_row(&init_sign_in_word(LoginState::NotSignedIn, Platform::Darwin), json!({}))), None);
        assert_eq!(snapshot_stage_line(Some(13.0 * 1024.0 * MIB)), "snapshotting about 13 GB, usually under a minute");
        assert_eq!(snapshot_stage_line(None), "snapshotting, usually under a minute");
        assert_eq!(SAVING_IMAGE_LINE, "saving the image");
        // The two stages the provider says nothing during count their own seconds; the wire carries the clock's start.
        let by_id = |id: &str| row(json!({ "id": id }));
        assert_eq!(
            [init_row_timed(&by_id("stage/snapshotting")), init_row_timed(&by_id("stage/promoting")), init_row_timed(&by_id("stage/creating")), init_row_timed(&by_id("sign-in/gh"))],
            [true, true, false, false]
        );
        assert_eq!([init_elapsed_line(0.0), init_elapsed_line(999.0), init_elapsed_line(41_400.0), init_elapsed_line(72_000.0)], ["0s", "0s", "41s", "1m 12s"]);
    }

    // Constants only: passes against the stubs. The TS stringifies the whole table, keys included; the strings are
    // listed here from the module's constants, which hold no key with these words.
    #[test]
    #[ignore = "checks constants alone, which the interface copied by script; no stub to fail on"]
    fn the_screens_words_never_ask_the_person_to_run_a_command_and_never_say_cloud_mach() {
        let text = ALL_WORDS.join("\n");
        assert!(!Regex::new(r"(?i)wsp init|terminal").unwrap().is_match(&text), "{text}");
        // These screens are about the image; a computer and a provider are PLACES_WORDS and are said nowhere here.
        assert!(!Regex::new(r"(?i)cloud machines").unwrap().is_match(&text), "{text}");
        // And the word table's own rule: setup is never the noun for what is built here.
        assert!(!Regex::new(r"(?i)\bsetup\b").unwrap().is_match(&text), "{text}");
        assert_eq!(cloud_setup_words::choice::HEADLINE, "What goes on your image");
    }
}

mod disk_sync_line_diskunsettledline_and_disksyncfailedline {
    use super::*;

    #[test]
    fn the_sync_s_stage_line_what_a_writer_left_after_it_and_the_refusal_when_the_guest() {
        assert_eq!(DISK_SYNC_LINE, "syncing the disk");
        assert_eq!(disk_unsettled_line(12288.0, 0.0), "synced, Dirty 12 MB and Writeback 0 B remain; a writer is still running");
        assert_eq!(disk_sync_failed_line("it exited 1 and said: sync: Input/output error"), "the disk could not be synced (it exited 1 and said: sync: Input/output error); nothing was snapshotted");
    }
}

const ALL_WORDS: &[&str] = &[
    cloud_setup_words::choice::HEADLINE,
    cloud_setup_words::choice::TOP,
    cloud_setup_words::choice::MANUAL,
    cloud_setup_words::choice::AGENT,
    cloud_setup_words::choice::AGENT_WITH,
    cloud_setup_words::choice::NO_TOOLS,
    cloud_setup_words::choice::NONE,
    cloud_setup_words::choice::KEYCAP,
    cloud_setup_words::keys::SAVED,
    cloud_setup_words::keys::UNSET,
    cloud_setup_words::keys::CHANGE_KEY,
    cloud_setup_words::screen::ALWAYS_ON,
    cloud_setup_words::screen::ALWAYS_ON_WHY,
    cloud_setup_words::screen::KEYCAP,
    cloud_setup_words::screen::BACK,
    cloud_setup_words::screen::AGAIN,
    cloud_setup_words::screen::NOT_ON_IMAGE,
    cloud_setup_words::screen::LEFT_ALONE,
    cloud_setup_words::screen::ASKS_YOU,
    cloud_setup_words::build::HEADLINE,
    cloud_setup_words::build::TOP,
    cloud_setup_words::build::SIGNING_IN,
    cloud_setup_words::build::SLIDE_HEADLINE,
    cloud_setup_words::build::SLIDE_TOP,
    cloud_setup_words::build::OPEN,
    cloud_setup_words::build::RETRY,
    cloud_setup_words::build::CODE_ASK,
    cloud_setup_words::build::CODE_SUBMIT,
    cloud_setup_words::build::CANCEL,
    cloud_setup_words::build::CANNOT_STOP,
    cloud_setup_words::build::CANCEL_SURE,
    cloud_setup_words::build::CANCEL_WHY,
    cloud_setup_words::build::CANCEL_KEEP,
    cloud_setup_words::build::DONE,
    cloud_setup_words::build::FAILED,
    cloud_setup_words::build::STOPPED,
    cloud_setup_words::build::AGAIN,
    cloud_setup_words::agent::HEADLINE,
    cloud_setup_words::agent::TOP,
    cloud_setup_words::agent::TITLE,
    cloud_setup_words::agent::OPEN,
    cloud_setup_words::agent::FAILED,
    cloud_setup_words::agent::STOPPED,
    cloud_setup_words::agent::RETRY,
    cloud_setup_words::agent::AGAIN,
    cloud_setup_words::agent::WAITING,
    cloud_setup_words::reading::HEADLINE,
    cloud_setup_words::reading::TOP,
    cloud_setup_words::reading::FIRST,
];


// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::words::init::*;

    use serde_json::json;
    use wsp_words::pending_wire::types::init_job::{InitPhase, InitRow, LoginChoice, LoginState};
    use wsp_words::pending_wire::views::golden_image::GoldenStage;
    use wsp_words::pending_wire::wire::capabilities::WorkspaceSize;
    use wsp_words::words::computer::Platform;

    fn wire<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
        serde_json::from_value(v).unwrap()
    }

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }

    #[test]
    fn every_phase_login_choice_and_stage_has_one_word() {
        assert_eq!(init_phase_word(InitPhase::Agent), "agent writing the recipe"); // initPhaseWord("agent")
        assert_eq!(init_phase_word(InitPhase::Reading), "reading this computer"); // initPhaseWord("reading")
        assert_eq!(init_phase_word(InitPhase::Answering), "waiting for you"); // initPhaseWord("answering")
        assert_eq!(init_phase_word(InitPhase::Building), "building"); // initPhaseWord("building")
        assert_eq!(init_phase_word(InitPhase::SigningIn), "signing in"); // initPhaseWord("signing-in")
        assert_eq!(init_phase_word(InitPhase::Sealing), "sealing"); // initPhaseWord("sealing")
        assert_eq!(init_phase_word(InitPhase::Finishing), "finishing"); // initPhaseWord("finishing")
        assert_eq!(init_phase_word(InitPhase::Done), "done"); // initPhaseWord("done")
        assert_eq!(init_phase_word(InitPhase::Failed), "failed"); // initPhaseWord("failed")
        assert_eq!(init_phase_word(InitPhase::Cancelled), "cancelled"); // initPhaseWord("cancelled")
        assert_eq!(login_state_word(LoginState::SignedIn), "signed in"); // LOGIN_STATE_WORDS["signed-in"]
        assert_eq!(login_state_word(LoginState::NotSignedIn), "not signed in"); // LOGIN_STATE_WORDS["not-signed-in"]
        assert_eq!(login_state_word(LoginState::NotVerified), "not verified"); // LOGIN_STATE_WORDS["not-verified"]
        assert_eq!(login_state_word(LoginState::Skipped), "skipped"); // LOGIN_STATE_WORDS["skipped"]
        assert_eq!(login_state_word(LoginState::Copied), "copied"); // LOGIN_STATE_WORDS["copied"]
        assert_eq!(login_state_word(LoginState::Deferred), "not signed in, sign in when you first need it"); // LOGIN_STATE_WORDS["deferred"]
        assert_eq!(init_sign_in_word(LoginState::SignedIn, Platform::Darwin), "done"); // INIT_SIGN_IN_WORDS["signed-in"]("darwin")
        assert_eq!(init_sign_in_word(LoginState::SignedIn, Platform::Linux), "done"); // INIT_SIGN_IN_WORDS["signed-in"]("linux")
        assert_eq!(init_sign_in_word(LoginState::NotSignedIn, Platform::Darwin), "not signed in"); // INIT_SIGN_IN_WORDS["not-signed-in"]("darwin")
        assert_eq!(init_sign_in_word(LoginState::NotSignedIn, Platform::Linux), "not signed in"); // INIT_SIGN_IN_WORDS["not-signed-in"]("linux")
        assert_eq!(init_sign_in_word(LoginState::NotVerified, Platform::Darwin), "not verified"); // INIT_SIGN_IN_WORDS["not-verified"]("darwin")
        assert_eq!(init_sign_in_word(LoginState::NotVerified, Platform::Linux), "not verified"); // INIT_SIGN_IN_WORDS["not-verified"]("linux")
        assert_eq!(init_sign_in_word(LoginState::Skipped, Platform::Darwin), "skipped"); // INIT_SIGN_IN_WORDS["skipped"]("darwin")
        assert_eq!(init_sign_in_word(LoginState::Skipped, Platform::Linux), "skipped"); // INIT_SIGN_IN_WORDS["skipped"]("linux")
        assert_eq!(init_sign_in_word(LoginState::Copied, Platform::Darwin), "copied from this Mac"); // INIT_SIGN_IN_WORDS["copied"]("darwin")
        assert_eq!(init_sign_in_word(LoginState::Copied, Platform::Linux), "copied from this computer"); // INIT_SIGN_IN_WORDS["copied"]("linux")
        assert_eq!(init_sign_in_word(LoginState::Deferred, Platform::Darwin), "not signed in, sign in when you first need it"); // INIT_SIGN_IN_WORDS["deferred"]("darwin")
        assert_eq!(init_sign_in_word(LoginState::Deferred, Platform::Linux), "not signed in, sign in when you first need it"); // INIT_SIGN_IN_WORDS["deferred"]("linux")
        assert_eq!(sign_in_answer_label(LoginChoice::Copy, Platform::Darwin), "copy from this Mac"); // SIGN_IN_ANSWERS["copy"].label("darwin")
        assert_eq!(sign_in_answer_short(LoginChoice::Copy), "copy"); // SIGN_IN_ANSWERS["copy"].short
        assert_eq!(sign_in_answer_label(LoginChoice::Machine, Platform::Darwin), "sign in during the build"); // SIGN_IN_ANSWERS["machine"].label("darwin")
        assert_eq!(sign_in_answer_short(LoginChoice::Machine), "during the build"); // SIGN_IN_ANSWERS["machine"].short
        assert_eq!(sign_in_answer_label(LoginChoice::Later, Platform::Darwin), "sign in when you first need it"); // SIGN_IN_ANSWERS["later"].label("darwin")
        assert_eq!(sign_in_answer_short(LoginChoice::Later), "when you need it"); // SIGN_IN_ANSWERS["later"].short
        assert_eq!(sign_in_answer_label(LoginChoice::Key, Platform::Darwin), "API key"); // SIGN_IN_ANSWERS["key"].label("darwin")
        assert_eq!(sign_in_answer_short(LoginChoice::Key), "API key"); // SIGN_IN_ANSWERS["key"].short
        assert_eq!(sign_in_answer_label(LoginChoice::Skip, Platform::Darwin), "skip"); // SIGN_IN_ANSWERS["skip"].label("darwin")
        assert_eq!(sign_in_answer_short(LoginChoice::Skip), "skip"); // SIGN_IN_ANSWERS["skip"].short
        assert_eq!(sign_in_answer_label(LoginChoice::Token, Platform::Darwin), "token from this computer"); // SIGN_IN_ANSWERS["token"].label("darwin")
        assert_eq!(sign_in_answer_short(LoginChoice::Token), "token"); // SIGN_IN_ANSWERS["token"].short
        assert_eq!(js(&sign_in_choices(Platform::Linux)), "[{\"value\":\"copy\",\"label\":\"copy from this computer\"},{\"value\":\"machine\",\"label\":\"sign in during the build\"},{\"value\":\"later\",\"label\":\"sign in when you first need it\"},{\"value\":\"key\",\"label\":\"API key\"},{\"value\":\"skip\",\"label\":\"skip\"},{\"value\":\"token\",\"label\":\"token from this computer\"}]"); // JSON.stringify(signInChoices("linux"))
        assert_eq!(js(&init_sign_in_outcome(LoginState::Copied, Platform::Darwin)), "{\"state\":\"copied from this Mac\",\"login\":\"copied\"}"); // JSON.stringify(initSignInOutcome("copied", "darwin"))
        assert_eq!(golden_stage_word(GoldenStage::Creating), Some("Creating the machine")); // GOLDEN_STAGE_WORDS["creating"]
        assert_eq!(golden_stage_word(GoldenStage::DeployingDaemon), Some("Installing the base tools")); // GOLDEN_STAGE_WORDS["deploying-daemon"]
        assert_eq!(golden_stage_word(GoldenStage::ApplyingSetup), Some("Applying your setup")); // GOLDEN_STAGE_WORDS["applying-setup"]
        assert_eq!(golden_stage_word(GoldenStage::UploadingFiles), Some("Copying your files")); // GOLDEN_STAGE_WORDS["uploading-files"]
        assert_eq!(golden_stage_word(GoldenStage::InstallingHarness), Some("Installing agents")); // GOLDEN_STAGE_WORDS["installing-harness"]
        assert_eq!(golden_stage_word(GoldenStage::InstallingTools), Some("Installing tools")); // GOLDEN_STAGE_WORDS["installing-tools"]
        assert_eq!(golden_stage_word(GoldenStage::InstallingMcp), Some("Installing MCP servers")); // GOLDEN_STAGE_WORDS["installing-mcp"]
        assert_eq!(golden_stage_word(GoldenStage::Ready), Some("Checking the machine answers")); // GOLDEN_STAGE_WORDS["ready"]
        assert_eq!(golden_stage_word(GoldenStage::Snapshotting), Some("Taking the snapshot")); // GOLDEN_STAGE_WORDS["snapshotting"]
        assert_eq!(golden_stage_word(GoldenStage::Promoting), Some("Saving the image")); // GOLDEN_STAGE_WORDS["promoting"]
        assert_eq!(golden_stage_word(GoldenStage::SmokeForking), Some("Checking a fork boots")); // GOLDEN_STAGE_WORDS["smoke-forking"]
        assert_eq!(golden_stage_word(GoldenStage::Sealed), Some("Finishing")); // GOLDEN_STAGE_WORDS["sealed"]
        assert_eq!(golden_stage_word(GoldenStage::Failed), None); // GOLDEN_STAGE_WORDS["failed"]
        assert_eq!(init_stage_row_id(GoldenStage::Creating), "stage/creating"); // initStageRowId("creating")
        assert_eq!(init_stage_row_id(GoldenStage::DeployingDaemon), "stage/deploying-daemon"); // initStageRowId("deploying-daemon")
        assert_eq!(init_stage_row_id(GoldenStage::ApplyingSetup), "stage/applying-setup"); // initStageRowId("applying-setup")
        assert_eq!(init_stage_row_id(GoldenStage::UploadingFiles), "stage/uploading-files"); // initStageRowId("uploading-files")
        assert_eq!(init_stage_row_id(GoldenStage::InstallingHarness), "stage/installing-harness"); // initStageRowId("installing-harness")
        assert_eq!(init_stage_row_id(GoldenStage::InstallingTools), "stage/installing-tools"); // initStageRowId("installing-tools")
        assert_eq!(init_stage_row_id(GoldenStage::InstallingMcp), "stage/installing-mcp"); // initStageRowId("installing-mcp")
        assert_eq!(init_stage_row_id(GoldenStage::Ready), "stage/ready"); // initStageRowId("ready")
        assert_eq!(init_stage_row_id(GoldenStage::Snapshotting), "stage/snapshotting"); // initStageRowId("snapshotting")
        assert_eq!(init_stage_row_id(GoldenStage::Promoting), "stage/promoting"); // initStageRowId("promoting")
        assert_eq!(init_stage_row_id(GoldenStage::SmokeForking), "stage/smoke-forking"); // initStageRowId("smoke-forking")
        assert_eq!(init_stage_row_id(GoldenStage::Sealed), "stage/sealed"); // initStageRowId("sealed")
        assert_eq!(init_stage_row_id(GoldenStage::Failed), "stage/failed"); // initStageRowId("failed")
    }

    #[test]
    fn stage_rows_clocks_and_snapshot_lines() {
        assert_eq!(init_row_timed(&wire(json!({"id":"stage/snapshotting","kind":"stage","label":"L","state":"waiting"}))), true); // initRowTimed({id:"stage/snapshotting"})
        assert_eq!(init_row_timed(&wire(json!({"id":"stage/promoting","kind":"stage","label":"L","state":"waiting"}))), true); // initRowTimed({id:"stage/promoting"})
        assert_eq!(init_row_timed(&wire(json!({"id":"stage/creating","kind":"stage","label":"L","state":"waiting"}))), false); // initRowTimed({id:"stage/creating"})
        assert_eq!(init_row_timed(&wire(json!({"id":"snapshotting","kind":"stage","label":"L","state":"waiting"}))), false); // initRowTimed({id:"snapshotting"})
        assert_eq!(init_elapsed_line(-5.0), "0s"); // initElapsedLine(-5)
        assert_eq!(init_elapsed_line(0.0), "0s"); // initElapsedLine(0)
        assert_eq!(init_elapsed_line(999.0), "0s"); // initElapsedLine(999)
        assert_eq!(init_elapsed_line(59999.0), "59s"); // initElapsedLine(59999)
        assert_eq!(init_elapsed_line(60000.0), "1m"); // initElapsedLine(60000)
        assert_eq!(init_elapsed_line(61500.0), "1m 2s"); // initElapsedLine(61500)
        assert_eq!(disk_unsettled_line(0.5, 2048.0), "synced, Dirty 512 B and Writeback 2 MB remain; a writer is still running"); // diskUnsettledLine({dirtyKb:0.5,writebackKb:2048})
        assert_eq!(snapshot_stage_line(None), "snapshotting, usually under a minute"); // snapshotStageLine(undefined)
        assert_eq!(snapshot_stage_line(Some(5368709120.0)), "snapshotting about 5 GB, usually under a minute"); // snapshotStageLine(5*1024**3)
        assert_eq!(snapshot_progress_line(1024.0, None), "snapshotting, 1 KB written"); // snapshotProgressLine(1024, undefined)
        assert_eq!(snapshot_progress_line(1024.0, Some(4096.0)), "snapshotting, 1 KB of about 4 KB written"); // snapshotProgressLine(1024, 4096)
        assert_eq!(init_sign_in_line(&wire(json!({"id":"r","kind":"stage","label":"L","state":"running"}))), None); // initSignInLine({"id":"r","kind":"stage","label":"L","state":"running"})
        assert_eq!(init_sign_in_line(&wire(json!({"id":"r","kind":"stage","label":"L","state":"waiting for you"}))), Some("the page is open on this computer".to_string())); // initSignInLine({"id":"r","kind":"stage","label":"L","state":"waiting for you"})
        assert_eq!(init_sign_in_line(&wire(json!({"id":"r","kind":"stage","label":"L","state":"waiting for you","code":"AB-12"}))), Some("waiting for the code".to_string())); // initSignInLine({"id":"r","kind":"stage","label":"L","state":"waiting for you","code":"AB-12"})
        assert_eq!(init_sign_in_line(&wire(json!({"id":"r","kind":"stage","label":"L","state":"waiting for you","finish":"code"}))), Some("waiting for the code".to_string())); // initSignInLine({"id":"r","kind":"stage","label":"L","state":"waiting for you","finish":"code"})
        assert_eq!(init_sign_in_line(&wire(json!({"id":"r","kind":"stage","label":"L","state":"waiting for you","finish":"callback"}))), Some("the page is open on this computer".to_string())); // initSignInLine({"id":"r","kind":"stage","label":"L","state":"waiting for you","finish":"callback"})
    }

    #[test]
    fn a_rows_state_says_whether_it_ran_failed_or_ended() {
        assert_eq!(init_row_unrun("skipped"), true); // initRowUnrun("skipped")
        assert_eq!(init_row_unrun("not made"), true); // initRowUnrun("not made")
        assert_eq!(init_row_unrun("stopped"), true); // initRowUnrun("stopped")
        assert_eq!(init_row_unrun("done"), false); // initRowUnrun("done")
        assert_eq!(init_row_unrun("failed"), false); // initRowUnrun("failed")
        assert_eq!(init_row_failed(&wire(json!({"id":"r","kind":"stage","label":"L","state":"failed"}))), true); // initRowFailed({"id":"r","kind":"stage","label":"L","state":"failed"})
        assert_eq!(init_row_over(&wire(json!({"id":"r","kind":"stage","label":"L","state":"failed"}))), true); // initRowOver({"id":"r","kind":"stage","label":"L","state":"failed"})
        assert_eq!(init_row_failed(&wire(json!({"id":"r","kind":"stage","label":"L","state":"done","login":"not-signed-in"}))), true); // initRowFailed({"id":"r","kind":"stage","label":"L","state":"done","login":"not-signed-in"})
        assert_eq!(init_row_over(&wire(json!({"id":"r","kind":"stage","label":"L","state":"done","login":"not-signed-in"}))), true); // initRowOver({"id":"r","kind":"stage","label":"L","state":"done","login":"not-signed-in"})
        assert_eq!(init_row_failed(&wire(json!({"id":"r","kind":"stage","label":"L","state":"done"}))), false); // initRowFailed({"id":"r","kind":"stage","label":"L","state":"done"})
        assert_eq!(init_row_over(&wire(json!({"id":"r","kind":"stage","label":"L","state":"done"}))), true); // initRowOver({"id":"r","kind":"stage","label":"L","state":"done"})
        assert_eq!(init_row_failed(&wire(json!({"id":"r","kind":"stage","label":"L","state":"running","login":"signed-in"}))), false); // initRowFailed({"id":"r","kind":"stage","label":"L","state":"running","login":"signed-in"})
        assert_eq!(init_row_over(&wire(json!({"id":"r","kind":"stage","label":"L","state":"running","login":"signed-in"}))), true); // initRowOver({"id":"r","kind":"stage","label":"L","state":"running","login":"signed-in"})
        assert_eq!(init_row_failed(&wire(json!({"id":"r","kind":"stage","label":"L","state":"gone"}))), false); // initRowFailed({"id":"r","kind":"stage","label":"L","state":"gone"})
        assert_eq!(init_row_over(&wire(json!({"id":"r","kind":"stage","label":"L","state":"gone"}))), true); // initRowOver({"id":"r","kind":"stage","label":"L","state":"gone"})
        assert_eq!(init_row_failed(&wire(json!({"id":"r","kind":"stage","label":"L","state":"waiting"}))), false); // initRowFailed({"id":"r","kind":"stage","label":"L","state":"waiting"})
        assert_eq!(init_row_over(&wire(json!({"id":"r","kind":"stage","label":"L","state":"waiting"}))), false); // initRowOver({"id":"r","kind":"stage","label":"L","state":"waiting"})
    }

    #[test]
    fn a_stopped_build_says_one_reason_once() {
        assert_eq!(init_stage_while("Creating the machine"), "while creating the machine"); // initStageWhile("Creating the machine")
        assert_eq!(init_stopped_at("while creating"), "Stopped while creating."); // initStoppedAt("while creating")
        assert_eq!(init_stopped_line(""), ""); // initStoppedLine("")
        assert_eq!(init_stopped_line(" ; ; "), " ; ; "); // initStoppedLine(" ; ; ")
        assert_eq!(init_stopped_line("fetch failed"), "This computer lost its network"); // initStoppedLine("fetch failed")
        assert_eq!(init_stopped_line("fetch failed; getaddrinfo ENOTFOUND api; fetch failed"), "This computer lost its network"); // initStoppedLine("fetch failed; getaddrinfo ENOTFOUND api; fetch failed")
        assert_eq!(init_stopped_line("x ECONNREFUSED y"), "This computer lost its network"); // initStoppedLine("x ECONNREFUSED y")
        assert_eq!(init_stopped_line("fetch failed; quota exceeded"), "fetch failed; quota exceeded"); // initStoppedLine("fetch failed; quota exceeded")
        assert_eq!(init_stopped_line("a; b; a"), "a; b"); // initStoppedLine("a; b; a")
        assert_eq!(init_stopped_line("fetch failedx"), "fetch failedx"); // initStoppedLine("fetch failedx")
        assert_eq!(init_failed_line("boom", None), "boom"); // initFailedLine("boom", undefined)
        assert_eq!(init_failed_line("fetch failed", Some(MachineEnd::Gone)), "This computer lost its network The machine is gone; nothing is billing."); // initFailedLine("fetch failed", "gone")
        assert_eq!(init_failed_line("boom", Some(MachineEnd::Left)), "boom The machine did not stop yet; wsp keeps trying."); // initFailedLine("boom", "left")
    }

    #[test]
    fn the_jobs_phase_and_step() {
        assert_eq!(init_job_over(InitPhase::Agent), false); // initJobOver("agent")
        assert_eq!(init_job_building(InitPhase::Agent), false); // initJobBuilding("agent")
        assert_eq!(init_job_over(InitPhase::Reading), false); // initJobOver("reading")
        assert_eq!(init_job_building(InitPhase::Reading), false); // initJobBuilding("reading")
        assert_eq!(init_job_over(InitPhase::Answering), false); // initJobOver("answering")
        assert_eq!(init_job_building(InitPhase::Answering), false); // initJobBuilding("answering")
        assert_eq!(init_job_over(InitPhase::Building), false); // initJobOver("building")
        assert_eq!(init_job_building(InitPhase::Building), true); // initJobBuilding("building")
        assert_eq!(init_job_over(InitPhase::SigningIn), false); // initJobOver("signing-in")
        assert_eq!(init_job_building(InitPhase::SigningIn), true); // initJobBuilding("signing-in")
        assert_eq!(init_job_over(InitPhase::Sealing), false); // initJobOver("sealing")
        assert_eq!(init_job_building(InitPhase::Sealing), true); // initJobBuilding("sealing")
        assert_eq!(init_job_over(InitPhase::Finishing), false); // initJobOver("finishing")
        assert_eq!(init_job_building(InitPhase::Finishing), true); // initJobBuilding("finishing")
        assert_eq!(init_job_over(InitPhase::Done), true); // initJobOver("done")
        assert_eq!(init_job_building(InitPhase::Done), false); // initJobBuilding("done")
        assert_eq!(init_job_over(InitPhase::Failed), true); // initJobOver("failed")
        assert_eq!(init_job_building(InitPhase::Failed), false); // initJobBuilding("failed")
        assert_eq!(init_job_over(InitPhase::Cancelled), true); // initJobOver("cancelled")
        assert_eq!(init_job_building(InitPhase::Cancelled), false); // initJobBuilding("cancelled")
        assert_eq!(init_agent_step(&wire(json!({"id":"j","road":"agent","phase":"agent","screens":[],"rows":[],"progress":{"done":0,"total":0}}))), true); // initAgentStep({"id":"j","road":"agent","phase":"agent","screens":[],"rows":[],"progress":{"done":0,"total":0}})
        assert_eq!(init_agent_step(&wire(json!({"id":"j","road":"agent","phase":"failed","screens":[],"rows":[],"progress":{"done":0,"total":0}}))), true); // initAgentStep({"id":"j","road":"agent","phase":"failed","screens":[],"rows":[],"progress":{"done":0,"total":0}})
        assert_eq!(init_agent_step(&wire(json!({"id":"j","road":"agent","phase":"failed","screens":[{"id":"tools","title":"","top":"","items":[],"ticks":[],"answers":{},"footer":[]}],"rows":[],"progress":{"done":0,"total":0}}))), false); // initAgentStep({"id":"j","road":"agent","phase":"failed","screens":[{"id":"tools","title":"","top":"","items":[],"ticks":[],"answers":{},"footer":[]}],"rows":[],"progress":{"done":0,"total":0}})
        assert_eq!(init_agent_step(&wire(json!({"id":"j","road":"manual","phase":"agent","screens":[],"rows":[],"progress":{"done":0,"total":0}}))), false); // initAgentStep({"id":"j","road":"manual","phase":"agent","screens":[],"rows":[],"progress":{"done":0,"total":0}})
    }

    #[test]
    fn the_need_the_title_mark_and_the_sidebar_line() {
        assert_eq!(paste_here_line("token", None), "paste the token; it stays on this computer"); // pasteHereLine("token")
        assert_eq!(paste_here_line("token", Some("gh auth token")), "run gh auth token on this computer and paste the token it prints; it stays there"); // pasteHereLine("token", "gh auth token")
        assert_eq!(init_need_what(&wire(json!({"id":"j","road":"manual","phase":"building","screens":[],"rows":[{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"Claude","state":"waiting for you"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"s2","kind":"sign-in","label":"GitHub","state":"running"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}],"progress":{"done":0,"total":0}}))), Some("sign in to Claude".to_string())); // initNeedWhat({"id":"j","road":"manual","phase":"building","screens":[],"rows":[{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"Claude","state":"waiting for you"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"s2","kind":"sign-in","label":"GitHub","state":"running"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}],"progress":{"done":0,"total":0}})
        assert_eq!(init_need_what(&wire(json!({"id":"j","road":"manual","phase":"building","screens":[],"rows":[{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"L","state":"done","login":"signed-in"},{"id":"s2","kind":"sign-in","label":"L","state":"not signed in","login":"not-signed-in"},{"id":"stage/ready","kind":"stage","label":"L","state":"failed"},{"id":"stage/x","kind":"stage","label":"L","state":"skipped"}],"progress":{"done":0,"total":0}}))), None); // initNeedWhat({"id":"j","road":"manual","phase":"building","screens":[],"rows":[{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"L","state":"done","login":"signed-in"},{"id":"s2","kind":"sign-in","label":"L","state":"not signed in","login":"not-signed-in"},{"id":"stage/ready","kind":"stage","label":"L","state":"failed"},{"id":"stage/x","kind":"stage","label":"L","state":"skipped"}],"progress":{"done":0,"total":0}})
        assert_eq!(title_with_need("Title", true), "\u{2022} Title"); // titleWithNeed("Title", true)
        assert_eq!(title_with_need("\u{2022} Title", true), "\u{2022} Title"); // titleWithNeed("• Title", true)
        assert_eq!(title_with_need("\u{2022} Title", false), "Title"); // titleWithNeed("• Title", false)
        assert_eq!(title_with_need("Title", false), "Title"); // titleWithNeed("Title", false)
        assert_eq!(title_with_need("\u{2022} \u{2022} T", false), "\u{2022} T"); // titleWithNeed("• • T", false)
        assert_eq!(init_sweeping(&wire::<Vec<InitRow>>(json!([{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"Claude","state":"waiting for you"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"s2","kind":"sign-in","label":"GitHub","state":"running"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}]))), true); // initSweeping([{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"Claude","state":"waiting for you"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"s2","kind":"sign-in","label":"GitHub","state":"running"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}])
        assert_eq!(init_sweeping(&wire::<Vec<InitRow>>(json!([{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"L","state":"done","login":"signed-in"},{"id":"s2","kind":"sign-in","label":"L","state":"not signed in","login":"not-signed-in"},{"id":"stage/ready","kind":"stage","label":"L","state":"failed"},{"id":"stage/x","kind":"stage","label":"L","state":"skipped"}]))), false); // initSweeping([{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"L","state":"done","login":"signed-in"},{"id":"s2","kind":"sign-in","label":"L","state":"not signed in","login":"not-signed-in"},{"id":"stage/ready","kind":"stage","label":"L","state":"failed"},{"id":"stage/x","kind":"stage","label":"L","state":"skipped"}])
        assert_eq!(init_progress_line(&wire(json!({"id":"j","road":"manual","phase":"building","screens":[],"rows":[{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"Claude","state":"waiting for you"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"s2","kind":"sign-in","label":"GitHub","state":"running"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}],"progress":{"done":0,"total":0}}))), "sign in to Claude"); // initProgressLine({"id":"j","road":"manual","phase":"building","screens":[],"rows":[{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"Claude","state":"waiting for you"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"s2","kind":"sign-in","label":"GitHub","state":"running"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}],"progress":{"done":0,"total":0}})
        assert_eq!(init_progress_line(&wire(json!({"id":"j","road":"manual","phase":"building","screens":[],"rows":[{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}],"progress":{"done":0,"total":0}}))), "a machine from the last build is still being removed"); // initProgressLine({"id":"j","road":"manual","phase":"building","screens":[],"rows":[{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}],"progress":{"done":0,"total":0}})
        assert_eq!(init_progress_line(&wire(json!({"id":"j","road":"manual","phase":"sealing","screens":[],"rows":[],"progress":{"done":3,"total":12}}))), "sealing 3/12"); // initProgressLine({"id":"j","road":"manual","phase":"sealing","screens":[],"rows":[],"progress":{"done":3,"total":12}})
        assert_eq!(init_progress_line(&wire(json!({"id":"j","road":"manual","phase":"sealing","screens":[],"rows":[],"progress":{"done":0,"total":0}}))), "sealing"); // initProgressLine({"id":"j","road":"manual","phase":"sealing","screens":[],"rows":[],"progress":{"done":0,"total":0}})
        assert_eq!(init_progress_line(&wire(json!({"id":"j","road":"manual","phase":"answering","screens":[],"rows":[],"progress":{"done":1,"total":4}}))), "waiting for you"); // initProgressLine({"id":"j","road":"manual","phase":"answering","screens":[],"rows":[],"progress":{"done":1,"total":4}})
    }

    #[test]
    fn the_sign_ins_fold_into_one_stage_and_the_count_reads_the_fold() {
        assert_eq!({ let b = init_build_rows(&wire::<Vec<InitRow>>(json!([{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"Claude","state":"waiting for you"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"s2","kind":"sign-in","label":"GitHub","state":"running"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}]))); js(&json!({ "rows": b.rows, "signIns": b.sign_ins })) }, "{\"rows\":[{\"id\":\"f\",\"kind\":\"fact\",\"label\":\"L\",\"state\":\"done\"},{\"id\":\"stage/creating\",\"kind\":\"stage\",\"label\":\"L\",\"state\":\"done\"},{\"id\":\"stage/sign-ins\",\"kind\":\"stage\",\"label\":\"Signing in on the computer\",\"state\":\"waiting for you\"},{\"id\":\"stage/snapshotting\",\"kind\":\"stage\",\"label\":\"L\",\"state\":\"running\"},{\"id\":\"m\",\"kind\":\"machine\",\"label\":\"L\",\"state\":\"machine still running, retrying\"}],\"signIns\":[{\"id\":\"s1\",\"kind\":\"sign-in\",\"label\":\"Claude\",\"state\":\"waiting for you\"},{\"id\":\"s2\",\"kind\":\"sign-in\",\"label\":\"GitHub\",\"state\":\"running\"}]}"); // JSON.stringify(initBuildRows([{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"Claude","state":"waiting for you"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"s2","kind":"sign-in","label":"GitHub","state":"running"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}]))
        assert_eq!(js(&init_stage_count(&init_build_rows(&wire::<Vec<InitRow>>(json!([{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"Claude","state":"waiting for you"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"s2","kind":"sign-in","label":"GitHub","state":"running"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}]))).rows)), "{\"done\":1,\"total\":3}"); // JSON.stringify(initStageCount(initBuildRows([{"id":"f","kind":"fact","label":"L","state":"done"},{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"Claude","state":"waiting for you"},{"id":"a","kind":"agent","label":"L","state":"MCP added"},{"id":"s2","kind":"sign-in","label":"GitHub","state":"running"},{"id":"stage/snapshotting","kind":"stage","label":"L","state":"running"},{"id":"m","kind":"machine","label":"L","state":"machine still running, retrying"}]).rows))
        assert_eq!({ let b = init_build_rows(&wire::<Vec<InitRow>>(json!([{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"L","state":"done","login":"signed-in"},{"id":"s2","kind":"sign-in","label":"L","state":"not signed in","login":"not-signed-in"},{"id":"stage/ready","kind":"stage","label":"L","state":"failed"},{"id":"stage/x","kind":"stage","label":"L","state":"skipped"}]))); js(&json!({ "rows": b.rows, "signIns": b.sign_ins })) }, "{\"rows\":[{\"id\":\"stage/creating\",\"kind\":\"stage\",\"label\":\"L\",\"state\":\"done\"},{\"id\":\"stage/sign-ins\",\"kind\":\"stage\",\"label\":\"Signing in on the computer\",\"state\":\"not signed in\",\"login\":\"not-signed-in\"},{\"id\":\"stage/ready\",\"kind\":\"stage\",\"label\":\"L\",\"state\":\"failed\"},{\"id\":\"stage/x\",\"kind\":\"stage\",\"label\":\"L\",\"state\":\"skipped\"}],\"signIns\":[{\"id\":\"s1\",\"kind\":\"sign-in\",\"label\":\"L\",\"state\":\"done\",\"login\":\"signed-in\"},{\"id\":\"s2\",\"kind\":\"sign-in\",\"label\":\"L\",\"state\":\"not signed in\",\"login\":\"not-signed-in\"}]}"); // JSON.stringify(initBuildRows([{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"L","state":"done","login":"signed-in"},{"id":"s2","kind":"sign-in","label":"L","state":"not signed in","login":"not-signed-in"},{"id":"stage/ready","kind":"stage","label":"L","state":"failed"},{"id":"stage/x","kind":"stage","label":"L","state":"skipped"}]))
        assert_eq!(js(&init_stage_count(&init_build_rows(&wire::<Vec<InitRow>>(json!([{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"L","state":"done","login":"signed-in"},{"id":"s2","kind":"sign-in","label":"L","state":"not signed in","login":"not-signed-in"},{"id":"stage/ready","kind":"stage","label":"L","state":"failed"},{"id":"stage/x","kind":"stage","label":"L","state":"skipped"}]))).rows)), "{\"done\":1,\"total\":4}"); // JSON.stringify(initStageCount(initBuildRows([{"id":"stage/creating","kind":"stage","label":"L","state":"done"},{"id":"s1","kind":"sign-in","label":"L","state":"done","login":"signed-in"},{"id":"s2","kind":"sign-in","label":"L","state":"not signed in","login":"not-signed-in"},{"id":"stage/ready","kind":"stage","label":"L","state":"failed"},{"id":"stage/x","kind":"stage","label":"L","state":"skipped"}]).rows))
        assert_eq!({ let b = init_build_rows(&wire::<Vec<InitRow>>(json!([{"id":"s1","kind":"sign-in","label":"L","state":"skipped"},{"id":"s2","kind":"sign-in","label":"L","state":"skipped"}]))); js(&json!({ "rows": b.rows, "signIns": b.sign_ins })) }, "{\"rows\":[{\"id\":\"stage/sign-ins\",\"kind\":\"stage\",\"label\":\"Signing in on the computer\",\"state\":\"skipped\"}],\"signIns\":[{\"id\":\"s1\",\"kind\":\"sign-in\",\"label\":\"L\",\"state\":\"skipped\"},{\"id\":\"s2\",\"kind\":\"sign-in\",\"label\":\"L\",\"state\":\"skipped\"}]}"); // JSON.stringify(initBuildRows([{"id":"s1","kind":"sign-in","label":"L","state":"skipped"},{"id":"s2","kind":"sign-in","label":"L","state":"skipped"}]))
        assert_eq!(js(&init_stage_count(&init_build_rows(&wire::<Vec<InitRow>>(json!([{"id":"s1","kind":"sign-in","label":"L","state":"skipped"},{"id":"s2","kind":"sign-in","label":"L","state":"skipped"}]))).rows)), "{\"done\":0,\"total\":1}"); // JSON.stringify(initStageCount(initBuildRows([{"id":"s1","kind":"sign-in","label":"L","state":"skipped"},{"id":"s2","kind":"sign-in","label":"L","state":"skipped"}]).rows))
        assert_eq!({ let b = init_build_rows(&wire::<Vec<InitRow>>(json!([{"id":"s1","kind":"sign-in","label":"L","state":"waiting"},{"id":"s2","kind":"sign-in","label":"L","state":"key set"}]))); js(&json!({ "rows": b.rows, "signIns": b.sign_ins })) }, "{\"rows\":[{\"id\":\"stage/sign-ins\",\"kind\":\"stage\",\"label\":\"Signing in on the computer\",\"state\":\"running\"}],\"signIns\":[{\"id\":\"s1\",\"kind\":\"sign-in\",\"label\":\"L\",\"state\":\"waiting\"},{\"id\":\"s2\",\"kind\":\"sign-in\",\"label\":\"L\",\"state\":\"key set\"}]}"); // JSON.stringify(initBuildRows([{"id":"s1","kind":"sign-in","label":"L","state":"waiting"},{"id":"s2","kind":"sign-in","label":"L","state":"key set"}]))
        assert_eq!(js(&init_stage_count(&init_build_rows(&wire::<Vec<InitRow>>(json!([{"id":"s1","kind":"sign-in","label":"L","state":"waiting"},{"id":"s2","kind":"sign-in","label":"L","state":"key set"}]))).rows)), "{\"done\":0,\"total\":1}"); // JSON.stringify(initStageCount(initBuildRows([{"id":"s1","kind":"sign-in","label":"L","state":"waiting"},{"id":"s2","kind":"sign-in","label":"L","state":"key set"}]).rows))
        assert_eq!({ let b = init_build_rows(&wire::<Vec<InitRow>>(json!([]))); js(&json!({ "rows": b.rows, "signIns": b.sign_ins })) }, "{\"rows\":[],\"signIns\":[]}"); // JSON.stringify(initBuildRows([]))
        assert_eq!(js(&init_stage_count(&init_build_rows(&wire::<Vec<InitRow>>(json!([]))).rows)), "{\"done\":0,\"total\":0}"); // JSON.stringify(initStageCount(initBuildRows([]).rows))
        assert_eq!(init_step_counter(2.0, 4.0), "2/4"); // initStepCounter(2, 4)
        assert_eq!(init_stage_count_line(&StageCount { done: 3, total: 12 }), "3 of 12"); // initStageCountLine({done:3,total:12})
    }

    #[test]
    fn keys_cost_and_the_agent_road() {
        assert_eq!(provider_said_line(401.0, ""), "401"); // providerSaidLine(401, "")
        assert_eq!(provider_said_line(401.0, "invalid key"), "401 invalid key"); // providerSaidLine(401, "invalid key")
        assert_eq!(init_cost_line(&WorkspaceSize { cpu: 2.0, mem_mb: 4096.0 }, 0.125), "A 2\u{a0}vCPU,\u{a0}4\u{a0}GB workspace costs about $0.13 an hour while it runs and naps when idle"); // initCostLine({cpu:2,memMb:4096}, 0.125)
        assert_eq!(init_agent_no_recipe_line("/r.json", None), "the thread ended without writing /r.json"); // initAgentNoRecipeLine("/r.json")
        assert_eq!(init_agent_no_recipe_line("/r.json", Some("")), "the thread ended without writing /r.json"); // initAgentNoRecipeLine("/r.json", "")
        assert_eq!(init_agent_no_recipe_line("/r.json", Some("it stopped")), "the thread ended without writing /r.json: it stopped"); // initAgentNoRecipeLine("/r.json", "it stopped")
        assert_eq!(init_agent_prompt("/r.json"), "Write the recipe for this person's wsp machine image from what their agents actually used on this computer. Use the wsp tools recipe_scan and recipe, and nothing else: run no commands and read no files. Call recipe_scan first and read every row's recommended value and its reason. Then call recipe once, with tick set to used, set for every row whose reason says it is worth changing, signin for every sign-in row at its recommended choice, and out set to /r.json. Ask them nothing: they review every row in the app once the file is written. Reply with one line saying the recipe is written."); // initAgentPrompt("/r.json")
    }
}
