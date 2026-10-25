// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/setup.test.ts (the cloud setup view, the need mark, the sign-in words) and, of
//! what init-job.test.ts held at a3b870a6e, the tests whose words the cut kept: the stopped build's sentence, the
//! screens' sentences, the size and disk tones, the snapshot lines and the disk sync lines (format.test.ts:1467-1473).

use regex::Regex;
use serde_json::{json, Value};
use wsp_wire::types::init_job::{LoginChoice, LOGIN_CHOICES};
use wsp_wire::wire::capabilities::InitSetup;
use wsp_words::words::computer::Platform;
use wsp_words::words::init::*;
use wsp_words::words::units::*;

const MIB: f64 = 1024.0 * 1024.0;

mod the_cloud_setup_view {
    use super::*;

    // setup.test.ts: "wsp setup prints the keys as held or not and the price, and no key". The wire's InitSetup still
    // requires `agents` and `job`, which the cut dropped; the fixture carries them empty until its drift pass.
    #[test]
    fn wsp_setup_prints_the_keys_as_held_or_not_and_the_price_and_no_key() {
        let setup: InitSetup = serde_json::from_value(json!({
            "keys": { "box": false, "solari": true },
            "home": "/Users/me",
            "agents": [],
            "pricing": { "size": { "cpu": 2, "memMb": 4096 }, "rateUsdPerHour": 0.11 },
            "job": null,
        }))
        .unwrap();
        let lines: Vec<String> = init_setup_lines(&setup).iter().map(|line| line.replace('\u{a0}', " ")).collect();
        assert_eq!(
            lines,
            vec![
                "Boat API key: not set",
                "Solari API key: saved",
                "A 2 vCPU, 4 GB workspace costs about $0.11 an hour while it runs and naps when idle"
            ]
        );
        let printed = serde_json::to_string(&init_setup_lines(&setup)).unwrap();
        assert!(!Regex::new(r"slr_live|sk-ant").unwrap().is_match(&printed), "{printed}");
    }

    #[test]
    fn a_setup_with_no_price_prints_the_keys_alone() {
        let setup: InitSetup =
            serde_json::from_value(json!({ "keys": { "solari": false }, "home": "/Users/me", "agents": [], "pricing": null, "job": null }))
                .unwrap();
        assert_eq!(init_setup_lines(&setup), vec!["Solari API key: not set"]);
    }
}

mod the_words_the_clients_print_for_the_job {
    use super::*;

    #[test]
    fn the_need_mark_is_carried_once_however_often_it_is_set() {
        assert_eq!(NEEDS_YOU, "wsp needs you");
        assert_eq!(title_with_need("wsp", true), "\u{2022} wsp");
        assert_eq!(title_with_need("wsp", false), "wsp");
        assert_eq!(title_with_need(&title_with_need("wsp", true), true), "\u{2022} wsp");
        assert_eq!(title_with_need(&title_with_need("wsp", true), false), "wsp");
    }

    #[test]
    fn the_sign_in_answers_offer_six_words_each_with_the_words_its_row_and_its_counts_p() {
        let names: Vec<Value> = LOGIN_CHOICES.iter().map(|c| serde_json::to_value(c).unwrap()).collect();
        assert_eq!(names, vec![json!("copy"), json!("machine"), json!("later"), json!("key"), json!("skip"), json!("token")]);
        for word in LOGIN_CHOICES {
            let value: LoginChoice = serde_json::from_value(json!(word)).unwrap();
            assert!(!sign_in_answer_label(value, Platform::Darwin).is_empty(), "{word}");
        }
        assert_eq!(sign_in_answer_label(LoginChoice::Token, Platform::Darwin), "token from this computer");
        assert_eq!(sign_in_answer_short(LoginChoice::Token), "token");
    }

    #[test]
    fn a_rollback_the_provider_refused_is_a_line_on_the_stage_s_block_with_the_provider_s_own_words() {
        assert_eq!(
            machine_left_line("getaddrinfo ENOTFOUND api.getsolari.com"),
            "the machine could not be removed and bills on: getaddrinfo ENOTFOUND api.getsolari.com"
        );
    }

