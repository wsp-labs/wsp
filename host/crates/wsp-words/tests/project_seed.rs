// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/project-seed.test.ts lines 45-206 (the memory key describe, lines 33-43, is
//! claudeProjectKey and claudeMemoryDir of project-path and is skipped here). The plan is built as a struct because
//! wsp-wire's nullable deserializer is todo!() too.

use wsp_wire::views::workspace::{SeedChoice, SeedFile, SeedFileRow, SeedKind, SeedPlan, SeedPlanMemory, SeedPlanUnpushed};
use wsp_words::exit::{exit_class_of, ExitClass};
use wsp_words::project_seed::*;

const NONE: [&str; 0] = [];

fn file(path: &str, kind: SeedKind, bytes: i64, row: Option<(&str, &str)>) -> SeedFile {
    SeedFile {
        path: path.to_string(),
        dir: false,
        bytes,
        kind,
        row: row.map(|(id, name)| SeedFileRow { id: id.to_string(), name: name.to_string() }),
        ticked: kind == SeedKind::Config,
    }
}

fn plan() -> SeedPlan {
    SeedPlan {
        source: "/Users/dev/spoo-landing".to_string(),
        remote: Some("https://github.com/spoo-me/frontend.git".to_string()),
        branch: "refactor/dashboard-polish".to_string(),
        default_branch: Some("main".to_string()),
        unpushed: Some(SeedPlanUnpushed { commits: 2, base: "9f1c2e4aa11b0c3d4e5f60718293a4b5c6d7e8f9".to_string() }),
        uncommitted: 4,
        memory: Some(SeedPlanMemory { key: "-Users-dev-spoo-landing".to_string(), files: 5, bytes: 28_000 }),
        files: vec![
            file(".env.local", SeedKind::Config, 4096, Some(("next", "Next"))),
            file(".claude/settings.local.json", SeedKind::Config, 4096, Some(("agents", "agents"))),
            file("node_modules", SeedKind::Rebuilt, 2_600_000_000, Some(("node", "Node"))),
            file("dev.db", SeedKind::Data, 900_000, Some(("databases", "databases"))),
            file(".git-credentials", SeedKind::Never, 300, Some(("logins", "logins"))),
            file(".next-mock", SeedKind::Unknown, 3_200_000_000, None),
        ],
        remembered: false,
        provider: None,
    }
}

fn choice(files: &[&str], memory: bool, commits: bool) -> SeedChoice {
    SeedChoice { files: files.iter().map(|s| s.to_string()).collect(), memory, commits, remember: None }
}

