// SPDX-License-Identifier: AGPL-3.0-only
//! The seed menu for a folder on this computer. Port of packages/collect/test/seed.test.ts.
mod common;

use common::fake_host::*;
use wsp_collect::seed::{seed_menu, SeedOptions};
use wsp_wire::views::workspace::{SeedFileRow, SeedKind, SeedPlan, SeedPlanMemory, SeedPlanUnpushed};
use wsp_words::project_path::claude_project_key;

const FOLDER: &str = "/Users/dev/spoo-landing";
const CLAUDE: &str = "/Users/dev/.claude";

/// What git prints for a folder ignoring a local env file, the agent's folder, two package trees, a docs folder,
/// two build products and the Finder's own metadata: the listing read off spoo-landing on 2026-09-17.
fn listing() -> String {
    [
        ".env.local",
        ".claude/",
        "node_modules/",
        ".next-mock/",
        "docs/",
        "tsconfig.tsbuildinfo",
        "next-env.d.ts",
        ".DS_Store",
        "components/.DS_Store",
    ]
    .join("\0")
}

fn inside_claude() -> String {
    [".claude/settings.local.json", ".claude/sessions/last.json"].join("\0")
}

const DU_ROWS: [(&str, &str); 10] = [
    ("4", ".env.local"),
    ("44", ".claude"),
    ("2600000", "node_modules"),
    ("3200000", ".next-mock"),
    ("18000", "docs"),
    ("120", "tsconfig.tsbuildinfo"),
    ("4", "next-env.d.ts"),
    ("4", ".DS_Store"),
    ("4", "components/.DS_Store"),
    ("4", ".claude/settings.local.json"),
];

fn du() -> String {
    DU_ROWS.iter().map(|(kb, path)| format!("{kb}\t{FOLDER}/{path}")).collect::<Vec<_>>().join("\n")
}

fn git(args: &[&str]) -> String {
    let mut words = vec!["git", "-C", FOLDER];
    words.extend_from_slice(args);
    words.join(" ")
}

fn origin_key() -> String {
    git(&["remote", "get-url", "origin"])
}

fn laptop(over: &[(String, &str)], files: &[(String, u64)]) -> FakeLaptop {
    let du_key = format!("du -sk {}", DU_ROWS.iter().map(|(_, path)| format!("{FOLDER}/{path}")).collect::<Vec<_>>().join(" "));
    let mut laptop = FakeLaptop::new()
        .exec(&git(&["ls-files", "-z", "-o", "-i", "--exclude-standard", "--directory"]), &listing())
        .exec(&git(&["ls-files", "-z", "-o", "-i", "--exclude-standard", "--", ".claude"]), &inside_claude())
        .exec(&du_key, &du())
        .exec(&origin_key(), "https://github.com/spoo-me/frontend.git")
        .exec(&git(&["rev-parse", "--abbrev-ref", "HEAD"]), "refactor/dashboard-polish")
        .exec(&git(&["symbolic-ref", "refs/remotes/origin/HEAD"]), "refs/remotes/origin/main")
        .exec(&git(&["merge-base", "HEAD", "origin/main"]), "9f1c2e4aa11b0c3d4e5f60718293a4b5c6d7e8f9")
        .exec(&git(&["rev-list", "--count", "9f1c2e4aa11b0c3d4e5f60718293a4b5c6d7e8f9..HEAD"]), "0")
        .exec(&git(&["status", "--porcelain"]), " M app/page.tsx\n M app/layout.tsx\n?? scratch.md\n?? notes/todo.md");
    for (key, stdout) in over {
        laptop = laptop.exec(key, stdout);
    }
    for (path, bytes) in files {
        laptop = laptop.file(path, *bytes);
    }
    laptop
}

fn menu(host: &FakeHost, claude_state_home: &str) -> SeedPlan {
    block_on(seed_menu(&host.host, FOLDER, &SeedOptions { claude_state_home: claude_state_home.to_string() }))
}

mod the_seed_menu_for_a_folder_on_this_computer {
    use super::*;

    #[test]
    fn shows_every_path_git_ignores_sized_once_with_the_ticks_the_catalog_s_rows_decide() {
        let host = fake_host(laptop(&[], &[]));
        let plan = menu(&host, CLAUDE);
        let row = |path: &str, kind: SeedKind, bytes: i64, ticked: bool| (path.to_string(), kind, bytes, ticked);
        assert_eq!(
            plan.files.iter().map(|f| (f.path.clone(), f.kind, f.bytes, f.ticked)).collect::<Vec<_>>(),
            vec![
                row(".claude", SeedKind::Unknown, 40 * 1024, false),
                row(".claude/settings.local.json", SeedKind::Config, 4 * 1024, true),
                row(".env.local", SeedKind::Secret, 4 * 1024, true),
                row(".next-mock", SeedKind::Unknown, 3_200_000 * 1024, false),
                row("docs", SeedKind::Unknown, 18_000 * 1024, false),
                row("next-env.d.ts", SeedKind::Rebuilt, 4 * 1024, false),
                row("node_modules", SeedKind::Rebuilt, 2_600_000 * 1024, false),
                row("tsconfig.tsbuildinfo", SeedKind::Rebuilt, 120 * 1024, false),
            ]
        );
        // One du for the whole menu, never one per row.
        assert_eq!(host.calls().iter().filter(|call| call.starts_with("run du ")).count(), 1);
    }

