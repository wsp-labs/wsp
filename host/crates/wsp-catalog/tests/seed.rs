// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/seed.test.ts.
use wsp_catalog::catalog::{catalog_tool_for, smoke_of, CatalogEntry};
use wsp_catalog::seed::{seed_installs_for, seed_row_for, SeedKind, SEED_ROWS};

fn kind_of(path: &str, dir: bool) -> Option<SeedKind> {
    seed_row_for(path, dir).map(|(_, kind)| kind)
}

fn found(path: &str, dir: bool) -> Option<(String, SeedKind)> {
    seed_row_for(path, dir).map(|(row, kind)| (row.id.clone(), kind))
}

fn row_kind(id: &str, kind: SeedKind) -> Option<(String, SeedKind)> {
    Some((id.to_string(), kind))
}

fn runs(names: &[&str]) -> Vec<String> {
    let names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    seed_installs_for(&names).iter().map(|(_, install)| install.run.clone()).collect()
}

mod the_row_a_path_git_ignores_lands_on {
    use super::*;

    #[test]
    fn reads_a_local_env_file_as_a_file_of_values_which_only_a_box_of_the_persons_own_g() {
        let (row, kind) = seed_row_for(".env.local", false).unwrap();
        assert_eq!(row.id, "next");
        assert_eq!(kind, SeedKind::Secret);
    }

    #[test]
    fn reads_every_file_of_values_and_every_key_as_secret_before_any_configuration_row() {
        for path in [
            ".env",
            ".env.production",
            "apps/web/.npmrc",
            ".envrc",
            ".pypirc",
            "certs/local.pem",
            "certs/local.key",
            "prod.tfvars",
            "infra/stage.auto.tfvars",
            "app/release.keystore",
            "app/upload.jks",
            "config/master.key",
            "config/credentials/production.key",
            "certs/client.p12",
        ] {
            assert_eq!(kind_of(path, false), Some(SeedKind::Secret), "{path}");
        }
        for path in ["certs/local.crt", "local.properties", ".claude/settings.local.json", "app/local_settings.py"] {
            assert_eq!(kind_of(path, false), Some(SeedKind::Config), "{path}");
        }
    }

    #[test]
    fn reads_an_install_directory_as_one_the_computer_rebuilds() {
        assert_eq!(found("node_modules", true), row_kind("node", SeedKind::Rebuilt));
    }

    #[test]
    fn reads_a_database_file_as_data() {
        assert_eq!(found("dev.db", false), row_kind("databases", SeedKind::Data));
    }

    #[test]
    fn reads_a_login_as_one_that_never_travels_before_any_ecosystems_own_rule() {
        assert_eq!(found(".git-credentials", false), row_kind("logins", SeedKind::Never));
        // auth.json under a tool's folder is the tool's login, not a config file the agents row would carry.
        assert_eq!(found(".config/glab/auth.json", false), row_kind("logins", SeedKind::Never));
    }

    #[test]
    fn hides_the_metadata_the_operating_system_leaves_in_a_folder() {
        assert_eq!(kind_of(".DS_Store", false), Some(SeedKind::Junk));
        assert_eq!(kind_of("components/.DS_Store", false), Some(SeedKind::Junk));
    }

    #[test]
    fn answers_nothing_for_a_path_no_row_names_and_for_a_sample_env_file_the_catalog_do() {
        assert_eq!(found(".next-mock", true), None);
        assert_eq!(found("docs", true), None);
        assert_eq!(found(".env.example", false), None);
        assert_eq!(found(".env.sample", false), None);
    }

    #[test]
    fn asks_for_a_directory_where_the_pattern_names_one() {
        assert_eq!(found("dist", true), row_kind("node", SeedKind::Rebuilt));
        // A source file called dist is a file somebody wrote, whatever it is named.
        assert_eq!(found("dist", false), None);
    }

    #[test]
    fn matches_a_pattern_with_a_slash_at_a_segment_boundary_and_nowhere_else() {
        assert_eq!(found("config/master.key", false), row_kind("rails", SeedKind::Secret));
        assert_eq!(found("myconfig/master.key", false), row_kind("certificates", SeedKind::Secret));
    }
}

mod the_rows_themselves {
    use super::*;

    #[test]
    fn carry_one_id_each() {
        let ids: std::collections::HashSet<&str> = SEED_ROWS.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids.len(), SEED_ROWS.len());
    }

    #[test]
    fn each_judge_their_own_examples_and_no_earlier_row_takes_one_first() {
        assert!(!SEED_ROWS.is_empty());
        for row in SEED_ROWS.iter() {
            for (kind, examples) in &row.examples {
                for example in examples {
                    let dir = row.patterns.get(kind).map(|ps| ps.iter().any(|p| p.ends_with('/'))).unwrap_or(false);
                    let got = found(example, dir);
                    assert!(got.is_some(), "{} {:?} {}", row.id, kind, example);
                    assert_eq!(got, Some((row.id.clone(), *kind)), "{} {:?} {}", row.id, kind, example);
                }
            }
        }
    }
}

mod the_install_a_projects_own_root_picks {
    use super::*;

    #[test]
    fn answers_one_per_ecosystem_in_catalog_order() {
        assert_eq!(runs(&["package-lock.json", "Cargo.lock"]), vec!["npm ci".to_string(), "cargo fetch --locked".to_string()]);
    }

    #[test]
    fn takes_the_first_lockfile_a_row_lists_that_is_there_so_one_ecosystem_installs_onc() {
        assert_eq!(runs(&["pnpm-lock.yaml", "package-lock.json"]), vec!["npm ci".to_string()]);
    }

    #[test]
    fn takes_buns_text_lockfile_and_a_root_carrying_both_of_buns_installs_once() {
        assert_eq!(runs(&["bun.lock"]), vec!["bun install --frozen-lockfile".to_string()]);
        assert_eq!(runs(&["bun.lock", "bun.lockb"]), vec!["bun install --frozen-lockfile".to_string()]);
    }

    #[test]
    fn answers_nothing_for_a_root_with_no_lockfile_on_any_row() {
        assert_eq!(runs(&["package.json", "README.md"]), Vec::<String>::new());
    }
}

mod a_managers_check_on_its_install_row {
    use super::*;

    #[test]
    fn is_the_version_line_its_catalog_entry_answers_to_the_commands_own_or_the_one_the() {
        let mut checks: Vec<String> = vec![];
        for check in SEED_ROWS.iter().flat_map(|row| row.installs.iter().flatten()).filter_map(|i| i.check.clone()) {
            if !checks.contains(&check) {
                checks.push(check);
            }
        }
        assert_eq!(checks, vec!["npm --version", "pnpm --version", "yarn --version", "bun --version"]);
        for check in &checks {
            let bin = check.split(' ').next().unwrap();
            let entry = catalog_tool_for(bin).unwrap();
            let said = if entry.bin == bin {
                smoke_of(&CatalogEntry::Tool(entry.clone()))
            } else {
                entry.brings.iter().flatten().find(|b| b.bin == bin).map(|b| b.version.clone()).unwrap_or_default()
            };
            assert_eq!(&said, check);
        }
    }
}
