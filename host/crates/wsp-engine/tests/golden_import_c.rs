// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts:2171-2553 (inside "golden import stages"): the tools' floor readings and cleanups, the df failures, the upload's size check,
//! the tools' dependencies, the ledger's missing, silenced and left-behind lists, and the attach that finds a recipe already applied.
//! A stage frame's `step` is not carried by the engine's tools loop, so no assertion here reads one.
#![allow(clippy::type_complexity)]
mod golden_import_support;
mod golden_roads_common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use golden_roads_common::*;
use indexmap::IndexMap;
use sha2::{Digest, Sha256};
use wsp_catalog::mcp::MCP_SERVERS_JSON;
use wsp_engine::golden::{ApplyImportOptions, ImportStage, ResultFiles};
use wsp_engine::golden_seam::{
    install_tools, GoldenFiles, GoldenImport, GoldenVolatile, InstallToolsOptions, McpAgentPlan, McpPlan, McpScope, PackFiles, PackedFiles,
    SkippedPath, SkippedRow, ToolInstall, ToolStage,
};
use wsp_wire::views::golden_image::{GoldenLeftBehind, GoldenMissingToolOutcome, GoldenStage};

use golden_import_support::*;

// golden.test.ts:2171
#[test]
fn under_the_floor_with_no_homebrew_the_cache_sweep_alone_runs() {
    let boot = Rc::new(Cell::new(false));
    let sweeps = Rc::new(Cell::new(0u32));
    let failing = exec(1, "", "Error: bootstrap failed");
    let (b1, s1, s2) = (boot.clone(), sweeps.clone(), sweeps.clone());
    let b = backend_for(
        vec![
            (
                "brew-bootstrap",
                Answer::Each(Rc::new(move || {
                    b1.set(true);
                    failing.clone()
                })),
            ),
            (
                SWEEP,
                Answer::Each(Rc::new(move || {
                    s1.set(s1.get() + 1);
                    ok()
                })),
            ),
        ],
        Free::Each(Rc::new(move || {
            mb(if !boot.get() {
                3000
            } else if s2.get() == 2 {
                1800
            } else {
                2500
            })
        })),
    );
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    assert!(!cmds.iter().any(|c| c.contains("brew cleanup -s --prune=all")));
    assert_eq!(
        starting(&st.all(), "installing-tools"),
        strings(&[
            "installing-tools:Homebrew (1/3)",
            "installing-tools:1.8 GB free, under the 2 GB floor; cleaning up before skipping",
            "installing-tools:caches swept, 700 MB back; 2.4 GB free",
            "installing-tools:bun@1.4.0 (3/3)",
            "installing-tools:1 installed, 1 failed: Homebrew (Error: bootstrap failed), 1 skipped: gh (Homebrew did not install); caches swept; 2.4 GB free",
        ])
    );
    let r = res.borrow();
    let rows: Vec<(String, String)> = r[0].tools.iter().map(|t| (t.id.clone(), t.outcome.clone())).collect();
    assert_eq!(
        rows,
        vec![
            ("tools/homebrew".to_string(), "failed".to_string()),
            ("tools/brew/gh".to_string(), "skipped".to_string()),
            ("tools/npm/bun".to_string(), "installed".to_string()),
        ]
    );
    // The base's sweep, the agents' sweep, the rescue in the loop, the housekeeping after it.
    assert_eq!(sweeps.get(), 4);
}

// golden.test.ts:2195
#[test]
fn when_df_cannot_be_read_after_the_cleanup_the_tools_left_are_skipped() {
    let boot = Rc::new(Cell::new(false));
    let cleanups = Rc::new(Cell::new(0u32));
    let (b1, c1, b2, c2) = (boot.clone(), cleanups.clone(), boot.clone(), cleanups.clone());
    let broken = exec(1, "", "df: /root: Input/output error");
    let b = backend_for(
        vec![
            (
                "brew-bootstrap",
                Answer::Each(Rc::new(move || {
                    b1.set(true);
                    ok()
                })),
            ),
            (
                "brew cleanup -s --prune=all",
                Answer::Each(Rc::new(move || {
                    c1.set(c1.get() + 1);
                    ok()
                })),
            ),
            (
                FREE_KB_CMD,
                Answer::Each(Rc::new(move || {
                    if c2.get() == 0 {
                        exec(0, &format!("{}\n", mb(if b2.get() { 1800 } else { 3000 })), "")
                    } else {
                        broken.clone()
                    }
                })),
            ),
        ],
        Free::Kb(mb(3000)),
    );
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert!(!b.cmds().iter().any(|c| c.contains("brew install gh")));
    assert_eq!(
        starting(&st.all(), "installing-tools"),
        strings(&[
            "installing-tools:Homebrew (1/3)",
            "installing-tools:1.8 GB free, under the 2 GB floor; cleaning up before skipping",
            "installing-tools:caches swept; df failed: df: /root: Input/output error",
            "installing-tools:1 installed, 2 skipped: gh, bun@1.4.0 (1.8 GB free before cleanup and unknown after (df failed: df: /root: Input/output error), keeping 2 GB free); caches swept",
        ])
    );
    let unknown = "1.8 GB free before cleanup and unknown after (df failed: df: /root: Input/output error), keeping 2 GB free";
    assert_eq!(
        tuples(&res.borrow()[0].tools),
        vec![
            row("tools/homebrew", "installed", None),
            row("tools/brew/gh", "skipped", Some(unknown)),
            row("tools/npm/bun", "skipped", Some(unknown))
        ]
    );
}

