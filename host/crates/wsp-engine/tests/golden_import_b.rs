// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts:1722-2553 (inside "golden import stages"): the plugin seed's failed link road, the agents' and the Node
//! step's floors, the tools' guard and timeouts, the disk floor and its cleanups, the df readings, the upload's size check,
//! the ledger's missing, silenced and left-behind lists, and the attach that finds a recipe already applied.
#![allow(clippy::type_complexity)]
mod golden_import_support;
mod golden_roads_common;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use golden_roads_common::*;
use sha2::{Digest, Sha256};
use wsp_catalog::road_modules::{road_step, CURL_NET};
use wsp_catalog::roads::{node_install_script, node_release, RoadName, CLAUDE_INSTALL, GOLDEN_SETUP, NODE_PATH_LINE};
use wsp_engine::fetch::Fetch;
use wsp_engine::golden::{AgentResult, ImportStage, AGENT_INSTALLERS};
use wsp_engine::golden_seam::{
    tool_installs_for_rows, AgentInstall, BrewTable, GoldenImport, PluginSeedPlan, PluginsPlan, RecipeEntry, SeedPlugin, SkippedRow,
    ToolInstall, TOOLS_PATH,
};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::GoldenStep;

use golden_import_support::*;

// --- the tests ----------------------------------------------------------------------------------------------------

// golden.test.ts:1722
#[test]
fn older_image_copied_plugin_list_comes_off_and_a_failed_link_step_is_said_not_fatal() {
    let bare = backend_default();
    let plugins = PluginsPlan { dropped: strings(&[KNOWN, INSTALLED]), seed: None };
    build(prep(&bare, GoldenImport { plugins: Some(plugins), ..import_of() }));
    assert!(bare.cmds().contains(
        &"rm -f '/root/.claude-cfg/plugins/known_marketplaces.json' '/root/.claude-cfg/plugins/installed_plugins.json'".to_string()
    ));

    let b = backend_default();
    let seed = PluginSeedPlan {
        dir: "/seed".into(),
        plugins: vec![SeedPlugin { id: "plugins/v@acme".into(), label: "v@acme".into(), cmd: "seed-build-good".into(), asked: None }],
        links: Arc::new(FixedLinks(vec![("cache/acme/v/aaaaaaaaaaaa".into(), "1.0.0".into())])),
    };
    let mut o = prep(&b, GoldenImport { plugins: Some(PluginsPlan { dropped: vec![], seed: Some(seed) }), ..import_of() });
    o.backend = failing_exec(b.rec.dyn_backend(), "ln -sfn '1.0.0' '/seed/", "socket hang up");
    let st = stage_recorder();
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert!(st.all().iter().any(|l| links_left_out(l)), "stages: {:#?}", st.all());
}

