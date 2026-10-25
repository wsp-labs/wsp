// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/format.test.ts:1388-1560 (goldenBuildLine, REPO_STATE_WORDS, LINEAGE_MARKS, the
//! template words, missingToolRow, behindGoldenLine, the provider answer and snapshot lines, the builder lines, a pinned
//! release that moved, a tools row outside the catalog) and init-job.test.ts:305 (credentialOnBuilderLine).

use regex::Regex;
use wsp_wire::views::golden_image::ToolPin;
use wsp_words::words::image::*;

fn change(count: f64, noun: &str, word: &str) -> GoldenChange {
    GoldenChange { count, noun: noun.to_string(), word: word.to_string() }
}

fn pin(v: serde_json::Value) -> ToolPin {
    serde_json::from_value(v).unwrap()
}

fn read(name: &str, tag: &str, words: Option<&str>) -> PinRead {
    PinRead { name: name.to_string(), tag: tag.to_string(), words: words.map(str::to_string) }
}

mod golden_build_line {
    use super::*;

    #[test]
    fn names_the_version_it_builds_and_the_one_it_builds_on_top_of_then_what_it_changes() {
        assert_eq!(golden_build_line(2.0, 3.0, &[change(2.0, "tool", "added")]), "Builds version 3 on top of version 2: 2 tools added");
    }

    #[test]
    fn pluralises_each_noun_on_its_own_count_and_drops_what_did_not_change() {
        assert_eq!(
            golden_build_line(1.0, 2.0, &[change(1.0, "tool", "added"), change(0.0, "agent", "added"), change(3.0, "row", "retired")]),
            "Builds version 2 on top of version 1: 1 tool added, 3 rows retired"
        );
    }

    #[test]
    fn a_build_with_no_version_under_it_names_no_version_to_build_on_and_a_build_that_c() {
        assert_eq!(golden_build_line(0.0, 1.0, &[change(4.0, "tool", "added")]), "Builds version 1: 4 tools added");
        assert_eq!(golden_build_line(2.0, 3.0, &[change(0.0, "tool", "added")]), "Builds version 3 on top of version 2");
    }
}

mod repo_state_words {
    use super::*;

    #[test]
    fn carries_no_word_for_any_folder_git_named_no_branch_for_a_slot_shows_the_branch_o() {
        // The TS checks each row's keys are exactly ["pane"]; the Rust word has the one accessor, so each is read through it.
        for word in [RepoStateWord::Unknown, RepoStateWord::None, RepoStateWord::Refused] {
            let _ = word.pane();
        }
        assert_eq!([RepoStateWord::Unknown, RepoStateWord::None, RepoStateWord::Refused].map(|w| w.pane().is_empty()), [true, false, true]);
    }

    #[test]
    fn gives_an_empty_diff_pane_one_dry_sentence_only_for_a_folder_outside_any_reposito() {
        assert_eq!(RepoStateWord::None.pane(), "This folder is not inside a git repository, so there is nothing to diff.");
        assert!(Regex::new(r"^[^.]+\.$").unwrap().is_match(RepoStateWord::None.pane()), "{}", RepoStateWord::None.pane());
        assert_eq!(RepoStateWord::Refused.pane(), "");
        assert_eq!(RepoStateWord::Unknown.pane(), "");
    }
}

mod lineage_marks {
    use super::*;

    #[test]
    fn names_every_outcome_a_missing_tool_can_carry_and_every_state_a_lineage_row_shows() {
        for (o, word) in [(LineageMark::Skipped, "skipped"), (LineageMark::Failed, "failed")] {
            assert_eq!(o.words(), word);
        }
        let marks = [
            (LineageMark::Now, "now"),
            (LineageMark::Head, "newest"),
            (LineageMark::Fork, "this one"),
            (LineageMark::Failed, "failed"),
            (LineageMark::Skipped, "skipped"),
            (LineageMark::Volatile, "snapshot only"),
        ];
        for (mark, want) in marks {
            assert_eq!(mark.words(), want);
        }
        let shape = Regex::new(r"^[a-z]+( [a-z]+)?$").unwrap();
        for (mark, _) in marks {
            let word = mark.words();
            assert!(shape.is_match(word), "{word}");
            assert!(word.chars().count() <= 13, "{word}");
        }
    }
}

mod template_words {
    use super::*;

    #[test]
    fn says_the_image_is_being_saved_and_whether_the_seal_asks_again_never_the_template() {
        assert_eq!(template_status_line("building"), "saving the image, the provider says building; asking again");
        assert_eq!(template_status_line("ready"), "the image is saved");
    }

    #[test]
    fn names_a_failed_template_with_the_provider_s_reason_or_that_none_was_given_and_a() {
        assert_eq!(
            template_failed_line("tpl_a", Some("restore copy failed")),
            "the provider failed the template tpl_a: restore copy failed"
        );
        assert_eq!(template_failed_line("tpl_a", None), "the provider failed the template tpl_a: no reason given");
        assert_eq!(template_waited_line("tpl_a", "building", 300_000.0), "the template tpl_a still reads building after 5m");
    }

    #[test]
    fn the_doctor_s_line_per_version_names_the_template_it_promoted_and_when_other_temp() {
        assert_eq!(
            template_recorded_line("default", 2.0, "tpl_0f1e", Some(0.0)),
            "image default v2: template tpl_0f1e promoted and recorded"
        );
        assert_eq!(
            template_recorded_line("default", 1.0, "tpl_0f1e", Some(1.0)),
            "image default v1: template tpl_0f1e promoted and recorded; 1 other template carries its name"
        );
        assert_eq!(
            template_recorded_line("default", 1.0, "tpl_0f1e", Some(2.0)),
            "image default v1: template tpl_0f1e promoted and recorded; 2 other templates carry its name"
        );
        assert_eq!(template_recorded_line("default", 1.0, "tpl_0f1e", None), "image default v1: template tpl_0f1e promoted and recorded");
        assert_eq!(
            template_skipped_line("default", 1.0, "its snapshot is gone at the provider"),
            "image default v1: no template recorded, its snapshot is gone at the provider"
        );
        assert_eq!(NO_TEMPLATES_LINE, "this backend has no templates; image versions stay as snapshots");
    }
}

