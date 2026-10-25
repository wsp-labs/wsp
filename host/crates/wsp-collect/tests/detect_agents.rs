// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/agents.test.ts.
mod common;

use common::fake_host::*;
use serde_json::{json, Value};
use wsp_catalog::catalog::catalog_agents;
use wsp_collect::detect::agents::{agents, detect_agents};
use wsp_collect::manifest::ManifestEntry;

fn values(rows: &[ManifestEntry]) -> Vec<Value> {
    rows.iter().map(|r| serde_json::to_value(r).unwrap()).collect()
}

fn laptop(files: &[(&str, u64)]) -> FakeLaptop {
    files.iter().fold(FakeLaptop::new(), |l, (p, n)| l.file(p, *n))
}

mod agents {
    use super::*;

    #[test]
    fn claude_code_brings_settings_memory_skills_and_mcp_config_never_the_credential_fi() {
        let host = fake_host(
            laptop(&[
                ("~/.claude/settings.json", 400),
                ("~/.claude/CLAUDE.md", 900),
                ("~/.claude/skills/x/SKILL.md", 3000),
                ("~/.claude/agents/a.md", 100),
                ("~/.claude/.credentials.json", 800),
                ("~/.claude/plugins/installed_plugins.json", 600),
                ("~/.claude/plugins/marketplaces/x/.git/pack", 90_000_000),
                ("~/.claude/projects/p/transcript.jsonl", 5_000_000),
                ("~/.claude.json", 2000),
            ])
            .which(&["claude"]),
        );
        let rows = block_on(detect_agents(&host)).unwrap();
        assert_eq!(
            values(&rows),
            vec![json!({
                "rung": "agents", "id": "agents/claude", "label": "Claude Code",
                "paths": ["~/.claude/settings.json", "~/.claude/CLAUDE.md", "~/.claude/skills", "~/.claude/agents", "~/.claude.json"],
                "bytes": 6400, "default": "bring",
                // Rewritten by Claude Code while it runs (project state): it travels but never decides the recipe hash.
                "volatile": ["~/.claude.json"],
            })]
        );
    }