// golden.test.ts:1737
#[test]
fn when_every_ticked_agent_fails_the_build_stops_names_each_and_kills_the_builder() {
    let b = backend_for(
        vec![
            ("claude-install", Answer::Fixed(exec(1, "", "curl: (6) Could not resolve host"))),
            ("codex-install", Answer::Fixed(exec(124, "", ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let err = build_err(o);
    assert!(err.message().contains("2 agents did not install"), "{}", err.message());
    assert_eq!(
        st.all().last().map(String::as_str),
        Some("failed:2 agents did not install, so nothing is sealed:\nClaude Code: curl: (6) Could not resolve host\nCodex: timed out after 900s")
    );
    assert_eq!(b.rec.state.killed(), strings(&["m1"]));
    let r = res.borrow();
    let outcomes: Vec<&str> = r[0].agents.iter().map(|a| a.outcome.as_str()).collect();
    assert_eq!(outcomes, ["failed", "failed"]);
}

// golden.test.ts:1750
#[test]
fn a_machine_that_stops_answering_after_the_installs_is_not_sealed_and_is_killed() {
    let b = backend_for(vec![("echo ok", Answer::Fixed(exec(1, "", "")))], Free::Kb(mb(3000)));
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let err = build_err(o);
    assert!(err.message().contains("stopped answering commands after the installs (exit 1)"), "{}", err.message());
    assert_eq!(
        st.all().last().map(String::as_str),
        Some("failed:the machine stopped answering commands after the installs (exit 1); nothing is sealed")
    );
    assert_eq!(b.rec.state.killed(), strings(&["m1"]));
    let r = res.borrow();
    let outcomes: Vec<&str> = r[0].tools.iter().map(|t| t.outcome.as_str()).collect();
    assert_eq!(outcomes, ["installed", "installed", "installed"]);
    let cmds = b.cmds();
    assert!(exact(&cmds, "echo ok") > at(&cmds, "bun@1.4.0"));
    // A machine that answers with exit 0 and the wrong words is not serving either.
    let mute = backend_for(vec![("echo ok", Answer::Fixed(exec(0, "", "")))], Free::Kb(mb(3000)));
    let err = build_err(prep(&mute, import_of()));
    assert!(err.message().contains("stopped answering commands after the installs (exit 0)"), "{}", err.message());
}

// golden.test.ts:1765
#[test]
fn the_node_step_runs_once_before_the_agents_kept_installed_or_failing_the_seal() {
    let agents = vec![
        agent("agents/codex", "Codex", "codex-install", "codex --version", "npm", Some(16)),
        agent("agents/pi", "Pi", "pi-install", "pi --version", "npm", Some(22)),
    ];
    let kept = backend_for(vec![("node-step", Answer::Fixed(exec(0, "NODE_HAVE v22.1.0\nNODE_KEPT v22.1.0\n", "")))], Free::Kb(mb(3000)));
    let k = stage_recorder();
    let mut o = prep(&kept, GoldenImport { node: Some(node_plan()), agents: agents.clone(), ..import_of() });
    o.on_stage = Some(k.on_stage.clone());
    let b1 = build(o);
    let all = k.all();
    let from = all.iter().position(|s| s == "installing-harness").expect("the harness stage");
    assert_stages(
        &all[from..],
        &[
            Want::Is("installing-harness"),
            Want::Is("installing-harness:Node for Pi"),
            Want::Is("installing-harness:Node v22.1.0 kept; Pi run on it"),
            Want::Is("installing-harness:Codex (1/2)"),
            Want::Is("installing-harness:Pi (2/2)"),
            Want::Is("installing-harness:Codex, Pi installed; caches swept; 2.9 GB free"),
            Want::Is("installing-tools:Homebrew (1/3)"),
            Want::Is("installing-tools:gh (2/3)"),
            Want::Is("installing-tools:bun@1.4.0 (3/3)"),
            Want::Is("installing-tools:3 installed; caches swept; 2.9 GB free"),
            Want::Is("installing-mcp:none configured"),
            Want::Re(is_context_written),
            Want::Is("ready"),
        ],
    );
    let cmds = kept.cmds();
    assert!(at(&cmds, "node-step") < at(&cmds, "codex-install"));
    assert_eq!(b1.setup_sha, hex::encode(Sha256::digest(b"true\nnode-step\ncodex-install\npi-install")));

    let installed =
        backend_for(vec![("node-step", Answer::Fixed(exec(0, "NODE_HAVE v18.20.4\nNODE_INSTALLED v22.23.2\n", "")))], Free::Kb(mb(3000)));
    let i = stage_recorder();
    let mut io = prep(&installed, GoldenImport { node: Some(node_plan()), agents: agents.clone(), ..import_of() });
    io.on_stage = Some(i.on_stage.clone());
    build(io);
    assert!(i.all().contains(&"installing-harness:Node v22.23.2 installed for Pi (the base had v18)".to_string()));

    let failed = backend_for(
        vec![("node-step", Answer::Fixed(exec(22, "NODE_HAVE v18.20.4\n", "curl: (22) The requested URL returned error: 404")))],
        Free::Kb(mb(3000)),
    );
    let f = stage_recorder();
    let (res, on_result) = collect_results();
    let mut fo = prep(&failed, GoldenImport { node: Some(node_plan()), agents, on_result: Some(on_result), ..import_of() });
    fo.on_stage = Some(f.on_stage.clone());
    let err = build_err(fo);
    assert!(err.message().contains("an agent did not install"), "{}", err.message());
    assert!(f
        .all()
        .contains(&"installing-harness:Node 22.23.2 did not install: curl: (22) The requested URL returned error: 404".to_string()));
    assert_eq!(
        f.all().last().map(String::as_str),
        Some("failed:an agent did not install, so nothing is sealed:\nPi: Node 22.23.2 did not install: curl: (22) The requested URL returned error: 404")
    );
    let fcmds = failed.cmds();
    assert!(!fcmds.iter().any(|c| c.contains("pi-install")));
    assert!(fcmds.iter().any(|c| c.contains("codex-install")));
    let r = res.borrow();
    let agents_out = &r[0].agents;
    assert_eq!(agents_out.len(), 2);
    assert_eq!(
        (agents_out[0].id.as_str(), agents_out[0].name.as_str(), agents_out[0].outcome.as_str(), agents_out[0].note.as_deref()),
        ("agents/codex", "Codex", "installed", None)
    );
    assert!(agents_out[0].ms.is_some(), "codex carries a number of ms");
    assert_eq!(
        agents_out[1],
        AgentResult {
            id: "agents/pi".into(),
            name: "Pi".into(),
            outcome: "failed".into(),
            note: Some("Node 22.23.2 did not install: curl: (22) The requested URL returned error: 404".into()),
            ms: Some(0),
        }
    );
}

// golden.test.ts:1801
#[test]
fn node_floor_and_agent_installers_run_under_the_road_tables_network_lines() {
    let agents = vec![agent("agents/pi", "Pi", "pi-install", "pi --version", "npm", Some(22))];
    let b = backend_for(vec![("node-step", Answer::Fixed(exec(0, "NODE_HAVE v22.1.0\nNODE_KEPT v22.1.0\n", "")))], Free::Kb(mb(3000)));
    build(prep(&b, GoldenImport { node: Some(node_plan()), agents, ..import_of() }));
    let cmds = b.cmds();
    for needle in ["node-step", "pi-install"] {
        let run = first_with(&cmds, needle);
        for line in road_step(RoadName::Script).env {
            assert!(run.contains(line.as_str()), "{needle}: {line}");
        }
        assert!(in_text(&run, CURL_NET.as_str()) < in_text(&run, needle), "{needle}");
        assert!(in_text(&run, "set -euo pipefail") < in_text(&run, needle), "{needle}");
    }
    // The two scripts that reach this path type curl with no flags of their own.
    let release = node_release(22).expect("node 22 is pinned");
    let aider = AGENT_INSTALLERS.get("aider").expect("aider has an installer").install.clone();
    for script in [node_install_script(22, &release), aider] {
        assert!(script.contains("curl -o"));
        assert!(!curl_fsl_flag(&script));
    }
}

// golden.test.ts:1820
#[test]
fn the_setup_line_and_the_agent_installers_run_under_the_guard_with_the_road_lines() {
    let b = backend_default();
    let agents: Vec<AgentInstall> = ["claude", "codex"]
        .iter()
        .map(|id| {
            let i = AGENT_INSTALLERS.get(*id).expect("the catalog pins this agent");
            agent(&format!("agents/{id}"), &i.name, &i.install, &i.smoke, &i.road, i.node)
        })
        .collect();
    let builder = build(prep_setup(&b, GOLDEN_SETUP.as_str(), GoldenImport { agents, ..import_of() }));
    let cmds = b.cmds();
    let runs = all_with(&cmds, "claude-code-releases/");
    assert_eq!(runs.iter().map(|r| r.contains(NODE_PATH_LINE)).collect::<Vec<_>>(), vec![false, true]);
    let cases = [
        (runs[0].clone(), Vec::<String>::new(), GOLDEN_SETUP.clone()),
        (runs[1].clone(), strings(&[NODE_PATH_LINE]), CLAUDE_INSTALL.clone()),
    ];
    for (run, path, script) in cases {
        let mut parts = road_step(RoadName::Script).env;
        parts.extend(path);
        parts.push(script);
        assert!(run.contains(&format!("setsid bash -c {} &", shell_quote(&parts.join("\n")))));
        assert!(run.contains("while [ $t -lt 900 ]"));
        assert!(!pipes_into_shell(&run));
        let at_run = |needle: &str| {
            let i = in_text(&run, needle);
            assert!(i > -1, "{needle}");
            i
        };
        assert!(at_run(CURL_NET.as_str()) < at_run("curl -o /tmp/claude"));
        assert!(at_run("curl -o /tmp/claude") < at_run("sha256sum -c -"));
        assert!(at_run("sha256sum -c -") < at_run("install -D -m 0755 /tmp/claude"));
    }
    let codex = all_with(&cmds, "npm install -g @openai/codex@");
    assert_eq!(codex.len(), 1);
    let npm_env = road_step(RoadName::Npm).env;
    assert!(codex[0].contains(&format!("{}\n", npm_env.join("\n"))));
    assert!(in_text(&codex[0], &npm_env[0]) < in_text(&codex[0], "npm install -g @openai/codex@"));
    let codex_install = AGENT_INSTALLERS.get("codex").expect("codex has an installer").install.clone();
    assert_eq!(
        builder.setup_sha,
        hex::encode(Sha256::digest(format!("{}\n{}\n{}", GOLDEN_SETUP.as_str(), CLAUDE_INSTALL.as_str(), codex_install)))
    );
}

// golden.test.ts:1843
#[test]
fn a_failures_reason_is_homebrews_error_line_not_the_advice_after_it() {
    let b = backend_for(
        vec![(
            "brew install gh",
            Answer::Fixed(exec(
                1,
                "==> Fetching downloads for: gh\n",
                "Error: gh: A `brew install gh` process has already locked /home/linuxbrew/.linuxbrew/Cellar/gcc.\nPlease wait for it to finish or terminate it to continue.\n",
            )),
        )],
        Free::Kb(mb(3000)),
    );
    let (res, on_result) = collect_results();
    build(prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() }));
    assert_eq!(
        res.borrow()[0].tools[1].note.as_deref(),
        Some("Error: gh: A `brew install gh` process has already locked /home/linuxbrew/.linuxbrew/Cellar/gcc.")
    );
}

// golden.test.ts:1852
#[test]
fn an_agent_set_aside_at_plan_time_lands_in_the_result_as_skipped() {
    let aside = vec![SkippedRow { id: "agents/zed".into(), label: "Zed".into(), note: "no installer known".into() }];
    let b = backend_default();
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b, GoldenImport { skipped_agents: aside.clone(), on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let built = build(o);
    assert_eq!(
        res.borrow()[0].agents[0],
        AgentResult {
            id: "agents/zed".into(),
            name: "Zed".into(),
            outcome: "skipped".into(),
            note: Some("no installer known".into()),
            ms: None
        }
    );
    assert!(st.all().contains(
        &"installing-harness:Claude Code, Codex installed; Zed skipped (no installer known); caches swept; 2.9 GB free".to_string()
    ));
    assert_eq!(built.import.as_ref().map(|l| l.smoke.as_str()), Some("claude --version && codex --version && bun --version"));

    let only = backend_default();
    let rec = stage_recorder();
    let mut oo = prep(&only, GoldenImport { agents: vec![], skipped_agents: aside, ..import_of() });
    oo.on_stage = Some(rec.on_stage.clone());
    let err = build_err(oo);
    assert!(err.message().contains("no agent installed"), "{}", err.message());
    assert_eq!(
        rec.all().last().map(String::as_str),
        Some("failed:no agent installed, so there is nothing to seal:\nZed: no installer known")
    );
    assert_eq!(only.rec.state.killed(), strings(&["m1"]));
}

// golden.test.ts:1869
#[test]
fn an_agent_that_does_not_start_under_the_floor_is_recorded_failed_with_the_reading() {
    let started = Rc::new(Cell::new(false));
    let (s1, s2) = (started.clone(), started.clone());
    let b = backend_for(
        vec![(
            "claude-install",
            Answer::Each(Rc::new(move || {
                s1.set(true);
                ok()
            })),
        )],
        Free::Each(Rc::new(move || mb(if s2.get() { 500 } else { 3000 }))),
    );
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let err = build_err(o);
    assert!(
        err.message().contains("an agent did not install, so nothing is sealed:\nCodex: 500 MB free, keeping 800 MB free"),
        "{}",
        err.message()
    );
    let rows: Vec<(String, String, Option<String>)> =
        res.borrow()[0].agents.iter().map(|a| (a.id.clone(), a.outcome.clone(), a.note.clone())).collect();
    assert_eq!(
        rows,
        vec![
            ("agents/claude".to_string(), "installed".to_string(), None),
            ("agents/codex".to_string(), "failed".to_string(), Some("500 MB free, keeping 800 MB free".to_string())),
        ]
    );
    assert!(!b.cmds().iter().any(|c| c.contains("codex-install")));
    assert_eq!(b.rec.state.killed(), strings(&["m1"]));
    assert_eq!(
        st.all().last().map(String::as_str),
        Some("failed:an agent did not install, so nothing is sealed:\nCodex: 500 MB free, keeping 800 MB free")
    );
}

// golden.test.ts:1884
#[test]
fn when_homebrew_itself_fails_every_brew_formula_is_skipped_rather_than_tried() {
    let b = backend_for(vec![("brew-bootstrap", Answer::Fixed(exec(1, "", "git: not found")))], Free::Kb(mb(3000)));
    let st = stage_recorder();
    let mut o = prep(&b, import_of());
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    assert!(!cmds.iter().any(|c| c.contains("brew install gh")));
    assert!(st.all().contains(&"installing-tools:1 installed, 1 failed: Homebrew (git: not found), 1 skipped: gh (Homebrew did not install); caches swept; 2.9 GB free".to_string()));
    // No Homebrew, nothing of its to clean.
    assert!(!cmds.iter().any(|c| c.contains("brew autoremove") || c.contains("brew cleanup")));
}

// golden.test.ts:1894
#[test]
fn a_tool_that_hits_its_timeout_is_failed_and_its_guard_kills_the_tree() {
    let b = backend_for(vec![("brew install gh", Answer::Fixed(exec(124, "==> Downloading gh\n", "")))], Free::Kb(mb(3000)));
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert_eq!(
        tuples(&res.borrow()[0].tools),
        vec![
            row("tools/homebrew", "installed", None),
            row("tools/brew/gh", "failed", Some("timed out after 600s")),
            row("tools/npm/bun", "installed", None)
        ]
    );
    assert!(st.all().contains(&"installing-tools:2 installed, 1 failed: gh (timed out after 600s); caches swept; 2.9 GB free".to_string()));
    let cmds = b.cmds();
    // The install runs in its own session; at the timeout that session's group and everything descended from it
    // (found through /proc by parent pid, since su starts its command in a session of its own) get TERM, then KILL.
    let guard = first_with(&cmds, "brew install gh");
    assert!(guard.starts_with("tree() {\n"));
    assert!(guard.contains("for f in /proc/[0-9]*/stat"));
    assert!(in_order(&guard, &["\nsetsid bash -c '", "' &\np=$!\n"]));
    assert!(guard.contains("v=\"$p $(tree $p)\"\n  kill -TERM -- -$p $v"));
    assert!(guard.contains("kill -KILL -- -$p $v"));
    // The guard returns 124 only once the tree is gone, so the next brew never meets a lock the last one still holds.
    assert!(kill_then_exit(&guard));
    assert!(!guard.contains("pkill") && !guard.contains("killall"));
}

// golden.test.ts:1918
#[test]
fn a_download_road_that_hits_its_limit_is_tried_once_more_and_the_frames_say_so() {
    let tries = Rc::new(Cell::new(0u32));
    let t = tries.clone();
    let b = backend_for(
        vec![(
            "npm install -g bun@1.4.0",
            Answer::Each(Rc::new(move || {
                t.set(t.get() + 1);
                if t.get() == 1 {
                    exec(124, "", "")
                } else {
                    ok()
                }
            })),
        )],
        Free::Kb(mb(3000)),
    );
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert_eq!(tries.get(), 2);
    assert_eq!(
        tuples(&res.borrow()[0].tools),
        vec![row("tools/homebrew", "installed", None), row("tools/brew/gh", "installed", None), row("tools/npm/bun", "installed", None)]
    );
    assert!(st.all().contains(&"installing-tools:bun@1.4.0: timed out after 300s; trying once more".to_string()));
    // The step's script opens with the road's network clock, and the guard's limit is the road's, not one number for every tool.
    let cmds = b.cmds();
    let guards = all_with(&cmds, "npm install -g bun@1.4.0");
    assert_eq!(guards.len(), 2);
    for g in &guards {
        assert!(g.contains("export npm_config_fetch_timeout=60000 npm_config_fetch_retries=1 npm_config_fetch_retry_maxtimeout=10000\nnpm install -g bun@1.4.0"));
        assert!(g.contains("while [ $t -lt 300 ]"));
    }
    let brew = first_with(&cmds, "brew install gh");
    assert!(brew.contains("while [ $t -lt 600 ]"));
    assert!(!brew.contains("npm_config_fetch_timeout"));
}

// golden.test.ts:1943
#[test]
fn a_download_road_that_hits_its_limit_twice_is_recorded_failed_with_both_timeouts() {
    let b = backend_for(vec![("npm install -g bun@1.4.0", Answer::Fixed(exec(124, "", "")))], Free::Kb(mb(3000)));
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let bun = res.borrow()[0].tools.iter().find(|t| t.id == "tools/npm/bun").cloned().expect("the bun row");
    assert_eq!(bun.outcome, "failed");
    assert_eq!(bun.note.as_deref(), Some("timed out after 300s, twice"));
    assert!(st.all().contains(
        &"installing-tools:2 installed, 1 failed: bun@1.4.0 (timed out after 300s, twice); caches swept; 2.9 GB free".to_string()
    ));
    assert_eq!(all_with(&b.cmds(), "npm install -g bun@1.4.0").len(), 2);
}

// golden.test.ts:1953
#[test]
fn every_frame_of_a_step_names_the_step_and_its_line_and_stage_lines_name_none() {
    let rb = recording_backend(Opts {
        stream: true,
        exec: Some(Box::new(|cmd: &str| {
            Some(if cmd.contains("brew install gh") {
                exec(0, "==> Downloading gh\n==> Pouring gh\n", "")
            } else if cmd == FREE_KB_CMD {
                exec(0, &format!("{}\n", mb(3000)), "")
            } else if cmd == "echo ok" {
                reach_ok()
            } else if cmd.contains("echo WSP_CTX") {
                exec(0, "WSP_CTX\nWSP_CTX_END\n", "")
            } else {
                ok()
            })
        })),
        ..Opts::default()
    });
    let fetch: Arc<dyn Fetch> = FetchStub::ok();
    let st = stage_recorder();
    let tools = vec![
        ToolInstall {
            id: "tools/brew/gh".into(),
            label: "gh".into(),
            manager: "brew".into(),
            cmd: "su -c 'brew install gh'".into(),
            shown: Some("brew install gh".into()),
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
    let mut o = prepare_opts(&rb, "true");
    o.fetch = Some(fetch);
    o.import = Some(GoldenImport { tools, ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let frames: Vec<(String, Option<GoldenStep>)> = st.framed().into_iter().filter(|(l, _)| l.starts_with("installing-tools")).collect();
    let step = |label: &str, command: &str| Some(GoldenStep { label: label.into(), command: command.into() });
    let gh = step("gh", "brew install gh");
    assert_eq!(
        frames[..3],
        [
            ("installing-tools:gh (1/2)".to_string(), gh.clone()),
            ("installing-tools:gh: ==> Downloading gh".to_string(), gh.clone()),
            ("installing-tools:gh: ==> Pouring gh".to_string(), gh),
        ]
    );
    // A step with no line of its own is read by its command.
    assert!(frames.contains(&("installing-tools:bun@1.4.0 (2/2)".to_string(), step("bun@1.4.0", "npm install -g bun@1.4.0"))));
    let (last, last_step) = frames.last().expect("frames");
    assert!(last_step.is_none() && last.starts_with("installing-tools:2 installed"));
}

// golden.test.ts:1976
#[test]
fn an_image_whose_pnpm_does_not_run_seals_nothing_and_its_check_rides_the_smoke() {
    let dir = tempfile::tempdir().expect("a temp dir");
    // pnpm as npm leaves it: the command loads the package's dist, which this one has lost.
    let pkg = dir.path().join("lib/node_modules/pnpm");
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&pkg).expect("the package folder");
    std::fs::create_dir_all(&bin).expect("the bin folder");
    write_stub(&bin.join("pnpm"), &format!("#!{}\nimport(\"{}\");\n", node_exe().display(), pkg.join("dist/pnpm.mjs").display()));
    // The row the Mac's own pnpm becomes, planned by the catalog's road.
    let plan = tool_installs_for_rows(
        &[RecipeEntry {
            rung: "tools".into(),
            id: "tools/npm/pnpm".into(),
            label: "pnpm".into(),
            paths: vec![],
            bytes: 0,
            default: "skip".into(),
            bring: Some(true),
            linux: Some("yes".into()),
            version: Some("10.34.5".into()),
            ..Default::default()
        }],
        &BrewTable::new(),
        &[],
        TOOLS_PATH,
        None,
    )
    .expect("the plan")
    .installs;

    let (broken_rec, broken) = pnpm_seal(&bin, plan.clone());
    let err = broken.expect_err("a seal over a broken pnpm fails");
    assert!(in_order(&err.message(), &["golden smoke failed ", "\"pnpm --version\"", "ERR_MODULE_NOT_FOUND"]), "{}", err.message());
    assert_eq!(broken_rec.state.deleted_snapshots(), strings(&["snap_wsp-h1-default-v1"]));

    std::fs::create_dir_all(pkg.join("dist")).expect("the dist folder");
    std::fs::write(pkg.join("dist/pnpm.mjs"), "console.log(\"10.34.5\");\n").expect("the module");
    let (_rec, sealed) = pnpm_seal(&bin, plan);
    let version = sealed.expect("a pnpm that runs seals").version;
    assert_eq!(version.smoke.cmd, "pnpm --version");
    assert_eq!(version.smoke.exit_code.0, 0.0);
}

// golden.test.ts:2011
#[test]
fn a_tool_with_no_version_check_of_its_own_adds_nothing_to_the_smoke() {
    // `go --version` exits 2; only the install rows that name their manager's check are checked.
    let rows: Vec<RecipeEntry> = ["go", "rust"]
        .iter()
        .map(|name| RecipeEntry {
            rung: "tools".into(),
            id: format!("tools/catalog/{name}"),
            label: name.to_string(),
            paths: vec![],
            bytes: 0,
            default: "skip".into(),
            bring: Some(true),
            linux: Some("yes".into()),
            ..Default::default()
        })
        .collect();
    let plan = tool_installs_for_rows(&rows, &BrewTable::new(), &[], TOOLS_PATH, None).expect("the plan").installs;
    let ids: Vec<&str> = plan.iter().map(|i| i.id.as_str()).collect();
    assert!(ids.contains(&"tools/catalog/go") && ids.contains(&"tools/catalog/rust"), "{ids:?}");
    let b = backend_default();
    let builder = build(prep(&b, GoldenImport { tools: plan, agents: vec![], ..import_of() }));
    assert_eq!(builder.import.as_ref().map(|l| l.smoke.as_str()), Some("true"));
}

// golden.test.ts:2021
#[test]
fn a_cellar_lock_error_waits_once_for_every_homebrew_lock_then_tries_once_more() {
    let tries = Rc::new(Cell::new(0u32));
    let t = tries.clone();
    let locked = exec(
        1,
        "",
        "Error: A `brew install glibc` process has already locked /home/linuxbrew/.linuxbrew/Cellar/linux-headers@6.8.\nPlease wait for it to finish or terminate it to continue.",
    );
    let l = locked.clone();
    let b = backend_for(
        vec![(
            "brew install gh",
            Answer::Each(Rc::new(move || {
                t.set(t.get() + 1);
                if t.get() == 1 {
                    l.clone()
                } else {
                    ok()
                }
            })),
        )],
        Free::Kb(mb(3000)),
    );
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert_eq!(tries.get(), 2);
    let cmds = b.cmds();
    assert_eq!(
        first_with(&cmds, "flock -w"),
        "for l in /home/linuxbrew/.linuxbrew/var/homebrew/locks/*.lock; do [ -e \"$l\" ] && flock -w 600 \"$l\" true; done; true"
    );
    assert_eq!(all_with(&cmds, "flock -w").len(), 1);
    assert!(at(&cmds, "brew install gh") < at(&cmds, "flock -w"));
    let gh = first_with(&cmds, "brew install gh");
    assert!(last_exact(&cmds, &gh) > at(&cmds, "flock -w"));
    assert!(st.all().contains(&"installing-tools:gh: another brew holds its cellar; waiting for it, then once more".to_string()));
    let gh_row = res.borrow()[0].tools.iter().find(|t| t.id == "tools/brew/gh").cloned().expect("the gh row");
    assert_eq!(gh_row.outcome, "installed");

    // A second lock error is the tool's failure, with Homebrew's line as the reason.
    let again = backend_for(vec![("brew install gh", Answer::Fixed(locked))], Free::Kb(mb(3000)));
    let (more, on_more) = collect_results();
    build(prep(&again, GoldenImport { on_result: Some(on_more), ..import_of() }));
    let again_cmds = again.cmds();
    assert_eq!(all_with(&again_cmds, "flock -w").len(), 1);
    assert_eq!(all_with(&again_cmds, "brew install gh").len(), 2);
    let gh_again = more.borrow()[0].tools.iter().find(|t| t.id == "tools/brew/gh").cloned().expect("the gh row");
    assert_eq!(gh_again.outcome, "failed");
    assert_eq!(
        gh_again.note.as_deref(),
        Some("Error: A `brew install glibc` process has already locked /home/linuxbrew/.linuxbrew/Cellar/linux-headers@6.8.")
    );
}

// golden.test.ts:2046
#[test]
fn after_the_loop_homebrew_autoremoves_then_cleans_up_both_guarded_and_says_what_it_freed() {
    let cleaned = Rc::new(Cell::new(false));
    let (c1, c2) = (cleaned.clone(), cleaned.clone());
    let b = backend_for(
        vec![(
            "brew cleanup -s --prune=all",
            Answer::Each(Rc::new(move || {
                c1.set(true);
                ok()
            })),
        )],
        Free::Each(Rc::new(move || mb(if c2.get() { 4000 } else { 3000 }))),
    );
    let st = stage_recorder();
    let mut o = prep(&b, import_of());
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    let autoremove = cmds[at(&cmds, "brew autoremove") as usize].clone();
    assert!(autoremove.starts_with("tree() {\n"));
    assert!(in_order(&autoremove, &["\nsetsid bash -c 'export PATH=", "su -s /bin/bash linuxbrew -c ", "brew autoremove", " &\np=$!\n"]));
    assert!(!autoremove.contains("HOMEBREW_NO_INSTALL_CLEANUP"));
    assert!(at(&cmds, "bun@1.4.0") < at(&cmds, "brew autoremove"));
    assert!(at(&cmds, "brew autoremove") < at(&cmds, "brew cleanup -s --prune=all"));
    assert!(at(&cmds, "brew cleanup -s --prune=all") < exact(&cmds, "echo ok"));
    assert!(st.all().contains(&"installing-tools:3 installed; Homebrew cleanup freed 1000 MB; caches swept; 3.9 GB free".to_string()));
    let sweep = last_exact(&cmds, "rm -f /tmp/wsp-vault-*.tgz");
    assert!(sweep > at(&cmds, "tar xzf"));
    assert!(sweep < at(&cmds, "brew-bootstrap"));

    // A cleanup that fails is named, and the tools it followed still count.
    let failing = backend_for(
        vec![("brew cleanup -s --prune=all", Answer::Fixed(exec(1, "", "Error: Permission denied @ apply2files")))],
        Free::Kb(mb(3000)),
    );
    let rec = stage_recorder();
    let mut fo = prep(&failing, import_of());
    fo.on_stage = Some(rec.on_stage.clone());
    build(fo);
    assert!(rec.all().contains(
        &"installing-tools:3 installed; Homebrew cleanup failed (Error: Permission denied @ apply2files); caches swept; 2.9 GB free"
            .to_string()
    ));
}

// golden.test.ts:2073
#[test]
fn after_the_agents_and_after_the_tools_the_caches_are_swept_under_the_guard() {
    let sweeps = Rc::new(Cell::new(0u64));
    let (s1, s2) = (sweeps.clone(), sweeps.clone());
    let b = backend_for(
        vec![(
            SWEEP,
            Answer::Each(Rc::new(move || {
                s1.set(s1.get() + 1);
                ok()
            })),
        )],
        Free::Each(Rc::new(move || mb(3000 + s2.get() * 700))),
    );
    let st = stage_recorder();
    let mut o = prep(&b, import_of());
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    let sweep_cmds = all_with(&cmds, SWEEP);
    // Once after the base floor, once after the agents, once after the tools.
    assert_eq!(sweep_cmds.len(), 3);
    for sweep in &sweep_cmds {
        assert!(sweep.contains("\nsetsid bash -c 'set -euo pipefail\nexport PATH=/root/.local/bin:"));
        assert!(sweep.contains("if command -v go >/dev/null 2>&1; then go clean -cache -modcache; fi"));
        assert!(sweep.contains("if command -v apt-get >/dev/null 2>&1; then apt-get clean; fi"));
        assert!(sweep.contains("while [ $t -lt 300 ]"));
    }
    let sweeps_at: Vec<isize> = cmds.iter().enumerate().filter(|(_, c)| c.contains(SWEEP)).map(|(i, _)| i as isize).collect();
    // The base's caches go before the daemon; the agents' before the tools stage reads the disk against its floor; the tools' after Homebrew's own housekeeping.
    let setup_mark = if at(&cmds, "the-setup") < 0 { at(&cmds, "claude-install") } else { at(&cmds, "the-setup") };
    assert!(sweeps_at[0] < setup_mark);
    assert!(at(&cmds, "codex-install") < sweeps_at[1]);
    assert!(sweeps_at[1] < at(&cmds, "brew-bootstrap"));
    assert!(at(&cmds, "brew cleanup -s --prune=all") < sweeps_at[2]);
    assert!(sweeps_at[2] < exact(&cmds, "echo ok"));
    // Each closing line carries what the sweep gave back and the df reading the stage left.
    assert!(st.all().contains(&"deploying-daemon:16 installed; caches swept, 700 MB back; 3.6 GB free".to_string()));
    assert!(st.all().contains(&"installing-harness:Claude Code, Codex installed; caches swept, 700 MB back; 4.3 GB free".to_string()));
    assert!(st.all().contains(&"installing-tools:3 installed; caches swept, 700 MB back; 5 GB free".to_string()));

    // A sweep that fails is named, and the build goes on to the next stage.
    let failing = backend_for(
        vec![(SWEEP, Answer::Fixed(exec(1, "", "rm: cannot remove '/root/.npm': Device or resource busy")))],
        Free::Kb(mb(3000)),
    );
    let rec = stage_recorder();
    let mut fo = prep(&failing, import_of());
    fo.on_stage = Some(rec.on_stage.clone());
    let builder = build(fo);
    assert!(rec.all().contains(&"installing-harness:Claude Code, Codex installed; cache sweep failed (rm: cannot remove '/root/.npm': Device or resource busy); 2.9 GB free".to_string()));
    assert!(rec.all().contains(
        &"installing-tools:3 installed; cache sweep failed (rm: cannot remove '/root/.npm': Device or resource busy); 2.9 GB free"
            .to_string()
    ));
    assert!(applied_of(&builder).contains(&ImportStage::InstallingTools));
}

// golden.test.ts:2111
#[test]
fn stops_installing_tools_when_the_disk_drops_under_the_floor_and_stays_there_after_the_cleanup() {
    let boot = Rc::new(Cell::new(false));
    let cleanups = Rc::new(Cell::new(0u32));
    let sweeps = Rc::new(Cell::new(0u32));
    let (b1, c1, s1) = (boot.clone(), cleanups.clone(), sweeps.clone());
    let (b2, c2, s2) = (boot.clone(), cleanups.clone(), sweeps.clone());
    // The base floor and the two agents (a sweep each), the upload and Homebrew read a roomy disk; the first formula reads it low, and the cleanup gives little back.
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
                SWEEP,
                Answer::Each(Rc::new(move || {
                    s1.set(s1.get() + 1);
                    ok()
                })),
            ),
        ],
        Free::Each(Rc::new(move || {
            mb(if !b2.get() {
                3000
            } else if c2.get() == 0 {
                1800
            } else if s2.get() == 2 {
                1900
            } else {
                1950
            })
        })),
    );
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    let all = st.all();
    assert!(!cmds.iter().any(|c| c.contains("brew install gh")));
    assert!(all.contains(&"installing-tools:1.8 GB free, under the 2 GB floor; cleaning up before skipping".to_string()));
    assert!(all.contains(&"installing-tools:Homebrew cleanup freed 100 MB; caches swept, 50 MB back; 1.9 GB free".to_string()));
    assert!(all.contains(
        &"installing-tools:1 installed, 2 skipped: gh, bun@1.4.0 (1.9 GB free after cleanup, keeping 2 GB free); caches swept; 1.9 GB free"
            .to_string()
    ));
    assert_eq!(
        tuples(&res.borrow()[0].tools),
        vec![
            row("tools/homebrew", "installed", None),
            row("tools/brew/gh", "skipped", Some("1.9 GB free after cleanup, keeping 2 GB free")),
            row("tools/npm/bun", "skipped", Some("1.9 GB free after cleanup, keeping 2 GB free")),
        ]
    );
    // The cleanup at the floor is guarded, autoremove first, the sweep after Homebrew, and runs once in the loop; the housekeeping after the loop still runs.
    let cleanup_at = at(&cmds, "brew cleanup -s --prune=all") as usize;
    assert!(cmds[cleanup_at].starts_with("tree() {\n"));
    assert!(at(&cmds, "brew autoremove") > at(&cmds, "brew-bootstrap"));
    assert!(at(&cmds, "brew autoremove") < at(&cmds, "brew cleanup -s --prune=all"));
    let sweeps_at: Vec<isize> = cmds.iter().enumerate().filter(|(_, c)| c.contains(SWEEP)).map(|(i, _)| i as isize).collect();
    assert!(sweeps_at[2] > at(&cmds, "brew cleanup -s --prune=all"));
    let last_cleanup = last_exact_contains(&cmds, "brew cleanup -s --prune=all");
    assert!(sweeps_at[2] < last_cleanup);
    assert_eq!(cleanups.get(), 2);
    assert_eq!(sweeps.get(), 4);
}

/// TS `cmds.length - 1 - [...cmds].reverse().findIndex(c => c.includes(needle))`: the last command that contains `needle`.
fn last_exact_contains(cmds: &[String], needle: &str) -> isize {
    cmds.iter().rposition(|c| c.contains(needle)).map_or(-1, |i| i as isize)
}

// golden.test.ts:2144
#[test]
fn at_the_first_reading_under_the_floor_the_cleanup_runs_and_df_is_read_again() {
    let boot = Rc::new(Cell::new(false));
    let cleanups = Rc::new(Cell::new(0u32));
    let gh_done = Rc::new(Cell::new(false));
    let (b1, c1, g1) = (boot.clone(), cleanups.clone(), gh_done.clone());
    let (b2, c2, g2) = (boot.clone(), cleanups.clone(), gh_done.clone());
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
                "brew install gh",
                Answer::Each(Rc::new(move || {
                    g1.set(true);
                    ok()
                })),
            ),
        ],
        Free::Each(Rc::new(move || {
            mb(if !b2.get() {
                3000
            } else if g2.get() {
                500
            } else if c2.get() == 0 {
                1800
            } else {
                4200
            })
        })),
    );
    let (res, on_result) = collect_results();
    let st = stage_recorder();
    let mut o = prep(&b, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    assert!(at(&cmds, "brew cleanup -s --prune=all") > at(&cmds, "brew-bootstrap"));
    assert!(at(&cmds, "brew cleanup -s --prune=all") < at(&cmds, "brew install gh"));
    assert_eq!(
        starting(&st.all(), "installing-tools"),
        strings(&[
            "installing-tools:Homebrew (1/3)",
            "installing-tools:1.8 GB free, under the 2 GB floor; cleaning up before skipping",
            "installing-tools:Homebrew cleanup freed 2.3 GB; caches swept; 4.1 GB free",
            "installing-tools:gh (2/3)",
            "installing-tools:2 installed, 1 skipped: bun@1.4.0 (500 MB free, keeping 2 GB free); caches swept; 500 MB free",
        ])
    );
    let r = res.borrow();
    let rows: Vec<(String, String)> = r[0].tools.iter().map(|t| (t.id.clone(), t.outcome.clone())).collect();
    assert_eq!(
        rows,
        vec![
            ("tools/homebrew".to_string(), "installed".to_string()),
            ("tools/brew/gh".to_string(), "installed".to_string()),
            ("tools/npm/bun".to_string(), "skipped".to_string()),
        ]
    );
    // One rescue in the loop, one housekeeping after it; the base, the agents, the rescue and the tools each sweep.
    assert_eq!(cleanups.get(), 2);
    assert_eq!(all_with(&cmds, SWEEP).len(), 4);
}
