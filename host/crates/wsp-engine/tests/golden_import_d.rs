// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts:1437-1698 (inside "golden import stages"): the login shell's first start, the tools' rows and their checks, the
//! agents and the plugin seed. A stage frame's `step` is not carried by the engine's tools loop, so no assertion here reads one.
#![allow(clippy::type_complexity)]
mod golden_import_a_support;
mod golden_roads_common;

use golden_import_a_support::*;
use std::sync::Arc;

use async_trait::async_trait;
use golden_roads_common::*;
use wsp_engine::golden::{prepare_builder, AgentResult, ApplyImportOptions};
use wsp_engine::golden_seam::{
    custom_installs_for, BrewFormula, BrewTable, GoldenImport, LoginShell, PinRead, PluginSeedPlan, PluginsPlan, RecipeEntry, SeedLinks,
    SeedPlugin, SeedRead, ShellInstall, ToolInstall, ToolResult, ToolRoad, ToolSource, TOOLS_PATH,
};
use wsp_wire::views::golden_image::{CustomLit, RecipeCustomRow};

// golden.test.ts:1437
#[test]
fn login_shell_check_runs_after_the_installs_and_records_its_noise_or_none() {
    let noisy = backend_for(
        vec![("zsh -lic true", Answer::Fixed(exec(0, "", "zsh: command not found: starship\nzsh eza plugin: eza not found.\n")))],
        Free::Kb(mb(3000)),
    );
    let st = stage_recorder();
    let shell = ShellInstall { shell: LoginShell::Zsh, frameworks: Vec::new(), cmd: "install-zsh".into() };
    let up = Uploads::new(&[]);
    let mut o = prep(&noisy.rec, &up, GoldenImport { shell: Some(shell.clone()), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    let cmds = noisy.cmds();
    let at = |needle: &str| -> isize {
        let i = find_index(&cmds, needle);
        assert!(i >= 0, "{needle}");
        i
    };
    let check = cmds[at("zsh -lic true") as usize].clone();
    // A login shell like the app's terminal: profile.d puts the tools on PATH before the rc files are read.
    assert!(check.contains("TERM=xterm-256color zsh -lic true </dev/null"));
    // A job the rc file leaves holding the output open ends with the check rather than holding the exec to its deadline.
    assert!(check.contains(&format!("{} || exit 124", bounded(60, "TERM=xterm-256color zsh -lic true </dev/null"))));
    assert!(check.contains("\nsetsid bash -c '"));
    // A tool or an agent the rc calls is on the machine by then: the check runs after the last install of each stage.
    for before in ["tar xzf", "claude-install", "codex-install", "brew install gh", "bun@1.4.0", "autoremove"] {
        assert!(at("zsh -lic true") > at(before), "{before}");
    }
    assert!(st
        .all()
        .iter()
        .any(|l| l.starts_with("installing-mcp:shell noise: zsh: command not found: starship, 2 lines; machine context:")));
    assert_eq!(ledger_of(&builder).shell_noise.as_deref(), Some("zsh: command not found: starship, 2 lines"));
    let sealed = seal(builder, seal_opts(&noisy.rec, "true"));
    assert_eq!(sealed.version.shell_noise.as_deref(), Some("zsh: command not found: starship, 2 lines"));

    let quiet = backend_default();
    let q = stage_recorder();
    let mut qo = prep(&quiet.rec, &Uploads::new(&[]), GoldenImport { shell: Some(shell.clone()), ..import_of() });
    qo.on_stage = Some(q.on_stage.clone());
    let qb = build(qo);
    assert!(q.all().iter().any(|l| l.starts_with("installing-mcp:zsh starts quiet; machine context:")));
    let hung = backend_for(vec![("zsh -lic true", Answer::Fixed(exec(124, "", "")))], Free::Kb(mb(3000)));
    let h = build(prep(&hung.rec, &Uploads::new(&[]), GoldenImport { shell: Some(shell), ..import_of() }));
    assert_eq!(ledger_of(&h).shell_noise.as_deref(), Some("timed out after 60s"));
    assert!(ledger_of(&qb).shell_noise.is_none());
    let qs = seal(qb, seal_opts(&quiet.rec, "true"));
    assert!(qs.version.shell_noise.is_none());
    let none = backend_default();
    build(prep(&none.rec, &Uploads::new(&[]), import_of()));
    assert!(!none.cmds().iter().any(|c| c.contains("-lic true")));
}

// golden.test.ts:1473
#[test]
fn a_builder_that_already_carries_the_files_does_not_run_the_shell_step_again() {
    let b = backend_default();
    let up = Uploads::new(&[]);
    let imp = GoldenImport {
        shell: Some(ShellInstall { shell: LoginShell::Zsh, frameworks: Vec::new(), cmd: "install-zsh".into() }),
        ..import_of()
    };
    let builder = build(prep(&b.rec, &up, imp.clone()));
    assert_eq!(b.cmds().iter().filter(|c| c.contains("install-zsh")).count(), 1);
    apply(
        &builder,
        ApplyImportOptions {
            import: Some(imp),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: up_fetch(&up),
            ..Default::default()
        },
    );
    assert_eq!(b.cmds().iter().filter(|c| c.contains("install-zsh")).count(), 1);
}

// golden.test.ts:1482
#[test]
fn a_failing_tool_is_a_warning_in_the_detail_and_the_next_one_still_runs() {
    // stdout ends on a progress line; the reason is the last stderr line, not that.
    let b = backend_for(
        vec![("brew install gh", Answer::Fixed(exec(1, "==> Installing gh dependency: oniguruma\n", "Error: gh: no bottle available!\n")))],
        Free::Kb(mb(3000)),
    );
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &up, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    assert!(st
        .all()
        .contains(&"installing-tools:2 installed, 1 failed: gh (Error: gh: no bottle available!); caches swept; 2.9 GB free".to_string()));
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools[1..2]),
        vec![ToolResult {
            id: "tools/brew/gh".into(),
            label: "gh".into(),
            outcome: "failed".into(),
            note: Some("Error: gh: no bottle available!".into()),
            ms: Some(0),
            ..Default::default()
        }]
    );
    let version = seal(builder, seal_opts(&b.rec, "should-not-run")).version;
    assert_eq!(version.smoke.cmd, "claude --version && codex --version && bun --version");
}