mod missing_tool_row_words {
    use super::*;

    #[test]
    fn shows_a_record_as_its_name_reason_and_outcome_and_one_sealed_without_a_name_or_a() {
        assert_eq!(
            missing_tool_row(&MissingTool {
                id: "tools/brew/gopls".into(),
                name: Some("gopls".into()),
                outcome: Some(LineageMark::Skipped),
                note: "no Linux bottle".into()
            }),
            MissingToolRow { name: "gopls".into(), note: "no Linux bottle".into(), mark: LineageMark::Skipped }
        );
        assert_eq!(
            missing_tool_row(&MissingTool {
                id: "base/docker".into(),
                name: None,
                outcome: None,
                note: "E: Unable to locate package docker-compose-v2".into()
            }),
            MissingToolRow {
                name: "base/docker".into(),
                note: "E: Unable to locate package docker-compose-v2".into(),
                mark: LineageMark::Failed
            }
        );
        assert_eq!(
            missing_tool_row(&MissingTool {
                id: "base/docker".into(),
                name: Some("".into()),
                outcome: Some(LineageMark::Failed),
                note: "E: Unable to locate package docker-compose-v2".into()
            })
            .name,
            "base/docker"
        );
    }
}

mod behind_golden_line_words {
    use super::*;

    #[test]
    fn names_the_version_it_is_on_and_the_one_available_in_words_short_enough_for_the_r() {
        assert_eq!(behind_golden_line(11.0, 12.0), "on image v11, v12 available");
        assert!(behind_golden_line(11.0, 12.0).chars().count() <= 30);
    }
}

mod provider_answer_line_snapshot_attempt_line_and_snapshot_failed_line {
    use super::*;

    fn refused() -> ProviderAnswer {
        ProviderAnswer {
            status: 502.0,
            message: "Failed to snapshot sandbox".into(),
            request_id: Some("req_7".into()),
            at: "2026-09-07T01:19:43.352Z".into(),
        }
    }

    fn running() -> BuilderReading {
        BuilderReading::State("running".into())
    }

    #[test]
    fn provideranswerline_carries_the_status_the_message_and_the_request_id_without_one() {
        assert_eq!(provider_answer_line(&refused()), "502 Failed to snapshot sandbox (request req_7)");
        assert_eq!(
            provider_answer_line(&ProviderAnswer {
                status: 502.0,
                message: "Failed to snapshot sandbox".into(),
                request_id: None,
                at: "2026-09-07T01:19:43.352Z".into()
            }),
            "502 Failed to snapshot sandbox (no request id from the provider, at 2026-09-07T01:19:43.352Z)"
        );
    }

    #[test]
    fn an_attempt_line_names_the_attempt_the_answer_what_the_builder_reads_and_when_the() {
        assert_eq!(
            snapshot_attempt_line(1.0, 3.0, &refused(), &running(), 60_000.0),
            "attempt 1 of 3 answered 502 Failed to snapshot sandbox (request req_7); the builder reads running, next attempt in 1m"
        );
    }

    #[test]
    fn the_failure_line_counts_the_attempts_and_says_whether_the_provider_still_has_the() {
        assert_eq!(
            snapshot_failed_line(3.0, &refused(), &running(), None),
            "the snapshot failed 3 times: the provider answered 502 Failed to snapshot sandbox (request req_7) while the builder read running"
        );
        assert_eq!(
            snapshot_failed_line(1.0, &refused(), &BuilderReading::State("gone".into()), None),
            "the snapshot failed 1 time: the provider answered 502 Failed to snapshot sandbox (request req_7) and no longer has the builder (404)"
        );
        assert_eq!(
            snapshot_failed_line(1.0, &refused(), &BuilderReading::Unread, Some("upstream sad (503)")),
            "the snapshot failed 1 time: the provider answered 502 Failed to snapshot sandbox (request req_7) and could not be read about the builder (upstream sad (503))"
        );
    }
}

mod builder_stays_line_and_the_seal_s_and_the_update_s_last_lines {
    use super::*;

    #[test]
    fn the_stays_up_sentence_is_one_rule_and_the_failed_seal_s_last_line_wraps_it() {
        let stays = builder_stays_line("m1", 0.11, "wsp init --recipe '/tmp/r.json'");
        assert_eq!(
            stays,
            "Builder m1 stays up at about $0.11/hr; wsp init --recipe '/tmp/r.json' attaches to it again, and the sweep stops it once it is six hours old."
        );
        assert_eq!(
            seal_failed_builder_stays_line("m1", 0.11, "wsp init --recipe '/tmp/r.json'"),
            format!("Seal failed; the builder is as you left it. {stays}")
        );
        assert_eq!(
            seal_failed_builder_unread_line("m1", 0.11, "wsp init --recipe '/tmp/r.json'"),
            format!("Seal failed; the provider could not be read about the builder, so nothing on it was touched. {stays}")
        );
        assert_eq!(
            SEAL_FAILED_BUILDER_GONE_LINE,
            "Seal failed and the builder is gone: the provider dropped it after refusing the snapshot. Run wsp init again; the recipe is kept."
        );
    }

    #[test]
    fn the_update_s_last_lines_say_the_image_stands_and_whether_the_provider_still_has() {
        assert_eq!(
            upgrade_seal_failed_stays_line(1.0, "m1", 0.11),
            "Image v1 is unchanged. Builder m1 is as it was, up at about $0.11/hr; run wsp init again to retry, and the sweep stops it once it is six hours old."
        );
        assert_eq!(
            upgrade_seal_failed_gone_line(1.0),
            "Image v1 is unchanged and the builder is gone: the provider dropped it after refusing the snapshot. Run wsp init again to retry."
        );
        assert_eq!(
            upgrade_seal_failed_unread_line(1.0, "m1"),
            "Image v1 is unchanged. The provider could not be read about builder m1, so nothing on it was touched; run wsp init again to retry, and the sweep stops it once it is six hours old."
        );
        assert_eq!(SEAL_FAILED_LINE, "Seal failed and the builder is gone. Run wsp init again; the recipe is kept.");
    }
}

mod a_pinned_release_that_moved {
    use super::*;

