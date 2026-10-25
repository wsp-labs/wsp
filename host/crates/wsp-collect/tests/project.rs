// SPDX-License-Identifier: AGPL-3.0-only
//! What a project folder's own files say it needs: the readers one by one, the names they land on in the catalog, the
//! candidates the catalog has no row for, and the line each row shows. Port of packages/collect/test/project.test.ts.
mod common;

use std::collections::HashSet;

use common::fake_host::*;
use serde_json::json;
use wsp_collect::project::{scan_project, ProjectNeed, ProjectScan, PROJECT_READERS};

const PROJ: &str = "/Users/dev/proj";

fn project(files: &[(&str, String)]) -> FakeHost {
    let mut laptop = FakeLaptop::new();
    for (k, v) in files {
        laptop = laptop.file(&format!("{PROJ}/{k}"), v.as_str());
    }
    fake_host(laptop)
}

fn scan(host: &FakeHost) -> ProjectScan {
    block_on(scan_project(&host.host, PROJ))
}

/// The (id, why) of every row, in row order.
fn why(scan: &ProjectScan) -> Vec<(String, String)> {
    scan.rows.iter().map(|n| (n.id.clone(), n.why.clone())).collect()
}

fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

fn need(id: &str, name: &str, why: &str) -> ProjectNeed {
    ProjectNeed { id: id.to_string(), name: name.to_string(), why: why.to_string() }
}

fn s(text: &str) -> String {
    text.to_string()
}

mod scan_project_cases {
    use super::*;

    #[test]
    fn a_repo_with_a_compose_file_a_pnpm_lock_and_a_go_mod_ticks_docker_pnpm_and_go_eac() {
        let scan = scan(&project(&[
            ("compose.yaml", s("services:\n  db:\n    image: postgres\n")),
            ("pnpm-lock.yaml", s("lockfileVersion: '9.0'\n")),
            ("go.mod", s("module example.com/x\n\ngo 1.24\n")),
        ]));
        assert_eq!(
            scan.rows,
            vec![
                need("pnpm", "pnpm", "pnpm-lock.yaml needs pnpm"),
                need("go", "Go", "go.mod needs Go"),
                need("docker", "Docker engine and compose", "compose.yaml needs Docker"),
            ]
        );
        assert_eq!(scan.candidates, vec![]);
        assert_eq!(scan.dir, PROJ);
    }

    #[test]
    fn package_json_the_manager_it_pins_every_runtime_its_engines_field_accepts_and_the() {
        let pkg = json!({ "packageManager": "pnpm@10.1.0", "engines": { "node": ">=22" }, "scripts": { "build": "tsup src/index.ts", "deploy": "pnpm build && wrangler deploy", "ci": "docker compose up -d" } });
        let scan = scan(&project(&[("package.json", pkg.to_string())]));
        assert_eq!(
            why(&scan),
            pairs(&[
                ("pnpm", "packageManager pnpm@10.1.0"),
                ("node", "engines.node >=22"),
                ("wrangler", "package.json scripts run wrangler"),
                ("docker", "package.json scripts run docker"),
            ])
        );
        // A script's first word names a devDependency as often as a machine tool: tsup is not a candidate for a row of its own.
        assert_eq!(scan.candidates, vec![]);
    }

    #[test]
    fn the_toolchain_pins_name_a_tool_and_its_version_in_asdf_s_syntax_and_in_mise_s_an() {
        let asdf = scan(&project(&[(".tool-versions", s("# pinned\nnodejs 22.1.0\ngolang 1.24.0\n\nruby 3.3.5\n"))]));
        assert_eq!(
            why(&asdf),
            pairs(&[
                ("node", ".tool-versions names nodejs 22.1.0"),
                ("go", ".tool-versions names golang 1.24.0"),
                ("ruby", ".tool-versions names ruby 3.3.5"),
                ("mise", ".tool-versions needs mise"),
            ])
        );
        assert_eq!(asdf.candidates, vec![]);
        let mise = scan(&project(&[("mise.toml", s("[tools]\ngo = \"1.24\"\npython = [\"3.12\"]\n\n[env]\nGOFLAGS = \"-mod=mod\"\n"))]));
        assert_eq!(
            why(&mise),
            pairs(&[("go", "mise.toml names go 1.24"), ("python", "mise.toml names python 3.12"), ("mise", "mise.toml needs mise")])
        );
    }