    #[test]
    fn names_the_row_that_judged_each_path_so_the_menu_says_why_a_path_is_ticked() {
        let plan = menu(&fake_host(laptop(&[], &[])), CLAUDE);
        let row_of = |path: &str| plan.files.iter().find(|f| f.path == path).and_then(|f| f.row.clone());
        assert_eq!(row_of(".env.local"), Some(SeedFileRow { id: "next".to_string(), name: "Next".to_string() }));
        assert_eq!(row_of(".claude/settings.local.json"), Some(SeedFileRow { id: "agents".to_string(), name: "agents".to_string() }));
        assert_eq!(row_of("node_modules").map(|r| r.id), Some("node".to_string()));
        assert_eq!(row_of("docs"), None);
    }

    #[test]
    fn reads_the_branch_the_remote_s_default_the_edits_that_stay_here_and_nothing_ahead() {
        let plan = menu(&fake_host(laptop(&[], &[])), CLAUDE);
        assert_eq!(plan.remote.as_deref(), Some("https://github.com/spoo-me/frontend.git"));
        assert_eq!(plan.branch, "refactor/dashboard-polish");
        assert_eq!(plan.default_branch.as_deref(), Some("main"));
        assert_eq!(plan.unpushed, None);
        assert_eq!(plan.uncommitted, 4);
        assert!(!plan.remembered);
    }

    #[test]
    fn counts_the_commits_the_remote_does_not_have_against_the_branch_s_own_upstream() {
        let over = [
            (git(&["rev-parse", "--abbrev-ref", "@{upstream}"]), "origin/refactor/dashboard-polish"),
            (git(&["merge-base", "HEAD", "origin/refactor/dashboard-polish"]), "aa11bb22cc33"),
            (git(&["rev-list", "--count", "aa11bb22cc33..HEAD"]), "3"),
        ];
        let plan = menu(&fake_host(laptop(&over, &[])), CLAUDE);
        assert_eq!(plan.unpushed, Some(SeedPlanUnpushed { commits: 3, base: "aa11bb22cc33".to_string() }));
    }

    #[test]
    fn answers_no_remote_for_a_folder_git_has_no_origin_for_which_is_what_refuses_the_a() {
        let mut no_origin = laptop(&[], &[]);
        no_origin.exec.shift_remove(&origin_key());
        let host = fake_host(FakeLaptop { exec: no_origin.exec, ..FakeLaptop::new() });
        let plan = menu(&host, CLAUDE);
        assert_eq!(plan.remote, None);
    }

    #[test]
    fn carries_the_agent_s_memory_folder_for_this_folder_under_the_state_home_it_was_gi() {
        let key = claude_project_key(FOLDER);
        let files =
            [(format!("{CLAUDE}/projects/{key}/memory/MEMORY.md"), 2000), (format!("{CLAUDE}/projects/{key}/memory/one.md"), 26_000)];
        let plan = menu(&fake_host(laptop(&[], &files)), CLAUDE);
        assert_eq!(plan.memory, Some(SeedPlanMemory { key, files: 2, bytes: 28_000 }));
    }

    #[test]
    fn carries_no_memory_row_where_the_agent_kept_none_for_this_folder_or_kept_it_under() {
        let key = claude_project_key(FOLDER);
        let files = [(format!("/Users/dev/.claude-other/projects/{key}/memory/MEMORY.md"), 2000)];
        assert_eq!(menu(&fake_host(laptop(&[], &[])), CLAUDE).memory, None);
        assert_eq!(menu(&fake_host(laptop(&[], &files)), CLAUDE).memory, None);
        assert_eq!(
            menu(&fake_host(laptop(&[], &files)), "/Users/dev/.claude-other").memory,
            Some(SeedPlanMemory { key, files: 1, bytes: 2000 })
        );
    }

    #[test]
    fn reads_no_file_s_content_anywhere() {
        let host = fake_host(laptop(&[], &[]));
        menu(&host, CLAUDE);
        let reads: Vec<String> = host.calls().into_iter().filter(|call| call.starts_with("read ") || call.starts_with("lines ")).collect();
        assert_eq!(reads, Vec::<String>::new());
    }
}

/// Every case of tests/fixtures/oracle/w2/seed.json (oracle/w2-seed.oracle.ts): spec seed's wave-2 rules. The one
/// case whose du answer has a fraction of a kilobyte is skipped: the TS keeps 4403.2 bytes, and wsp-wire's
/// `SeedFile.bytes` is an `i64` (flagged on #2446).
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, host, w2};
    use serde_json::{json, Value};

    #[test]
    fn every_menu_is_the_plan_the_ts_wrote_with_the_same_calls() {
        for case in cases(&w2("seed"), "seed_menu") {
            let what = case["name"].as_str().unwrap();
            if what.starts_with("du lines: Number() parsing") {
                continue;
            }
            let h = host(&case["laptop"]);
            let o = SeedOptions { claude_state_home: case["args"][1]["claudeStateHome"].as_str().unwrap().to_string() };
            let plan = block_on(seed_menu(&h, case["args"][0].as_str().unwrap(), &o));
            assert_eq!(serde_json::to_value(&plan).unwrap(), case["out"], "{what}");
            assert_eq!(serde_json::to_string(&plan).unwrap(), serde_json::to_string(&case["out"]).unwrap(), "key order of {what}");
            assert_eq!(json!(h.calls()), case["calls"], "calls of {what}");
            let _: &Value = &case["out"];
        }
    }
}