    #[test]
    fn the_failure_names_the_download_its_tag_and_the_recorded_and_served_sums_both_cut() {
        assert_eq!(SUM_SHOWN, 12);
        assert_eq!(short_sum(&"b".repeat(64)), "b".repeat(12));
        assert_eq!(
            pin_mismatch_line("gh_2.86.0_linux_amd64.tar.gz", "v2.86.0", &short_sum(&"b".repeat(64)), &short_sum(&"c".repeat(64))),
            "gh_2.86.0_linux_amd64.tar.gz at v2.86.0 does not match the checksum recorded on its first install: recorded bbbbbbbbbbbb, served cccccccccccc"
        );
        // With the widest tool and asset names the catalog has, the line stays under the 160 characters the reason rule keeps.
        assert!(
            pin_mismatch_line("google-cloud-cli-575.0.0-linux-x86_64.tar.gz", "575.0.0", &"b".repeat(12), &"c".repeat(12)).chars().count()
                < 160
        );
    }

    #[test]
    fn why_a_tool_installs_differently_now_the_road_in_the_roads_words_the_lines_otherw() {
        assert_eq!(road_moved_line("with Homebrew", "by its own installer"), "now by its own installer, was with Homebrew");
        assert_eq!(INSTALLER_MOVED_LINE, "its install lines changed");
        assert_eq!(road_moved_line("with Homebrew", NO_ROAD_WORDS), "now by no road, was with Homebrew");
    }

    #[test]
    fn a_pin_in_words_the_version_alone_where_a_copy_gets_it_with_the_latest_mark_where() {
        assert_eq!(INSTALLS_LATEST, "installs latest");
        assert_eq!(pin_words(&pin(serde_json::json!({ "tag": "v2.86.0", "sha256": "b".repeat(64) }))), "v2.86.0");
        assert_eq!(pin_words(&pin(serde_json::json!({ "tag": "3.3a-3", "latest": true }))), "3.3a-3, installs latest");
        // The record's line: the row by name, the version, the checksum where the road hashed one, the latest mark in the road's words.
        assert_eq!(
            sealed_pin_line("GitHub CLI", &pin(serde_json::json!({ "tag": "v2.86.0", "sha256": "b".repeat(64) })), None),
            "GitHub CLI  v2.86.0  checksum bbbbbbbbbbbb"
        );
        assert_eq!(
            sealed_pin_line("tmux", &pin(serde_json::json!({ "tag": "3.3a-3", "latest": true })), Some("by apt")),
            "tmux  3.3a-3  installs latest by apt"
        );
        assert_eq!(
            sealed_pin_line("Claude Code", &pin(serde_json::json!({ "tag": "2.1.3", "latest": true })), None),
            "Claude Code  2.1.3  installs latest"
        );
        assert_eq!(sealed_pin_line("wrangler", &pin(serde_json::json!({ "tag": "4.1.0" })), None), "wrangler  4.1.0");
        // The stage's one line: the pinned rows, then once the rows that install latest with the version this build got; nothing when nothing was read.
        assert_eq!(
            pins_read_line(
                &[read("wrangler", "4.1.0", None), read("GitHub CLI", "v2.86.0", None)],
                &[read("tmux", "3.3a-3", Some("by apt")), read("Go", "1.22.1", Some("with Homebrew"))]
            ),
            Some(
                "pinned: wrangler 4.1.0, GitHub CLI v2.86.0; installs latest on every place: tmux 3.3a-3 by apt, Go 1.22.1 with Homebrew"
                    .to_string()
            )
        );
        assert_eq!(
            pins_read_line(&[], &[read("Claude Code", "2.1.3", None)]),
            Some("installs latest on every place: Claude Code 2.1.3".to_string())
        );
        assert_eq!(pins_read_line(&[read("a, b", "1", None)], &[]), Some("pinned: \"a, b\" 1".to_string()));
        assert_eq!(pins_read_line(&[], &[]), None);
    }
}

mod a_tools_row_outside_the_catalog {
    use super::*;
    use wsp_words::words::computer::Platform;

    #[test]
    fn says_it_is_on_this_computer_what_the_build_does_with_it_and_when_a_file_s_tick_h() {
        assert_eq!(installed_here_line(Platform::Darwin, None), "installed on this Mac");
        assert_eq!(installed_here_line(Platform::Darwin, Some("0.1.0")), "installed on this Mac, 0.1.0");
        assert_eq!(installed_here_line(Platform::Linux, Some("0.1.0")), "installed on this computer, 0.1.0");
        assert_eq!(
            installs_by_line("from its release", "the v0.1.0 release of github.com/Zingzy/diskbloom"),
            "installs from its release: the v0.1.0 release of github.com/Zingzy/diskbloom"
        );
        assert_eq!(left_out_line("no Linux bottle known"), "left out of the build: no Linux bottle known");
        assert_eq!(
            not_here_line(Platform::Darwin, "zingzy/tap/diskbloom", "/tmp/given.json"),
            "zingzy/tap/diskbloom is ticked in /tmp/given.json, but this Mac has no row that installs it; it is left out."
        );
        assert_eq!(
            not_here_line(Platform::Linux, "direnv", "/tmp/given.json"),
            "direnv is ticked in /tmp/given.json, but this computer has no row that installs it; it is left out."
        );
    }
}

mod the_words_the_clients_print_for_the_job {
    use super::*;