// golden.test.ts:1496
#[test]
fn a_custom_row_that_fails_is_listed_with_its_reason_and_the_golden_still_seals() {
    let custom = custom_installs_for(
        &[RecipeCustomRow {
            kind: CustomLit,
            id: "just".into(),
            name: "just".into(),
            install: vec!["brew install just".into()],
            check: "command -v just".into(),
            manager: None,
            size: None,
            why: "added by the agent".into(),
        }],
        &|_: &RecipeCustomRow| None::<String>,
        TOOLS_PATH,
        None,
    );
    // The needle is the line the row installs with: the prelude around it is quoted again by the guard.
    let b = backend_for(vec![("brew install just", Answer::Fixed(exec(1, "", "Error: no formula just\n")))], Free::Kb(mb(3000)));
    let up = Uploads::new(&[]);
    let (res, on_result) = collect_results();
    let mut tools = import_of().tools;
    tools.extend(custom);
    let builder = build(prep(&b.rec, &up, GoldenImport { tools, on_result: Some(on_result), ..import_of() }));
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools[res[0].tools.len() - 1..]),
        vec![ToolResult {
            id: "tools/custom/just".into(),
            label: "just".into(),
            outcome: "failed".into(),
            note: Some("Error: no formula just".into()),
            ms: Some(0),
            ..Default::default()
        }]
    );
    let version = seal(builder, seal_opts(&b.rec, "should-not-run")).version;
    assert_eq!(version.version.0, 1.0);
}