    #[test]
    fn a_stopped_build_s_sentence_is_this_computer_s_own_word_for_what_happened_and_a_r() {
        // The provider's client raises one fetch failure per call, so an outage arrives as the same line repeated.
        assert_eq!(init_stopped_line("fetch failed; fetch failed"), "This computer lost its network");
        assert_eq!(init_stopped_line("getaddrinfo ENOTFOUND api.getsolari.com"), "This computer lost its network");
        assert_eq!(init_stopped_line("connect ECONNREFUSED 127.0.0.1:443; fetch failed"), "This computer lost its network");
        // A refusal the provider gave is the provider's word, and it stands as it is.
        assert_eq!(init_stopped_line("Sandbox limit reached"), "Sandbox limit reached");
        assert_eq!(init_stopped_line("Unauthorized; fetch failed"), "Unauthorized; fetch failed");
        assert_eq!(init_stopped_line("the snapshot failed; the snapshot failed"), "the snapshot failed");
    }

    #[test]
    fn every_step_has_a_sentence_under_its_title_and_no_title_or_sentence_ends_in_a_per() {
        use wsp_words::words::init::cloud_setup_words::{agent, build, choice, reading};
        assert!(!build::SLIDE_TOP.ends_with('.') && !build::SLIDE_TOP.ends_with(';'));
        for (headline, top) in [
            (choice::HEADLINE, choice::TOP),
            (reading::HEADLINE, reading::TOP),
            (agent::HEADLINE, agent::TOP),
            (build::HEADLINE, build::TOP),
        ] {
            assert!(top.chars().count() > 20, "{top}");
            assert!(!top.ends_with('.'), "{top}");
            assert!(!headline.ends_with('.'), "{headline}");
        }
    }

    #[test]
    fn the_size_and_disk_tones_and_a_row_s_calls_come_from_one_formatter_each() {
        assert_eq!(
            [
                size_tone(1024.0 * MIB),
                size_tone(1024.0 * MIB - 1.0),
                size_tone(200.0 * MIB),
                size_tone(200.0 * MIB - 1.0),
                size_tone(50.0 * MIB),
                size_tone(50.0 * MIB - 1.0),
                size_tone(0.0)
            ],
            [SizeTone::Danger, SizeTone::Warning, SizeTone::Warning, SizeTone::Yellow, SizeTone::Yellow, SizeTone::Muted, SizeTone::Muted]
        );
        assert_eq!(
            [disk_tone(13.0, 20.0), disk_tone(14.0, 20.0), disk_tone(18.0, 20.0), disk_tone(21.0, 20.0), disk_tone(1.0, 0.0)],
            [SizeTone::Muted, SizeTone::Warning, SizeTone::Danger, SizeTone::Danger, SizeTone::Danger]
        );
        assert_eq!(fmt_calls(29_623.0), "29,623 calls");
        assert_eq!(fmt_calls(1.0), "1 call");
    }

    #[test]
    fn the_snapshot_stage_names_its_size_when_it_has_one() {
        assert_eq!(snapshot_stage_line(Some(13.0 * 1024.0 * MIB)), "snapshotting about 13 GB, usually under a minute");
        assert_eq!(snapshot_stage_line(None), "snapshotting, usually under a minute");
        assert_eq!(SAVING_IMAGE_LINE, "saving the image");
    }

    // The TS stringifies the whole table, keys included; the strings are listed here from the module's constants,
    // which hold no key with these words.
    #[test]
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
        assert_eq!(
            disk_sync_failed_line("it exited 1 and said: sync: Input/output error"),
            "the disk could not be synced (it exited 1 and said: sync: Input/output error); nothing was snapshotted"
        );
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

    use wsp_wire::types::init_job::{LoginChoice, LoginState};
    use wsp_wire::views::golden_image::GoldenStage;
    use wsp_wire::wire::capabilities::WorkspaceSize;
    use wsp_words::words::computer::Platform;

