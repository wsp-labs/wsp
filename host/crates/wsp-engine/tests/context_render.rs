// SPDX-License-Identifier: AGPL-3.0-only
//! packages/engine/test/machine-context.test.ts: "the document", "the document's base line", "each agent's hooks" and
//! "applyMachineContext on a local guest", over fixtures and a guest that runs its commands under this box's bash.
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexSet;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use wsp_catalog::catalog::AgentEntry;
use wsp_catalog::context::ContextHooks;
use wsp_engine::errors::{EngineError, ErrorKind, RoadError, WspError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::golden_seam::{ImportResult, ImportRow, EXEC_ENV};
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::machine_context::{
    agent_files, apply_machine_context, context_agents, context_line, context_path, render_machine_context,
    render_short_context, render_skill, skill_path, Alias, AliasesUnread, ApplyContextOptions, BuildFacts,
    ContextInput, ContextOutcomeKind, ContextProbe, ContextResult, Disk, FileNote, GoldenPick, GuestRoots,
    MissingItem, WorkspaceName, CONTEXT_MARKER, SERVERS_OFF_LINE, SKILL_DESCRIPTION, SKILL_NAME,
};
use wsp_words::words::thread::{background_work_words, ANOTHER_AGENT_WORDS, BACKGROUND_WORK_WORDS, TURN_END_WORDS};
use wsp_words::words::units::fmt_bytes;
use wsp_wire::types::run_block::RUN_BLOCK_WORDS;

type Agent = &'static AgentEntry;

fn facts() -> BuildFacts {
    BuildFacts {
        tools: vec![MissingItem { id: "tools/brew-cask/raycast".into(), label: "raycast".into(), note: "macOS app, no Linux build".into() }],
        agents: vec![MissingItem { id: "agents/zed".into(), label: "Zed".into(), note: "no installer known".into() }],
        files: vec![FileNote { path: "~/.ssh/id_ed25519".into(), note: "private key, never copied".into() }],
    }
}

fn probe_out() -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&facts()).unwrap());
    [
        "some rc noise", "WSP_CTX", "KERNEL 6.6.30", "DISK 20466256 11720704", "OVERLAY no", "HAS tmux", "HAS fish", "HAS brew",
        "HAS golden-path", "HAS wsp-open", "VERSION python3: Python 3.12.13", "VERSION uv: uv 0.12.9", "VERSION cc: ",
        "AGENT claude", "AGENT codex", "AGENT gemini", "AGENT opencode", "AGENT pi", "AGENT hermes", "SECRET OPENAI_API_KEY",
        "SECRET GH_TOKEN", "SHELL zsh", "ALIAS o open", "ALIAS rc code", &format!("FACTS {b64}"), "WSP_CTX_END", "",
    ]
    .join("\n")
}

fn set(names: &[&str]) -> IndexSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

fn all_ids() -> IndexSet<String> {
    context_agents().iter().map(|a| a.id.clone()).collect()
}

fn probe_of(over: impl FnOnce(&mut ContextProbe)) -> ContextProbe {
    let mut p = ContextProbe {
        kernel: Some("6.6.30".into()),
        disk: Some(Disk { size_bytes: 20466256.0 * 1024.0, free_bytes: 11720704.0 * 1024.0 }),
        overlay: false,
        has: set(&["tmux", "fish", "brew", "golden-path", "wsp-open"]),
        versions: vec![],
        agents: context_agents().iter().map(|a| a.id.clone()).collect(),
        secrets: vec!["OPENAI_API_KEY".into(), "GH_TOKEN".into()],
        shell: "zsh".into(),
        aliases: vec![Alias { name: "o".into(), word: "open".into() }, Alias { name: "rc".into(), word: "code".into() }],
        aliases_unread: None,
        conflicts: IndexSet::new(),
        facts: None,
    };
    over(&mut p);
    p
}

fn ctx(id: &str) -> Agent {
    context_agents().into_iter().find(|a| a.id == id).unwrap_or_else(|| panic!("no context agent {id}"))
}

fn golden() -> GoldenPick {
    GoldenPick { version: 3, created_at: "2026-09-05T14:02:11.000Z".into(), setup_sha: "9f2a7c1d4e5b6a7f8091a2b3c4d5e6f7".into(), npm_bin: None }
}

fn task_input(agent: Option<Agent>) -> ContextInput {
    ContextInput { workspace: Some(WorkspaceName { name: "task-1".into() }), golden: Some(golden()), probe: probe_of(|_| {}), facts: facts(), agent, at_provider: false }
}

fn bare_input(workspace: Option<&str>, probe: ContextProbe, facts: BuildFacts) -> ContextInput {
    ContextInput { workspace: workspace.map(|name| WorkspaceName { name: name.into() }), golden: None, probe, facts, agent: None, at_provider: false }
}

fn empty_facts() -> BuildFacts {
    BuildFacts { tools: vec![], agents: vec![], files: vec![] }
}

fn has_em_dash(s: &str) -> bool {
    s.contains('\u{2014}')
}

/// machine-context.test.ts:204. The document for a workspace on a sealed golden.
#[test]
fn renders_a_workspace_on_a_sealed_golden() {
    let doc = render_machine_context(&task_input(None));
    assert!(doc.starts_with(&format!("{CONTEXT_MARKER}\n")), "{doc}");
    assert!(doc.contains("- Workspace: task-1."));
    assert!(doc.contains("- Golden: v3, sealed 2026-09-05, setup 9f2a7c1d4e5b."));
    assert!(doc.contains("- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free"));
    assert!(doc.contains("- Every agent session starts in the thread's folder; terminal panes open in the home folder."));
    assert!(doc.contains("- Work in a folder: cd <dir> && <cmd> on one line, or absolute paths."));
    assert!(doc.contains(&format!("- {TURN_END_WORDS}; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. {BACKGROUND_WORK_WORDS}. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.")));
    assert!(doc.contains(&format!("- {ANOTHER_AGENT_WORDS}.")));
    assert!(doc.contains(&format!("- {RUN_BLOCK_WORDS}.")));
    assert!(!doc.contains("does not move the thread"));
    assert!(!doc.contains("persists across"));
    let claude = render_machine_context(&task_input(Some(ctx("claude"))));
    assert!(claude.contains("- Every agent session starts in the thread's folder; terminal panes open in the home folder. A cd moves your own shell, which persists across your tool calls, not the thread's folder, and the files pane follows that shell's folder unless you pinned the panes."));
    assert!(claude.contains("- Work in a folder: cd <dir> in your shell and it stays there across your tool calls; the thread's folder does not move. Absolute paths work from anywhere."));
    assert!(!claude.contains("open in the home folder.\n"));
    assert_eq!(render_machine_context(&task_input(Some(ctx("codex")))), doc);
    assert!(doc.contains("- Sign-ins go through wsp: BROWSER is /usr/local/bin/wsp-open"));
    assert!(doc.contains("run wsp-open <url>"));
    assert!(doc.contains("- Ask for a sign-in: run the tool's own login command"));
    assert!(doc.contains("- Secrets set in /etc/profile.d/wsp-secrets.sh: OPENAI_API_KEY, GH_TOKEN."));
    assert!(doc.contains("- Tools that did not install: raycast (macOS app, no Linux build)."));
    assert!(doc.contains("- Aliases whose command is not here: o runs open; rc runs code."));
    assert!(doc.contains("Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed."));
    assert!(doc.contains("or tmux new -d -s <name> '<cmd>'"));
    assert!(!has_em_dash(&doc));
    assert_eq!(doc, SNAP_DOC_WORKSPACE);
}

/// machine-context.test.ts:234
#[test]
fn says_the_aliases_were_not_read_where_no_shell_was_opened_rather_than_none() {
    let probe = probe_of(|p| {
        p.aliases = vec![];
        p.aliases_unread = Some(AliasesUnread::Shared);
    });
    let doc = render_machine_context(&bare_input(Some("box-1"), probe, facts()));
    assert!(doc.contains("- Aliases: not read here. This computer's home is shared with every workspace on it, so wsp opens no login shell on it."));
    assert!(!doc.contains("- Aliases whose command is not here:"));
}

