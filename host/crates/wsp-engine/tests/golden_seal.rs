// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts "interactive golden: prepare then seal" (:256-554): the prepare half, the sudo and profile files it
//! writes, the seal's snapshot, the npm folder and the PATH a fork of the image gets.
#![allow(clippy::type_complexity)]
mod golden_roads_common;
mod golden_seal_support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use golden_roads_common::*;
use golden_seal_support::*;
use serde_json::json;
use wsp_engine::errors::{EngineError, RetryClock};
use wsp_engine::golden::{claude_link_line, npm_bin_of, prepare_builder, seal_golden, smoke_tally, Builder, SealResult, QUIET_LOGS};
use wsp_engine::golden_seam::{profile_path_line, GoldenImport, BUILDER_IDLE_MS, PROFILE_PATH_FILE, TOOLS_PATH};
use wsp_engine::machine::{Machine, MachineKind};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::{RecipeDigest, AGENTS_BIN};

/// Prepare a builder on the clock the test holds.
fn prep(opts: wsp_engine::golden::PrepareBuilderOptions, clock: &Manual) -> Builder {
    run(prepare_builder(opts, clock)).expect("the builder is prepared")
}

/// Seal a builder and take the version.
fn seal_ok(builder: Builder, opts: wsp_engine::golden::SealGoldenOptions, clock: &Manual) -> SealResult {
    run(seal_golden(builder, opts, clock)).expect("the seal sealed")
}

/// The relocation the TS tests make of a command: the agents folder, /usr/local/bin and the profile file move under
/// a temporary root.
fn relocate(root: &str, cmd: &str) -> String {
    cmd.replace(AGENTS_BIN, &format!("{root}{AGENTS_BIN}"))
        .replace("/usr/local/bin", &format!("{root}/usr/local/bin"))
        .replace(PROFILE_PATH_FILE, &format!("{root}{PROFILE_PATH_FILE}"))
}

/// golden.test.ts:458 and :496 `say`/`write`: a script that echoes its words, executable.
fn echo_script(words: &str) -> String {
    format!("#!/bin/sh\necho \"{words}\"\n")
}