// golden.test.ts:2221
#[test]
fn a_full_disk_is_a_reading_of_zero_not_an_unknown_one() {
    let bootstrapped = Rc::new(Cell::new(false));
    let (b1, b2) = (bootstrapped.clone(), bootstrapped.clone());
    let b = backend_for(
        vec![(
            "brew-bootstrap",
            Answer::Each(Rc::new(move || {
                b1.set(true);
                ok()
            })),
        )],
        Free::Each(Rc::new(move || mb(if b2.get() { 0 } else { 3000 }))),
    );
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert_eq!(
        starting(&st.all(), "installing-tools"),
        strings(&[
            "installing-tools:Homebrew (1/3)",
            "installing-tools:0 B free, under the 2 GB floor; cleaning up before skipping",
            "installing-tools:caches swept; 0 B free",
            "installing-tools:1 installed, 2 skipped: gh, bun@1.4.0 (0 B free after cleanup, keeping 2 GB free); caches swept; 0 B free",
        ])
    );
    assert!(!b.cmds().iter().any(|c| c.contains("brew install gh")));
    assert_eq!(
        tuples(&res.borrow()[0].tools),
        vec![
            row("tools/homebrew", "installed", None),
            row("tools/brew/gh", "skipped", Some("0 B free after cleanup, keeping 2 GB free")),
            row("tools/npm/bun", "skipped", Some("0 B free after cleanup, keeping 2 GB free")),
        ]
    );
}

// golden.test.ts:2241
#[test]
fn a_homebrew_cleanup_that_fails_at_the_floor_is_named_in_the_rescues_line() {
    let bootstrapped = Rc::new(Cell::new(false));
    let (b1, b2) = (bootstrapped.clone(), bootstrapped.clone());
    let b = backend_for(
        vec![
            (
                "brew-bootstrap",
                Answer::Each(Rc::new(move || {
                    b1.set(true);
                    ok()
                })),
            ),
            ("brew cleanup -s --prune=all", Answer::Fixed(exec(1, "", "Error: Permission denied @ apply2files"))),
        ],
        Free::Each(Rc::new(move || mb(if b2.get() { 1800 } else { 3000 }))),
    );
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert!(!b.cmds().iter().any(|c| c.contains("brew install gh")));
    assert_eq!(
        starting(&st.all(), "installing-tools"),
        strings(&[
            "installing-tools:Homebrew (1/3)",
            "installing-tools:1.8 GB free, under the 2 GB floor; cleaning up before skipping",
            "installing-tools:Homebrew cleanup failed (Error: Permission denied @ apply2files); caches swept; 1.8 GB free",
            "installing-tools:1 installed, 2 skipped: gh, bun@1.4.0 (1.8 GB free after cleanup, keeping 2 GB free); Homebrew cleanup failed (Error: Permission denied @ apply2files); caches swept; 1.8 GB free",
        ])
    );
    let outcomes: Vec<(String, String)> = res.borrow()[0].tools.iter().map(|t| (t.id.clone(), t.outcome.clone())).collect();
    assert_eq!(
        outcomes,
        vec![
            ("tools/homebrew".to_string(), "installed".to_string()),
            ("tools/brew/gh".to_string(), "skipped".to_string()),
            ("tools/npm/bun".to_string(), "skipped".to_string()),
        ]
    );
}