fn strings(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

mod what_a_seed_ticks_before_anybody_touches_the_menu {
    use super::*;

    #[test]
    fn ticks_the_configuration_the_memory_and_the_commits_and_nothing_of_any_other_kind() {
        assert_eq!(default_seed_choice(&plan()), choice(&[".env.local", ".claude/settings.local.json"], true, true));
    }

    #[test]
    fn takes_the_ticks_the_plan_carries_so_an_untick_remembered_for_that_folder_holds() {
        // The menu a remembered choice was applied to carries their ticks, not the catalogue's: a config row they
        // unticked last time stays unticked, and a row they kept that no row names travels.
        let base = plan();
        let remembered = SeedPlan {
            remembered: true,
            files: base.files.iter().cloned().map(|f| SeedFile { ticked: f.path == ".next-mock", ..f }).collect(),
            ..base
        };
        assert_eq!(default_seed_choice(&remembered), choice(&[".next-mock"], true, true));
    }

    #[test]
    fn never_ticks_a_login_whatever_a_remembered_choice_says_about_it() {
        let base = plan();
        let remembered = SeedPlan { remembered: true, files: base.files.iter().cloned().map(|f| SeedFile { ticked: true, ..f }).collect(), ..base };
        let files = default_seed_choice(&remembered).files;
        assert!(!files.contains(&".git-credentials".to_string()), "{files:?}");
    }

    #[test]
    fn ticks_no_memory_and_no_commits_where_the_folder_has_neither() {
        let bare = SeedPlan { memory: None, unpushed: None, ..plan() };
        assert_eq!(default_seed_choice(&bare), choice(&[".env.local", ".claude/settings.local.json"], false, false));
    }
}

mod what_a_person_s_own_words_make_of_the_menu {
    use super::*;

    #[test]
    fn unticks_a_path_they_cut_and_ticks_one_the_catalog_does_not_carry_when_they_keep() {
        assert_eq!(seed_choice_from(&plan(), &NONE, &[".env.local"], &SeedFlags::default()).unwrap().files, strings(&[".claude/settings.local.json"]));
        assert_eq!(
            seed_choice_from(&plan(), &[".next-mock"], &NONE, &SeedFlags::default()).unwrap().files,
            strings(&[".env.local", ".claude/settings.local.json", ".next-mock"])
        );
    }

    #[test]
    fn drops_the_memory_and_the_patch_on_the_words_that_say_so_and_remembers_only_when() {
        let no_memory = seed_choice_from(&plan(), &NONE, &NONE, &SeedFlags { memory: Some(false), ..Default::default() }).unwrap();
        assert_eq!((no_memory.memory, no_memory.commits), (false, true));
        let no_commits = seed_choice_from(&plan(), &NONE, &NONE, &SeedFlags { commits: Some(false), ..Default::default() }).unwrap();
        assert_eq!((no_commits.memory, no_commits.commits), (true, false));
        assert_eq!(seed_choice_from(&plan(), &NONE, &NONE, &SeedFlags { remember: Some(true), ..Default::default() }).unwrap().remember, Some(true));
        assert_eq!(seed_choice_from(&plan(), &NONE, &NONE, &SeedFlags::default()).unwrap().remember, None);
    }

    #[test]
    fn refuses_to_carry_a_login_by_name_even_when_it_is_the_path_they_kept() {
        let e = seed_choice_from(&plan(), &[".git-credentials"], &NONE, &SeedFlags::default()).unwrap_err();
        assert!(e.message.contains("never travels in a seed"), "{}", e.message);
    }

    #[test]
    fn refuses_a_path_the_menu_never_showed_and_says_what_it_holds() {
        let e = seed_choice_from(&plan(), &["app/page.tsx"], &NONE, &SeedFlags::default()).unwrap_err();
        assert!(e.message.contains("is not on the seed menu"), "{}", e.message);
    }

    #[test]
    fn takes_a_folder_named_with_the_slash_its_row_prints() {
        let files = seed_choice_from(&plan(), &["node_modules/"], &NONE, &SeedFlags::default()).unwrap().files;
        assert!(files.contains(&"node_modules".to_string()), "{files:?}");
    }

    #[test]
    fn refuses_a_cut_that_names_no_row_so_a_typo_never_leaves_the_path_it_meant_ticked() {
        // ... and the repository's own folder, as usage.
        let shown: Vec<String> = plan().files.iter().map(|f| f.path.clone()).collect();
        let cases: Vec<(Vec<&str>, Vec<&str>, String)> = vec![
            (vec![], vec![".env.locl"], not_in_the_menu_line(".env.locl", &shown)),
            (vec![".git"], vec![], never_git_line(".git")),
            (vec![], vec![".git/config"], never_git_line(".git/config")),
        ];
        for (keep, cut, line) in cases {
            let refused = seed_choice_from(&plan(), &keep, &cut, &SeedFlags::default()).unwrap_err();
            assert_eq!(refused.message, line);
            assert_eq!(exit_class_of(&refused), ExitClass::Usage);
        }
    }
}

mod a_project_s_seed_picks {
    use super::*;

    #[test]
    fn start_where_they_are_told_and_keep_a_start_path_the_menu_no_longer_shows_as_miss() {
        assert_eq!(
            seed_picks(&plan(), &[".env.local", "old.env"], &["dev.db"], &NONE).unwrap(),
            SeedPicks { files: strings(&[".env.local", "dev.db"]), missing: strings(&["old.env"]) }
        );
        assert_eq!(
            seed_picks(&plan(), &[".env.local", "old.env"], &NONE, &["old.env"]).unwrap(),
            SeedPicks { files: strings(&[".env.local"]), missing: vec![] }
        );
        let e = seed_picks(&plan(), &NONE, &["old.env"], &NONE).unwrap_err();
        assert!(e.message.contains("is not on the seed menu"), "{}", e.message);
        let e = seed_picks(&plan(), &["old.env"], &["old.env"], &["old.env"]).unwrap_err();
        assert!(e.message.contains("is not on the seed menu"), "{}", e.message);
    }

    #[test]
    fn draw_the_tick_column_one_wide_whether_anything_is_picked_or_not_and_name_the_mis() {
        let p = plan();
        let none = project_seed_lines(&project_seed_of("lab", &p, &SeedPicks { files: vec![], missing: vec![] }, false));
        assert_eq!(none.rows[1..].iter().map(|r| r[0].as_str()).collect::<Vec<_>>(), p.files.iter().map(|_| " ").collect::<Vec<_>>());
        assert_eq!(none.foot, strings(&["0 picked. Nothing was saved; add --yes to save these picks."]));
        let some = project_seed_lines(&project_seed_of("lab", &p, &SeedPicks { files: strings(&[".env.local"]), missing: strings(&["old.env"]) }, true));
        assert_eq!(some.rows[1][0], "x");
        assert_eq!(
            some.foot,
            strings(&["Picked but not in the folder now, so a sub-thread starts without: old.env.", "1 picked, 4 KB. Saved: every later sub-thread of lab gets these."])
        );
    }
}

mod a_file_of_values_or_a_key {
    use super::*;

    fn values(provider: bool) -> SeedPlan {
        SeedPlan {
            files: vec![
                SeedFile { ticked: true, ..file(".env", SeedKind::Secret, 120, Some(("node", "Node"))) },
                file(".claude/settings.local.json", SeedKind::Config, 4096, Some(("agents", "agents"))),
            ],
            provider: if provider { Some(true) } else { None },
            ..plan()
        }
    }

    #[test]
    fn is_ticked_for_a_box_as_configuration_is_and_named_by_its_row() {
        let b = values(false);
        assert_eq!(default_seed_choice(&b).files, strings(&[".env", ".claude/settings.local.json"]));
        let files = seed_choice_from(&b, &NONE, &NONE, &SeedFlags::default()).unwrap().files;
        assert!(files.contains(&".env".to_string()), "{files:?}");
        assert_eq!(seed_row_words(&b.files[0], Some(&b)), "Node");
    }

    #[test]
    fn never_goes_to_a_provider_place_unticked_whatever_was_remembered_shown_as_staying() {
        let cloud = values(true);
        assert_eq!(default_seed_choice(&cloud).files, strings(&[".claude/settings.local.json"]));
        let rows = seed_menu_rows(&cloud, &default_seed_choice(&cloud));
        let row = strings(&["[ ]", ".env", "120 B", "stays on this computer"]);
        assert!(rows.contains(&row), "{rows:?}");
        let e = seed_choice_from(&cloud, &[".env"], &NONE, &SeedFlags::default()).unwrap_err();
        assert!(e.message.contains(&stays_here_line(".env")), "{}", e.message);
        let e = seed_choice_from(&cloud, &[".env"], &NONE, &SeedFlags::default()).unwrap_err();
        assert_eq!(exit_class_of(&e), ExitClass::Usage);
    }

    #[test]
    fn drops_out_of_saved_picks_for_a_provider_place_and_stays_in_them_for_a_box() {
        assert_eq!(
            seed_picks(&values(true), &[".env", ".claude/settings.local.json"], &NONE, &NONE).unwrap(),
            SeedPicks { files: strings(&[".claude/settings.local.json"]), missing: vec![] }
        );
        assert_eq!(
            seed_picks(&values(false), &[".env", ".claude/settings.local.json"], &NONE, &NONE).unwrap(),
            SeedPicks { files: strings(&[".env", ".claude/settings.local.json"]), missing: vec![] }
        );
    }
}

mod the_words_a_row_ends_in {
    use super::*;

    #[test]
    fn say_why_each_kind_is_ticked_or_not() {
        let p = plan();
        assert_eq!(seed_row_words(&p.files[0], None), "Next");
        assert_eq!(seed_row_words(&p.files[2], None), "rebuilt on the box");
        assert_eq!(seed_row_words(&p.files[3], None), "database, stop what uses it first");
        assert_eq!(seed_row_words(&p.files[4], None), "never travels");
        assert_eq!(seed_row_words(&p.files[5], None), "not in the catalogue");
    }
}

mod what_the_person_is_told_would_travel {
    use super::*;

    #[test]
    fn is_the_ticked_files_the_memory_the_patch_and_what_stays_here() {
        let p = plan();
        assert_eq!(
            seed_summary_lines(&p, &default_seed_choice(&p)),
            strings(&[
                "2 files, 8 KB: .env.local, .claude/settings.local.json",
                "Claude Code memory, 5 files, 27 KB",
                "2 commits the remote does not have, as a patch from 9f1c2e4",
                "4 uncommitted changes stay on this computer",
            ])
        );
    }

    #[test]
    fn says_no_files_travel_where_nothing_is_ticked_and_nothing_about_a_folder_with_no() {
        let bare = SeedPlan { uncommitted: 0, memory: None, unpushed: None, ..plan() };
        assert_eq!(seed_summary_lines(&bare, &choice(&[], false, false)), strings(&["no files travel"]));
    }
}

mod what_the_seeding_stage_says_as_the_files_go_and_once_they_have_landed {
    use super::*;

    #[test]
    fn names_the_files_the_memory_and_the_commits_in_the_menu_s_own_counts_and_where_th() {
        let p = plan();
        assert_eq!(
            seeding_line(&p, &default_seed_choice(&p)),
            "Seeding 2 files (8 KB), Claude Code memory (5 files, 27 KB), 2 commits the remote does not have, from /Users/dev/spoo-landing."
        );
        // A memory folder is no file of the seed's: the menu counts it on a row of its own and so does this line,
        // so a seed of nothing but the memory reads as nought files here and as one file there.
        let only_memory = SeedPlan { unpushed: None, memory: Some(SeedPlanMemory { key: "-Users-dev-spoo-landing".to_string(), files: 1, bytes: 69 }), ..plan() };
        assert_eq!(seeding_line(&only_memory, &choice(&[], true, false)), "Seeding 0 files, Claude Code memory (1 file, 69 B), from /Users/dev/spoo-landing.");
        // Nothing ticked at all still says where it came from.
        let bare = SeedPlan { memory: None, unpushed: None, ..plan() };
        assert_eq!(seeding_line(&bare, &choice(&[], false, false)), "Seeding 0 files, from /Users/dev/spoo-landing.");
    }

    #[test]
    fn says_what_the_commits_did_off_the_count_the_computer_s_own_git_read_back() {
        assert_eq!(seed_commits_landed_line(1.0), "1 commit landed");
        assert_eq!(seed_commits_landed_line(2.0), "2 commits landed");
        assert_eq!(seed_commits_landed_line(0.0), "no commits to land");
    }
}