/// The stdout of a command under bash, read as text.
fn stdout_text(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

// golden.test.ts:257
#[test]
fn prepare_boots_a_sandbox_from_the_sandbox_template_with_the_builder_disk_runs_daemon_then_harness_and_reports_stages() {
    let rec = recording();
    let clock = Manual::new();
    let st = stage_recorder();
    let daemon_on: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let mut opts = prepare_opts(&rec, "install harness");
    let seen = daemon_on.clone();
    opts.deploy_daemon = Some(deploy_with(move |m: Arc<dyn Machine>| {
        let seen = seen.clone();
        Box::pin(async move {
            seen.borrow_mut().push(m.id().to_string());
            Ok::<Option<String>, EngineError>(None)
        })
    }));
    opts.on_stage = Some(st.on_stage.clone());
    let builder = prep(opts, &clock);
    let created = rec.state.created();
    match_object(&created[0], json!({"kind": "sandbox", "template": "base", "onIdle": "kill", "diskGb": 20}));
    assert_eq!(builder.kind, MachineKind::Sandbox);
    assert!(builder.first_life);
    assert!(builder.machine.stream_url().is_none());
    assert_eq!(*daemon_on.borrow(), vec!["m1"]);
    assert_eq!(sans_base(&st.all()), vec!["creating:sandbox from base", "deploying-daemon", "installing-harness", "ready"]);
    let floor = rec.state.ran_on("m1");
    assert_eq!(find_index(&floor, |s| s.contains("> /etc/profile.d/wsp-golden.sh")), 0);
    assert_eq!(find_index(&floor, |s| s.contains("apt-get update -qq")), 1);
    assert_eq!(find_index(&floor, |s| s.contains("apt-get install -y -qq curl")), 2);
    assert_eq!(find_index(&floor, |s| s.contains("astral-sh/uv/releases")), 3);
    assert!(!floor.iter().any(|s| s.contains("nodejs.org/dist")));
    assert_eq!(rec.state.timeline(), vec!["create m1"]);
}

// golden.test.ts:286
#[test]
fn a_builder_keeps_every_command_line_out_of_sudos_and_systemds_logs_before_anything_else_runs_on_it() {
    let rec = recording();
    let clock = Manual::new();
    prep(prepare_opts(&rec, "true"), &clock);
    let first =
        rec.state.inline.borrow().iter().find(|(id, _, _)| id == "m1").map(|(_, cmd, _)| cmd.clone()).expect("an inline exec on m1");
    assert_eq!(first, QUIET_LOGS);
}

// golden.test.ts:293
#[test]
fn the_step_writes_sudos_two_lines_once_visudo_passed_them_a_manager_file_the_box_login_can_read_under_any_mask_and_re_executes_each_running_manager(
) {
    let dir = tempfile::tempdir().unwrap();
    let etc = dir.path().join("etc");
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(etc.join("sudoers.d")).unwrap();
    std::fs::create_dir(&bin).unwrap();
    let asked = dir.path().join("asked");
    let asked_q = shell_quote(&asked.display().to_string());
    write_stub(&bin.join("visudo"), &format!("#!/bin/sh\necho \"visudo $*\" >> {asked_q}\n"));
    write_stub(&bin.join("loginctl"), "#!/bin/sh\necho '1000 user yes active'\n");
    write_stub(&bin.join("systemctl"), &format!("#!/bin/sh\necho \"systemctl $*\" >> {asked_q}\n"));
    let script = QUIET_LOGS.replace("/etc/", &format!("{}/", etc.display()));
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let ran = bash_env(&format!("umask 077\n{script}"), &[("PATH", &path)]);
    assert!(ran.status.success(), "the quiet-logs step runs: {}", String::from_utf8_lossy(&ran.stderr));
    let sudoers = etc.join("sudoers.d/wsp-quiet-logs");
    assert_eq!(std::fs::read_to_string(&sudoers).unwrap(), "Defaults !log_allowed\nDefaults !pam_session\n");
    assert_eq!(mode_of(&sudoers), "440");
    assert_eq!(mode_of(&etc.join("systemd/user.conf.d")), "755");
    assert_eq!(mode_of(&etc.join("systemd/user.conf.d/wsp-quiet-logs.conf")), "644");
    assert_eq!(std::fs::read_to_string(etc.join("systemd/user.conf.d/wsp-quiet-logs.conf")).unwrap(), "[Manager]\nLogLevel=notice\n");
    let said = read_lines(&asked);
    assert!(said[0].starts_with("visudo -cqf "), "{}", said[0]);
    assert_eq!(said[1..].to_vec(), vec!["systemctl --user -M user@ daemon-reexec"]);

    std::fs::remove_file(&sudoers).unwrap();
    write_stub(&bin.join("visudo"), "#!/bin/sh\nexit 1\n");
    let refused = bash_env(&script, &[("PATH", &path)]);
    assert!(!refused.status.success(), "a sudoers line visudo refuses fails the step");
    assert!(!sudoers.exists(), "a refused sudoers line is never installed");
}

// golden.test.ts:329
#[test]
fn the_builders_created_at_is_taken_before_the_create_so_the_age_a_person_reads_covers_the_whole_prepare() {
    let rec = recording();
    let clock = Manual::new();
    let called_at = Rc::new(std::cell::Cell::new(0i64));
    let hook_clock = clock.clone();
    let hook_called = called_at.clone();
    let mut opts = prepare_opts(&rec, "true");
    opts.backend = Wrap { on_create: Some(Rc::new(move || hook_called.set(hook_clock.now()))), ..Wrap::over(&rec) }.arc();
    opts.deploy_daemon = Some(sleeping_deploy(clock.clone()));
    let builder = prep(opts, &clock);
    let created_at = parse_ms(&builder.created_at).expect("createdAt is an ISO time");
    assert!(created_at <= called_at.get(), "createdAt {created_at} is after the create at {}", called_at.get());
}

// golden.test.ts:341
#[test]
fn a_daemon_hook_that_reports_a_detail_gets_it_on_a_second_deploying_daemon_frame() {
    let rec = recording();
    let clock = Manual::new();
    let st = stage_recorder();
    let mut opts = prepare_opts(&rec, "true");
    opts.deploy_daemon =
        Some(deploy_with(|_m: Arc<dyn Machine>| Box::pin(async { Ok::<Option<String>, EngineError>(Some("node v22.23.2".to_string())) })));
    opts.on_stage = Some(st.on_stage.clone());
    prep(opts, &clock);
    assert_eq!(
        sans_base(&st.all()),
        vec!["creating:sandbox from base", "deploying-daemon", "deploying-daemon:node v22.23.2", "installing-harness", "ready"]
    );
}

// golden.test.ts:348
#[test]
fn prepare_with_kind_desktop_picks_the_desktop_template_and_streams_a_display() {
    let rec = recording();
    let clock = Manual::new();
    let mut opts = prepare_opts(&rec, "true");
    opts.kind = Some(MachineKind::Desktop);
    opts.backend = Wrap::over(&rec).arc();
    let builder = prep(opts, &clock);
    match_object(&rec.state.created()[0], json!({"kind": "desktop", "template": "default"}));
    assert_eq!(builder.kind, MachineKind::Desktop);
    assert_eq!(builder.machine.stream_url(), Some("wss://fake/stream/m1"));
}

// golden.test.ts:356
#[test]
fn only_the_builder_idles_to_kill_after_a_window_long_enough_for_a_person_the_smoke_fork_keeps_the_provider_default() {
    let rec = recording();
    let clock = Manual::new();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    seal_ok(builder, seal_opts(&rec, "true"), &clock);
    let created = rec.state.created();
    match_object(&created[0], json!({"onIdle": "kill", "idleTimeoutMs": BUILDER_IDLE_MS}));
    const _: () = assert!(BUILDER_IDLE_MS >= 4 * 60 * 60_000);
    match_object(&created[1], json!({"fromSnapshot": "snap_wsp-h1-default-v1"}));
    assert!(created[1].on_idle.is_none());
    assert!(created[1].idle_timeout_ms.is_none());
}

// golden.test.ts:367
#[test]
fn prepare_kills_the_machine_and_reports_failed_when_the_harness_install_fails_with_the_install_s_own_last_line_as_the_reason() {
    let rec = recording_backend(Opts {
        exec: Some(Box::new(|cmd: &str| Some(if cmd.contains("bad install") { exec(1, "", "nope") } else { ok() }))),
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let mut opts = prepare_opts(&rec, "bad install");
    opts.on_stage = Some(st.on_stage.clone());
    let err = run(prepare_builder(opts, &*clock)).err().expect("the prepare fails");
    assert_message_has(&err.message(), "golden setup failed: nope");
    assert_eq!(rec.state.killed(), vec!["m1"]);
    assert_eq!(st.all().last().unwrap(), "failed:golden setup failed: nope");
}

// golden.test.ts:375
#[test]
fn a_setup_the_guard_ended_reads_as_its_timeout_not_as_a_bare_exit_code() {
    let rec = recording_backend(Opts {
        exec: Some(Box::new(|cmd: &str| Some(if cmd.contains("slow install") { exec(124, "", "") } else { ok() }))),
        ..Opts::default()
    });
    let clock = Manual::new();
    let err = run(prepare_builder(prepare_opts(&rec, "slow install"), &*clock)).err().expect("the prepare fails");
    assert_message_has(&err.message(), "golden setup failed: timed out after 900s");
}

// golden.test.ts:380
#[test]
fn seal_snapshots_kills_the_builder_before_the_smoke_fork_boots_and_records_the_kind() {
    let rec = recording();
    let clock = Manual::new();
    let st = stage_recorder();
    let mut po = prepare_opts(&rec, "echo setup");
    po.kind = Some(MachineKind::Desktop);
    po.on_stage = Some(st.on_stage.clone());
    let builder = prep(po, &clock);
    let setup_sha = builder.setup_sha.clone();
    let mut so = seal_opts(&rec, "claude --version");
    so.on_stage = Some(st.on_stage.clone());
    let sealed = seal_ok(builder, so, &clock);
    assert_eq!(rec.state.timeline(), vec!["create m1", "snapshot m1", "kill m1", "create m2", "kill m2"]);
    let created = rec.state.created();
    match_object(&created[1], json!({"kind": "desktop", "fromSnapshot": "snap_wsp-h1-default-v1"}));
    assert!(created[1].template.is_none());
    match_object(
        &sealed.version,
        json!({"version": 1, "kind": "desktop", "baseTemplate": "default", "snapshotId": "snap_wsp-h1-default-v1"}),
    );
    assert!(sealed.version.template_id.is_none());
    assert_eq!(sealed.version.setup_sha, setup_sha);
    assert_eq!(sealed.manifest.head.0, 1.0);
    assert_eq!(
        sans_base(&st.all()),
        vec![
            "creating:desktop from default",
            "deploying-daemon",
            "installing-harness",
            "ready",
            "snapshotting:syncing the disk",
            "snapshotting:snapshotting, usually under a minute",
            "smoke-forking:claude --version",
            "smoke-forking:1 agent answers: Claude Code",
            "sealed:v1",
        ]
    );
}

// golden.test.ts:398
#[test]
fn the_smoke_stage_ends_on_what_the_checks_proved_not_on_the_last_line_an_agent_printed() {
    let smoke = "claude --version && hermes --version";
    let banner = "Update available: 5343 commits behind, run 'hermes update'";
    let rec = recording_backend(Opts {
        exec_results: results(vec![(smoke, exec(0, &format!("2.1.263 (Claude Code)\n{banner}\n"), ""))]),
        stream: true,
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, smoke);
    so.on_stage = Some(st.on_stage.clone());
    seal_ok(builder, so, &clock);
    let all = st.all();
    let forking: Vec<String> = all.iter().filter(|s| s.starts_with("smoke-forking:")).cloned().collect();
    assert!(forking.contains(&format!("smoke-forking:{banner}")), "{forking:?}");
    assert_eq!(forking.last().unwrap(), "smoke-forking:2 agents answer: Claude Code, Hermes Agent");
    assert_eq!(all.last().unwrap(), "sealed:v1");
}

// golden.test.ts:411
#[test]
fn smoke_tally_of_each_check_line_is_the_words_the_stage_ends_on() {
    let rows: [(&str, &str); 5] = [
        ("true", "no agent to check; the fork booted"),
        ("codex --version", "1 agent answers: Codex"),
        ("aider --version && node --version", "1 agent answers: Aider; node runs"),
        ("pnpm --version", "no agent to check; pnpm runs"),
        ("claude --version && pnpm --version && bun --version", "1 agent answers: Claude Code; pnpm, bun run"),
    ];
    for (smoke, want) in rows {
        assert_eq!(smoke_tally(smoke), want, "smokeTally({smoke:?})");
    }
}

// golden.test.ts:421
#[test]
fn seal_records_whether_the_image_carries_the_browser_shim_read_on_the_smoke_fork() {
    let with_shim = recording_backend(Opts { exec: Some(Box::new(|_cmd: &str| Some(exec(0, "", "")))), ..Opts::default() });
    let clock = Manual::new();
    let b1 = prep(prepare_opts(&with_shim, "true"), &clock);
    assert_eq!(seal_ok(b1, seal_opts(&with_shim, "true"), &clock).version.browser_shim, Some(true));

    let without = recording_backend(Opts {
        exec: Some(Box::new(|cmd: &str| Some(exec(if cmd == "test -x /usr/local/bin/wsp-open" { 1 } else { 0 }, "", "")))),
        ..Opts::default()
    });
    let b2 = prep(prepare_opts(&without, "true"), &clock);
    assert_eq!(seal_ok(b2, seal_opts(&without, "true"), &clock).version.browser_shim, Some(false));
}

/// golden.test.ts:431 `sealed(bin)`: a builder whose npm folder answers `bin`, sealed with `pnpm --version`.
fn sealed_npm(bin: &str) -> (SealResult, Rec, String) {
    let folder = bin.to_string();
    let rb = recording_backend(Opts {
        exec: Some(Box::new(move |cmd: &str| Some(if cmd.contains("npm prefix -g") { exec(0, &format!("{folder}\n"), "") } else { ok() }))),
        ..Opts::default()
    });
    let clock = Manual::new();
    let b = prep(prepare_opts(&rb, "true"), &clock);
    let builder_id = b.machine.id().to_string();
    let sealed = seal_ok(b, seal_opts(&rb, "pnpm --version"), &clock);
    (sealed, rb, builder_id)
}

// golden.test.ts:431
#[test]
fn seal_reads_npms_own_folder_once_on_the_builder_and_puts_it_first_on_the_image_s_profile_and_its_forks_path_where_the_tools_path_names_no_such_folder(
) {
    let nvm = "/opt/nvm/versions/node/v24.18.1/bin";
    let (off, rb, builder) = sealed_npm(nvm);
    assert_eq!(off.version.npm_bin.as_deref(), Some(nvm));
    let reads: Vec<(String, String)> = rb
        .state
        .inline
        .borrow()
        .iter()
        .filter(|(_, cmd, _)| cmd.contains("npm prefix -g"))
        .map(|(id, cmd, _)| (id.clone(), cmd.clone()))
        .collect();
    assert_eq!(reads.len(), 1, "the npm folder is read once");
    assert_eq!(reads[0].0, builder);
    assert!(reads[0].1.contains(r#"cd / && b="$(npm_config_userconfig=/dev/null npm prefix -g 2>/dev/null)/bin""#), "{}", reads[0].1);
    let on_builder: Vec<String> =
        rb.state.inline.borrow().iter().filter(|(id, _, _)| *id == builder).map(|(_, cmd, _)| cmd.clone()).collect();
    let want_profile = format!("{} && {}", claude_link_line(), profile_path_line(&format!("{AGENTS_BIN}:{nvm}:{TOOLS_PATH}")));
    assert!(on_builder.contains(&want_profile), "{on_builder:?}");
    let fork_path = |rb: &Rec| rb.state.created().last().unwrap().envs.as_ref().and_then(|e| e.get("PATH").cloned());
    assert_eq!(fork_path(&rb), Some(format!("{AGENTS_BIN}:{nvm}:{TOOLS_PATH}")));

    for bin in ["/usr/local/bin", "/root/.p/bin", ""] {
        let (on, rb2, _) = sealed_npm(bin);
        assert!(on.version.npm_bin.is_none(), "npmBin for {bin:?}");
        assert!(!rb2.state.inline.borrow().iter().any(|(_, cmd, _)| cmd.contains(PROFILE_PATH_FILE)), "no profile line for {bin:?}");
        assert!(fork_path(&rb2).is_none(), "no fork PATH for {bin:?}");
    }
}

// golden.test.ts:458
#[test]
fn the_image_s_path_answers_the_pinned_claude_ahead_of_npm_s_and_npm_s_pnpm_ahead_of_the_provider_s_outside_any_checkout() {
    let root = tempfile::tempdir().unwrap();
    let r = root.path().display().to_string();
    let npm = format!("{r}/nvm/bin");
    let usr = format!("{r}/usr/local/bin");
    std::fs::create_dir_all(&usr).unwrap();
    std::fs::create_dir_all(&npm).unwrap();
    std::fs::create_dir_all(format!("{r}/etc/profile.d")).unwrap();
    // The provider's pnpm and the agents stage's native claude share /usr/local/bin; npm's folder holds a newer claude.
    write_stub(std::path::Path::new(&format!("{usr}/claude")), &echo_script("2.1.280 (Claude Code)"));
    write_stub(std::path::Path::new(&format!("{usr}/pnpm")), &echo_script("10.34.5"));
    write_stub(std::path::Path::new(&format!("{npm}/claude")), &echo_script("2.1.293 (Claude Code)"));
    write_stub(std::path::Path::new(&format!("{npm}/pnpm")), &echo_script("11.9.0"));
    let (r2, npm2) = (r.clone(), npm.clone());
    let rb = recording_backend(Opts {
        exec: Some(Box::new(move |cmd: &str| {
            if cmd.contains("npm prefix -g") {
                return Some(exec(0, &format!("{npm2}\n"), ""));
            }
            if !cmd.contains(AGENTS_BIN) {
                return Some(ok());
            }
            Some(exec_of(&bash_inherit(&relocate(&r2, cmd))))
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let b = prep(prepare_opts(&rb, "true"), &clock);
    seal_ok(b, seal_opts(&rb, "claude --version"), &clock);

    let outside =
        |pre: &str, env: &[(&str, &str)]| stdout_text(&bash_env(&format!("{pre}cd / && claude --version && pnpm --version"), env));
    // A login shell reads the profile the seal wrote; a thread gets the PATH its fork was created with.
    assert_eq!(outside(&format!(". {r}{PROFILE_PATH_FILE} && "), &[]), "2.1.280 (Claude Code)\n11.9.0\n");
    let fork_path = relocate(&r, &rb.state.created().last().unwrap().envs.as_ref().unwrap()["PATH"]);
    let head: Vec<String> = fork_path.split(':').take(3).map(str::to_string).collect();
    assert_eq!(head, vec![format!("{r}{AGENTS_BIN}"), npm.clone(), "/root/.local/bin".to_string()]);
    assert_eq!(outside("", &[("PATH", &fork_path)]), "2.1.280 (Claude Code)\n11.9.0\n");
}

// golden.test.ts:496
#[test]
fn a_ticked_cursor_or_hermes_gets_no_link_in_the_pinned_agents_folder_so_the_provider_s_shim_at_its_name_never_finds_itself_first_on_a_fork_s_path(
) {
    let root = tempfile::tempdir().unwrap();
    let r = root.path().display().to_string();
    let usr = format!("{r}/usr/local/bin");
    std::fs::create_dir_all(&usr).unwrap();
    std::fs::create_dir_all(format!("{r}/adopted")).unwrap();
    std::fs::create_dir_all(format!("{r}/etc/profile.d")).unwrap();
    write_stub(std::path::Path::new(&format!("{usr}/claude")), &echo_script("2.1.280 (Claude Code)"));
    // The provider's boot shim: it looks its name up on the caller's PATH past /usr/local/bin, else runs the install it adopted.
    for name in ["cursor-agent", "hermes"] {
        write_stub(std::path::Path::new(&format!("{r}/adopted/{name}")), &echo_script(&format!("wsp install of {name}")));
        let shim = format!(
            "#!/bin/bash\np=$(printf %s \"$PATH\" | tr : '\\n' | grep -vx '{usr}' | paste -sd:)\nif t=$(PATH=\"$p\" command -v {name}); then exec \"$t\" \"$@\"; fi\nexec '{r}/adopted/{name}' \"$@\"\n"
        );
        write_stub(std::path::Path::new(&format!("{usr}/{name}")), &shim);
    }
    let (r2, npm) = (r.clone(), format!("{r}/nvm/bin"));
    let rb = recording_backend(Opts {
        exec: Some(Box::new(move |cmd: &str| {
            if cmd.contains("npm prefix -g") {
                return Some(exec(0, &format!("{npm}\n"), ""));
            }
            if cmd == "echo ok" {
                return Some(reach_ok());
            }
            if !cmd.contains(AGENTS_BIN) {
                return Some(ok());
            }
            Some(exec_of(&bash_inherit(&relocate(&r2, cmd))))
        })),
        ..Opts::default()
    });
    let recipe: RecipeDigest = serde_json::from_value(json!({
        "ticks": [{"id": "agents/claude"}, {"id": "agents/cursor"}, {"id": "agents/hermes"}],
        "files": []
    }))
    .expect("a recipe digest");
    let clock = Manual::new();
    let mut po = prepare_opts(&rb, "true");
    po.import = Some(GoldenImport { recipe_hash: "h1".to_string(), recipe: Some(recipe), ..Default::default() });
    let b = prep(po, &clock);
    seal_ok(b, seal_opts(&rb, "claude --version"), &clock);
    let fork_path = relocate(&r, &rb.state.created().last().unwrap().envs.as_ref().unwrap()["PATH"]);
    let run_fork = |cmd: &str| bash_env(&format!("cd / && {cmd}"), &[("PATH", &fork_path)]);
    assert_eq!(stdout_text(&run_fork("claude --version")), "2.1.280 (Claude Code)\n");
    for name in ["cursor-agent", "hermes"] {
        let out = run_fork(&format!("{name} --version"));
        assert_eq!(out.status.code(), Some(0), "{name}");
        assert_eq!(stdout_text(&out), format!("wsp install of {name}\n"), "{name}");
    }
    let mut listed: Vec<String> =
        std::fs::read_dir(format!("{r}{AGENTS_BIN}")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    listed.sort();
    assert_eq!(listed, vec!["claude"]);
}

// golden.test.ts:534
#[test]
fn npm_s_folder_is_read_past_a_prefix_an_npmrc_in_the_home_names_a_workspace_planting_one_there_moves_nothing() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path().display().to_string();
    std::fs::create_dir_all(format!("{h}/.p/bin")).unwrap();
    std::fs::write(format!("{h}/.npmrc"), format!("prefix={h}/.p\n")).unwrap();
    let node = dir_of_tool("node");
    // The real npm does take the planted prefix when its user npmrc is read.
    let real = bash_env(&format!("export PATH={node}:$PATH; cd / && npm prefix -g"), &[("HOME", &h)]);
    assert_eq!(stdout_text(&real).trim(), format!("{h}/.p"));
    let machine = Shell { home: h.clone(), node_dir: node.clone() };
    let bin = run(npm_bin_of(&machine)).expect("the npm folder is read");
    assert!(bin.as_deref().is_none_or(|b| !b.starts_with(&h)), "{bin:?}");
}

// golden.test.ts:554
#[test]
fn seal_reads_the_vault_off_the_builder_before_it_asks_for_the_snapshot_and_hands_it_back_with_the_version() {
    let paths = vec!["/root/.config/gh/hosts.yml".to_string(), "/etc/profile.d/wsp-secrets.sh".to_string()];
    let order: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let (o_exec, o_snap) = (order.clone(), order.clone());
    let p_exec = paths.clone();
    let rec = recording_backend(Opts {
        exec: Some(Box::new(move |cmd: &str| {
            if cmd.starts_with("for p in ") && cmd.contains(&p_exec[0]) {
                o_exec.borrow_mut().push("vault".to_string());
                return Some(exec(0, &format!("{}\n", p_exec.join("\n")), ""));
            }
            Some(ok())
        })),
        snapshot: Some(Box::new(move |_id: &str, _nth: u32| -> Result<(), EngineError> {
            o_snap.borrow_mut().push("snapshot".to_string());
            Ok(())
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let tgz = vault_tgz();
    let mut so = seal_opts(&rec, "true");
    so.vault_paths = Some(paths.clone());
    so.fetch = Some(fetch_of(tgz.clone()));
    let sealed = seal_ok(b, so, &clock);
    assert_eq!(*order.borrow(), vec!["vault", "snapshot"]);
    assert!(rec.state.timeline().iter().any(|t| t == "snapshot m1"));
    let vault = sealed.vault.as_ref().expect("the vault comes back");
    assert_eq!(vault.paths, 2);
    assert_eq!(vault.sha256, sha256_hex(&tgz));
    assert!(vault.tar == tgz);
}