/// machine-context.test.ts:240
#[test]
fn renders_a_builder_before_the_seal_with_nothing_missing_and_no_tmux_brew_or_fish() {
    let probe = probe_of(|p| {
        p.has = IndexSet::new();
        p.overlay = true;
        p.secrets = vec![];
        p.aliases = vec![];
        p.disk = None;
        p.kernel = None;
    });
    let doc = render_machine_context(&bare_input(None, probe, empty_facts()));
    assert!(doc.contains("- This machine is a golden builder, not a workspace yet."));
    assert!(doc.contains("- Golden: not sealed yet."));
    assert!(doc.contains("- Disk: size unknown when this file was written."));
    assert!(doc.contains("- Secrets set in /etc/profile.d/wsp-secrets.sh: none."));
    assert!(doc.contains("- Tools that did not install: none."));
    assert!(doc.contains("- Aliases whose command is not here: none."));
    assert!(doc.contains("- Docker and Podman are not installed."));
    assert!(!doc.contains("overlayfs"));
    assert!(!doc.contains("tmux"));
    assert!(!doc.contains("Homebrew"));
    assert!(!doc.contains("wsp-secrets.fish"));
    assert!(!doc.contains("wsp-open"));
    assert!(!doc.contains("sign-in"));
    assert!(!doc.contains("Sign-ins"));
    assert!(doc.contains("- Linux, user root, home /root."));
    assert_eq!(doc, SNAP_DOC_BUILDER);
}

/// machine-context.test.ts:261
#[test]
fn the_short_always_loaded_text_carries_the_facts_an_agent_must_not_get_wrong_and_points_at_the_skill() {
    let short = render_short_context(&task_input(None));
    assert!(short.starts_with(&format!("{CONTEXT_MARKER}\n")), "{short}");
    assert!(short.contains("The wsp-machine skill has the full picture"));
    assert!(short.contains("- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'."));
    assert!(!short.contains("dies when that tool call ends"));
    let claude = render_short_context(&task_input(Some(ctx("claude"))));
    assert!(claude.contains(&background_work_words(Some("run_in_background on the Bash tool"))));
    assert!(render_short_context(&task_input(Some(ctx("codex")))).contains(BACKGROUND_WORK_WORDS));
    assert!(short.contains("- Every agent session starts in the thread's folder; terminal panes open in the home folder."));
    assert!(short.contains(&format!("- {TURN_END_WORDS}; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. {BACKGROUND_WORK_WORDS}. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.")));
    assert!(short.contains(&format!("- {ANOTHER_AGENT_WORDS}.")));
    assert!(short.contains(&format!("- {RUN_BLOCK_WORDS}.")));
    assert!(!short.contains("lasts for that command only"));
    assert!(!short.contains("persists across"));
    assert!(claude.contains("- Every agent session starts in the thread's folder; terminal panes open in the home folder. A cd moves your own shell, which persists across your tool calls, not the thread's folder, and the files pane follows that shell's folder unless you pinned the panes."));
    assert_eq!(render_short_context(&task_input(Some(ctx("gemini")))), short);
    assert!(short.contains("- Sign-ins go through wsp: run the tool's own login command"));
    assert!(short.contains("bind 0.0.0.0, not 127.0.0.1"));
    assert!(short.contains("- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed."));
    assert!(short.contains("- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free."));
    assert!(short.contains("- Secrets are exported by /etc/profile.d/wsp-secrets.sh. Use them by name ($NAME); never print, log or commit a value"));
    assert!(!short.contains("OPENAI_API_KEY"));
    assert!(!short.contains("raycast"));
    assert!(short.split('\n').count() < 16, "{short}");
    assert_eq!(short, SNAP_SHORT);
    let bare_probe = probe_of(|p| {
        p.has = IndexSet::new();
        p.overlay = true;
        p.disk = None;
    });
    let bare = render_short_context(&bare_input(None, bare_probe, facts()));
    assert!(!bare.contains("tmux"));
    assert!(bare.contains("- Docker and Podman are not installed."));
    assert!(bare.contains("- Disk: size unknown when this file was written. wsp keeps 2 GB free."));
    assert!(!bare.contains("Sign-ins"));
}

/// machine-context.test.ts:295
#[test]
fn a_machine_at_a_provider_says_servers_needing_the_persons_sign_in_do_not_run_there_and_no_other_does() {
    let input = task_input(Some(ctx("claude")));
    let at_provider = ContextInput { at_provider: true, ..input.clone() };
    for render in [render_short_context, render_machine_context] {
        assert!(render(&at_provider).split('\n').any(|l| l == SERVERS_OFF_LINE), "{}", render(&at_provider));
        assert!(!render(&input).contains(SERVERS_OFF_LINE));
    }
    assert!(SERVERS_OFF_LINE.contains("not available on cloud machines"));
    assert!(SERVERS_OFF_LINE.contains("do not suggest signing it in here (no /mcp, no mcp login command)"));
}

/// machine-context.test.ts:305
#[test]
fn the_skill_is_the_full_document_under_a_name_and_a_one_line_description_every_parser_reads() {
    let doc = render_machine_context(&task_input(None));
    let skill = render_skill(&doc);
    assert_eq!(skill, format!("---\nname: wsp-machine\ndescription: {SKILL_DESCRIPTION}\n---\n\n{doc}"));
    assert!(SKILL_DESCRIPTION.chars().count() < 1024);
    assert!(!SKILL_DESCRIPTION.chars().any(|c| matches!(c, '\n' | ':' | '"' | '#' | '\u{2014}')), "{SKILL_DESCRIPTION}");
    assert!((1..=64).contains(&SKILL_NAME.len()) && SKILL_NAME.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'), "{SKILL_NAME}");
}

/// machine-context.test.ts:316
#[test]
fn names_the_base_floors_versions_when_the_probe_read_them_and_says_nothing_when_it_read_none() {
    use wsp_engine::golden_seam::GoldenBaseTool;
    let versions = vec![
        GoldenBaseTool { name: "node".into(), version: "22.23.2".into() },
        GoldenBaseTool { name: "python3".into(), version: "3.12.13".into() },
        GoldenBaseTool { name: "unzip".into(), version: "6.00".into() },
    ];
    let doc = render_machine_context(&bare_input(None, probe_of(|p| p.versions = versions), facts()));
    assert!(doc.contains("- On every wsp machine: node 22.23.2, python3 3.12.13, unzip 6.00."));
    assert!(!render_machine_context(&bare_input(None, probe_of(|_| {}), facts())).contains("On every wsp machine"));
    let brew = probe_of(|p| p.has = set(&["brew"]));
    assert!(render_machine_context(&bare_input(None, brew, facts())).contains("- Homebrew is at /home/linuxbrew/.linuxbrew."));
}

fn paths(out: &ContextHooks) -> Vec<&str> {
    out.files.iter().map(|f| f.path.as_str()).collect()
}

fn content<'a>(out: &'a ContextHooks, path: &str) -> &'a str {
    &out.files.iter().find(|f| f.path == path).unwrap_or_else(|| panic!("no file {path} in {:?}", paths(out))).content
}

fn short_for(agent: Option<Agent>) -> String {
    render_short_context(&task_input(agent))
}

fn skill_for(agent: Option<Agent>) -> String {
    render_skill(&render_machine_context(&task_input(agent)))
}

fn hooks(agent: Agent, short: &str, skill: &str, probe: &ContextProbe) -> ContextHooks {
    agent_files(agent, short, skill, probe, None)
}

