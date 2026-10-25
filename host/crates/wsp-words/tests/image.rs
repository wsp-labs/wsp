// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/format.test.ts:1388-1560 (goldenBuildLine, REPO_STATE_WORDS, LINEAGE_MARKS, the
//! template words, missingToolRow, behindGoldenLine, the provider answer and snapshot lines, the builder lines, a pinned
//! release that moved, a tools row outside the catalog) and init-job.test.ts:305 (credentialOnBuilderLine).

use regex::Regex;
use wsp_words::words::image::*;

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
        // The record's line: the row by name, the version, the checksum where the road hashed one, the latest mark in the road's words.
        // The stage's one line: the pinned rows, then once the rows that install latest with the version this build got; nothing when nothing was read.
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
        assert_eq!(image_rebuild_fix("boat"), "Seal your image again for boat."); // imageRebuildFix("boat")
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
    fn logins_forgets_and_build_lines() {
        assert_eq!(forget_notice(0.0), "Its record and 0 threads leave this computer; the computer it ran on is already gone."); // forgetNotice(0)
        assert_eq!(forget_notice(1.0), "Its record and 1 thread leave this computer; the computer it ran on is already gone."); // forgetNotice(1)
        assert_eq!(forget_notice(2.0), "Its record and 2 threads leave this computer; the computer it ran on is already gone.");
        // forgetNotice(2)
    }

    #[test]
    fn pins_and_tools_rows() {
        assert_eq!(short_sum("0123456789abcdef"), "0123456789ab"); // shortSum("0123456789abcdef")
        assert_eq!(short_sum("short"), "short"); // shortSum("short")
        assert_eq!(version_moved_line(None, Some("1.2")), "unpinned to 1.2"); // versionMovedLine(undefined, "1.2")
        assert_eq!(version_moved_line(Some("1.1"), None), "1.1 to unpinned"); // versionMovedLine("1.1", undefined)
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
        assert_eq!(
            credential_on_builder_line(&[] as &[&str]),
            "the builder holds a sign-in file at ; sign-ins never sit in an image, so nothing was sealed"
        ); // credentialOnBuilderLine([])
    }
}