// golden.test.ts:2261
#[test]
fn refuses_to_upload_when_the_archive_would_not_fit_kills_the_builder_and_says_why() {
    let b = backend_for(vec![], Free::Kb(mb(100)));
    let st = stage_recorder();
    let mut o = prep(&b, import_of());
    o.on_stage = Some(st.on_stage.clone());
    let err = build_err(o);
    let msg = err.message();
    assert!(in_order(&msg, &["1 KB packed and 4 KB unpacked", "256 MB", "100 MB free"]), "{msg}");
    assert!(b.fetch.puts().is_empty());
    assert_eq!(b.rec.state.killed(), strings(&["m1"]));
    assert!(st.all().last().is_some_and(
        |l| l.starts_with("failed:your files need 1 KB packed and 4 KB unpacked, plus 256 MB of headroom, but the machine has 100 MB free")
    ));
    // The check reads the archive, not the recipe's estimate: a small tar of a large estimate still fits.
    let roomy = backend_default();
    let files = GoldenFiles { bytes: 10 * 1024 * 1024 * 1024, ..import_of().files.expect("the files") };
    build(prep(&roomy, GoldenImport { files: Some(files), ..import_of() }));
}

// golden.test.ts:2273
#[test]
fn a_df_that_fails_is_a_warning_that_names_itself_and_the_build_goes_on() {
    let b = backend_for(vec![(FREE_KB_CMD, Answer::Fixed(exec(1, "", "df: /root: No such file or directory")))], Free::Kb(mb(3000)));
    let st = stage_recorder();
    let mut o = prep(&b, import_of());
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    assert!(st.all().contains(
        &"uploading-files:free disk unknown (df failed: df: /root: No such file or directory); uploading 1 KB anyway".to_string()
    ));
    assert_eq!(
        starting(&st.all(), "installing-tools:free disk unknown"),
        strings(&[
            "installing-tools:free disk unknown (df failed: df: /root: No such file or directory); installing without the 2 GB floor"
        ])
    );
    assert_eq!(b.fetch.puts().len(), 2);
    let cmds = b.cmds();
    assert_eq!(cmds.iter().filter(|c| c.contains("brew install gh") || c.contains("brew-bootstrap") || c.contains("bun@1.4.0")).count(), 3);
    assert!(applied_of(&builder).contains(&ImportStage::InstallingHarness));
}

// golden.test.ts:2284
#[test]
fn a_df_that_exits_0_printing_something_it_cannot_use_names_what_it_printed() {
    let b = backend_for(vec![(FREE_KB_CMD, Answer::Fixed(exec(0, "abc\n", "")))], Free::Kb(mb(3000)));
    let st = stage_recorder();
    let mut o = prep(&b, import_of());
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let unknown: Vec<String> = st.all().into_iter().filter(|s| s.contains("free disk unknown")).collect();
    assert_eq!(
        unknown,
        strings(&[
            "deploying-daemon:free disk unknown (df answered abc); installing without the 2 GB floor",
            "uploading-files:free disk unknown (df answered abc); uploading 1 KB anyway",
            "installing-tools:free disk unknown (df answered abc); installing without the 2 GB floor",
        ])
    );
}

// golden.test.ts:2295
#[test]
fn a_df_that_exits_0_printing_nothing_is_a_failure_since_the_pipe_exits_as_awk_does() {
    let b = backend_for(vec![(FREE_KB_CMD, Answer::Fixed(exec(0, "", "df: /root: No such file or directory")))], Free::Kb(mb(3000)));
    let st = stage_recorder();
    let mut o = prep(&b, import_of());
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let unknown: Vec<String> = st.all().into_iter().filter(|s| s.contains("free disk unknown")).collect();
    assert_eq!(
        unknown,
        strings(&[
            "deploying-daemon:free disk unknown (df failed: df: /root: No such file or directory); installing without the 2 GB floor",
            "uploading-files:free disk unknown (df failed: df: /root: No such file or directory); uploading 1 KB anyway",
            "installing-tools:free disk unknown (df failed: df: /root: No such file or directory); installing without the 2 GB floor",
        ])
    );
}

// golden.test.ts:2306
#[test]
fn an_exec_that_rejects_under_the_free_read_throws_before_anything_installs() {
    let m = FreeReadFails { ran: RefCell::new(Vec::new()) };
    let tools = vec![ToolInstall {
        id: "tools/brew/gh".into(),
        label: "gh".into(),
        manager: "brew".into(),
        cmd: "brew install gh".into(),
        ..Default::default()
    }];
    let stage: ToolStage = Arc::new(|_: GoldenStage, _: Option<&str>| {});
    let err = run(install_tools(&m, &tools, stage, InstallToolsOptions::default())).expect_err("the free read throws");
    assert!(err.to_string().contains("exec lost the machine"), "{err}");
    assert!(m.ran.borrow().is_empty());
}