/// machine-context.test.ts:334
#[test]
fn writes_the_short_text_through_each_hook_and_the_skill_into_each_global_skills_directory() {
    let short = short_for(None);
    let skill = skill_for(None);
    let out: Vec<(Agent, ContextHooks)> =
        context_agents().into_iter().map(|a| (a, hooks(a, &short_for(Some(a)), &skill_for(Some(a)), &probe_of(|_| {})))).collect();
    let of = |id: &str| &out.iter().find(|(a, _)| a.id == id).unwrap().1;
    for (a, o) in &out {
        assert_eq!(o.outcome, ContextOutcomeKind::Written, "{}", a.id);
    }
    assert_eq!(paths(of("claude")), ["/etc/claude-code/CLAUDE.md", "/etc/claude-code/.claude/skills/wsp-machine/SKILL.md"]);
    assert_eq!(paths(of("codex")), ["/etc/codex/requirements.toml", "/etc/codex/skills/wsp-machine/SKILL.md"]);
    assert_eq!(paths(of("gemini")), ["/etc/gemini-cli/system-defaults.json", "/root/.gemini/WSP-MACHINE.md", "/root/.gemini/skills/wsp-machine/SKILL.md"]);
    assert_eq!(paths(of("opencode")), ["/etc/opencode/opencode.json", "/root/.config/opencode/skills/wsp-machine/SKILL.md"]);
    assert_eq!(paths(of("pi")), ["/root/.pi/agent/APPEND_SYSTEM.md", "/root/.pi/agent/skills/wsp-machine/SKILL.md"]);
    assert_eq!(paths(of("hermes")), ["/etc/profile.d/wsp-machine.sh", "/etc/fish/conf.d/wsp-machine.fish", "/root/.hermes/skills/wsp-machine/SKILL.md"]);
    for (a, o) in &out {
        for f in &o.files {
            assert_eq!(f.mode, 0o644, "{} {}", a.id, f.path);
        }
        let last = o.files.last().unwrap();
        assert_eq!(o.skill, last.path);
        let want = if a.id == "claude" { skill_for(Some(ctx("claude"))) } else { skill.clone() };
        assert_eq!(last.content, want, "{}", a.id);
        let first = &o.files[0].path;
        let want_path = if first == "/etc/gemini-cli/system-defaults.json" { "/root/.gemini/WSP-MACHINE.md" } else { first.as_str() };
        assert_eq!(o.path.as_deref(), Some(want_path), "{}", a.id);
        let joined = o.files.iter().map(|f| format!("{}\n{}", f.path, f.content)).collect::<Vec<_>>().join("\n----\n");
        let snap = match a.id.as_str() {
            "claude" => SNAP_HOOKS_CLAUDE,
            "codex" => SNAP_HOOKS_CODEX,
            "gemini" => SNAP_HOOKS_GEMINI,
            "opencode" => SNAP_HOOKS_OPENCODE,
            "pi" => SNAP_HOOKS_PI,
            "hermes" => SNAP_HOOKS_HERMES,
            other => panic!("no snapshot for {other}"),
        };
        assert_eq!(joined, snap, "{}", a.id);
    }
    assert_eq!(content(of("claude"), "/etc/claude-code/CLAUDE.md"), short_for(Some(ctx("claude"))));
    assert!(content(of("claude"), "/etc/claude-code/CLAUDE.md").contains("persists across your tool calls"));
    assert_eq!(content(of("gemini"), "/root/.gemini/WSP-MACHINE.md"), short);
    assert_eq!(content(of("pi"), "/root/.pi/agent/APPEND_SYSTEM.md"), short);
}

/// machine-context.test.ts:357
#[test]
fn the_codex_string_is_a_literal_toml_reads_back_under_the_additive_key_and_falls_back_when_the_text_would_end_it() {
    let short = short_for(None);
    let skill = skill_for(None);
    let toml = content(&hooks(ctx("codex"), &short, &skill, &probe_of(|_| {})), "/etc/codex/requirements.toml").to_string();
    let (first, rest) = toml.split_once('\n').expect("a first line");
    assert!(first.starts_with("# "), "{toml}");
    let body = rest.strip_prefix("additional_developer_instructions = '''\n").and_then(|r| r.strip_suffix("'''\n"));
    assert_eq!(body, Some(short.as_str()), "{toml}");
    assert!(!toml.contains("\ndeveloper_instructions"));
    let odd = content(&hooks(ctx("codex"), "a '''b", &skill, &probe_of(|_| {})), "/etc/codex/requirements.toml").to_string();
    assert!(odd.contains(&format!("additional_developer_instructions = {}", serde_json::to_string("a '''b").unwrap())), "{odd}");
}

/// machine-context.test.ts:366
#[test]
fn gemini_and_opencode_get_json_that_names_ours_and_nothing_of_the_persons() {
    let short = short_for(None);
    let skill = skill_for(None);
    let gemini = hooks(ctx("gemini"), &short, &skill, &probe_of(|_| {}));
    let g: serde_json::Value = serde_json::from_str(content(&gemini, "/etc/gemini-cli/system-defaults.json")).unwrap();
    assert_eq!(g, json!({ "context": { "fileName": ["GEMINI.md", "WSP-MACHINE.md"] } }));
    let opencode = hooks(ctx("opencode"), &short, &skill, &probe_of(|_| {}));
    let o: serde_json::Value = serde_json::from_str(content(&opencode, "/etc/opencode/opencode.json")).unwrap();
    assert_eq!(o, json!({ "instructions": ["/etc/wsp/machine-context.md"] }));
}