    #[test]
    fn package_json_dependencies_a_package_the_project_installs_itself_by_npm_is_not_a() {
        let pkg = json!({ "dependencies": { "react": "^19.0.0", "@railway/cli": "^4.0.0" }, "devDependencies": { "@playwright/test": "^1.62.1", "eslint": "^9.0.0", "typescript": "^5.0.0", "vercel": "^41.0.0" } });
        let scan_pkg = scan(&project(&[("package.json", pkg.to_string())]));
        // eslint, typescript, vercel and the Railway CLI are npm globals in the catalog: the project's own install covers them.
        // Playwright's row installs its Chromium and the browser's Debian packages, which no dependency brings.
        assert_eq!(why(&scan_pkg), pairs(&[("playwright", "package.json depends on @playwright/test ^1.62.1")]));
        // A dependency the catalog has no row for is the project's to install, never a custom row candidate.
        assert_eq!(scan_pkg.candidates, vec![]);
        let plain = scan(&project(&[("package.json", json!({ "devDependencies": { "playwright": "1.62.1" } }).to_string())]));
        assert_eq!(why(&plain), pairs(&[("playwright", "package.json depends on playwright 1.62.1")]));
        // A client library shares its name with the server's tools: node-redis is not a need for redis-cli, the sqlite
        // binding not one for the sqlite3 shell, and an npm package called chromium is not a browser.
        let clients = scan(&project(&[(
            "package.json",
            json!({ "dependencies": { "redis": "^4.7.0", "sqlite": "^5.1.1", "pg": "^8.0.0", "chromium": "^3.0.0", "bun": "^1.0.0" } })
                .to_string(),
        )]));
        assert_eq!(clients, ProjectScan { dir: s(PROJ), rows: vec![], candidates: vec![] });
    }

    #[test]
    fn a_workspace_package_s_own_package_json_counts_under_apps_or_packages_and_says_wh() {
        let scan = scan(&project(&[
            ("package.json", json!({ "private": true }).to_string()),
            ("apps/web/package.json", json!({ "devDependencies": { "playwright": "1.62.1" } }).to_string()),
            ("packages/cli/package.json", json!({ "scripts": { "release": "gh release create" } }).to_string()),
            ("apps/README.md", s("# apps")),
        ]));
        assert_eq!(
            why(&scan),
            pairs(&[
                ("playwright", "apps/web/package.json depends on playwright 1.62.1"),
                ("gh", "packages/cli/package.json scripts run gh")
            ])
        );
    }

    #[test]
    fn a_workflow_s_setup_actions_and_the_tools_its_run_steps_call_inline_and_in_a_bloc() {
        let yml = [
            "jobs:",
            "  build:",
            "    steps:",
            "      - uses: actions/setup-go@v5",
            "      - uses: pnpm/action-setup@v4",
            "      - run: go build ./...",
            "      - run: |",
            "          docker compose up -d",
            "          npm ci",
            "      - name: after",
            "        run: echo done",
        ]
        .join("\n");
        let scan = scan(&project(&[(".github/workflows/ci.yml", yml)]));
        assert_eq!(
            why(&scan),
            pairs(&[
                ("go", ".github/workflows/ci.yml sets up go"),
                ("pnpm", ".github/workflows/ci.yml sets up pnpm"),
                ("docker", ".github/workflows/ci.yml runs docker"),
                ("node", ".github/workflows/ci.yml runs npm"),
            ])
        );
    }