// golden.test.ts:2324
#[test]
fn a_tool_waits_on_the_install_it_needs_and_a_failed_manager_skips_its_rows() {
    let tools = vec![
        ToolInstall {
            id: "tools/homebrew".into(),
            label: "Homebrew".into(),
            manager: "brew".into(),
            cmd: "brew-bootstrap".into(),
            ..Default::default()
        },
        ToolInstall {
            id: "tools/manager/pipx".into(),
            label: "pipx".into(),
            manager: "brew".into(),
            cmd: "brew install pipx".into(),
            after: Some("tools/homebrew".into()),
            ..Default::default()
        },
        ToolInstall {
            id: "tools/pipx/black".into(),
            label: "black 24.1.0".into(),
            manager: "pipx".into(),
            cmd: "pipx install black==24.1.0".into(),
            after: Some("tools/manager/pipx".into()),
            ..Default::default()
        },
        ToolInstall {
            id: "tools/npm/bun".into(),
            label: "bun@1.4.0".into(),
            manager: "npm".into(),
            cmd: "npm install -g bun@1.4.0".into(),
            ..Default::default()
        },
    ];
    let b = backend_for(vec![("brew install pipx", Answer::Fixed(exec(1, "", "Error: pipx: no bottle")))], Free::Kb(mb(3000)));
    let (res, on_result) = collect_results();
    build(prep(&b, GoldenImport { tools, on_result: Some(on_result), ..import_of() }));
    assert!(!b.cmds().iter().any(|c| c.contains("pipx install black")));
    assert_eq!(
        tuples(&res.borrow()[0].tools),
        vec![
            row("tools/homebrew", "installed", None),
            row("tools/manager/pipx", "failed", Some("Error: pipx: no bottle")),
            row("tools/pipx/black", "skipped", Some("pipx did not install")),
            row("tools/npm/bun", "installed", None),
        ]
    );
}

// golden.test.ts:2343
#[test]
fn a_failure_packing_or_extracting_the_files_is_fatal_with_its_reason() {
    let pack_fails = GoldenFiles {
        count: 1,
        rungs: rungs_of(&[("shell", 1)]),
        lands: vec![],
        bytes: 10,
        skipped: vec![],
        pack: pack_failing("Keychain: user cancelled"),
        volatile: None,
    };
    let a = backend_default();
    let err = build_err(prep(&a, GoldenImport { files: Some(pack_fails), ..import_of() }));
    assert!(err.message().contains("Keychain: user cancelled"), "{}", err.message());
    assert_eq!(a.rec.state.killed(), strings(&["m1"]));

    let b = backend_for(vec![("tar xzf", Answer::Fixed(exec(2, "", "gzip: stdin: not in gzip format")))], Free::Kb(mb(3000)));
    let st = stage_recorder();
    let mut o = prep(&b, import_of());
    o.on_stage = Some(st.on_stage.clone());
    let err = build_err(o);
    assert!(err.message().contains("not in gzip format"), "{}", err.message());
    assert!(st.all().last().is_some_and(|l| l.starts_with("failed:vault import untar failed")));
    assert_eq!(b.rec.state.killed(), strings(&["m1"]));
}

fn rungs_of(pairs: &[(&str, u64)]) -> IndexMap<String, u64> {
    let mut m = IndexMap::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), *v);
    }
    m
}

