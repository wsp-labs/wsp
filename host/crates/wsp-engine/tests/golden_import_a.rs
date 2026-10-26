// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts:1113-1427 (inside "golden import stages"): the machine context and its upload, the upload parts, the
//! stage order of a build and an attach, and the shell step.
//! A stage frame's `step` is not carried by the engine's tools loop, so no assertion here reads one.
#![allow(clippy::type_complexity)]
mod golden_import_a_support;
mod golden_roads_common;

use golden_import_a_support::*;
use std::rc::Rc;

use golden_roads_common::*;
use sha2::{Digest, Sha256};
use wsp_catalog::context::ContextOutcomeKind;
use wsp_catalog::road_modules::CURL_NET;
use wsp_engine::golden::{ApplyImportOptions, ImportLedger, ImportStage, ResultFiles};
use wsp_engine::golden_seam::{
    GoldenImport, HomebrewCheckout, LoginShell, McpPlan, NodeInstall, PackedFiles, ShellInstall, SkippedPath, SkippedRow, ToolResult,
};
use wsp_engine::machine_context::ContextResult;
use wsp_wire::views::golden_image::{GoldenMissingTool, GoldenMissingToolOutcome};

// golden.test.ts:1113
#[test]
fn writes_machine_context_after_the_stages_with_the_plans_skips_and_agent_hooks() {
    let probe = "WSP_CTX\nKERNEL 6.6.30\nDISK 20466256 11720704\nOVERLAY no\nAGENT claude\nAGENT codex\nAGENT pi\nCONFLICT pi\nSHELL zsh\nWSP_CTX_END\n";
    let b = backend_for(vec![("echo WSP_CTX", Answer::Fixed(exec(0, probe, "")))], Free::Kb(mb(3000)));
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &up, GoldenImport { skipped_tools: vec![skipped_tool_raycast()], on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let res = res.borrow();
    assert_eq!(
        res[0].tools[0],
        ToolResult {
            id: "tools/brew-cask/raycast".into(),
            label: "Raycast".into(),
            outcome: "skipped".into(),
            note: Some("macOS app, no Linux build".into()),
            ..Default::default()
        }
    );
    assert_eq!(
        res[0].context,
        Some(vec![
            ContextResult {
                agent: "claude".into(),
                outcome: ContextOutcomeKind::Written,
                path: Some("/etc/claude-code/CLAUDE.md".into()),
                note: None,
                skill: "/etc/claude-code/.claude/skills/wsp-machine/SKILL.md".into(),
            },
            ContextResult {
                agent: "codex".into(),
                outcome: ContextOutcomeKind::Written,
                path: Some("/etc/codex/requirements.toml".into()),
                note: None,
                skill: "/etc/codex/skills/wsp-machine/SKILL.md".into(),
            },
            ContextResult {
                agent: "pi".into(),
                outcome: ContextOutcomeKind::Fallback,
                path: Some("/root/.pi/agent/extensions/wsp-machine.ts".into()),
                note: Some("~/.pi/agent/APPEND_SYSTEM.md already exists".into()),
                skill: "/root/.pi/agent/skills/wsp-machine/SKILL.md".into(),
            },
        ])
    );
    assert!(res[0].context_failure.is_none());
    let all = st.all();
    assert!(all.len() >= 2 && pi_fallback_line(&all[all.len() - 2]), "stages: {all:#?}");
    // The texts go up as one archive after the person's files and are untarred at the root before the reach check.
    let cmds = b.cmds();
    let write = find_index(&cmds, "tar xzf - -C '/' ");
    assert!(write > find_index(&cmds, "echo WSP_CTX"));
    assert!(index_of(&cmds, "echo ok") > write);
    assert!(!cmds.iter().any(|c| c.contains("base64 --decode")));
    let puts = up.puts();
    assert_eq!(puts.len(), 2);
    let tgz = &puts[1];
    let skill = file_in(tgz, "etc/wsp/skills/wsp-machine/SKILL.md");
    assert!(skill.contains("- This machine is a golden builder, not a workspace yet."));
    assert!(skill.contains("- Golden: not sealed yet."));
    assert!(skill.contains("- Tools that did not install: Raycast (macOS app, no Linux build)."));
    assert!(names_in(tgz).contains(&"etc/wsp/machine-context.json".to_string()));
    assert!(!names_in(tgz).iter().any(|n| n.ends_with("APPEND_SYSTEM.md")));
}

// golden.test.ts:1143
#[test]
fn a_refused_context_upload_is_a_failure_the_result_counts_not_the_end_of_the_build() {
    let b = backend_default();
    // The person's files go up first and land; the context archive is the second PUT.
    let up = Uploads::new(&[1]);
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &up, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    let failure = "write failed: vault import upload failed: HTTP 413 Payload Too Large; the upload takes at most 1024 bytes per PUT";
    assert!(st.all().contains(&format!("installing-mcp:machine context: not written ({failure})")));
    let res = res.borrow();
    assert_eq!(res[0].context, Some(Vec::new()));
    assert_eq!(res[0].context_failure.as_deref(), Some(failure));
    // The stage is applied only once the context landed, so a later attach runs the write again.
    assert!(!applied_of(&builder).contains(&ImportStage::InstallingMcp));
    assert_eq!(b.cmds().last().map(String::as_str), Some("echo ok"));
}

// golden.test.ts:1163
#[test]
fn a_context_write_that_failed_on_the_build_runs_again_on_attach_and_marks_mcp_once_it_landed() {
    let b = backend_default();
    // The person's files are the first PUT; the build's context archive is the second and is refused, the attach's goes through.
    let up = Uploads::new(&[1]);
    let builder = build(prep(&b.rec, &up, import_of()));
    assert_eq!(
        applied_of(&builder),
        vec![ImportStage::ApplyingSetup, ImportStage::UploadingFiles, ImportStage::InstallingHarness, ImportStage::InstallingTools]
    );
    let before_cmds = b.cmds().len();
    let before_puts = up.count();
    let st = stage_recorder();
    let again = apply(
        &builder,
        ApplyImportOptions {
            import: Some(import_of()),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: up_fetch(&up),
            on_stage: Some(st.on_stage.clone()),
            ..Default::default()
        },
    );
    assert_stages(
        &st.all(),
        &[
            Want::Is("applying-setup:already applied"),
            Want::Is("uploading-files:already applied"),
            Want::Is("installing-harness:already applied"),
            Want::Is("installing-tools:already applied"),
            Want::Is("installing-mcp:none configured"),
            Want::Re(is_context_written),
        ],
    );
    assert_eq!(up.count(), before_puts + 1);
    assert!(names_in(&up.puts().last().unwrap().clone()).contains(&"etc/wsp/machine-context.json".to_string()));
    let cmds = b.cmds();
    let shape: Vec<&str> = cmds[before_cmds..]
        .iter()
        .map(|c| {
            if c.contains("echo WSP_CTX") {
                "probe"
            } else if c.contains("tar xzf - -C '/' ") {
                "write"
            } else {
                c.as_str()
            }
        })
        .collect();
    assert_eq!(shape, vec!["probe", "write"]);
    assert_eq!(
        again.ledger.applied,
        vec![
            ImportStage::ApplyingSetup,
            ImportStage::UploadingFiles,
            ImportStage::InstallingHarness,
            ImportStage::InstallingTools,
            ImportStage::InstallingMcp
        ]
    );
    // With the context on the machine, the next attach runs nothing.
    let third = stage_recorder();
    apply(
        &builder,
        ApplyImportOptions {
            import: Some(import_of()),
            setup: "true".into(),
            ledger: Some(again.ledger.clone()),
            fetch: up_fetch(&up),
            on_stage: Some(third.on_stage.clone()),
            ..Default::default()
        },
    );
    assert_eq!(third.all().last().map(String::as_str), Some("installing-mcp:already applied"));
    assert_eq!(up.count(), before_puts + 1);

    // A retry that fails again leaves the stage unmarked, so the attach after it tries once more.
    up.refuse_at(up.count() + 1);
    up.refuse_at(up.count() + 2);
    let other = build(prep(&b.rec, &up, import_of()));
    let rec = stage_recorder();
    let still = apply(
        &other,
        ApplyImportOptions {
            import: Some(import_of()),
            setup: "true".into(),
            ledger: other.import.clone(),
            fetch: up_fetch(&up),
            on_stage: Some(rec.on_stage.clone()),
            ..Default::default()
        },
    );
    assert!(rec.all().last().is_some_and(|l| not_written_413(l)), "stages: {:#?}", rec.all());
    assert!(!still.ledger.applied.contains(&ImportStage::InstallingMcp));
}

// golden.test.ts:1204
#[test]
fn a_retried_context_write_names_what_the_build_left_off_and_hands_its_outcome_over() {
    let b = backend_default();
    let up = Uploads::new(&[1]);
    let skipped_tools = vec![skipped_tool_raycast()];
    let skipped_agents = vec![SkippedRow { id: "agents/zed".into(), label: "Zed".into(), note: "no installer known".into() }];
    let builder = build(prep(
        &b.rec,
        &up,
        GoldenImport { skipped_tools: skipped_tools.clone(), skipped_agents: skipped_agents.clone(), ..import_of() },
    ));
    assert_eq!(
        ledger_of(&builder).missing_tools,
        Some(vec![GoldenMissingTool {
            id: "tools/brew-cask/raycast".into(),
            name: "Raycast".into(),
            outcome: GoldenMissingToolOutcome::Skipped,
            note: "macOS app, no Linux build".into(),
        }])
    );
    let (res, on_result) = collect_results();
    let (outs, on_context) = collect_outcomes();
    let plan = || GoldenImport {
        skipped_tools: skipped_tools.clone(),
        skipped_agents: skipped_agents.clone(),
        on_result: Some(on_result.clone()),
        on_context: Some(on_context.clone()),
        ..import_of()
    };
    let again = apply(
        &builder,
        ApplyImportOptions {
            import: Some(plan()),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: up_fetch(&up),
            ..Default::default()
        },
    );
    // The skipped stages ran on the build: the ledger's tools, the plan's agents and files are the facts the guest gets.
    let tgz = up.puts().last().unwrap().clone();
    let context: serde_json::Value = serde_json::from_str(&file_in(&tgz, "etc/wsp/machine-context.json")).expect("the context is JSON");
    assert_eq!(
        context,
        serde_json::json!({
            "tools": [{ "id": "tools/brew-cask/raycast", "label": "Raycast", "note": "macOS app, no Linux build" }],
            "agents": [{ "id": "agents/zed", "label": "Zed", "note": "no installer known" }],
            "files": [{ "path": "~/.bashrc", "note": "no longer on this computer" }],
        })
    );
    let skill = file_in(&tgz, "etc/wsp/skills/wsp-machine/SKILL.md");
    assert!(skill.contains("- Tools that did not install: Raycast (macOS app, no Linux build)."));
    assert!(skill.contains("- Agents that did not install: Zed (no installer known)."));
    // The attach reports no result, so the saved one from the build stands; the write's outcome alone goes out.
    assert!(res.borrow().is_empty());
    assert_eq!(*outs.borrow(), vec![ctx_none()]);
    assert!(again.result.tools.is_empty());
    assert!(again.ledger.applied.contains(&ImportStage::InstallingMcp));

    // A retry that fails again hands the new reason over the same way.
    up.refuse_at(up.count() + 1);
    up.refuse_at(up.count() + 2);
    let other = build(prep(
        &b.rec,
        &up,
        GoldenImport { skipped_tools: skipped_tools.clone(), skipped_agents: skipped_agents.clone(), ..import_of() },
    ));
    apply(
        &other,
        ApplyImportOptions {
            import: Some(plan()),
            setup: "true".into(),
            ledger: other.import.clone(),
            fetch: up_fetch(&up),
            ..Default::default()
        },
    );
    assert!(res.borrow().is_empty());
    let outs = outs.borrow();
    assert_eq!(outs.len(), 2);
    assert!(outs[1].0.is_empty() && outs[1].1.as_deref().is_some_and(write_failed_413), "{:?}", outs[1]);
}

// golden.test.ts:1245
#[test]
fn a_quiet_attach_whose_mcp_rewrite_is_refused_hands_nothing_over() {
    let b = backend_default();
    let up = Uploads::new(&[]);
    let mcp = McpPlan { agents: Vec::new(), guest_home: "/root".into(), rewrites: Vec::new(), bin_dirs: Vec::new(), tools: Vec::new() };
    let builder = build(prep(&b.rec, &up, GoldenImport { mcp: Some(mcp.clone()), ..import_of() }));
    assert!(applied_of(&builder).contains(&ImportStage::InstallingMcp));
    let (outs, on_context) = collect_outcomes();
    // The MCP plan rewrites the context on every attach; this one is refused.
    up.refuse_at(up.count());
    let st = stage_recorder();
    let again = apply(
        &builder,
        ApplyImportOptions {
            import: Some(GoldenImport { mcp: Some(mcp.clone()), on_context: Some(on_context.clone()), ..import_of() }),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: up_fetch(&up),
            on_stage: Some(st.on_stage.clone()),
            ..Default::default()
        },
    );
    assert!(st.all().last().is_some_and(|l| not_written_413(l)), "stages: {:#?}", st.all());
    assert!(again.ledger.applied.contains(&ImportStage::InstallingMcp));
    assert!(outs.borrow().is_empty());

    // A rewrite that lands is handed over as before: the same outcome the build saved.
    apply(
        &builder,
        ApplyImportOptions {
            import: Some(GoldenImport { mcp: Some(mcp.clone()), on_context: Some(on_context.clone()), ..import_of() }),
            setup: "true".into(),
            ledger: Some(again.ledger.clone()),
            fetch: up_fetch(&up),
            ..Default::default()
        },
    );
    assert_eq!(*outs.borrow(), vec![ctx_none()]);
}

// golden.test.ts:1270
#[test]
fn an_archive_over_one_upload_part_says_how_many_parts_it_went_up_in() {
    let b = backend_default();
    let up = Uploads::new(&[]);
    let mut files = import_of().files.expect("the files");
    files.pack = pack_of(PackedFiles { tar: vec![0u8; 33 * 1024 * 1024], bytes: 33 * 1024 * 1024, unpacked: 4096, ..Default::default() });
    let st = stage_recorder();
    let mut o = prep(&b.rec, &up, GoldenImport { files: Some(files), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert_eq!(up.count(), 3);
    let all = st.all();
    assert!(all.contains(&"uploading-files:part 1 of 2, 32 MB of 33 MB".to_string()));
    assert!(all.contains(&"uploading-files:part 2 of 2, 33 MB of 33 MB".to_string()));
    assert!(all.iter().any(|l| timed(l, "uploading-files:33 MB in 2 parts in ", "; 2.9 GB free")), "stages: {all:#?}");
}

// golden.test.ts:1281
#[test]
fn runs_setup_upload_agents_then_tools_in_order_with_a_detail_and_checks_the_machine_answers() {
    let b = backend_default();
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &up, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.deploy_daemon = Some(daemon("node v22"));
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    assert_stages(
        &sans_base(&st.all()),
        &[
            Want::Is("creating:sandbox from base"),
            Want::Is("deploying-daemon"),
            Want::Is("deploying-daemon:node v22; 2.9 GB free"),
            Want::Is("applying-setup:3 files: identity 1, shell 2"),
            Want::Is("applying-setup:1 KB packed; skipped ~/.bashrc (no longer on this computer)"),
            Want::Is("uploading-files:1 KB"),
            Want::Re(|l| timed(l, "uploading-files:1 KB in ", "; 2.9 GB free")),
            Want::Is("installing-harness"),
            Want::Is("installing-harness:Claude Code (1/2)"),
            Want::Is("installing-harness:Codex (2/2)"),
            Want::Is("installing-harness:Claude Code, Codex installed; caches swept; 2.9 GB free"),
            Want::Is("installing-tools:Homebrew (1/3)"),
            Want::Is("installing-tools:gh (2/3)"),
            Want::Is("installing-tools:bun@1.4.0 (3/3)"),
            Want::Is("installing-tools:3 installed; caches swept; 2.9 GB free"),
            Want::Is("installing-mcp:none configured"),
            Want::Re(is_context_written),
            Want::Is("ready"),
        ],
    );
    let puts = up.puts();
    assert_eq!(puts[0], b"tgz-bytes".to_vec());
    assert_eq!(puts.len(), 2);
    let cmds = b.cmds();
    let untar = found(&cmds, "tar xzf");
    assert!(untar.contains("-C '/root' --no-same-owner"));
    let last_untar = cmds.iter().rfind(|c| c.contains("tar xzf")).unwrap();
    assert!(last_untar.contains("-C '/' --no-same-owner"));
    assert!(index_of(&cmds, FREE_KB_CMD) < index_of(&cmds, &untar));
    let tool = found(&cmds, "brew install gh");
    assert!(tool.contains("\nsetsid bash -c 'brew install gh' &\np=$!\n"));
    assert!(tool.contains("while [ $t -lt 600 ]"));
    assert_eq!(cmds.iter().filter(|c| c.contains("brew install gh") || c.contains("brew-bootstrap") || c.contains("bun@1.4.0")).count(), 3);
    let agent = found(&cmds, "codex-install");
    assert!(agent.contains("\nsetsid bash -c 'set -euo pipefail\nexport npm_config_fetch_timeout="));
    assert!(agent.contains(&format!("{}\nexport PATH=\"/usr/local/bin:$PATH\"\ncodex-install", CURL_NET.as_str())));
    assert!(agent.contains("while [ $t -lt 900 ]"));
    // Agents and their checks run with the Node the golden installed ahead of any the image shipped.
    assert!(cmds.contains(&"export PATH=\"/usr/local/bin:$PATH\"\nclaude --version".to_string()));
    assert!(cmds.contains(&"export PATH=\"/usr/local/bin:$PATH\"\ncodex --version".to_string()));
    assert!(index_of(&cmds, "true") < index_of(&cmds, &agent));
    // Agents are the point and tools the long tail: the agents go on first, so a tools stage that fills the disk cannot starve them.
    assert!(index_of(&cmds, &agent) < index_of(&cmds, &tool));
    // The reach check is the last thing on the machine before the hand-off.
    assert_eq!(cmds.last().map(String::as_str), Some("echo ok"));
    assert_eq!(
        builder.import,
        Some(ImportLedger {
            recipe_hash: "h1".into(),
            applied: vec![
                ImportStage::ApplyingSetup,
                ImportStage::UploadingFiles,
                ImportStage::InstallingHarness,
                ImportStage::InstallingTools,
                ImportStage::InstallingMcp
            ],
            smoke: "claude --version && codex --version && bun --version".into(),
            recipe: None,
            missing_tools: None,
            silenced: None,
            shell_noise: None,
            left_behind: None,
        })
    );
    assert_eq!(builder.setup_sha, hex::encode(Sha256::digest(b"true\nclaude-install\ncodex-install")));
    let res = res.borrow();
    assert_eq!(res.len(), 1);
    let r = &res[0];
    assert_eq!(r.recipe_hash, "h1");
    assert!(r.base.is_some());
    assert_eq!(
        r.files,
        Some(ResultFiles {
            bytes: 1200,
            skipped: vec![SkippedPath { id: "shell/bashrc".into(), path: "~/.bashrc".into(), note: "no longer on this computer".into() }],
            cut: Some(Vec::new()),
            blanked: None,
        })
    );
    assert_eq!(r.homebrew, Some(HomebrewCheckout { tag: "6.0.21".into(), commit: "560147012b9678b42ef5e83b690f0895552d1366".into() }));
    assert_eq!(
        norm_tools(&r.tools),
        vec![done("tools/homebrew", "Homebrew"), done("tools/brew/gh", "gh"), done("tools/npm/bun", "bun@1.4.0")]
    );
    assert_eq!(norm_agents(&r.agents), vec![agent_done("agents/claude", "Claude Code"), agent_done("agents/codex", "Codex")]);
    assert_eq!(r.context, Some(Vec::new()));
    assert!(r.mcp.is_none() && r.retired.is_none() && r.context_failure.is_none());
}

// golden.test.ts:1347
#[test]
fn every_command_that_outlives_one_exec_goes_through_run_and_inline_execs_are_bounded() {
    let locked_once = Rc::new(std::cell::Cell::new(false));
    let l = locked_once.clone();
    let b = backend_for(
        vec![
            (
                "brew install gh",
                Answer::Each(Rc::new(move || {
                    if l.get() {
                        return ok();
                    }
                    l.set(true);
                    exec(1, "", "Error: A `brew install glibc` process has already locked /home/linuxbrew/.linuxbrew/Cellar/x.\n")
                })),
            ),
            ("NODE_HAVE", Answer::Fixed(exec(0, "NODE_HAVE v18\nNODE_INSTALLED v22.0.0\n", ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let up = Uploads::new(&[]);
    let shell = ShellInstall { shell: LoginShell::Zsh, frameworks: Vec::new(), cmd: "install-zsh".into() };
    let node = NodeInstall {
        floor: 22,
        version: "v22.0.0".into(),
        agents: vec!["Codex".into()],
        cmd: "echo NODE_HAVE v$(node -v); node-install".into(),
    };
    let mut o = prep(&b.rec, &up, GoldenImport { shell: Some(shell), node: Some(node), ..import_of() });
    o.setup = "the-setup".into();
    o.deploy_daemon = Some(daemon("node v22"));
    let builder = build(o);
    seal(builder, seal_opts(&b.rec, "unused"));
    let scripts = b.rec.state.ran_on("m1");
    let one_of = |needle: &str| -> String {
        let hits: Vec<&String> = scripts.iter().filter(|s| s.contains(needle)).collect();
        assert_eq!(hits.len(), 1, "{needle}");
        hits[0].clone()
    };
    for body in [
        "the-setup",
        "install-zsh",
        "node-install",
        "claude-install",
        "codex-install",
        "brew-bootstrap",
        "bun@1.4.0",
        "autoremove",
        "cleanup -s --prune=all",
    ] {
        assert!(one_of(body).contains("\nsetsid bash -c '"), "{body}");
    }
    assert_eq!(scripts.iter().filter(|s| s.contains("brew install gh")).count(), 2);
    assert!(one_of("flock -w 600").contains("/home/linuxbrew/.linuxbrew/var/homebrew/locks/*.lock"));
    assert_eq!(scripts.iter().filter(|s| s.contains("tar xzf")).count(), 2);
    assert!(scripts.contains(&"export PATH=\"/usr/local/bin:$PATH\"\nclaude --version".to_string()));
    assert_eq!(b.rec.state.ran_on("m2"), vec!["claude --version && codex --version && bun --version".to_string()]);
    let inline_cmds: Vec<String> = inline_of(&b).into_iter().map(|(_, c, _)| c).collect();
    assert!(inline_cmds.contains(&FREE_KB_CMD.to_string()));
    assert!(inline_cmds.contains(&"rm -f /tmp/wsp-vault-*.tgz".to_string()));
    assert!(inline_cmds.contains(&"echo ok".to_string()));
    assert!(inline_cmds.contains(&"test -x /usr/local/bin/wsp-open".to_string()));
    for (_, cmd, timeout) in inline_of(&b) {
        assert!(timeout.is_some_and(|t| t <= 20_000), "{cmd}: {timeout:?}");
        // The base floor's versions read is a --version call per row, inline and bounded like the rest.
        if cmd.contains("echo \"VERSION curl:") {
            continue;
        }
        assert!(
            !(cmd.contains("setsid bash -c") || cmd.contains("tar ") || cmd.contains("the-setup") || cmd.contains("--version")),
            "a long command ran inline: {cmd}"
        );
    }
}

// golden.test.ts:1388
#[test]
fn the_lines_a_run_writes_stream_into_its_stage_frames_named_for_the_tool_or_agent() {
    let rec = recording_backend(Opts {
        stream: true,
        exec: Some(Box::new(|cmd: &str| {
            if cmd.contains("brew install gh") {
                return Some(exec(0, "==> Fetching gh\n==> Pouring gh\n", ""));
            }
            if cmd.contains("claude-install") {
                return Some(exec(0, "claude 1.2.3 installed\n", ""));
            }
            if cmd == FREE_KB_CMD {
                return Some(exec(0, &format!("{}\n", mb(3000)), ""));
            }
            if cmd == "echo ok" {
                return Some(reach_ok());
            }
            Some(ok())
        })),
        ..Opts::default()
    });
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let mut o = prep(&rec, &up, import_of());
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let all = st.all();
    let at = all.iter().position(|s| s == "installing-tools:gh (2/3)").expect("the gh frame");
    assert_eq!(
        all[at..at + 3].to_vec(),
        vec![
            "installing-tools:gh (2/3)".to_string(),
            "installing-tools:gh: ==> Fetching gh".to_string(),
            "installing-tools:gh: ==> Pouring gh".to_string()
        ]
    );
    assert!(all.contains(&"installing-harness:Claude Code: claude 1.2.3 installed".to_string()));
}

// golden.test.ts:1408
#[test]
fn the_shell_step_runs_after_the_pack_and_before_the_files_land_guarded_on_the_guest() {
    let b = backend_default();
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let shell = ShellInstall {
        shell: LoginShell::Zsh,
        frameworks: vec!["shell/oh-my-zsh".into(), "shell/antidote".into()],
        cmd: "set -euo pipefail\ninstall-zsh-and-frameworks".into(),
    };
    let mut o = prep(&b.rec, &up, GoldenImport { shell: Some(shell), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    let step = found(&cmds, "install-zsh-and-frameworks");
    assert!(step.contains("\nsetsid bash -c 'set -euo pipefail\ninstall-zsh-and-frameworks' &\np=$!\n"));
    assert!(step.contains("while [ $t -lt 300 ]"));
    let untar = found(&cmds, "tar xzf");
    assert!(index_of(&cmds, &step) < index_of(&cmds, &untar));
    assert!(index_of(&cmds, &step) < index_of(&cmds, &found(&cmds, "claude-install")));
    let all = st.all();
    let from =
        all.iter().position(|s| s == "applying-setup:1 KB packed; skipped ~/.bashrc (no longer on this computer)").expect("the pack frame");
    let to = all.iter().position(|s| s == "uploading-files:1 KB").expect("the upload frame") + 1;
    assert_eq!(
        all[from..to].to_vec(),
        vec![
            "applying-setup:1 KB packed; skipped ~/.bashrc (no longer on this computer)".to_string(),
            "applying-setup:zsh: installing, with shell/oh-my-zsh, shell/antidote".to_string(),
            "applying-setup:zsh installed as the login shell; shell/oh-my-zsh, shell/antidote reinstalled; 2.9 GB free".to_string(),
            "uploading-files:1 KB".to_string(),
        ]
    );
}

// golden.test.ts:1427
#[test]
fn a_shell_step_that_fails_is_a_warning_with_its_reason_and_the_files_still_land() {
    let b = backend_for(vec![("install-zsh", Answer::Fixed(exec(100, "", "E: Unable to locate package zsh\n")))], Free::Kb(mb(3000)));
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let shell = ShellInstall { shell: LoginShell::Zsh, frameworks: Vec::new(), cmd: "install-zsh".into() };
    let mut o = prep(&b.rec, &up, GoldenImport { shell: Some(shell), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    assert!(st.all().contains(&"applying-setup:zsh step failed, chsh skipped: E: Unable to locate package zsh".to_string()));
    assert!(b.cmds().iter().any(|c| c.contains("tar xzf")));
    assert_eq!(st.all().last().map(String::as_str), Some("ready"));
    assert!(applied_of(&builder).contains(&ImportStage::UploadingFiles));
}