    #[test]
    fn claude_code_s_row_lists_every_local_file_a_hook_in_its_settings_runs_and_nothing() {
        let settings = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [
                        { "type": "command", "command": "~/.claude/hooks/commit-batching", "timeout": 10 },
                        { "type": "command", "command": "bash $HOME/.claude/hooks/no-secret-print.sh" },
                    ] },
                    { "hooks": [
                        { "type": "command", "command": "/usr/bin/true" },
                        { "type": "command", "command": "jq -r .tool_input.command" },
                        { "type": "command", "command": "~/.claude/hooks/gone" },
                    ] },
                ],
                "Stop": [{ "hooks": [{ "type": "command", "command": "~/.claude/skills/x/run.sh" }] }],
            },
        })
        .to_string();
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.claude/settings.json", settings.as_str())
                .file("~/.claude/skills/x/run.sh", 30)
                .file("~/.claude/hooks/commit-batching", 120)
                .file("~/.claude/hooks/no-secret-print.sh", 80)
                .which(&["claude"]),
        );
        let rows = block_on(detect_agents(&host)).unwrap();
        let claude = rows.first().expect("a claude row");
        assert_eq!(
            claude.paths,
            vec!["~/.claude/settings.json", "~/.claude/skills", "~/.claude/hooks/commit-batching", "~/.claude/hooks/no-secret-print.sh"]
        );
        assert_eq!(claude.bytes, settings.len() as u64 + 30 + 120 + 80);
    }

    #[test]
    fn config_travels_without_its_login_file() {
        let rows: Vec<(&str, Vec<(&str, u64)>, &str, Vec<&str>, u64)> = vec![
            (
                "codex",
                vec![("~/.codex/config.toml", 50), ("~/.codex/auth.json", 900), ("~/.codex/prompts/p.md", 20)],
                "Codex",
                vec!["~/.codex/config.toml", "~/.codex/prompts"],
                70,
            ),
            (
                "gemini",
                vec![("~/.gemini/settings.json", 30), ("~/.gemini/oauth_creds.json", 500)],
                "Gemini CLI",
                vec!["~/.gemini/settings.json"],
                30,
            ),
            (
                "opencode",
                vec![
                    ("~/.config/opencode/opencode.json", 60),
                    ("~/.config/opencode/node_modules/x/index.js", 5000),
                    ("~/.local/share/opencode/auth.json", 200),
                ],
                "OpenCode",
                vec!["~/.config/opencode/opencode.json"],
                60,
            ),
            ("aider", vec![("~/.aider.conf.yml", 40)], "Aider", vec!["~/.aider.conf.yml"], 40),
            (
                "pi",
                vec![
                    ("~/.pi/agent/settings.json", 80),
                    ("~/.pi/agent/keybindings.json", 10),
                    ("~/.pi/agent/AGENTS.md", 500),
                    ("~/.pi/agent/prompts/p.md", 20),
                    ("~/.pi/agent/skills/s/SKILL.md", 30),
                    ("~/.pi/agent/extensions/e.ts", 40),
                    ("~/.pi/agent/themes/t.json", 50),
                    ("~/.pi/agent/auth.json", 900),
                    ("~/.pi/agent/models.json", 300),
                    ("~/.pi/agent/trust.json", 70),
                    ("~/.pi/agent/sessions/x/s.jsonl", 5_000_000),
                    ("~/.pi/agent/git/pkg/index.ts", 8000),
                    ("~/.pi/agent/npm/node_modules/x/index.js", 9000),
                ],
                "Pi",
                vec![
                    "~/.pi/agent/settings.json",
                    "~/.pi/agent/keybindings.json",
                    "~/.pi/agent/AGENTS.md",
                    "~/.pi/agent/prompts",
                    "~/.pi/agent/skills",
                    "~/.pi/agent/extensions",
                    "~/.pi/agent/themes",
                ],
                730,
            ),
            (
                "hermes",
                vec![
                    ("~/.hermes/config.yaml", 600),
                    ("~/.hermes/SOUL.md", 200),
                    ("~/.hermes/memories/MEMORY.md", 300),
                    ("~/.hermes/skills/s/SKILL.md", 40),
                    ("~/.hermes/cron/jobs.json", 50),
                    ("~/.hermes/hooks/h.py", 60),
                    ("~/.hermes/.env", 25_000),
                    ("~/.hermes/auth.json", 400),
                    ("~/.hermes/state.db", 9_000_000),
                    ("~/.hermes/sessions/s.json", 700),
                    ("~/.hermes/hermes-agent/.git/pack", 90_000_000),
                    ("~/.hermes/cache/x", 100),
                    ("~/.hermes/logs/gateway.log", 800),
                ],
                "Hermes Agent",
                vec![
                    "~/.hermes/config.yaml",
                    "~/.hermes/SOUL.md",
                    "~/.hermes/memories",
                    "~/.hermes/skills",
                    "~/.hermes/cron",
                    "~/.hermes/hooks",
                ],
                1250,
            ),
        ];
        for (name, files, label, paths, bytes) in rows {
            let rows = block_on(detect_agents(&fake_host(laptop(&files)))).unwrap();
            assert_eq!(
                values(&rows),
                vec![
                    json!({ "rung": "agents", "id": format!("agents/{name}"), "label": label, "paths": paths, "bytes": bytes, "default": "bring" })
                ],
                "{name}"
            );
        }
    }

    #[test]
    fn on_path_with_no_config_is_still_an_installed_agent() {
        for (bin, label) in [("codex", "Codex"), ("pi", "Pi"), ("hermes", "Hermes Agent")] {
            let rows = block_on(detect_agents(&fake_host(FakeLaptop::new().which(&[bin])))).unwrap();
            assert_eq!(
                values(&rows),
                vec![
                    json!({ "rung": "agents", "id": format!("agents/{bin}"), "label": label, "paths": [], "bytes": 0, "default": "bring" })
                ],
                "{bin}"
            );
        }
    }

    #[test]
    fn lists_the_catalog_s_agents_in_its_order_then_aider_which_the_catalog_does_not_sh() {
        let got: Vec<String> = agents().iter().map(|a| a.id.clone()).collect();
        let mut want: Vec<String> = catalog_agents().iter().map(|a| a.id.clone()).collect();
        want.push("aider".to_string());
        assert_eq!(got, want);
    }

    #[test]
    fn agents_the_laptop_does_not_have_are_not_listed() {
        assert_eq!(block_on(detect_agents(&empty_host())).unwrap(), Vec::<ManifestEntry>::new());
    }
}

/// The coordinator's ruling on #2446 covers a throw from `hook_files` only. A settings file that is not JSON is no
/// throw in the TS (the catalog's carry catches the parse and carries nothing), so the row stands on its config alone
/// (spec detect_agents rule 3). Value printed by the TS: `hookFiles(CLAUDE_HOOKS, "{bad", home)` is `[]`.
mod a_malformed_hooks_file {
    use super::*;

    #[test]
    fn is_no_hook_scripts_and_the_agents_row_stands() {
        let host = fake_host(FakeLaptop::new().file("~/.claude/settings.json", "{bad").file("~/.claude/hooks/x.sh", 10).which(&["claude"]));
        let rows = block_on(detect_agents(&host)).expect("a parse failure is not an Err");
        let claude = rows.first().expect("a claude row");
        assert_eq!(claude.id, "agents/claude");
        assert_eq!(claude.paths, vec!["~/.claude/settings.json"]);
        assert_eq!(claude.bytes, 4);
    }
}