// golden.test.ts:2356
#[test]
fn with_nothing_ticked_the_file_stages_still_report_and_no_agent_means_smoke_true() {
    let b = backend_default();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { recipe_hash: "h0".into(), tools: vec![], agents: vec![], ..Default::default() });
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    assert_stages(
        &sans_base(&st.all()),
        &[
            Want::Is("creating:sandbox from base"),
            Want::Is("deploying-daemon"),
            Want::Is("deploying-daemon:2.9 GB free"),
            Want::Is("applying-setup:nothing ticked"),
            Want::Is("uploading-files:nothing to upload"),
            Want::Is("installing-harness"),
            Want::Is("installing-harness:no agent ticked"),
            Want::Is("installing-tools:nothing ticked"),
            Want::Is("installing-mcp:none configured"),
            Want::Re(is_context_written),
            Want::Is("ready"),
        ],
    );
    assert_eq!(b.fetch.puts().len(), 1);
    // The harness ran, so the context is written and the machine is asked whether it still answers before the hand-off.
    let cmds = b.cmds();
    let setup_at = cmds.iter().position(|c| c.contains("\ntrue' &")).expect("the setup line ran");
    let shape: Vec<String> = cmds[setup_at..]
        .iter()
        .map(|c| {
            if c.contains("echo WSP_CTX") {
                "probe".to_string()
            } else if c.contains("tar xzf - -C '/' ") {
                "write".to_string()
            } else if c.contains("\ntrue' &") {
                "setup".to_string()
            } else {
                c.clone()
            }
        })
        .collect();
    assert_eq!(shape, ["setup", "probe", "write", "echo ok"]);
    assert_eq!(builder.import.as_ref().map(|l| l.smoke.as_str()), Some("true"));

    let (res, on_result) = collect_results();
    let gone_files = GoldenFiles {
        count: 0,
        rungs: IndexMap::new(),
        lands: vec![],
        bytes: 0,
        skipped: vec![SkippedPath { id: "shell/zshrc".into(), path: "~/.zshrc".into(), note: "no longer on this computer".into() }],
        pack: pack_failing("must not pack"),
        volatile: None,
    };
    let mut go = prep(&b, GoldenImport { files: Some(gone_files), on_result: Some(on_result), ..import_of() });
    go.on_stage = Some(st.on_stage.clone());
    let gone = build(go);
    assert!(st.all().contains(&"applying-setup:nothing left to pack; skipped ~/.zshrc (no longer on this computer)".to_string()));
    assert!(applied_of(&gone).contains(&ImportStage::UploadingFiles));
    // No pack ran, so nothing was cut: the result says nothing about it rather than claiming an empty cut.
    let r = res.borrow();
    assert_eq!(
        r[0].files,
        Some(ResultFiles {
            bytes: 0,
            skipped: vec![SkippedPath { id: "shell/zshrc".into(), path: "~/.zshrc".into(), note: "no longer on this computer".into() }],
            cut: None,
            blanked: None,
        })
    );
}

// golden.test.ts:2387
#[test]
fn applying_the_same_recipe_again_skips_every_stage_and_runs_nothing() {
    let b = backend_default();
    let builder = build(prep(&b, import_of()));
    let before = b.cmds().len();
    let st = stage_recorder();
    let again = apply(
        &builder,
        ApplyImportOptions {
            import: Some(import_of()),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: Some(b.dyn_fetch()),
            on_stage: Some(st.on_stage.clone()),
            ..Default::default()
        },
    );
    assert_eq!(b.cmds().len(), before);
    assert_eq!(
        st.all(),
        [
            "applying-setup:already applied",
            "uploading-files:already applied",
            "installing-harness:already applied",
            "installing-tools:already applied",
            "installing-mcp:already applied",
        ]
    );
    assert_eq!(again.ledger, ledger_of(&builder));
    // Nothing ran, so there is no result to report; the saved list from the first run stands.
    let (res, on_result) = collect_results();
    apply(
        &builder,
        ApplyImportOptions {
            import: Some(GoldenImport { on_result: Some(on_result), ..import_of() }),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: Some(b.dyn_fetch()),
            ..Default::default()
        },
    );
    assert!(res.borrow().is_empty());

    let other = apply(
        &builder,
        ApplyImportOptions {
            import: Some(GoldenImport { recipe_hash: "h2".into(), ..import_of() }),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: Some(b.dyn_fetch()),
            on_stage: Some(st.on_stage.clone()),
            ..Default::default()
        },
    );
    assert!(b.cmds().len() > before);
    assert_eq!(other.ledger.recipe_hash, "h2");
}

// golden.test.ts:2412
#[test]
fn an_attach_with_an_mcp_plan_runs_the_config_edit_and_reports_no_result() {
    let b = backend_default();
    let builder = build(prep(&b, import_of()));
    let before = b.cmds().len();
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mcp = McpPlan {
        agents: vec![McpAgentPlan {
            id: "claude".into(),
            label: "Claude Code".into(),
            scopes: vec![McpScope {
                files: strings(&["/root/.claude-cfg/.claude.json"]),
                format: MCP_SERVERS_JSON,
                project: None,
                keep: strings(&["github"]),
                drop: vec![],
                travelled: None,
                own: None,
                folder: None,
                trust: None,
                edit: None,
            }],
            aside: vec![],
        }],
        guest_home: "/root".into(),
        rewrites: vec![],
        bin_dirs: vec![],
        tools: vec![],
    };
    apply(
        &builder,
        ApplyImportOptions {
            import: Some(GoldenImport { mcp: Some(mcp), on_result: Some(on_result), ..import_of() }),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: Some(b.dyn_fetch()),
            on_stage: Some(st.on_stage.clone()),
            ..Default::default()
        },
    );
    assert!(res.borrow().is_empty());
    let all = st.all();
    assert_eq!(
        all[..5],
        strings(&[
            "applying-setup:already applied",
            "uploading-files:already applied",
            "installing-harness:already applied",
            "installing-tools:already applied",
            "installing-mcp:Claude Code 1",
        ])[..]
    );
    assert_eq!(all[all.len() - 2], "installing-mcp:github skipped (the config edit did not run (exit 0)); 2.9 GB free");
    assert!(mcp_context_skipped(&all[all.len() - 1]), "{}", all[all.len() - 1]);
    let cmds = b.cmds();
    assert!(cmds.len() > before);
    assert_eq!(cmds.last().map(String::as_str), Some("echo ok"));
}