/// machine-context.test.ts:371
#[test]
fn hermes_reads_the_short_text_into_its_hint_through_fish_too_only_when_fish_is_there() {
    let out = hooks(ctx("hermes"), &short_for(None), &skill_for(None), &probe_of(|p| p.has = IndexSet::new()));
    assert_eq!(paths(&out), ["/etc/profile.d/wsp-machine.sh", "/root/.hermes/skills/wsp-machine/SKILL.md"]);
    assert!(out.files[0].content.contains(r#"export HERMES_ENVIRONMENT_HINT="$(cat /etc/wsp/machine-context.md)""#), "{}", out.files[0].content);
}

/// machine-context.test.ts:377
#[test]
fn falls_back_when_the_persons_own_file_sets_the_key_codex_is_additive_and_never_falls_back() {
    let short = short_for(None);
    let skill = skill_for(None);
    let all = probe_of(|p| p.conflicts = all_ids());
    let codex = hooks(ctx("codex"), &short, &skill, &all);
    assert_eq!(codex.outcome, ContextOutcomeKind::Written);
    assert_eq!(paths(&codex), ["/etc/codex/requirements.toml", "/etc/codex/skills/wsp-machine/SKILL.md"]);
    assert!(!paths(&codex).contains(&"/etc/codex/config.toml"));

    let gemini = hooks(ctx("gemini"), &short, &skill, &all);
    assert_eq!(gemini.outcome, ContextOutcomeKind::Fallback);
    assert_eq!(gemini.path.as_deref(), Some("/root/.gemini/extensions/wsp-machine"));
    assert_eq!(gemini.note.as_deref(), Some("~/.gemini/settings.json sets context.fileName"));
    assert_eq!(
        paths(&gemini),
        ["/root/.gemini/extensions/wsp-machine/gemini-extension.json", "/root/.gemini/extensions/wsp-machine/WSP-MACHINE.md", "/root/.gemini/skills/wsp-machine/SKILL.md"]
    );
    let ext: serde_json::Value = serde_json::from_str(content(&gemini, "/root/.gemini/extensions/wsp-machine/gemini-extension.json")).unwrap();
    assert_eq!(ext, json!({ "name": "wsp-machine", "version": "1.0.0", "contextFileName": "WSP-MACHINE.md" }));
    assert_eq!(content(&gemini, "/root/.gemini/extensions/wsp-machine/WSP-MACHINE.md"), short);
    assert!(!paths(&gemini).contains(&"/etc/gemini-cli/system-defaults.json"));

    let pi = hooks(ctx("pi"), &short, &skill, &all);
    assert_eq!(pi.outcome, ContextOutcomeKind::Fallback);
    assert_eq!(pi.path.as_deref(), Some("/root/.pi/agent/extensions/wsp-machine.ts"));
    assert_eq!(pi.note.as_deref(), Some("~/.pi/agent/APPEND_SYSTEM.md already exists"));
    assert_eq!(paths(&pi), ["/root/.pi/agent/extensions/wsp-machine.ts", "/root/.pi/agent/skills/wsp-machine/SKILL.md"]);
    let pi_ext = content(&pi, "/root/.pi/agent/extensions/wsp-machine.ts");
    assert!(pi_ext.contains(r#"pi.on("before_agent_start""#));
    assert!(pi_ext.contains(r#"readFileSync("/etc/wsp/machine-context.md", "utf8")"#));
    assert_eq!(pi_ext, SNAP_PI_EXTENSION);

    let hermes = hooks(ctx("hermes"), &short, &skill, &all);
    assert_eq!(hermes.outcome, ContextOutcomeKind::NotLoaded);
    assert_eq!(hermes.note.as_deref(), Some("~/.hermes/config.yaml sets agent.environment_hint"));
    assert_eq!(paths(&hermes), ["/root/.hermes/skills/wsp-machine/SKILL.md"]);

    assert_eq!(hooks(ctx("claude"), &short, &skill, &all).outcome, ContextOutcomeKind::Written);
    assert_eq!(hooks(ctx("opencode"), &short, &skill, &all).outcome, ContextOutcomeKind::Written);

    let line = context_line(&ContextResult {
        agent: "claude".into(),
        outcome: ContextOutcomeKind::Written,
        path: Some("/etc/claude-code/CLAUDE.md".into()),
        note: None,
        skill: "/etc/claude-code/.claude/skills/wsp-machine/SKILL.md".into(),
    });
    assert_eq!(line, "context.claude: /etc/claude-code/CLAUDE.md; skill /etc/claude-code/.claude/skills/wsp-machine/SKILL.md");
    let line = context_line(&ContextResult {
        agent: "gemini".into(),
        outcome: ContextOutcomeKind::Fallback,
        path: gemini.path.clone(),
        note: gemini.note.clone(),
        skill: gemini.skill.clone(),
    });
    assert_eq!(line, "context.gemini: fallback /root/.gemini/extensions/wsp-machine, ~/.gemini/settings.json sets context.fileName; skill /root/.gemini/skills/wsp-machine/SKILL.md");
    let line = context_line(&ContextResult {
        agent: "hermes".into(),
        outcome: ContextOutcomeKind::NotLoaded,
        path: None,
        note: hermes.note.clone(),
        skill: hermes.skill.clone(),
    });
    assert_eq!(line, "context.hermes: not loaded, ~/.hermes/config.yaml sets agent.environment_hint; only the wsp-machine skill's description is in the prompt; skill /root/.hermes/skills/wsp-machine/SKILL.md");
}

/// machine-context.test.ts:434
#[test]
fn never_names_the_two_paths_that_would_replace_a_file_of_the_persons() {
    let short = short_for(None);
    let skill = skill_for(None);
    for a in context_agents() {
        for probe in [probe_of(|_| {}), probe_of(|p| p.conflicts = all_ids())] {
            for f in hooks(a, &short, &skill, &probe).files {
                assert!(!f.path.ends_with(".codex/AGENTS.override.md") && !f.path.ends_with(".config/opencode/AGENTS.md"), "{}: {}", a.id, f.path);
            }
        }
    }
}

/// The fake exec refuses a request body over this many bytes, as the provider did on a six-agent golden.
const EXEC_BODY_CAP: usize = 16 * 1024;

fn body_bytes(cmd: &str) -> usize {
    json!({ "cmd": "bash", "args": ["-c", format!("{EXEC_ENV}\n{cmd}")], "timeoutMs": 20_000 }).to_string().len()
}

async fn bash(script: &str) -> ExecResult {
    let out = tokio::process::Command::new("bash").arg("-c").arg(script).output().await.expect("bash runs");
    ExecResult {
        exit_code: out.status.code().unwrap_or(1) as i64,
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

struct Put {
    path: String,
    bytes: usize,
}

/// The upload road's server role, in process: an upload lands at the path the URL names, or is refused over `cap`.
struct UploadRoad {
    cap: usize,
    puts: Mutex<Vec<Put>>,
}

#[async_trait]
impl Fetch for UploadRoad {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let url = url::Url::parse(&req.url).expect("an upload url");
        let path = url.query_pairs().find(|(k, _)| k == "path").map(|(_, v)| v.into_owned()).expect("a path in the url");
        let body = req.body.unwrap_or_default();
        self.puts.lock().unwrap().push(Put { path: path.clone(), bytes: body.len() });
        if body.len() > self.cap {
            let said = json!({ "error": "Payload Too Large", "limit": self.cap }).to_string();
            return Ok(FetchResponse { status: 413, headers: vec![("content-type".into(), "application/json".into())], body: said.into_bytes() });
        }
        std::fs::write(&path, body).expect("the upload lands");
        Ok(FetchResponse { status: 200, headers: vec![], body: vec![] })
    }
}

/// This computer stands in for the guest: the probe answers from a fixture, every other exec runs under bash and is
/// refused over the body cap, run() is the same bash, so the untar is the real command against a temp root.
struct TestGuest {
    probe: Option<String>,
    road: Arc<UploadRoad>,
    execs: Mutex<Vec<String>>,
    reject: Mutex<Option<String>>,
    untar: Mutex<Option<ExecResult>>,
}

fn not_here<T>() -> wsp_engine::errors::Result<T> {
    Err(EngineError::Other { message: "not on this guest".into(), code: None })
}

#[async_trait]
impl Machine for TestGuest {
    fn id(&self) -> &str {
        "m1"
    }

    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }

    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> wsp_engine::errors::Result<ExecResult> {
        let reject = self.reject.lock().unwrap().clone();
        if let Some(message) = reject {
            return Err(EngineError::Other { message, code: None });
        }
        let untar = self.untar.lock().unwrap().clone();
        if let (true, Some(res)) = (cmd.contains("tar xzf"), untar) {
            return Ok(res);
        }
        self.execs.lock().unwrap().push(cmd.to_string());
        if body_bytes(cmd) > EXEC_BODY_CAP {
            return Err(EngineError::Provider(WspError {
                kind: ErrorKind::Unknown,
                status: 413,
                code: None,
                message: "Payload Too Large".into(),
                request_id: None,
                window: None,
                said: None,
            }));
        }
        if cmd.contains("echo WSP_CTX") {
            let stdout = self.probe.clone().unwrap_or_default();
            return Ok(ExecResult { exit_code: 0, stdout, stderr: String::new() });
        }
        Ok(bash(cmd).await)
    }

    async fn run(&self, script: &str, _opts: RunOptions) -> wsp_engine::errors::Result<ExecResult> {
        self.exec(script, ExecOptions::default()).await
    }

    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> wsp_engine::errors::Result<String> {
        not_here()
    }

    async fn pause(&self, _wait_ms: Option<u64>) -> wsp_engine::errors::Result<()> {
        not_here()
    }

    async fn resume(&self, _signal: Option<CancellationToken>) -> wsp_engine::errors::Result<()> {
        not_here()
    }

    async fn kill(&self) -> wsp_engine::errors::Result<()> {
        not_here()
    }

    async fn state(&self) -> wsp_engine::errors::Result<MachineState> {
        not_here()
    }

    async fn download_url(&self, _path: &str) -> wsp_engine::errors::Result<String> {
        not_here()
    }

    async fn upload_url(&self, path: &str) -> wsp_engine::errors::Result<String> {
        Ok(url::Url::parse_with_params("http://guest.test/upload", [("path", path)]).unwrap().to_string())
    }
}

struct Guest {
    _dir: tempfile::TempDir,
    root: PathBuf,
    roots: GuestRoots,
    m: Arc<TestGuest>,
}

impl Guest {
    fn execs(&self) -> Vec<String> {
        self.m.execs.lock().unwrap().clone()
    }

    fn puts(&self) -> Vec<(String, usize)> {
        self.m.road.puts.lock().unwrap().iter().map(|p| (p.path.clone(), p.bytes)).collect()
    }

    async fn apply(&self, mut opts: ApplyContextOptions) -> wsp_engine::machine_context::ContextOutcome {
        opts.roots = Some(self.roots.clone());
        opts.fetch = Some(self.m.road.clone() as Arc<dyn Fetch>);
        apply_machine_context(self.m.as_ref(), opts).await
    }

    fn at(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        for p in self.m.road.puts.lock().unwrap().iter() {
            let _ = std::fs::remove_file(&p.path);
        }
    }
}

fn guest_with(probe: Option<String>, put_cap: usize) -> Guest {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let roots = GuestRoots { etc: root.join("etc").to_string_lossy().into_owned(), home: root.join("home").to_string_lossy().into_owned() };
    let road = Arc::new(UploadRoad { cap: put_cap, puts: Mutex::new(vec![]) });
    let m = Arc::new(TestGuest { probe, road, execs: Mutex::new(vec![]), reject: Mutex::new(None), untar: Mutex::new(None) });
    Guest { _dir: dir, root, roots, m }
}

fn guest() -> Guest {
    guest_with(Some(probe_out()), usize::MAX)
}

fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path.as_ref()).unwrap_or_else(|e| panic!("{}: {e}", path.as_ref().display()))
}

/// machine-context.test.ts:739
#[tokio::test]
async fn probes_renders_with_facts_folded_in_lands_every_hook_and_skill_through_the_upload_road_and_records_each_agent() {
    let g = guest();
    let result = ImportResult {
        recipe_hash: "h".into(),
        tools: vec![ImportRow { id: "tools/brew/x".into(), label: "x".into(), outcome: "failed".into(), note: Some("boom".into()) }],
        agents: vec![],
        ..Default::default()
    };
    let out = g
        .apply(ApplyContextOptions {
            workspace: Some(WorkspaceName { name: "task-1".into() }),
            golden: Some(golden()),
            result: Some(result),
            ..Default::default()
        })
        .await;
    assert_eq!(out.failure, None);
    let agents = context_agents();
    assert_eq!(out.context.len(), agents.len(), "{:?}", out.context);
    for (agent, got) in agents.iter().zip(&out.context) {
        assert_eq!(got.agent, agent.id);
        assert_eq!(got.outcome, ContextOutcomeKind::Written, "{}", agent.id);
        assert!(got.path.is_some(), "{}", agent.id);
        assert_eq!(got.note, None, "{}", agent.id);
        assert!(got.skill.ends_with("/wsp-machine/SKILL.md"), "{}", got.skill);
    }
    let puts = g.puts();
    assert_eq!(puts.len(), 1);
    assert_eq!(out.summary, format!("{} written for Claude Code, Codex, Gemini CLI, OpenCode, Pi, Hermes Agent", fmt_bytes(puts[0].1 as f64)));
    let execs = g.execs();
    assert_eq!(execs.len(), 2, "{execs:?}");
    assert!(!execs.iter().any(|c| c.contains("base64 --decode")));
    let roots = &g.roots;
    let short = read(context_path(Some(roots)));
    assert!(short.contains("The wsp-machine skill has the full picture"));
    assert!(short.contains("terminal panes open in the home folder.\n"));
    assert!(!short.contains("persists across"));
    let skill = read(skill_path(Some(roots)));
    assert!(skill.contains("- Workspace: task-1."));
    assert!(!skill.contains("persists across"));
    assert!(read(g.at("etc/claude-code/CLAUDE.md")).contains("persists across your tool calls"));
    assert!(read(g.at("etc/claude-code/.claude/skills/wsp-machine/SKILL.md")).contains("persists across your tool calls"));
    assert_eq!(read(g.at("home/.gemini/WSP-MACHINE.md")), short);
    assert_eq!(read(g.at("etc/codex/skills/wsp-machine/SKILL.md")), skill);
    assert!(skill.contains("- Tools that did not install: raycast (macOS app, no Linux build); x (boom)."));
    for path in [
        "etc/wsp/machine-context.json",
        "etc/claude-code/CLAUDE.md",
        "etc/claude-code/.claude/skills/wsp-machine/SKILL.md",
        "etc/codex/requirements.toml",
        "etc/codex/skills/wsp-machine/SKILL.md",
        "home/.gemini/WSP-MACHINE.md",
        "home/.gemini/skills/wsp-machine/SKILL.md",
        "etc/opencode/opencode.json",
        "home/.config/opencode/skills/wsp-machine/SKILL.md",
        "home/.pi/agent/APPEND_SYSTEM.md",
        "home/.pi/agent/skills/wsp-machine/SKILL.md",
        "etc/profile.d/wsp-machine.sh",
        "home/.hermes/skills/wsp-machine/SKILL.md",
    ] {
        let at = g.at(path);
        assert!(at.exists(), "{path}");
        assert_eq!(std::fs::metadata(&at).unwrap().permissions().mode() & 0o777, 0o644, "{path}");
    }
}

/// machine-context.test.ts:769
#[tokio::test]
async fn takes_the_fallback_for_a_claimed_hook_the_skill_alone_for_hermes_and_skips_agents_not_installed() {
    let stdout = "WSP_CTX\nAGENT claude\nAGENT gemini\nAGENT hermes\nCONFLICT gemini\nCONFLICT hermes\nWSP_CTX_END\n";
    let g = guest_with(Some(stdout.into()), usize::MAX);
    let out = g.apply(ApplyContextOptions::default()).await;
    let etc = |p: &str| g.at(&format!("etc/{p}")).to_string_lossy().into_owned();
    let home = |p: &str| g.at(&format!("home/{p}")).to_string_lossy().into_owned();
    assert_eq!(
        out.context,
        vec![
            ContextResult {
                agent: "claude".into(),
                outcome: ContextOutcomeKind::Written,
                path: Some(etc("claude-code/CLAUDE.md")),
                note: None,
                skill: etc("claude-code/.claude/skills/wsp-machine/SKILL.md"),
            },
            ContextResult {
                agent: "gemini".into(),
                outcome: ContextOutcomeKind::Fallback,
                path: Some(home(".gemini/extensions/wsp-machine")),
                note: Some("~/.gemini/settings.json sets context.fileName".into()),
                skill: home(".gemini/skills/wsp-machine/SKILL.md"),
            },
            ContextResult {
                agent: "hermes".into(),
                outcome: ContextOutcomeKind::NotLoaded,
                path: None,
                note: Some("~/.hermes/config.yaml sets agent.environment_hint".into()),
                skill: home(".hermes/skills/wsp-machine/SKILL.md"),
            },
        ]
    );
    let rest = out.summary.strip_suffix(" KB written for Claude Code; Gemini CLI by its fallback (~/.gemini/settings.json sets context.fileName); Hermes Agent not loaded (~/.hermes/config.yaml sets agent.environment_hint), skill only");
    let figure = rest.unwrap_or_else(|| panic!("{}", out.summary));
    assert!(!figure.is_empty() && figure.chars().all(|c| c.is_ascii_digit() || c == '.') && figure.matches('.').count() <= 1, "{}", out.summary);
    assert!(!g.at("etc/gemini-cli/system-defaults.json").exists());
    assert!(g.at("home/.gemini/extensions/wsp-machine/gemini-extension.json").exists());
    assert!(!g.at("etc/profile.d/wsp-machine.sh").exists());
    assert!(g.at("home/.hermes/skills/wsp-machine/SKILL.md").exists());
    assert!(!g.at("etc/opencode").exists());
    assert!(read(skill_path(Some(&g.roots))).contains("- This machine is a golden builder, not a workspace yet."));
}

/// machine-context.test.ts:787
#[tokio::test]
async fn reports_a_guest_that_does_not_answer_and_writes_nothing() {
    let silent = guest_with(None, usize::MAX);
    let out = silent.apply(ApplyContextOptions::default()).await;
    assert_eq!(
        out,
        wsp_engine::machine_context::ContextOutcome {
            context: vec![],
            summary: "not written (probe answered exit 0 without its markers)".into(),
            failure: Some("probe answered exit 0 without its markers".into()),
        }
    );
    assert_eq!(silent.execs().len(), 1);
    let gone = guest();
    *gone.m.reject.lock().unwrap() = Some("machine paused".into());
    assert_eq!(gone.apply(ApplyContextOptions::default()).await.failure.as_deref(), Some("probe failed: machine paused"));
}

/// machine-context.test.ts:796
#[tokio::test]
async fn a_refused_upload_is_a_failure_that_names_the_cap_and_nothing_lands() {
    let g = guest_with(Some("WSP_CTX\nAGENT claude\nWSP_CTX_END\n".into()), 1024);
    let out = g.apply(ApplyContextOptions::default()).await;
    assert!(out.context.is_empty());
    assert_eq!(out.failure.as_deref(), Some("write failed: vault import upload failed: HTTP 413 Payload Too Large; the upload takes at most 1024 bytes per PUT"));
    assert_eq!(out.summary, format!("not written ({})", out.failure.clone().unwrap_or_default()));
    assert_eq!(g.puts().len(), 1);
    assert!(!Path::new(&context_path(Some(&g.roots))).exists());
    assert!(!g.at("etc/claude-code/CLAUDE.md").exists());
}

/// machine-context.test.ts:807
#[tokio::test]
async fn an_untar_that_fails_is_a_failure_with_the_guests_last_line() {
    let g = guest_with(Some("WSP_CTX\nAGENT claude\nWSP_CTX_END\n".into()), usize::MAX);
    *g.m.untar.lock().unwrap() =
        Some(ExecResult { exit_code: 2, stdout: String::new(), stderr: "tar: etc/wsp: Cannot mkdir: Read-only file system\n".into() });
    let out = g.apply(ApplyContextOptions::default()).await;
    assert_eq!(out.failure.as_deref(), Some("write failed: vault import untar failed (exit 2): tar: etc/wsp: Cannot mkdir: Read-only file system"));
    assert!(out.context.is_empty());
}

const SNAP_PI_EXTENSION: &str = r######"// wsp writes this file when a workspace is forked; edits are overwritten.
import { readFileSync } from "node:fs";
export default function (pi) {
  pi.on("before_agent_start", async event => ({ systemPrompt: `${event.systemPrompt}\n\n${readFileSync("/etc/wsp/machine-context.md", "utf8")}` }));
}
"######;

const SNAP_HOOKS_CLAUDE: &str = r######"/etc/claude-code/CLAUDE.md
wsp writes this file when a workspace is forked; edits are overwritten.

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer; it is not that computer. The wsp-machine skill has the full picture: what did not install, the secret names, the aliases whose commands are missing, and how the common things are done here.

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with run_in_background on the Bash tool, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder. A cd moves your own shell, which persists across your tool calls, not the thread's folder, and the files pane follows that shell's folder unless you pinned the panes.
- A server listening on a port shows in the app for the person to open; bind 0.0.0.0, not 127.0.0.1. A printed http://localhost:<port> link has its port forwarded to their computer. Ports below 1024 are not forwarded.
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh. Use them by name ($NAME); never print, log or commit a value, and never read that file.
- Sign-ins go through wsp: run the tool's own login command and the page opens on the person's computer. Do not paste tokens into the terminal and do not ask for a password.

----
/etc/claude-code/.claude/skills/wsp-machine/SKILL.md
---
name: wsp-machine
description: How this cloud Linux machine differs from the person's computer, what did not install here, where secrets live, and how to keep a server alive, hand over a URL and sign in. Read it before starting a server, opening a port, using a secret or calling a tool that may not be installed.
---

wsp writes this file when a workspace is forked; edits are overwritten.

# This machine

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer: the files, tools and agents they ticked. It is not that computer.

- Linux 6.6.30, user root, home /root. macOS apps, casks and Mac App Store apps are not here.
- Homebrew is at /home/linuxbrew/.linuxbrew.
- Login shells get their PATH from /etc/profile.d/wsp-golden.sh: /root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- wsp-daemon listens on 0.0.0.0:7070 with its own token. Do not stop it and do not bind port 7070.
- wsp on this machine's PATH drives the person's host as this thread; wsp --help agent lists its verbs.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with run_in_background on the Bash tool, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder. A cd moves your own shell, which persists across your tool calls, not the thread's folder, and the files pane follows that shell's folder unless you pinned the panes.
- A server listening on a port shows in the app as a server the person can open. A loopback-only bind (127.0.0.1) is unreachable through the preview edge; bind 0.0.0.0. Ports below 1024 are not forwarded.
- Sign-ins go through wsp: BROWSER is /usr/local/bin/wsp-open and xdg-open is the same shim. The page opens on the person's computer and the callback port is tunnelled back here.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh and /etc/fish/conf.d/wsp-secrets.fish. They are never printed, logged or committed, and that file is never read or copied. .env files, .netrc and private keys were never copied from the person's computer.

# This workspace

- Workspace: task-1.
- Golden: v3, sealed 2026-09-05, setup 9f2a7c1d4e5b.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free and skips tool installs that would go under it.
- Secrets set in /etc/profile.d/wsp-secrets.sh: OPENAI_API_KEY, GH_TOKEN. Use them by name ($NAME); a missing one is asked for through wsp, not typed here.
- Tools that did not install: raycast (macOS app, no Linux build).
- Agents that did not install: Zed (no installer known).
- Files left on the person's computer: ~/.ssh/id_ed25519 (private key, never copied).
- Aliases whose command is not here: o runs open; rc runs code.

# How to

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'. Read the log file, not the tool output. Bind 0.0.0.0 if the person should open it.
- Hand the person a URL: print it as http://localhost:<port>; wsp forwards the port and the link opens on their computer. For a page that is not a local server, run wsp-open <url>; it appears in the app for them to open.
- Ask for a sign-in: run the tool's own login command (gh auth login, gcloud auth login). It opens through $BROWSER, the person signs in on their computer, and the callback comes back here. Do not paste tokens into the terminal and do not ask for a password.
- Work in a folder: cd <dir> in your shell and it stays there across your tool calls; the thread's folder does not move. Absolute paths work from anywhere.
- Use a secret: $NAME is set from /etc/profile.d/wsp-secrets.sh. Never echo, log or commit a value, and never cat that file.
"######;

const SNAP_HOOKS_CODEX: &str = r######"/etc/codex/requirements.toml
# wsp writes this file when a workspace is forked; edits are overwritten.
additional_developer_instructions = '''
wsp writes this file when a workspace is forked; edits are overwritten.

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer; it is not that computer. The wsp-machine skill has the full picture: what did not install, the secret names, the aliases whose commands are missing, and how the common things are done here.

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app for the person to open; bind 0.0.0.0, not 127.0.0.1. A printed http://localhost:<port> link has its port forwarded to their computer. Ports below 1024 are not forwarded.
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh. Use them by name ($NAME); never print, log or commit a value, and never read that file.
- Sign-ins go through wsp: run the tool's own login command and the page opens on the person's computer. Do not paste tokens into the terminal and do not ask for a password.
'''

----
/etc/codex/skills/wsp-machine/SKILL.md
---
name: wsp-machine
description: How this cloud Linux machine differs from the person's computer, what did not install here, where secrets live, and how to keep a server alive, hand over a URL and sign in. Read it before starting a server, opening a port, using a secret or calling a tool that may not be installed.
---

wsp writes this file when a workspace is forked; edits are overwritten.

# This machine

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer: the files, tools and agents they ticked. It is not that computer.

- Linux 6.6.30, user root, home /root. macOS apps, casks and Mac App Store apps are not here.
- Homebrew is at /home/linuxbrew/.linuxbrew.
- Login shells get their PATH from /etc/profile.d/wsp-golden.sh: /root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- wsp-daemon listens on 0.0.0.0:7070 with its own token. Do not stop it and do not bind port 7070.
- wsp on this machine's PATH drives the person's host as this thread; wsp --help agent lists its verbs.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app as a server the person can open. A loopback-only bind (127.0.0.1) is unreachable through the preview edge; bind 0.0.0.0. Ports below 1024 are not forwarded.
- Sign-ins go through wsp: BROWSER is /usr/local/bin/wsp-open and xdg-open is the same shim. The page opens on the person's computer and the callback port is tunnelled back here.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh and /etc/fish/conf.d/wsp-secrets.fish. They are never printed, logged or committed, and that file is never read or copied. .env files, .netrc and private keys were never copied from the person's computer.

# This workspace

- Workspace: task-1.
- Golden: v3, sealed 2026-09-05, setup 9f2a7c1d4e5b.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free and skips tool installs that would go under it.
- Secrets set in /etc/profile.d/wsp-secrets.sh: OPENAI_API_KEY, GH_TOKEN. Use them by name ($NAME); a missing one is asked for through wsp, not typed here.
- Tools that did not install: raycast (macOS app, no Linux build).
- Agents that did not install: Zed (no installer known).
- Files left on the person's computer: ~/.ssh/id_ed25519 (private key, never copied).
- Aliases whose command is not here: o runs open; rc runs code.

# How to

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'. Read the log file, not the tool output. Bind 0.0.0.0 if the person should open it.
- Hand the person a URL: print it as http://localhost:<port>; wsp forwards the port and the link opens on their computer. For a page that is not a local server, run wsp-open <url>; it appears in the app for them to open.
- Ask for a sign-in: run the tool's own login command (gh auth login, gcloud auth login). It opens through $BROWSER, the person signs in on their computer, and the callback comes back here. Do not paste tokens into the terminal and do not ask for a password.
- Work in a folder: cd <dir> && <cmd> on one line, or absolute paths.
- Use a secret: $NAME is set from /etc/profile.d/wsp-secrets.sh. Never echo, log or commit a value, and never cat that file.
"######;

const SNAP_HOOKS_GEMINI: &str = r######"/etc/gemini-cli/system-defaults.json
{
  "context": {
    "fileName": [
      "GEMINI.md",
      "WSP-MACHINE.md"
    ]
  }
}

----
/root/.gemini/WSP-MACHINE.md
wsp writes this file when a workspace is forked; edits are overwritten.

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer; it is not that computer. The wsp-machine skill has the full picture: what did not install, the secret names, the aliases whose commands are missing, and how the common things are done here.

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app for the person to open; bind 0.0.0.0, not 127.0.0.1. A printed http://localhost:<port> link has its port forwarded to their computer. Ports below 1024 are not forwarded.
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh. Use them by name ($NAME); never print, log or commit a value, and never read that file.
- Sign-ins go through wsp: run the tool's own login command and the page opens on the person's computer. Do not paste tokens into the terminal and do not ask for a password.

----
/root/.gemini/skills/wsp-machine/SKILL.md
---
name: wsp-machine
description: How this cloud Linux machine differs from the person's computer, what did not install here, where secrets live, and how to keep a server alive, hand over a URL and sign in. Read it before starting a server, opening a port, using a secret or calling a tool that may not be installed.
---

wsp writes this file when a workspace is forked; edits are overwritten.

# This machine

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer: the files, tools and agents they ticked. It is not that computer.

- Linux 6.6.30, user root, home /root. macOS apps, casks and Mac App Store apps are not here.
- Homebrew is at /home/linuxbrew/.linuxbrew.
- Login shells get their PATH from /etc/profile.d/wsp-golden.sh: /root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- wsp-daemon listens on 0.0.0.0:7070 with its own token. Do not stop it and do not bind port 7070.
- wsp on this machine's PATH drives the person's host as this thread; wsp --help agent lists its verbs.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app as a server the person can open. A loopback-only bind (127.0.0.1) is unreachable through the preview edge; bind 0.0.0.0. Ports below 1024 are not forwarded.
- Sign-ins go through wsp: BROWSER is /usr/local/bin/wsp-open and xdg-open is the same shim. The page opens on the person's computer and the callback port is tunnelled back here.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh and /etc/fish/conf.d/wsp-secrets.fish. They are never printed, logged or committed, and that file is never read or copied. .env files, .netrc and private keys were never copied from the person's computer.

# This workspace

- Workspace: task-1.
- Golden: v3, sealed 2026-09-05, setup 9f2a7c1d4e5b.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free and skips tool installs that would go under it.
- Secrets set in /etc/profile.d/wsp-secrets.sh: OPENAI_API_KEY, GH_TOKEN. Use them by name ($NAME); a missing one is asked for through wsp, not typed here.
- Tools that did not install: raycast (macOS app, no Linux build).
- Agents that did not install: Zed (no installer known).
- Files left on the person's computer: ~/.ssh/id_ed25519 (private key, never copied).
- Aliases whose command is not here: o runs open; rc runs code.

# How to

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'. Read the log file, not the tool output. Bind 0.0.0.0 if the person should open it.
- Hand the person a URL: print it as http://localhost:<port>; wsp forwards the port and the link opens on their computer. For a page that is not a local server, run wsp-open <url>; it appears in the app for them to open.
- Ask for a sign-in: run the tool's own login command (gh auth login, gcloud auth login). It opens through $BROWSER, the person signs in on their computer, and the callback comes back here. Do not paste tokens into the terminal and do not ask for a password.
- Work in a folder: cd <dir> && <cmd> on one line, or absolute paths.
- Use a secret: $NAME is set from /etc/profile.d/wsp-secrets.sh. Never echo, log or commit a value, and never cat that file.
"######;

const SNAP_HOOKS_HERMES: &str = r######"/etc/profile.d/wsp-machine.sh
# wsp writes this file when a workspace is forked; edits are overwritten.
export HERMES_ENVIRONMENT_HINT="$(cat /etc/wsp/machine-context.md)"

----
/etc/fish/conf.d/wsp-machine.fish
# wsp writes this file when a workspace is forked; edits are overwritten.
set -gx HERMES_ENVIRONMENT_HINT (cat /etc/wsp/machine-context.md | string collect)

----
/root/.hermes/skills/wsp-machine/SKILL.md
---
name: wsp-machine
description: How this cloud Linux machine differs from the person's computer, what did not install here, where secrets live, and how to keep a server alive, hand over a URL and sign in. Read it before starting a server, opening a port, using a secret or calling a tool that may not be installed.
---

wsp writes this file when a workspace is forked; edits are overwritten.

# This machine

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer: the files, tools and agents they ticked. It is not that computer.

- Linux 6.6.30, user root, home /root. macOS apps, casks and Mac App Store apps are not here.
- Homebrew is at /home/linuxbrew/.linuxbrew.
- Login shells get their PATH from /etc/profile.d/wsp-golden.sh: /root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- wsp-daemon listens on 0.0.0.0:7070 with its own token. Do not stop it and do not bind port 7070.
- wsp on this machine's PATH drives the person's host as this thread; wsp --help agent lists its verbs.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app as a server the person can open. A loopback-only bind (127.0.0.1) is unreachable through the preview edge; bind 0.0.0.0. Ports below 1024 are not forwarded.
- Sign-ins go through wsp: BROWSER is /usr/local/bin/wsp-open and xdg-open is the same shim. The page opens on the person's computer and the callback port is tunnelled back here.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh and /etc/fish/conf.d/wsp-secrets.fish. They are never printed, logged or committed, and that file is never read or copied. .env files, .netrc and private keys were never copied from the person's computer.

# This workspace

- Workspace: task-1.
- Golden: v3, sealed 2026-09-05, setup 9f2a7c1d4e5b.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free and skips tool installs that would go under it.
- Secrets set in /etc/profile.d/wsp-secrets.sh: OPENAI_API_KEY, GH_TOKEN. Use them by name ($NAME); a missing one is asked for through wsp, not typed here.
- Tools that did not install: raycast (macOS app, no Linux build).
- Agents that did not install: Zed (no installer known).
- Files left on the person's computer: ~/.ssh/id_ed25519 (private key, never copied).
- Aliases whose command is not here: o runs open; rc runs code.

# How to

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'. Read the log file, not the tool output. Bind 0.0.0.0 if the person should open it.
- Hand the person a URL: print it as http://localhost:<port>; wsp forwards the port and the link opens on their computer. For a page that is not a local server, run wsp-open <url>; it appears in the app for them to open.
- Ask for a sign-in: run the tool's own login command (gh auth login, gcloud auth login). It opens through $BROWSER, the person signs in on their computer, and the callback comes back here. Do not paste tokens into the terminal and do not ask for a password.
- Work in a folder: cd <dir> && <cmd> on one line, or absolute paths.
- Use a secret: $NAME is set from /etc/profile.d/wsp-secrets.sh. Never echo, log or commit a value, and never cat that file.
"######;

const SNAP_HOOKS_OPENCODE: &str = r######"/etc/opencode/opencode.json
{
  "instructions": [
    "/etc/wsp/machine-context.md"
  ]
}

----
/root/.config/opencode/skills/wsp-machine/SKILL.md
---
name: wsp-machine
description: How this cloud Linux machine differs from the person's computer, what did not install here, where secrets live, and how to keep a server alive, hand over a URL and sign in. Read it before starting a server, opening a port, using a secret or calling a tool that may not be installed.
---

wsp writes this file when a workspace is forked; edits are overwritten.

# This machine

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer: the files, tools and agents they ticked. It is not that computer.

- Linux 6.6.30, user root, home /root. macOS apps, casks and Mac App Store apps are not here.
- Homebrew is at /home/linuxbrew/.linuxbrew.
- Login shells get their PATH from /etc/profile.d/wsp-golden.sh: /root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- wsp-daemon listens on 0.0.0.0:7070 with its own token. Do not stop it and do not bind port 7070.
- wsp on this machine's PATH drives the person's host as this thread; wsp --help agent lists its verbs.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app as a server the person can open. A loopback-only bind (127.0.0.1) is unreachable through the preview edge; bind 0.0.0.0. Ports below 1024 are not forwarded.
- Sign-ins go through wsp: BROWSER is /usr/local/bin/wsp-open and xdg-open is the same shim. The page opens on the person's computer and the callback port is tunnelled back here.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh and /etc/fish/conf.d/wsp-secrets.fish. They are never printed, logged or committed, and that file is never read or copied. .env files, .netrc and private keys were never copied from the person's computer.

# This workspace

- Workspace: task-1.
- Golden: v3, sealed 2026-09-05, setup 9f2a7c1d4e5b.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free and skips tool installs that would go under it.
- Secrets set in /etc/profile.d/wsp-secrets.sh: OPENAI_API_KEY, GH_TOKEN. Use them by name ($NAME); a missing one is asked for through wsp, not typed here.
- Tools that did not install: raycast (macOS app, no Linux build).
- Agents that did not install: Zed (no installer known).
- Files left on the person's computer: ~/.ssh/id_ed25519 (private key, never copied).
- Aliases whose command is not here: o runs open; rc runs code.

# How to

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'. Read the log file, not the tool output. Bind 0.0.0.0 if the person should open it.
- Hand the person a URL: print it as http://localhost:<port>; wsp forwards the port and the link opens on their computer. For a page that is not a local server, run wsp-open <url>; it appears in the app for them to open.
- Ask for a sign-in: run the tool's own login command (gh auth login, gcloud auth login). It opens through $BROWSER, the person signs in on their computer, and the callback comes back here. Do not paste tokens into the terminal and do not ask for a password.
- Work in a folder: cd <dir> && <cmd> on one line, or absolute paths.
- Use a secret: $NAME is set from /etc/profile.d/wsp-secrets.sh. Never echo, log or commit a value, and never cat that file.
"######;

const SNAP_HOOKS_PI: &str = r######"/root/.pi/agent/APPEND_SYSTEM.md
wsp writes this file when a workspace is forked; edits are overwritten.

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer; it is not that computer. The wsp-machine skill has the full picture: what did not install, the secret names, the aliases whose commands are missing, and how the common things are done here.

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app for the person to open; bind 0.0.0.0, not 127.0.0.1. A printed http://localhost:<port> link has its port forwarded to their computer. Ports below 1024 are not forwarded.
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh. Use them by name ($NAME); never print, log or commit a value, and never read that file.
- Sign-ins go through wsp: run the tool's own login command and the page opens on the person's computer. Do not paste tokens into the terminal and do not ask for a password.

----
/root/.pi/agent/skills/wsp-machine/SKILL.md
---
name: wsp-machine
description: How this cloud Linux machine differs from the person's computer, what did not install here, where secrets live, and how to keep a server alive, hand over a URL and sign in. Read it before starting a server, opening a port, using a secret or calling a tool that may not be installed.
---

wsp writes this file when a workspace is forked; edits are overwritten.

# This machine

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer: the files, tools and agents they ticked. It is not that computer.

- Linux 6.6.30, user root, home /root. macOS apps, casks and Mac App Store apps are not here.
- Homebrew is at /home/linuxbrew/.linuxbrew.
- Login shells get their PATH from /etc/profile.d/wsp-golden.sh: /root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- wsp-daemon listens on 0.0.0.0:7070 with its own token. Do not stop it and do not bind port 7070.
- wsp on this machine's PATH drives the person's host as this thread; wsp --help agent lists its verbs.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app as a server the person can open. A loopback-only bind (127.0.0.1) is unreachable through the preview edge; bind 0.0.0.0. Ports below 1024 are not forwarded.
- Sign-ins go through wsp: BROWSER is /usr/local/bin/wsp-open and xdg-open is the same shim. The page opens on the person's computer and the callback port is tunnelled back here.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh and /etc/fish/conf.d/wsp-secrets.fish. They are never printed, logged or committed, and that file is never read or copied. .env files, .netrc and private keys were never copied from the person's computer.

# This workspace

- Workspace: task-1.
- Golden: v3, sealed 2026-09-05, setup 9f2a7c1d4e5b.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free and skips tool installs that would go under it.
- Secrets set in /etc/profile.d/wsp-secrets.sh: OPENAI_API_KEY, GH_TOKEN. Use them by name ($NAME); a missing one is asked for through wsp, not typed here.
- Tools that did not install: raycast (macOS app, no Linux build).
- Agents that did not install: Zed (no installer known).
- Files left on the person's computer: ~/.ssh/id_ed25519 (private key, never copied).
- Aliases whose command is not here: o runs open; rc runs code.

# How to

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'. Read the log file, not the tool output. Bind 0.0.0.0 if the person should open it.
- Hand the person a URL: print it as http://localhost:<port>; wsp forwards the port and the link opens on their computer. For a page that is not a local server, run wsp-open <url>; it appears in the app for them to open.
- Ask for a sign-in: run the tool's own login command (gh auth login, gcloud auth login). It opens through $BROWSER, the person signs in on their computer, and the callback comes back here. Do not paste tokens into the terminal and do not ask for a password.
- Work in a folder: cd <dir> && <cmd> on one line, or absolute paths.
- Use a secret: $NAME is set from /etc/profile.d/wsp-secrets.sh. Never echo, log or commit a value, and never cat that file.
"######;

const SNAP_DOC_BUILDER: &str = r######"wsp writes this file when a workspace is forked; edits are overwritten.

# This machine

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer: the files, tools and agents they ticked. It is not that computer.

- Linux, user root, home /root. macOS apps, casks and Mac App Store apps are not here.
- Docker and Podman are not installed.
- wsp-daemon listens on 0.0.0.0:7070 with its own token. Do not stop it and do not bind port 7070.
- wsp on this machine's PATH drives the person's host as this thread; wsp --help agent lists its verbs.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app as a server the person can open. A loopback-only bind (127.0.0.1) is unreachable through the preview edge; bind 0.0.0.0. Ports below 1024 are not forwarded.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh. They are never printed, logged or committed, and that file is never read or copied. .env files, .netrc and private keys were never copied from the person's computer.

# This workspace

- This machine is a golden builder, not a workspace yet.
- Golden: not sealed yet.
- Disk: size unknown when this file was written. wsp keeps 2 GB free and skips tool installs that would go under it.
- Secrets set in /etc/profile.d/wsp-secrets.sh: none. Use them by name ($NAME); a missing one is asked for through wsp, not typed here.
- Tools that did not install: none.
- Agents that did not install: none.
- Files left on the person's computer: none.
- Aliases whose command is not here: none.

# How to

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &. Read the log file, not the tool output. Bind 0.0.0.0 if the person should open it.
- Hand the person a URL: print it as http://localhost:<port>; wsp forwards the port and the link opens on their computer.
- Work in a folder: cd <dir> && <cmd> on one line, or absolute paths.
- Use a secret: $NAME is set from /etc/profile.d/wsp-secrets.sh. Never echo, log or commit a value, and never cat that file.
"######;

const SNAP_DOC_WORKSPACE: &str = r######"wsp writes this file when a workspace is forked; edits are overwritten.

# This machine

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer: the files, tools and agents they ticked. It is not that computer.

- Linux 6.6.30, user root, home /root. macOS apps, casks and Mac App Store apps are not here.
- Homebrew is at /home/linuxbrew/.linuxbrew.
- Login shells get their PATH from /etc/profile.d/wsp-golden.sh: /root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- wsp-daemon listens on 0.0.0.0:7070 with its own token. Do not stop it and do not bind port 7070.
- wsp on this machine's PATH drives the person's host as this thread; wsp --help agent lists its verbs.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app as a server the person can open. A loopback-only bind (127.0.0.1) is unreachable through the preview edge; bind 0.0.0.0. Ports below 1024 are not forwarded.
- Sign-ins go through wsp: BROWSER is /usr/local/bin/wsp-open and xdg-open is the same shim. The page opens on the person's computer and the callback port is tunnelled back here.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh and /etc/fish/conf.d/wsp-secrets.fish. They are never printed, logged or committed, and that file is never read or copied. .env files, .netrc and private keys were never copied from the person's computer.

# This workspace

- Workspace: task-1.
- Golden: v3, sealed 2026-09-05, setup 9f2a7c1d4e5b.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free and skips tool installs that would go under it.
- Secrets set in /etc/profile.d/wsp-secrets.sh: OPENAI_API_KEY, GH_TOKEN. Use them by name ($NAME); a missing one is asked for through wsp, not typed here.
- Tools that did not install: raycast (macOS app, no Linux build).
- Agents that did not install: Zed (no installer known).
- Files left on the person's computer: ~/.ssh/id_ed25519 (private key, never copied).
- Aliases whose command is not here: o runs open; rc runs code.

# How to

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'. Read the log file, not the tool output. Bind 0.0.0.0 if the person should open it.
- Hand the person a URL: print it as http://localhost:<port>; wsp forwards the port and the link opens on their computer. For a page that is not a local server, run wsp-open <url>; it appears in the app for them to open.
- Ask for a sign-in: run the tool's own login command (gh auth login, gcloud auth login). It opens through $BROWSER, the person signs in on their computer, and the callback comes back here. Do not paste tokens into the terminal and do not ask for a password.
- Work in a folder: cd <dir> && <cmd> on one line, or absolute paths.
- Use a secret: $NAME is set from /etc/profile.d/wsp-secrets.sh. Never echo, log or commit a value, and never cat that file.
"######;

const SNAP_SHORT: &str = r######"wsp writes this file when a workspace is forked; edits are overwritten.

This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer; it is not that computer. The wsp-machine skill has the full picture: what did not install, the secret names, the aliases whose commands are missing, and how the common things are done here.

- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &, or tmux new -d -s <name> '<cmd>'.
- A turn ends when the agent process exits, not at its reply, and the thread reads running until then; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.
- When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.
- A command you mean the person to run goes in a fenced block labelled `sh`, which the app shows with Run: the person runs it with one click, in this thread's folder, and sees its output under the block.
- Every agent session starts in the thread's folder; terminal panes open in the home folder.
- A server listening on a port shows in the app for the person to open; bind 0.0.0.0, not 127.0.0.1. A printed http://localhost:<port> link has its port forwarded to their computer. Ports below 1024 are not forwarded.
- Containers do not run here: the kernel has no overlayfs, and Docker and Podman are not installed. Install services natively.
- Disk: 19.5 GB root disk, 11.2 GB free when this file was written. wsp keeps 2 GB free.
- Secrets are exported by /etc/profile.d/wsp-secrets.sh. Use them by name ($NAME); never print, log or commit a value, and never read that file.
- Sign-ins go through wsp: run the tool's own login command and the page opens on the person's computer. Do not paste tokens into the terminal and do not ask for a password.
"######;