    #[test]
    fn every_phase_login_choice_and_stage_has_one_word() {
        assert_eq!(login_state_word(LoginState::SignedIn), "signed in"); // LOGIN_STATE_WORDS["signed-in"]
        assert_eq!(login_state_word(LoginState::NotSignedIn), "not signed in"); // LOGIN_STATE_WORDS["not-signed-in"]
        assert_eq!(login_state_word(LoginState::NotVerified), "not verified"); // LOGIN_STATE_WORDS["not-verified"]
        assert_eq!(login_state_word(LoginState::Skipped), "skipped"); // LOGIN_STATE_WORDS["skipped"]
        assert_eq!(login_state_word(LoginState::Copied), "copied"); // LOGIN_STATE_WORDS["copied"]
        assert_eq!(login_state_word(LoginState::Deferred), "not signed in, sign in when you first need it"); // LOGIN_STATE_WORDS["deferred"]
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
    }

    #[test]
    fn stage_rows_clocks_and_snapshot_lines() {
        assert_eq!(disk_unsettled_line(0.5, 2048.0), "synced, Dirty 512 B and Writeback 2 MB remain; a writer is still running"); // diskUnsettledLine({dirtyKb:0.5,writebackKb:2048})
        assert_eq!(snapshot_stage_line(None), "snapshotting, usually under a minute"); // snapshotStageLine(undefined)
        assert_eq!(snapshot_stage_line(Some(5368709120.0)), "snapshotting about 5 GB, usually under a minute"); // snapshotStageLine(5*1024**3)
        assert_eq!(snapshot_progress_line(1024.0, None), "snapshotting, 1 KB written"); // snapshotProgressLine(1024, undefined)
        assert_eq!(snapshot_progress_line(1024.0, Some(4096.0)), "snapshotting, 1 KB of about 4 KB written");
        // snapshotProgressLine(1024, 4096)
    }

    #[test]
    fn a_stopped_build_says_one_reason_once() {
        assert_eq!(init_stopped_line(""), ""); // initStoppedLine("")
        assert_eq!(init_stopped_line(" ; ; "), " ; ; "); // initStoppedLine(" ; ; ")
        assert_eq!(init_stopped_line("fetch failed"), "This computer lost its network"); // initStoppedLine("fetch failed")
        assert_eq!(init_stopped_line("fetch failed; getaddrinfo ENOTFOUND api; fetch failed"), "This computer lost its network"); // initStoppedLine("fetch failed; getaddrinfo ENOTFOUND api; fetch failed")
        assert_eq!(init_stopped_line("x ECONNREFUSED y"), "This computer lost its network"); // initStoppedLine("x ECONNREFUSED y")
        assert_eq!(init_stopped_line("fetch failed; quota exceeded"), "fetch failed; quota exceeded"); // initStoppedLine("fetch failed; quota exceeded")
        assert_eq!(init_stopped_line("a; b; a"), "a; b"); // initStoppedLine("a; b; a")
        assert_eq!(init_stopped_line("fetch failedx"), "fetch failedx"); // initStoppedLine("fetch failedx")
    }

    #[test]
    fn the_need_the_title_mark_and_the_sidebar_line() {
        assert_eq!(title_with_need("Title", true), "\u{2022} Title"); // titleWithNeed("Title", true)
        assert_eq!(title_with_need("\u{2022} Title", true), "\u{2022} Title"); // titleWithNeed("• Title", true)
        assert_eq!(title_with_need("\u{2022} Title", false), "Title"); // titleWithNeed("• Title", false)
        assert_eq!(title_with_need("Title", false), "Title"); // titleWithNeed("Title", false)
        assert_eq!(title_with_need("\u{2022} \u{2022} T", false), "\u{2022} T");
        // titleWithNeed("• • T", false)
    }

    #[test]
    fn keys_cost_and_the_agent_road() {
        assert_eq!(provider_said_line(401.0, ""), "401"); // providerSaidLine(401, "")
        assert_eq!(provider_said_line(401.0, "invalid key"), "401 invalid key"); // providerSaidLine(401, "invalid key")
        assert_eq!(
            init_cost_line(&serde_json::from_value::<WorkspaceSize>(serde_json::json!({ "cpu": 2.0, "memMb": 4096.0 })).unwrap(), 0.125),
            "A 2\u{a0}vCPU,\u{a0}4\u{a0}GB workspace costs about $0.13 an hour while it runs and naps when idle"
        ); // initCostLine({cpu:2,memMb:4096}, 0.125)
    }
}