// golden.test.ts:2428
#[test]
fn applying_the_same_recipe_uploads_its_volatile_files_again_and_nothing_else() {
    let b = backend_default();
    let builder = build(prep(&b, import_of()));
    let before_cmds = b.cmds().len();
    let before_puts = b.fetch.puts().len();
    let st = stage_recorder();
    let volatile = GoldenVolatile {
        paths: strings(&["~/.claude.json", "~/.claude/plugins/installed_plugins.json"]),
        pack: pack_with(|| Ok(PackedFiles { tar: b"volatile-tgz".to_vec(), bytes: 300, unpacked: 2048, ..Default::default() })),
    };
    let (res, on_result) = collect_results();
    let files = GoldenFiles { volatile: Some(volatile), ..import_of().files.expect("the files") };
    let again = apply(
        &builder,
        ApplyImportOptions {
            import: Some(GoldenImport { files: Some(files), on_result: Some(on_result), ..import_of() }),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: Some(b.dyn_fetch()),
            on_stage: Some(st.on_stage.clone()),
            ..Default::default()
        },
    );
    let puts = b.fetch.puts();
    assert_eq!(
        puts[before_puts..].iter().map(|p| String::from_utf8_lossy(p).to_string()).collect::<Vec<_>>(),
        vec!["volatile-tgz".to_string()]
    );
    let cmds = b.cmds();
    assert_eq!(cmds[before_cmds..].iter().filter(|c| c.contains("tar xzf")).count(), 1);
    assert_stages(
        &st.all(),
        &[
            Want::Is("applying-setup:already applied"),
            Want::Is("uploading-files:2 volatile files, 300 B"),
            Want::Re(|l| {
                timed_tenth(
                    l,
                    "uploading-files:~/.claude.json, ~/.claude/plugins/installed_plugins.json re-imported, 300 B in ",
                    "; 2.9 GB free",
                )
            }),
            Want::Is("installing-harness:already applied"),
            Want::Is("installing-tools:already applied"),
            Want::Is("installing-mcp:already applied"),
        ],
    );
    assert_eq!(again.ledger, ledger_of(&builder));
    // The saved result from the first run stands: a re-import of state files changes no cut and no install.
    assert!(res.borrow().is_empty());
}

// golden.test.ts:2454
#[test]
fn a_volatile_reimport_that_fails_on_attach_is_reported_and_never_fails_it() {
    let free = Rc::new(RefCell::new(mb(3000)));
    let f = free.clone();
    let b = backend_for(vec![], Free::Each(Rc::new(move || f.borrow().clone())));
    let builder = build(prep(&b, import_of()));
    let before_puts = b.fetch.puts().len();
    // A default recipe leaves about 250 MB free after the install stages, under the 256 MiB upload headroom.
    *free.borrow_mut() = mb(200);
    let volatile = GoldenVolatile { paths: strings(&["~/.claude.json"]), pack: pack_tgz_volatile() };
    let st = stage_recorder();
    let files = GoldenFiles { volatile: Some(volatile), ..import_of().files.expect("the files") };
    let again = apply(
        &builder,
        ApplyImportOptions {
            import: Some(GoldenImport { files: Some(files), ..import_of() }),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: Some(b.dyn_fetch()),
            on_stage: Some(st.on_stage.clone()),
            ..Default::default()
        },
    );
    assert_eq!(b.fetch.puts().len(), before_puts);
    assert_stages(
        &st.all(),
        &[
            Want::Is("applying-setup:already applied"),
            Want::Is("uploading-files:1 volatile file, 300 B"),
            Want::Re(|l| {
                l.strip_prefix("uploading-files:~/.claude.json not re-imported: ").is_some_and(|rest| {
                    rest == "your files need 300 B packed and 2 KB unpacked, plus 256 MB of headroom, but the machine has 200 MB free"
                })
            }),
            Want::Is("installing-harness:already applied"),
            Want::Is("installing-tools:already applied"),
            Want::Is("installing-mcp:already applied"),
        ],
    );
    assert_eq!(again.ledger, ledger_of(&builder));
    // A pack that throws takes the same road.
    let broken = GoldenVolatile { paths: strings(&["~/.claude.json"]), pack: pack_failing("EACCES: permission denied") };
    let rec = stage_recorder();
    let files = GoldenFiles { volatile: Some(broken), ..import_of().files.expect("the files") };
    apply(
        &builder,
        ApplyImportOptions {
            import: Some(GoldenImport { files: Some(files), ..import_of() }),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: Some(b.dyn_fetch()),
            on_stage: Some(rec.on_stage.clone()),
            ..Default::default()
        },
    );
    assert!(rec.all().contains(&"uploading-files:~/.claude.json not re-imported: EACCES: permission denied".to_string()));
}