// golden.test.ts:1507
#[test]
fn a_road_install_names_the_road_it_took_in_the_result_and_the_stage_summary() {
    let b = backend_for(
        vec![
            (
                "releases/tags/v0.1.0",
                Answer::Fixed(exec(
                    0,
                    &format!("WSP_ROAD release diskbloom_0.1.0_linux_arm64.tar.gz {} v0.1.0 aarch64\n", "a".repeat(64)),
                    "",
                )),
            ),
            (
                "releases/tags/v1.13.1",
                Answer::Fixed(exec(0, "go: downloading\nWSP_ROAD go github.com/TheZoraiz/ascii-image-converter@v1.13.1\n", "")),
            ),
        ],
        Free::Kb(mb(3000)),
    );
    let roads = vec![
        ToolInstall {
            id: "tools/brew/zingzy/tap/diskbloom".into(),
            label: "diskbloom".into(),
            manager: "release".into(),
            cmd: "curl https://api.github.com/repos/Zingzy/diskbloom/releases/tags/v0.1.0".into(),
            pin: Some(PinRead { read: None, fixed: true, words: "from its release".into() }),
            ..Default::default()
        },
        ToolInstall {
            id: "tools/brew/thezoraiz/ascii-image-converter/ascii-image-converter".into(),
            label: "ascii-image-converter".into(),
            manager: "release".into(),
            cmd: "curl https://api.github.com/repos/TheZoraiz/ascii-image-converter/releases/tags/v1.13.1".into(),
            ..Default::default()
        },
    ];
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut tools = import_of().tools;
    tools.extend(roads.clone());
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert!(st.all().contains(
        &"installing-tools:5 installed (diskbloom from its release, ascii-image-converter with go install); caches swept; 2.9 GB free"
            .to_string()
    ));
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools[3..]),
        vec![
            // The sum is recorded with the arch whose file it was, so a copy on the other arch never holds its own file to it.
            ToolResult {
                id: roads[0].id.clone(),
                label: "diskbloom".into(),
                outcome: "installed".into(),
                road: Some(ToolRoad {
                    kind: "release".into(),
                    from: "diskbloom_0.1.0_linux_arm64.tar.gz".into(),
                    sha256: Some("a".repeat(64)),
                    tag: Some("v0.1.0".into()),
                    arch: Some("aarch64".into()),
                }),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
            ToolResult {
                id: roads[1].id.clone(),
                label: "ascii-image-converter".into(),
                outcome: "installed".into(),
                road: Some(ToolRoad {
                    kind: "go".into(),
                    from: "github.com/TheZoraiz/ascii-image-converter@v1.13.1".into(),
                    ..Default::default()
                }),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
        ]
    );
    // A brew install carries no road: it took the one its plan named.
    assert!(res[0].tools[1].road.is_none());
}

// golden.test.ts:1529
#[test]
fn a_catalog_tool_without_a_local_row_installs_by_its_catalog_road_on_the_guest() {
    let b = backend_for(
        vec![("cli/cli/releases/download/v2.101.0", Answer::Fixed(exec(0, &format!("{GH_WSP_ROAD_B} {} v2.101.0\n", "b".repeat(64)), "")))],
        Free::Kb(mb(3000)),
    );
    let plan = vec![
        RecipeEntry {
            paths: Vec::new(),
            default: "skip".into(),
            ..tool_entry("tools/catalog/gh", "GitHub CLI", "skip", None, Some("yes"))
        },
        tool_entry("tools/npm/wrangler", "wrangler", "bring", Some("4.1.0"), None),
    ];
    let installs = plan_installs(&plan, &BrewTable::new());
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools: installs, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    let road = found(&cmds, "cli/cli/releases/download/v2.101.0");
    assert!(road.contains("install -m 0755 \"$bin\" \"/usr/local/bin/$name\""));
    // The sum the catalog pinned is checked on the guest before anything the download carries is unpacked.
    assert!(road.contains("sha256sum -c -"));
    assert!(!road.contains("api.github.com"));
    assert!(missing_check(&b).1.contains("for b in 'node' 'gh'; do"));
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools),
        vec![
            done("tools/manager/npm", "Node 22 with npm"),
            done("tools/npm/wrangler", "wrangler"),
            ToolResult {
                id: "tools/catalog/gh".into(),
                label: "GitHub CLI".into(),
                outcome: "installed".into(),
                note: Some("by an unmeasured road".into()),
                road: Some(ToolRoad {
                    kind: "release".into(),
                    from: "gh_2.101.0_linux_amd64.tar.gz".into(),
                    sha256: Some("b".repeat(64)),
                    tag: Some("v2.101.0".into()),
                    arch: None
                }),
                ms: Some(0),
                bytes: Some(0.0),
            },
        ]
    );
    assert!(st.all().contains(
        &"installing-tools:3 installed (GitHub CLI from its release (by an unmeasured road)); caches swept; 2.9 GB free".to_string()
    ));
}