    #[test]
    fn a_builder_holding_a_sign_in_file_is_refused_in_one_sentence_that_names_the_paths() {
        assert_eq!(
            credential_on_builder_line(&["/root/.claude-cfg/.credentials.json", "/root/.codex/auth.json"]),
            "the builder holds a sign-in file at /root/.claude-cfg/.credentials.json, /root/.codex/auth.json; sign-ins never sit in an image, so nothing was sealed"
        );
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::words::image::*;

    use serde_json::json;
    use wsp_wire::views::golden_image::GoldenStage;
    use wsp_words::words::computer::Platform;

    fn wire<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
        serde_json::from_value(v).unwrap()
    }

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }

    #[test]
    fn a_copy_stands_on_the_hash_alone_and_asks_sign_ins_only_without_a_vault() {
        assert!(copy_asks_sign_ins(&wire(
            json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"})
        ))); // copyAsksSignIns({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"})
        assert!(!copy_asks_sign_ins(&wire(
            json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","vault":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1,"paths":2,"takenAt":"2026-10-01T00:00:00.000Z"}})
        ))); // copyAsksSignIns({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","vault":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1,"paths":2}})
        assert!(!copy_asks_sign_ins(&wire(
            json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":true})
        ))); // copyAsksSignIns({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":true})
        assert!(copy_asks_sign_ins(&wire(
            json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":false})
        ))); // copyAsksSignIns({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":false})
        assert_eq!(image_held_sign_ins_line("Boat"), "Your image on Boat was sealed with sign-ins on it"); // imageHeldSignInsLine("Boat")
        assert_eq!(image_rebuild_fix("boat"), "Rebuild it: wsp init --on boat --rebuild."); // imageRebuildFix("boat")
        assert!(copy_is_current(
            &wire(
                json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"})
            ),
            &wire(
                json!({"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"})
            )
        )); // copyIsCurrent({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"}, {"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"})
        assert!(!copy_is_current(
            &wire(
                json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"})
            ),
            &wire(
                json!({"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"})
            )
        )); // copyIsCurrent({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"}, {"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"})
        assert!(!copy_is_current(
            &wire(
                json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"})
            ),
            &wire(json!({"place":"boat","version":2,"snapshotId":"s","builtAt":"t"}))
        )); // copyIsCurrent({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"}, {"place":"boat","version":2,"snapshotId":"s","builtAt":"t"})
        assert_eq!(
            sealed_logins_held(&wire(
                json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[{"name":"signed-in","state":"signed-in"},{"name":"copied","state":"copied"},{"name":"not-verified","state":"not-verified"},{"name":"skipped","state":"skipped"},{"name":"deferred","state":"deferred"},{"name":"not-signed-in","state":"not-signed-in"}],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"})
            )) as f64,
            3.0
        ); // sealedLoginsHeld({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[{"name":"signed-in","state":"signed-in"},{"name":"copied","state":"copied"},{"name":"not-verified","state":"not-verified"},{"name":"skipped","state":"skipped"},{"name":"deferred","state":"deferred"},{"name":"not-signed-in","state":"not-signed-in"}],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"})
        assert_eq!(
            copy_standing(
                &wire(
                    json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"})
                ),
                &wire(
                    json!({"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"})
                )
            ),
            None
        ); // copyStanding({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"}, {"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"})
        assert_eq!(
            copy_standing(
                &wire(
                    json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":true})
                ),
                &wire(
                    json!({"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"})
                )
            ),
            Some("current".to_string())
        ); // copyStanding({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":true}, {"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"})
        assert_eq!(
            copy_standing(
                &wire(
                    json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","vault":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1,"paths":1,"takenAt":"2026-10-01T00:00:00.000Z"}})
                ),
                &wire(json!({"place":"boat","version":2,"snapshotId":"s","builtAt":"t"}))
            ),
            Some("stale".to_string())
        ); // copyStanding({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","vault":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1,"paths":1}}, {"place":"boat","version":2,"snapshotId":"s","builtAt":"t"})
    }

    #[test]
    fn the_record_and_its_copies_in_one_line() {
        assert_eq!(sealed_image_line(&wire(json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"}))), "wsp v3  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  no sign-ins held; cut the next version to hold them  sealed on this Mac"); // sealedImageLine({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"})
        assert_eq!(
            sealed_image_line(&wire(
                json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":true,"usedBytes":5368709120_i64})
            )),
            "wsp v3  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  no sign-ins on it  5 GB  sealed on this Mac"
        ); // sealedImageLine({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":true,"usedBytes":5368709120})
        assert_eq!(
            sealed_image_line(&wire(
                json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[{"name":"a","state":"copied"}],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","vault":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1,"paths":1,"takenAt":"2026-10-01T00:00:00.000Z"}})
            )),
            "wsp v3  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  1 sign-in held, 1 path  sealed on this Mac"
        ); // sealedImageLine({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[{"name":"a","state":"copied"}],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","vault":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1,"paths":1}})
        assert_eq!(
            sealed_image_line(&wire(
                json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","vault":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1,"paths":0,"takenAt":"2026-10-01T00:00:00.000Z"}})
            )),
            "wsp v3  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  0 sign-ins held, 0 paths  sealed on this Mac"
        ); // sealedImageLine({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","vault":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1,"paths":0}})
        assert_eq!(
            sealed_copy_line(
                &wire(
                    json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":true})
                ),
                &wire(
                    json!({"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","sizeBytes":1536})
                )
            ),
            "boat  v2  2 KB  current"
        ); // sealedCopyLine({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":true}, {"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","sizeBytes":1536})
        assert_eq!(
            sealed_copy_line(
                &wire(
                    json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"})
                ),
                &wire(json!({"place":"boat","version":2,"snapshotId":"s","builtAt":"t"}))
            ),
            "boat  v2"
        ); // sealedCopyLine({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac"}, {"place":"boat","version":2,"snapshotId":"s","builtAt":"t"})
        assert_eq!(
            sealed_built_line(
                &wire(
                    json!({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":true})
                ),
                &wire(
                    json!({"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"})
                ),
                false
            ),
            "boat  v2  current  already built from this image; nothing was built"
        ); // sealedBuiltLine({"name":"wsp","version":3,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","recipeHash":"r","logins":[],"sealedAt":"2026-10-01T00:00:00.000Z","sealedFrom":"this Mac","noSecrets":true}, {copy:{"place":"boat","version":2,"snapshotId":"s","builtAt":"t","hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},built:false})
        assert_eq!(
            sealed_export_line(&wire(
                json!({"path":"/x.wspimage","bytes":2048,"hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"})
            )),
            "/x.wspimage  2 KB  opens with the passphrase you typed and nothing else"
        ); // sealedExportLine({path:"/x.wspimage",bytes:2048,hash:"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"})
    }

    #[test]
    fn a_copy_build_reads_its_frame() {
        assert_eq!(copy_building_line(GoldenStage::Creating), "copying your image: creating the machine"); // copyBuildingLine("creating")
        assert_eq!(copy_building_line(GoldenStage::DeployingDaemon), "copying your image: installing the base tools"); // copyBuildingLine("deploying-daemon")
        assert_eq!(copy_building_line(GoldenStage::Snapshotting), "copying your image: taking the snapshot"); // copyBuildingLine("snapshotting")
        assert_eq!(copy_building_line(GoldenStage::Sealed), "copying your image: finishing"); // copyBuildingLine("sealed")
        assert_eq!(copy_stopped_line(None), "the build stopped"); // copyStoppedLine(undefined)
        assert_eq!(copy_stopped_line(Some("")), "the build stopped"); // copyStoppedLine("")
        assert_eq!(copy_stopped_line(Some("fetch failed")), "the build stopped: This computer lost its network"); // copyStoppedLine("fetch failed")
        assert_eq!(copy_stopped_line(Some("a; a")), "the build stopped: a"); // copyStoppedLine("a; a")
        assert_eq!(js(&copy_build_of(GoldenStage::Sealed, None)), "null"); // JSON.stringify(copyBuildOf({stage:"sealed",detail:undefined}) ?? null)
        assert_eq!(js(&copy_build_of(GoldenStage::Failed, None)), "{\"line\":\"the build stopped\",\"stopped\":true}"); // JSON.stringify(copyBuildOf({stage:"failed",detail:undefined}) ?? null)
        assert_eq!(
            js(&copy_build_of(GoldenStage::Failed, Some("fetch failed"))),
            "{\"line\":\"the build stopped: This computer lost its network\",\"stopped\":true}"
        ); // JSON.stringify(copyBuildOf({stage:"failed",detail:"fetch failed"}) ?? null)
        assert_eq!(
            js(&copy_build_of(GoldenStage::Creating, Some("ignored"))),
            "{\"line\":\"copying your image: creating the machine\",\"stopped\":false}"
        ); // JSON.stringify(copyBuildOf({stage:"creating",detail:"ignored"}) ?? null)
    }

    #[test]
    fn project_images() {
        assert_eq!(
            sealed_project_line(&wire(
                json!({"snapshotId":"p1","projects":[{"name":"a","dest":"/a","importedAt":"t"},{"name":"b","dest":"/b","importedAt":"t"}],"golden":"g","workspaceId":"w","workspaceName":"ws","createdAt":"2026-10-01T00:00:00.000Z","sizeBytes":1024})
            )),
            "p1  project ws  a, b  1 KB  2026-10-01T00:00:00.000Z"
        ); // sealedProjectLine({snapshotId:"p1",projects:[{name:"a"},{name:"b"}],golden:"g",workspaceId:"w",workspaceName:"ws",createdAt:"2026-10-01T00:00:00.000Z",sizeBytes:1024})
        assert_eq!(
            sealed_project_line(&wire(
                json!({"snapshotId":"p1","projects":[],"golden":"g","workspaceId":"w","workspaceName":"ws","createdAt":"c"})
            )),
            "p1  project ws    c"
        ); // sealedProjectLine({snapshotId:"p1",projects:[],golden:"g",workspaceId:"w",workspaceName:"ws",createdAt:"c"})
        assert_eq!(
            project_image_in_use_refusal("p1", &["a"]),
            "project image p1 has a workspace standing on it: a; delete it first, and forget any whose machine is already gone"
        ); // projectImageInUseRefusal("p1", ["a"])
        assert_eq!(
            project_image_in_use_refusal("p1", &["a", "b, c"]),
            "project image p1 has workspaces standing on it: a, \"b, c\"; delete them first, and forget any whose machine is already gone"
        ); // projectImageInUseRefusal("p1", ["a", "b, c"])
        assert_eq!(
            project_image_remove_notice(&wire(
                json!({"snapshotId":"s","projects":[],"golden":"g","workspaceId":"w","workspaceName":"ws","createdAt":"2026-10-01T23:00:00.000Z"})
            )),
            "Its snapshot, taken from ws on 2026-10-01, is deleted at the provider, and no workspace starts from it again."
        ); // projectImageRemoveNotice({workspaceName:"ws",createdAt:"2026-10-01T23:00:00.000Z"})
        assert_eq!(project_image_removed_line("p1", true), "project image p1 was already gone at the provider; its record is removed"); // projectImageRemovedLine("p1", true)
        assert_eq!(
            project_image_removed_line("p1", false),
            "removed project image p1: its snapshot is gone at the provider, and its record with it"
        ); // projectImageRemovedLine("p1", false)
        assert_eq!(project_image_still_listed_line("p1", 1500.0), "project image p1 could not be read gone within 2 s of the delete answering, so its record stays; run wsp image remove p1 again"); // projectImageStillListedLine("p1", 1500)
        assert_eq!(project_image_refused_line("p1", &ProviderAnswer { status: 409.0, message: "busy".into(), request_id: None, at: "2026-10-25T10:00:00Z".into() }), "project image p1 was not removed: the provider answered 409 busy (no request id from the provider, at 2026-10-25T10:00:00Z); its record stays");
        // projectImageRefusedLine("p1", {"status":409,"message":"busy","at":"2026-10-25T10:00:00Z"})
    }

    #[test]
    fn logins_forgets_and_build_lines() {
        assert_eq!(copy_names_login("copy", "me@x"), "copy (me@x)"); // copyNamesLogin("copy", "me@x")
        assert_eq!(logins_here_line("me", &[] as &[&str]), "signed in here as me"); // loginsHereLine("me", [])
        assert_eq!(logins_here_line("me", &["a"] as &[&str]), "signed in here as me; a stays on this computer"); // loginsHereLine("me", ["a"])
        assert_eq!(logins_here_line("me", &["a", "b"] as &[&str]), "signed in here as me; a, b stay on this computer"); // loginsHereLine("me", ["a","b"])
        assert_eq!(forget_notice(0.0), "Its record and 0 threads leave this computer; the computer it ran on is already gone."); // forgetNotice(0)
        assert_eq!(forget_notice(1.0), "Its record and 1 thread leave this computer; the computer it ran on is already gone."); // forgetNotice(1)
        assert_eq!(forget_notice(2.0), "Its record and 2 threads leave this computer; the computer it ran on is already gone."); // forgetNotice(2)
        assert_eq!(golden_build_line(0.0, 1.0, &[]), "Builds version 1"); // goldenBuildLine(0, 1, [])
        assert_eq!(
            golden_build_line(
                2.0,
                3.0,
                &[
                    GoldenChange { count: 0.0, noun: "tool".into(), word: "added".into() },
                    GoldenChange { count: 1.0, noun: "tool".into(), word: "removed".into() },
                    GoldenChange { count: 2.0, noun: "sign-in".into(), word: "added".into() }
                ]
            ),
            "Builds version 3 on top of version 2: 1 tool removed, 2 sign-ins added"
        ); // goldenBuildLine(2, 3, [{count:0,noun:"tool",word:"added"},{count:1,noun:"tool",word:"removed"},{count:2,noun:"sign-in",word:"added"}])
    }

    #[test]
    fn pins_and_tools_rows() {
        assert_eq!(short_sum("0123456789abcdef"), "0123456789ab"); // shortSum("0123456789abcdef")
        assert_eq!(short_sum("short"), "short"); // shortSum("short")
        assert_eq!(version_moved_line(None, Some("1.2")), "unpinned to 1.2"); // versionMovedLine(undefined, "1.2")
        assert_eq!(version_moved_line(Some("1.1"), None), "1.1 to unpinned"); // versionMovedLine("1.1", undefined)
        assert_eq!(pin_words(&wire(json!({"tag":"v1"}))), "v1"); // pinWords({tag:"v1"})
        assert_eq!(pin_words(&wire(json!({"tag":"v1","latest":true}))), "v1, installs latest"); // pinWords({tag:"v1",latest:true})
        assert_eq!(pins_read_line(&[], &[]), None); // pinsReadLine([], [])
        assert_eq!(
            pins_read_line(
                &[
                    PinRead { name: "a, b".into(), tag: "1".into(), words: None },
                    PinRead { name: "c".into(), tag: "2".into(), words: None }
                ],
                &[]
            ),
            Some("pinned: \"a, b\" 1, c 2".to_string())
        ); // pinsReadLine([{name:"a, b",tag:"1"},{name:"c",tag:"2"}], [])
        assert_eq!(
            pins_read_line(
                &[],
                &[
                    PinRead { name: "bun".into(), tag: "1.3".into(), words: Some("by its installer".into()) },
                    PinRead { name: "x".into(), tag: "9".into(), words: None }
                ]
            ),
            Some("installs latest on every place: bun 1.3 by its installer, x 9".to_string())
        ); // pinsReadLine([], [{name:"bun",tag:"1.3",words:"by its installer"},{name:"x",tag:"9"}])
        assert_eq!(
            pins_read_line(
                &[PinRead { name: "a".into(), tag: "1".into(), words: None }],
                &[PinRead { name: "b".into(), tag: "2".into(), words: None }]
            ),
            Some("pinned: a 1; installs latest on every place: b 2".to_string())
        ); // pinsReadLine([{name:"a",tag:"1"}], [{name:"b",tag:"2"}])
        assert_eq!(
            sealed_pin_line("rg", &wire(json!({"tag":"14.1","sha256":"0123456789abcdef0123"})), None),
            "rg  14.1  checksum 0123456789ab"
        ); // sealedPinLine("rg", {tag:"14.1",sha256:"0123456789abcdef0123"})
        assert_eq!(sealed_pin_line("bun", &wire(json!({"tag":"1.3","latest":true})), Some("by curl")), "bun  1.3  installs latest by curl"); // sealedPinLine("bun", {tag:"1.3",latest:true}, "by curl")
        assert_eq!(sealed_pin_line("bun", &wire(json!({"tag":"1.3","latest":true})), None), "bun  1.3  installs latest"); // sealedPinLine("bun", {tag:"1.3",latest:true})
        assert_eq!(installed_here_line(Platform::Darwin, None), "installed on this Mac"); // installedHereLine("darwin", undefined)
        assert_eq!(installed_here_line(Platform::Linux, Some("1.0")), "installed on this computer, 1.0"); // installedHereLine("linux", "1.0")
        assert_eq!(
            add_already_here_line(Platform::Darwin, "ripgrep"),
            "--add ripgrep: a package manager on this Mac already has ripgrep, so it is a row of its own."
        ); // addAlreadyHereLine("darwin", "ripgrep")
        assert_eq!(
            not_here_line(Platform::Linux, "x", "recipe.json"),
            "x is ticked in recipe.json, but this computer has no row that installs it; it is left out."
        ); // notHereLine("linux", "x", "recipe.json")
        assert_eq!(behind_golden_line(2.0, 5.0), "on image v2, v5 available"); // behindGoldenLine(2, 5)
        assert_eq!(
            js(&missing_tool_row(&MissingTool { id: "rg".into(), name: None, outcome: None, note: "n".into() })),
            "{\"name\":\"rg\",\"note\":\"n\",\"mark\":\"failed\"}"
        ); // JSON.stringify(missingToolRow({id:"rg",note:"n"}))
        assert_eq!(
            js(&missing_tool_row(&MissingTool {
                id: "rg".into(),
                name: Some(String::new()),
                outcome: Some(LineageMark::Skipped),
                note: "n".into()
            })),
            "{\"name\":\"rg\",\"note\":\"n\",\"mark\":\"skipped\"}"
        ); // JSON.stringify(missingToolRow({id:"rg",name:"",outcome:"skipped",note:"n"}))
    }

    #[test]
    fn snapshot_refusals_name_the_disk_only_at_danger() {
        assert_eq!(
            provider_answer_line(&ProviderAnswer {
                status: 409.0,
                message: "busy".into(),
                request_id: None,
                at: "2026-10-25T10:00:00Z".into()
            }),
            "409 busy (no request id from the provider, at 2026-10-25T10:00:00Z)"
        ); // providerAnswerLine({"status":409,"message":"busy","at":"2026-10-25T10:00:00Z"})
        assert_eq!(
            provider_answer_line(&ProviderAnswer {
                status: 500.0,
                message: "boom".into(),
                request_id: Some("req-1".into()),
                at: "x".into()
            }),
            "500 boom (request req-1)"
        ); // providerAnswerLine({"status":500,"message":"boom","requestId":"req-1","at":"x"})
        assert_eq!(snapshot_refused_line("w", &ProviderAnswer { status: 409.0, message: "busy".into(), request_id: None, at: "2026-10-25T10:00:00Z".into() }, &DiskUse::Unknown { reason: "df failed".into() }), "w was not snapshotted: the provider answered 409 busy (no request id from the provider, at 2026-10-25T10:00:00Z); the disk could not be read (df failed)"); // snapshotRefusedLine("w", {"status":409,"message":"busy","at":"2026-10-25T10:00:00Z"}, {kind:"unknown",reason:"df failed"})
        assert_eq!(snapshot_refused_line("w", &ProviderAnswer { status: 409.0, message: "busy".into(), request_id: None, at: "2026-10-25T10:00:00Z".into() }, &DiskUse::Use { used_bytes: 95.0, size_bytes: 100.0 }), "w was not snapshotted: its disk is 95 percent full (95 of 100 B) and the provider answered 409 busy (no request id from the provider, at 2026-10-25T10:00:00Z); free space on it or delete the workspace, then snapshot again"); // snapshotRefusedLine("w", {"status":409,"message":"busy","at":"2026-10-25T10:00:00Z"}, {kind:"use",usedBytes:95,sizeBytes:100})
        assert_eq!(snapshot_refused_line("w", &ProviderAnswer { status: 500.0, message: "boom".into(), request_id: Some("req-1".into()), at: "x".into() }, &DiskUse::Use { used_bytes: 899.0, size_bytes: 1000.0 }), "w was not snapshotted: the provider answered 500 boom (request req-1); its disk is 89 percent full (899 of 1000 B), so the disk is not the reason"); // snapshotRefusedLine("w", {"status":500,"message":"boom","requestId":"req-1","at":"x"}, {kind:"use",usedBytes:899,sizeBytes:1000})
        assert_eq!(snapshot_manager_line("w", "pnpm -v", "/p", "ERR_PNPM", None), "w was not snapshotted: pnpm -v fails in /p (ERR_PNPM), so every fork of it would fail the same way; make pnpm -v answer there, then snapshot again"); // snapshotManagerLine("w", "pnpm -v", "/p", "ERR_PNPM")
        assert_eq!(snapshot_manager_line("w", "pnpm -v", "/p", "ERR", Some("at /home/u/.local/share/pnpm/.tools/pnpm/9.1.0_tmp_1234/bin/pnpm.cjs")), "w was not snapshotted: pnpm -v fails in /p (ERR), so every fork of it would fail the same way; remove /home/u/.local/share/pnpm/.tools/pnpm/9.1.0 and /home/u/.local/share/pnpm/.tools/pnpm/9.1.0_tmp_*, run pnpm -v in /p to fetch it again, then snapshot again"); // snapshotManagerLine("w", "pnpm -v", "/p", "ERR", "at /home/u/.local/share/pnpm/.tools/pnpm/9.1.0_tmp_1234/bin/pnpm.cjs")
        assert_eq!(snapshot_manager_line("w", "pnpm -v", "/p", "ERR", Some("x '/home/u/.local/share/pnpm/.tools/pnpm/9.1.0/bin/pnpm' y")), "w was not snapshotted: pnpm -v fails in /p (ERR), so every fork of it would fail the same way; remove /home/u/.local/share/pnpm/.tools/pnpm/9.1.0 and /home/u/.local/share/pnpm/.tools/pnpm/9.1.0_tmp_*, run pnpm -v in /p to fetch it again, then snapshot again"); // snapshotManagerLine("w", "pnpm -v", "/p", "ERR", "x '/home/u/.local/share/pnpm/.tools/pnpm/9.1.0/bin/pnpm' y")
        assert_eq!(snapshot_manager_line("w", "pnpm -v", "/p", "ERR", Some("at /s/store/v10/links/@/pnpm/11.0.0/node_modules/x")), "w was not snapshotted: pnpm -v fails in /p (ERR), so every fork of it would fail the same way; remove /s/store/v10/links/@/pnpm/11.0.0, run pnpm -v in /p to fetch it again, then snapshot again"); // snapshotManagerLine("w", "pnpm -v", "/p", "ERR", "at /s/store/v10/links/@/pnpm/11.0.0/node_modules/x")
        assert_eq!(
            snapshot_login_line("w", &["/a", "/b"]),
            "w was not snapshotted: it holds sign-in files (/a, /b), which every workspace forked from the snapshot would carry"
        ); // snapshotLoginLine("w", ["/a", "/b"])
        assert_eq!(snapshot_attempt_line(1.0, 3.0, &ProviderAnswer { status: 409.0, message: "busy".into(), request_id: None, at: "2026-10-25T10:00:00Z".into() }, &BuilderReading::State("running".into()), 30000.0), "attempt 1 of 3 answered 409 busy (no request id from the provider, at 2026-10-25T10:00:00Z); the builder reads running, next attempt in 30s"); // snapshotAttemptLine(1, 3, {"status":409,"message":"busy","at":"2026-10-25T10:00:00Z"}, "running", 30000)
        assert_eq!(snapshot_failed_line(1.0, &ProviderAnswer { status: 409.0, message: "busy".into(), request_id: None, at: "2026-10-25T10:00:00Z".into() }, &BuilderReading::State("gone".into()), None), "the snapshot failed 1 time: the provider answered 409 busy (no request id from the provider, at 2026-10-25T10:00:00Z) and no longer has the builder (404)"); // snapshotFailedLine(1, {"status":409,"message":"busy","at":"2026-10-25T10:00:00Z"}, "gone")
        assert_eq!(
            snapshot_failed_line(
                3.0,
                &ProviderAnswer { status: 500.0, message: "boom".into(), request_id: Some("req-1".into()), at: "x".into() },
                &BuilderReading::State("gone".into()),
                Some("ECONN")
            ),
            "the snapshot failed 3 times: the provider answered 500 boom (request req-1) and no longer has the builder (404)"
        ); // snapshotFailedLine(3, {"status":500,"message":"boom","requestId":"req-1","at":"x"}, "gone", "ECONN")
        assert_eq!(snapshot_failed_line(1.0, &ProviderAnswer { status: 409.0, message: "busy".into(), request_id: None, at: "2026-10-25T10:00:00Z".into() }, &BuilderReading::Unread, None), "the snapshot failed 1 time: the provider answered 409 busy (no request id from the provider, at 2026-10-25T10:00:00Z) and could not be read about the builder (no reason given)"); // snapshotFailedLine(1, {"status":409,"message":"busy","at":"2026-10-25T10:00:00Z"}, "unread")
        assert_eq!(
            snapshot_failed_line(
                3.0,
                &ProviderAnswer { status: 500.0, message: "boom".into(), request_id: Some("req-1".into()), at: "x".into() },
                &BuilderReading::Unread,
                Some("ECONN")
            ),
            "the snapshot failed 3 times: the provider answered 500 boom (request req-1) and could not be read about the builder (ECONN)"
        ); // snapshotFailedLine(3, {"status":500,"message":"boom","requestId":"req-1","at":"x"}, "unread", "ECONN")
        assert_eq!(snapshot_failed_line(1.0, &ProviderAnswer { status: 409.0, message: "busy".into(), request_id: None, at: "2026-10-25T10:00:00Z".into() }, &BuilderReading::State("paused".into()), None), "the snapshot failed 1 time: the provider answered 409 busy (no request id from the provider, at 2026-10-25T10:00:00Z) while the builder read paused"); // snapshotFailedLine(1, {"status":409,"message":"busy","at":"2026-10-25T10:00:00Z"}, "paused")
        assert_eq!(
            snapshot_failed_line(
                3.0,
                &ProviderAnswer { status: 500.0, message: "boom".into(), request_id: Some("req-1".into()), at: "x".into() },
                &BuilderReading::State("paused".into()),
                Some("ECONN")
            ),
            "the snapshot failed 3 times: the provider answered 500 boom (request req-1) while the builder read paused"
        ); // snapshotFailedLine(3, {"status":500,"message":"boom","requestId":"req-1","at":"x"}, "paused", "ECONN")
    }

    #[test]
    fn templates_builders_and_seals() {
        assert_eq!(template_status_line("ready"), "the image is saved"); // templateStatusLine("ready")
        assert_eq!(template_status_line("pending"), "saving the image, the provider says pending; asking again"); // templateStatusLine("pending")
        assert_eq!(template_status_line(""), "saving the image, the provider says ; asking again"); // templateStatusLine("")
        assert_eq!(template_failed_line("t1", None), "the provider failed the template t1: no reason given"); // templateFailedLine("t1", undefined)
        assert_eq!(template_waited_line("t1", "pending", 600000.0), "the template t1 still reads pending after 10m"); // templateWaitedLine("t1", "pending", 600000)
        assert_eq!(template_recorded_line("wsp", 3.0, "t1", None), "image wsp v3: template t1 promoted and recorded"); // templateRecordedLine("wsp", 3, "t1", undefined)
        assert_eq!(template_recorded_line("wsp", 3.0, "t1", Some(0.0)), "image wsp v3: template t1 promoted and recorded"); // templateRecordedLine("wsp", 3, "t1", 0)
        assert_eq!(
            template_recorded_line("wsp", 3.0, "t1", Some(1.0)),
            "image wsp v3: template t1 promoted and recorded; 1 other template carries its name"
        ); // templateRecordedLine("wsp", 3, "t1", 1)
        assert_eq!(
            template_recorded_line("wsp", 3.0, "t1", Some(2.0)),
            "image wsp v3: template t1 promoted and recorded; 2 other templates carry its name"
        ); // templateRecordedLine("wsp", 3, "t1", 2)
        assert_eq!(template_skipped_line("wsp", 3.0, "gone"), "image wsp v3: no template recorded, gone"); // templateSkippedLine("wsp", 3, "gone")
        assert_eq!(other_hosts_machines_line(&[]), "left alone 0 machines this host did not make: "); // otherHostsMachinesLine([])
        assert_eq!(
            other_hosts_machines_line(&[
                OwnedMachine { id: "m1".into(), owner: None },
                OwnedMachine { id: "m2".into(), owner: Some("lab".into()) }
            ]),
            "left alone 2 machines this host did not make: m1 (no owner), m2 (owner lab)"
        ); // otherHostsMachinesLine([{id:"m1"},{id:"m2",owner:"lab"}])
        assert_eq!(builder_stays_line("b1", 0.125, "wsp init --attach b1"), "Builder b1 stays up at about $0.13/hr; wsp init --attach b1 attaches to it again, and the sweep stops it once it is six hours old."); // builderStaysLine("b1", 0.125, "wsp init --attach b1")
        assert_eq!(seal_failed_builder_stays_line("b1", 0.5, "a"), "Seal failed; the builder is as you left it. Builder b1 stays up at about $0.50/hr; a attaches to it again, and the sweep stops it once it is six hours old."); // sealFailedBuilderStaysLine("b1", 0.5, "a")
        assert_eq!(seal_failed_builder_unread_line("b1", 0.5, "a"), "Seal failed; the provider could not be read about the builder, so nothing on it was touched. Builder b1 stays up at about $0.50/hr; a attaches to it again, and the sweep stops it once it is six hours old."); // sealFailedBuilderUnreadLine("b1", 0.5, "a")
        assert_eq!(
            credential_on_builder_line(&[] as &[&str]),
            "the builder holds a sign-in file at ; sign-ins never sit in an image, so nothing was sealed"
        ); // credentialOnBuilderLine([])
        assert_eq!(upgrade_seal_failed_stays_line(3.0, "b1", 2.005), "Image v3 is unchanged. Builder b1 is as it was, up at about $2.00/hr; run wsp init again to retry, and the sweep stops it once it is six hours old."); // upgradeSealFailedStaysLine(3, "b1", 2.005)
        assert_eq!(upgrade_seal_failed_unread_line(3.0, "b1"), "Image v3 is unchanged. The provider could not be read about builder b1, so nothing on it was touched; run wsp init again to retry, and the sweep stops it once it is six hours old."); // upgradeSealFailedUnreadLine(3, "b1")
        assert_eq!(upgrade_seal_failed_gone_line(3.0), "Image v3 is unchanged and the builder is gone: the provider dropped it after refusing the snapshot. Run wsp init again to retry.");
        // upgradeSealFailedGoneLine(3)
    }
}