/// A volatile pack that answers a 300 B archive, as the attach test's pack does.
fn pack_tgz_volatile() -> PackFiles {
    pack_with(|| Ok(PackedFiles { tar: b"volatile-tgz".to_vec(), bytes: 300, unpacked: 2048, ..Default::default() }))
}

// golden.test.ts:2481
#[test]
fn without_an_import_the_harness_path_is_unchanged() {
    let rec = recording();
    let st = stage_recorder();
    let mut o = prepare_opts(&rec, "echo setup");
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    assert_eq!(sans_base(&st.all()), ["creating:sandbox from base", "deploying-daemon", "installing-harness", "ready"]);
    assert!(builder.import.is_none());
    assert_eq!(builder.setup_sha, hex::encode(Sha256::digest(b"echo setup")));
}

// golden.test.ts:2490
#[test]
fn the_ledger_names_every_tool_not_on_the_image_and_the_seal_stamps_them() {
    let b = backend_for(vec![("brew-bootstrap", Answer::Fixed(exec(1, "", "git: not found")))], Free::Kb(mb(3000)));
    let skipped =
        vec![SkippedRow { id: "tools/brew-cask/raycast".into(), label: "Raycast".into(), note: "macOS app, no Linux build".into() }];
    let builder = build(prep(&b, GoldenImport { skipped_tools: skipped, ..import_of() }));
    let want = vec![
        missing("tools/brew-cask/raycast", "Raycast", GoldenMissingToolOutcome::Skipped, "macOS app, no Linux build"),
        missing("tools/homebrew", "Homebrew", GoldenMissingToolOutcome::Failed, "git: not found"),
        missing("tools/brew/gh", "gh", GoldenMissingToolOutcome::Skipped, "Homebrew did not install"),
    ];
    assert_eq!(ledger_of(&builder).missing_tools, Some(want.clone()));
    let sealed = seal(builder, seal_opts(&b.rec, "true"));
    assert_eq!(sealed.version.missing_tools, Some(want));
}

// golden.test.ts:2503
#[test]
fn a_base_floor_row_that_failed_reaches_the_ledger_and_the_sealed_version() {
    let b = backend_for(
        vec![("apt-get install -y -qq fd-find", Answer::Fixed(exec(100, "", "E: Unable to locate package fd-find")))],
        Free::Kb(mb(3000)),
    );
    let builder = build(prep(&b, import_of()));
    let fd = missing("base/fd", "fd", GoldenMissingToolOutcome::Failed, "E: Unable to locate package fd-find");
    assert_eq!(ledger_of(&builder).missing_tools, Some(vec![fd.clone()]));
    let sealed = seal(builder, seal_opts(&b.rec, "true"));
    assert_eq!(sealed.version.missing_tools, Some(vec![fd]));
}