// golden.test.ts:1554
#[test]
fn a_catalog_go_row_beside_a_go_row_installs_go_once_and_counts_each_install_once() {
    let b = backend_default();
    let plan = vec![
        tool_entry("tools/catalog/go", "Go", "skip", None, Some("yes")),
        RecipeEntry {
            paths: vec!["golang.org/x/tools/gopls@v0.16.2".into()],
            ..tool_entry("tools/go/gopls", "gopls", "bring", None, None)
        },
    ];
    let installs = plan_installs(&plan, &BrewTable::new());
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools: installs, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    assert_eq!(cmds.iter().filter(|c| c.contains("brew install go'")).count(), 1);
    assert!(
        index_of(&cmds, &found(&cmds, "brew install go'")) < index_of(&cmds, &found(&cmds, "go install golang.org/x/tools/gopls@v0.16.2"))
    );
    let res = res.borrow();
    let pairs: Vec<(String, String)> = res[0].tools.iter().map(|t| (t.id.clone(), t.outcome.clone())).collect();
    let want: Vec<(String, String)> = [
        ("tools/homebrew", "installed"),
        ("tools/brew-toolchain/glibc", "installed"),
        ("tools/brew-toolchain/gcc", "installed"),
        ("tools/catalog/go", "installed"),
        ("tools/go/gopls", "installed"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    assert_eq!(pairs, want);
    assert!(st.all().contains(&"installing-tools:5 installed; caches swept; 2.9 GB free".to_string()));
}

// golden.test.ts:1571
#[test]
fn a_catalog_row_with_a_version_pins_where_the_road_can_and_says_what_it_installed() {
    let b = backend_default();
    let plan = vec![
        tool_entry("tools/catalog/wrangler", "Cloudflare Wrangler", "skip", Some("4.1.0"), Some("yes")),
        tool_entry("tools/catalog/tmux", "tmux", "skip", Some("3.5a"), Some("yes")),
    ];
    let installs = plan_installs(&plan, &BrewTable::new());
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools: installs, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert!(b.cmds().iter().any(|c| c.contains("npm install -g wrangler@4.1.0")));
    let tmux_note = "by an unmeasured road; 3.5a asked, installed by apt at its current version";
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools),
        vec![
            done("tools/manager/npm", "Node 22 with npm"),
            ToolResult { note: Some("by an unmeasured road".into()), ..done("tools/catalog/wrangler", "Cloudflare Wrangler") },
            done("tools/apt-index", "apt index"),
            ToolResult { note: Some(tmux_note.into()), ..done("tools/catalog/tmux", "tmux") },
        ]
    );
    assert!(st.all().contains(&format!(
        "installing-tools:4 installed (Cloudflare Wrangler (by an unmeasured road), tmux ({tmux_note})); caches swept; 2.9 GB free"
    )));
}