    #[test]
    fn an_action_named_after_its_owner_is_that_owner_s_tool_not_a_candidate_for_the_act() {
        let yml = [
            "jobs:",
            "  build:",
            "    steps:",
            "      - uses: docker/setup-buildx-action@v3",
            "      - uses: docker/setup-qemu-action@v3",
            "      - uses: actions/setup-dotnet@v4",
        ]
        .join("\n");
        let scan = scan(&project(&[(".github/workflows/ci.yml", yml)]));
        assert_eq!(why(&scan), pairs(&[("docker", ".github/workflows/ci.yml sets up docker")]));
        // A setup action nobody's catalog carries is still a real ask, under the toolchain it names.
        assert_eq!(scan.candidates, vec![need("dotnet", "dotnet", ".github/workflows/ci.yml sets up dotnet")]);
    }

    #[test]
    fn an_engines_key_the_catalog_has_no_row_for_is_not_a_custom_row_candidate_engines() {
        let scan = scan(&project(&[("package.json", json!({ "engines": { "node": ">=22", "vscode": "^1.80.0" } }).to_string())]));
        assert_eq!(why(&scan), pairs(&[("node", "engines.node >=22")]));
        assert_eq!(scan.candidates, vec![]);
    }

    #[test]
    fn corepack_s_integrity_suffix_stays_out_of_the_line_the_row_shows() {
        let spec = format!("pnpm@10.15.0+sha512.{}abcdefgh", "1234567890".repeat(8));
        let scan = scan(&project(&[("package.json", json!({ "packageManager": spec }).to_string())]));
        assert_eq!(why(&scan), pairs(&[("pnpm", "packageManager pnpm@10.15.0")]));
    }

    #[test]
    fn a_marker_is_read_for_its_presence_alone_a_lockfile_of_megabytes_or_bytes_that_ar() {
        let host = project(&[("package-lock.json", s("{}")), ("pnpm-lock.yaml", s("lockfileVersion: '9.0'\n"))]);
        let scan = scan(&host);
        assert_eq!(scan.rows.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), vec!["pnpm", "node"]);
        assert_eq!(host.calls().into_iter().filter(|c| c.contains("lock")).collect::<Vec<_>>(), Vec::<String>::new());
    }

    #[test]
    fn a_gemfile_and_a_composer_json_tick_ruby_and_php_and_the_first_file_to_name_a_too() {
        let scan = scan(&project(&[
            ("Gemfile", s("source \"https://rubygems.org\"\n")),
            ("composer.json", s("{}")),
            ("package.json", json!({ "packageManager": "pnpm@10.1.0" }).to_string()),
            ("pnpm-lock.yaml", s("lockfileVersion: '9.0'\n")),
        ]));
        assert_eq!(
            why(&scan),
            pairs(&[("pnpm", "packageManager pnpm@10.1.0"), ("ruby", "Gemfile needs Ruby"), ("php", "composer.json needs PHP")])
        );
        assert_eq!(
            scan.rows.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(),
            vec!["pnpm", "Ruby 3.1 with bundler", "PHP 8.2 with Composer"]
        );
        assert_eq!(scan.candidates, vec![]);
    }

    #[test]
    fn a_folder_with_nothing_to_read_and_one_that_is_not_there_at_all_ask_for_nothing() {
        assert_eq!(scan(&project(&[("README.md", s("# x"))])), ProjectScan { dir: s(PROJ), rows: vec![], candidates: vec![] });
        assert_eq!(
            block_on(scan_project(&project(&[]).host, &format!("{PROJ}/"))),
            ProjectScan { dir: s(PROJ), rows: vec![], candidates: vec![] }
        );
    }

    #[test]
    fn every_reader_is_registered_once_and_names_the_files_it_reads() {
        assert_eq!(PROJECT_READERS.iter().map(|r| r.id).collect::<HashSet<_>>().len(), PROJECT_READERS.len());
        for reader in PROJECT_READERS.iter() {
            assert!(!reader.files.is_empty(), "reader {} names no files", reader.id);
        }
    }
}