// golden.test.ts:2511
#[test]
fn the_rc_calls_the_pack_silenced_land_on_the_ledger_and_the_seal_stamps_them() {
    let b = backend_default();
    let st = stage_recorder();
    let silenced = strings(&["starship", "eza", "diskbloom"]);
    let files = GoldenFiles { pack: pack_tgz(silenced.clone()), ..import_of().files.expect("the files") };
    let mut o = prep(&b, GoldenImport { files: Some(files), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    assert_eq!(ledger_of(&builder).silenced, Some(silenced.clone()));
    assert!(st.all().contains(
        &"applying-setup:1 KB packed; skipped ~/.bashrc (no longer on this computer); silenced in the shell: starship, eza, diskbloom"
            .to_string()
    ));
    let sealed = seal(builder.clone(), seal_opts(&b.rec, "true"));
    assert_eq!(sealed.version.silenced, Some(silenced.clone()));
    let again = apply(
        &builder,
        ApplyImportOptions {
            import: Some(import_of()),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: Some(b.dyn_fetch()),
            ..Default::default()
        },
    );
    assert_eq!(again.ledger.silenced, Some(silenced));

    // A pack that silenced nothing leaves both without.
    let quiet = build(prep(&b, import_of()));
    assert_eq!(ledger_of(&quiet).silenced, None);
    let quiet_sealed = seal(quiet, seal_opts(&b.rec, "true"));
    assert_eq!(quiet_sealed.version.silenced, None);
}

// golden.test.ts:2526
#[test]
fn the_stage_line_names_every_mac_path_the_copied_files_carried() {
    let b = backend_default();
    let macs = strings(&["/opt/homebrew/bin/gh now gh", "/Applications/Docker.app/Contents/Resources/bin out of .zshrc"]);
    let macs2 = macs.clone();
    let pack = pack_with(move || {
        Ok(PackedFiles { tar: b"tgz-bytes".to_vec(), bytes: 1200, unpacked: 4096, mac_paths: macs2.clone(), ..Default::default() })
    });
    let st = stage_recorder();
    let files = GoldenFiles { pack, ..import_of().files.expect("the files") };
    let mut o = prep(&b, GoldenImport { files: Some(files), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert!(st.all().contains(&"applying-setup:1 KB packed; skipped ~/.bashrc (no longer on this computer); Mac paths: /opt/homebrew/bin/gh now gh, /Applications/Docker.app/Contents/Resources/bin out of .zshrc".to_string()));
    let quiet = stage_recorder();
    let mut qo = prep(&b, import_of());
    qo.on_stage = Some(quiet.on_stage.clone());
    build(qo);
    assert!(!quiet.all().iter().any(|l| l.contains("Mac paths")));
}

// golden.test.ts:2538
#[test]
fn a_builder_whose_tools_all_installed_records_no_missing_list() {
    let b = backend_default();
    let builder = build(prep(&b, import_of()));
    assert_eq!(ledger_of(&builder).missing_tools, None);
    let sealed = seal(builder, seal_opts(&b.rec, "true"));
    assert_eq!(sealed.version.missing_tools, None);
}

// golden.test.ts:2545
#[test]
fn an_attach_whose_tools_stage_is_already_applied_carries_the_earlier_missing_list() {
    let b = backend_default();
    let skipped =
        vec![SkippedRow { id: "tools/brew-cask/raycast".into(), label: "Raycast".into(), note: "macOS app, no Linux build".into() }];
    let builder = build(prep(&b, GoldenImport { skipped_tools: skipped, ..import_of() }));
    let again = apply(
        &builder,
        ApplyImportOptions {
            import: Some(import_of()),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: Some(b.dyn_fetch()),
            ..Default::default()
        },
    );
    assert_eq!(
        again.ledger.missing_tools,
        Some(vec![missing("tools/brew-cask/raycast", "Raycast", GoldenMissingToolOutcome::Skipped, "macOS app, no Linux build")])
    );
}

// golden.test.ts:2553
#[test]
fn the_ledger_carries_what_the_pack_left_off_and_an_attach_keeps_it() {
    let b = backend_default();
    let left = vec![GoldenLeftBehind {
        id: "agents/claude".into(),
        path: "~/.claude/settings.json".into(),
        note: "hook left behind: /opt/homebrew/bin/terminal-notifier".into(),
    }];
    let left_pack = left.clone();
    let pack = pack_with(move || {
        Ok(PackedFiles {
            tar: b"tgz-bytes".to_vec(),
            bytes: 1200,
            unpacked: 4096,
            skipped: vec![SkippedPath {
                id: "agents/claude".into(),
                path: "~/.claude/settings.json".into(),
                note: left_pack[0].note.clone(),
            }],
            left_behind: Some(left_pack.clone()),
            ..Default::default()
        })
    });
    let files = GoldenFiles { pack, ..import_of().files.expect("the files") };
    let builder = build(prep(&b, GoldenImport { files: Some(files), ..import_of() }));
    assert_eq!(ledger_of(&builder).left_behind, Some(left.clone()));
    let sealed = seal(builder.clone(), seal_opts(&b.rec, "true"));
    assert_eq!(sealed.version.left_behind, Some(left.clone()));
    let again = apply(
        &builder,
        ApplyImportOptions {
            import: Some(import_of()),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: Some(b.dyn_fetch()),
            ..Default::default()
        },
    );
    assert_eq!(again.ledger.left_behind, Some(left));

    let clean = build(prep(&b, import_of()));
    assert_eq!(ledger_of(&clean).left_behind, None);
    let clean_sealed = seal(clean, seal_opts(&b.rec, "true"));
    assert_eq!(clean_sealed.version.left_behind, None);
}