// golden.test.ts:1591
#[test]
fn the_tally_names_every_install_once_with_its_notes_inside_their_brackets() {
    let b = backend_for(
        vec![("cli/cli/releases/download/v2.101.0", Answer::Fixed(exec(0, &format!("{GH_WSP_ROAD_B} {} v2.101.0\n", "c".repeat(64)), "")))],
        Free::Kb(mb(3000)),
    );
    let plan = vec![
        tool_entry("tools/catalog/gh", "GitHub CLI", "skip", None, Some("yes")),
        tool_entry("tools/catalog/tmux", "tmux", "skip", Some("3.5a"), Some("yes")),
    ];
    let installs = plan_installs(&plan, &BrewTable::new());
    let st = stage_recorder();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools: installs, ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let tmux_note = "by an unmeasured road; 3.5a asked, installed by apt at its current version";
    let all = st.all();
    let tally = all.iter().find(|s| s.starts_with("installing-tools:3 installed")).expect("the tally");
    assert_eq!(tally, &format!("installing-tools:3 installed (GitHub CLI from its release (by an unmeasured road), tmux ({tmux_note})); caches swept; 2.9 GB free"));
    // With the bracketed notes off, every comma left separates two named tools: the list names two, not four.
    assert_eq!(strip_brackets(tally), "installing-tools:3 installed (GitHub CLI from its release, tmux); caches swept; 2.9 GB free");
}

// golden.test.ts:1608
#[test]
fn after_the_loop_every_install_that_names_its_command_is_checked_on_the_tools_path_and_a_miss_fails() {
    let b = backend_for(
        vec![
            (
                "releases/tags/v0.4.1",
                Answer::Fixed(exec(0, &format!("WSP_ROAD release spoo_0.4.1_linux_amd64.tar.gz {} v0.4.1\n", "a".repeat(64)), "")),
            ),
            ("echo \"missing", Answer::Fixed(exec(0, "missing spoo\n", ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut tools = import_of().tools;
    tools.extend(gopls_and_spoo());
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let (_, check, timeout) = missing_check(&b);
    let first = check.split_inclusive('\n').next().unwrap_or("");
    assert!(
        first.starts_with("export PATH=/root/.local/bin:")
            && first.ends_with('\n')
            && first["export PATH=/root/.local/bin:".len()..].contains("/usr/local/bin"),
        "the check's PATH line: {first:?}"
    );
    assert!(check.contains("for b in 'gopls' 'spoo'; do command -v \"$b\" >/dev/null 2>&1 || echo \"missing $b\"; done"));
    assert_eq!(timeout, Some(20_000));
    let res = res.borrow();
    assert_eq!(
        res[0].tools.iter().find(|t| t.id == "tools/cli/spoo").map(|t| ToolResult { ms: t.ms.map(|_| 0), ..t.clone() }),
        Some(ToolResult {
            id: "tools/cli/spoo".into(),
            label: "spoo".into(),
            outcome: "failed".into(),
            note: Some("spoo is not on PATH after the install".into()),
            ms: Some(0),
            bytes: Some(0.0),
            ..Default::default()
        })
    );
    assert_eq!(res[0].tools.iter().find(|t| t.id == "tools/go/gopls").map(|t| t.outcome.as_str()), Some("installed"));
    assert!(st.all().contains(
        &"installing-tools:4 installed, 1 failed: spoo (spoo is not on PATH after the install); caches swept; 2.9 GB free".to_string()
    ));
    // Nothing of the person's named its command: the only check is the base floor's.
    let plain = backend_default();
    build(prep(&plain.rec, &Uploads::new(&[]), import_of()));
    let checks: Vec<String> = inline_cmds(&plain).into_iter().filter(|c| c.contains("echo \"missing")).collect();
    assert_eq!(checks.len(), 1);
    assert!(checks[0].contains("for b in 'curl' 'uv' 'python3' 'git' 'jq' 'rg' 'cc' 'fd' 'sqlite3' 'wget' 'zip' 'xz' 'rsync'; do"));
}

// golden.test.ts:1636
#[test]
fn two_tap_roads_whose_go_module_is_named_otherwise_pass_the_check_end_to_end() {
    let b = backend_for(
        vec![
            ("releases/tags/v0.4.1", Answer::Fixed(exec(0, "go: downloading\nWSP_ROAD go github.com/spoo-me/spoo-cli@v0.4.1\n", ""))),
            ("releases/tags/v0.2.0", Answer::Fixed(exec(0, "WSP_ROAD go github.com/Zingzy/bloom-cli@v0.2.0\n", ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let mut table = BrewTable::new();
    table.insert(
        "zingzy/tap/bloom".into(),
        BrewFormula {
            name: "bloom".into(),
            full_name: "zingzy/tap/bloom".into(),
            deps: Vec::new(),
            macos_only: false,
            source: Some(ToolSource { repo: "Zingzy/bloom-cli".into(), tag: "v0.2.0".into() }),
            ..Default::default()
        },
    );
    table.insert(
        "spoo-me/tap/spoo".into(),
        BrewFormula {
            name: "spoo".into(),
            full_name: "spoo-me/tap/spoo".into(),
            deps: Vec::new(),
            macos_only: false,
            source: Some(ToolSource { repo: "spoo-me/spoo-cli".into(), tag: "v0.4.1".into() }),
            ..Default::default()
        },
    );
    let plan = vec![
        RecipeEntry { bring: Some(true), ..tool_entry("tools/brew/zingzy/tap/bloom", "bloom", "bring", None, Some("unknown")) },
        RecipeEntry { bring: Some(true), ..tool_entry("tools/brew/spoo-me/tap/spoo", "spoo", "bring", None, Some("unknown")) },
    ];
    let installs = plan_installs(&plan, &table);
    assert_eq!(
        installs.iter().map(|i| (i.id.clone(), i.bin.clone())).collect::<Vec<_>>(),
        vec![
            ("tools/brew/zingzy/tap/bloom".to_string(), Some("bloom".to_string())),
            ("tools/brew/spoo-me/tap/spoo".to_string(), Some("spoo".to_string()))
        ]
    );
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools: installs, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    assert!(found(&cmds, "releases/tags/v0.2.0").contains(r#"mv '\''/usr/local/bin/bloom-cli'\'' "/usr/local/bin/$name""#));
    assert!(found(&cmds, "releases/tags/v0.4.1").contains(r#"mv '\''/usr/local/bin/spoo-cli'\'' "/usr/local/bin/$name""#));
    assert!(missing_check(&b).1.contains("for b in 'bloom' 'spoo'; do"));
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools),
        vec![
            ToolResult {
                id: "tools/brew/zingzy/tap/bloom".into(),
                label: "bloom".into(),
                outcome: "installed".into(),
                road: Some(ToolRoad { kind: "go".into(), from: "github.com/Zingzy/bloom-cli@v0.2.0".into(), ..Default::default() }),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
            ToolResult {
                id: "tools/brew/spoo-me/tap/spoo".into(),
                label: "spoo".into(),
                outcome: "installed".into(),
                road: Some(ToolRoad { kind: "go".into(), from: "github.com/spoo-me/spoo-cli@v0.4.1".into(), ..Default::default() }),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
        ]
    );
    assert!(st
        .all()
        .contains(&"installing-tools:2 installed (bloom with go install, spoo with go install); caches swept; 2.9 GB free".to_string()));
}

// golden.test.ts:1663
#[test]
fn a_check_that_itself_fails_counts_every_named_tool_as_failed() {
    let b = backend_for(
        vec![
            (
                "releases/tags/v0.4.1",
                Answer::Fixed(exec(0, &format!("WSP_ROAD release spoo_0.4.1_linux_amd64.tar.gz {} v0.4.1\n", "a".repeat(64)), "")),
            ),
            ("command -v \"$b\"", Answer::Fixed(exec(124, "", ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut tools = import_of().tools;
    tools.extend(gopls_and_spoo());
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools[3..]),
        vec![
            ToolResult {
                id: "tools/go/gopls".into(),
                label: "gopls".into(),
                outcome: "failed".into(),
                note: Some("gopls could not be checked on PATH: timed out after 20s".into()),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
            ToolResult {
                id: "tools/cli/spoo".into(),
                label: "spoo".into(),
                outcome: "failed".into(),
                note: Some("spoo could not be checked on PATH: timed out after 20s".into()),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
        ]
    );
    let all = st.all();
    assert!(all.contains(&"installing-tools:the PATH check failed (timed out after 20s): gopls, spoo count as failed".to_string()));
    assert!(all.contains(
        &"installing-tools:3 installed, 2 failed: gopls (gopls could not be checked on PATH: timed out after 20s), spoo (spoo could not be checked on PATH: timed out after 20s); caches swept; 2.9 GB free".to_string()
    ));
}

// golden.test.ts:1684
#[test]
fn an_agent_that_fails_refuses_the_seal_kills_the_builder_and_never_starts_the_tools() {
    let b = backend_for(vec![("codex-install", Answer::Fixed(exec(124, "", "")))], Free::Kb(mb(3000)));
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let err = run(prepare_builder(o, &*Manual::new())).err().expect("the build refuses");
    assert!(err.message().contains("an agent did not install"), "{}", err.message());
    assert_eq!(
        st.all().last().map(String::as_str),
        Some("failed:an agent did not install, so nothing is sealed:\nCodex: timed out after 900s")
    );
    assert_eq!(b.rec.state.killed(), vec!["m1".to_string()]);
    let res = res.borrow();
    assert_eq!(
        norm_agents(&res[0].agents),
        vec![
            agent_done("agents/claude", "Claude Code"),
            AgentResult {
                id: "agents/codex".into(),
                name: "Codex".into(),
                outcome: "failed".into(),
                note: Some("timed out after 900s".into()),
                ms: Some(0)
            },
        ]
    );
    let cmds = b.cmds();
    assert!(!cmds.iter().any(|c| c.contains("brew-bootstrap") || c.contains("brew install gh") || c.contains("bun@1.4.0")));
}

// golden.test.ts:1698
#[test]
fn switched_on_plugins_are_built_into_the_seed_and_one_that_does_not_build_is_left_out() {
    struct PluginLinks;
    #[async_trait(?Send)]
    impl SeedLinks for PluginLinks {
        async fn links(&self, read: &dyn SeedRead) -> Vec<(String, String)> {
            match read.read("installed_plugins.json").await {
                Some(text) if text.contains("1.0.0") => vec![("cache/acme/v/aaaaaaaaaaaa".to_string(), "1.0.0".to_string())],
                _ => Vec::new(),
            }
        }
    }
    let index = r#"{"version":2,"plugins":{"v@acme":[{"installPath":"/seed/cache/acme/v/1.0.0","gitCommitSha":"aaaaaaaaaaaabbbb"}]}}"#;
    let b = backend_for(
        vec![
            ("seed-build-bad", Answer::Fixed(exec(1, "", "fatal: repository not found"))),
            ("cat '/seed/installed_plugins.json'", Answer::Fixed(exec(0, index, ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let st = stage_recorder();
    let plugins = PluginsPlan {
        dropped: vec!["/root/.claude-cfg/plugins/known_marketplaces.json".into()],
        seed: Some(PluginSeedPlan {
            dir: "/seed".into(),
            plugins: vec![
                SeedPlugin { id: "plugins/v@acme".into(), label: "v@acme".into(), cmd: "seed-build-good".into(), asked: None },
                SeedPlugin { id: "plugins/w@acme".into(), label: "w@acme".into(), cmd: "seed-build-bad".into(), asked: None },
            ],
            links: Arc::new(PluginLinks),
        }),
    };
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { plugins: Some(plugins), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    let at = |needle: &str| find_index(&cmds, needle);
    assert!(at("rm -f '/root/.claude-cfg/plugins/known_marketplaces.json'") > at("codex-install"));
    assert!(at("seed-build-good") > at("rm -f '/root/.claude-cfg/plugins/known_marketplaces.json'"));
    assert!(at("seed-build-bad") > at("seed-build-good"));
    assert!(cmds.contains(&"ln -sfn '1.0.0' '/seed/cache/acme/v/aaaaaaaaaaaa'".to_string()));
    assert!(st.all().contains(&"installing-harness:plugins built in: v@acme; left out: w@acme (fatal: repository not found)".to_string()));
}
