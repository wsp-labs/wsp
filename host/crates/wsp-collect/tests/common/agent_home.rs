// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/agent-home.ts: a home as six harnesses leave one: skills as copies and as links into
//! the shared folder, Hermes' category folders, dot entries, a plugin's skills, an MCP config per agent with secrets in
//! it, login files beside them, and a project with its own skills and servers. Every value a reader must never print
//! carries SECRET, and every server command writes SPAWNED when run.
#![allow(dead_code)]
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;

use serde_json::json;

pub const SECRET: &str = "sk-fixture-SECRET";

fn skill(name: &str, description: &str, extra: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n{extra}---\n\n# {name}\n\nBody that is never read past the frontmatter.\n")
}

pub struct AgentHome {
    pub home: String,
    pub bin: String,
    pub project: String,
}

/// A script a test runs by path or by name on a PATH: the text written to `path` and made executable.
pub fn write_stub(path: &str, text: &str) -> String {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string()
}

fn put(path: &str, text: &str) {
    fs::create_dir_all(Path::new(path).parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn link(target: &str, path: &str) {
    fs::create_dir_all(Path::new(path).parent().unwrap()).unwrap();
    symlink(target, path).unwrap();
}

pub fn agent_home(root: &str) -> AgentHome {
    let home = format!("{root}/home");
    let bin = format!("{root}/bin");
    let project = format!("{home}/code/app");
    let h = |p: &str| format!("{home}/{p}");

    // Skills.
    put(&h(".agents/skills/pdf/SKILL.md"), &skill("pdf", ">\n  Read and fill\n  PDF forms", ""));
    put(&h(".agents/skills/.hidden/SKILL.md"), &skill("hidden", "a dot folder is no skill", ""));
    put(&h(".agents/skills/.DS_Store"), "");
    link("../../.agents/skills/pdf", &h(".claude/skills/pdf"));
    put(&h(".claude/skills/review/SKILL.md"), &skill("review", "\"Review a diff\"", ""));
    put(&h(".claude/skills/review/examples/inner/SKILL.md"), &skill("inner", "a skill's own example is no skill", ""));
    put(&h(".codex/skills/pdf/SKILL.md"), &skill("pdf", "Read and fill PDF forms", ""));
    put(&h(".gemini/skills/sql/SKILL.md"), &skill("sql", "'Query databases'", ""));
    put(&h(".config/opencode/skills/sql/SKILL.md"), &skill("sql", "Query databases", ""));
    link("../../../.agents/skills/pdf", &h(".pi/agent/skills/pdf"));
    put(&h(".hermes/skills/software-development/plan/SKILL.md"), &skill("plan", "Plan the work", ""));
    put(&h(".hermes/skills/.hub/cached/SKILL.md"), &skill("cached", "the hub's cache is no skill", ""));
    fs::create_dir_all(h(".hermes/skills/notes")).unwrap();
    let plugin = h(".claude/plugins/cache/official/frontend/abc123");
    let project_plugin = h(".claude/plugins/cache/official/other/def456");
    put(&format!("{plugin}/skills/frontend-design/SKILL.md"), &skill("frontend-design", "Design frontends", ""));
    put(&format!("{project_plugin}/skills/project-only/SKILL.md"), &skill("project-only", "a project's plugin", ""));
    put(
        &h(".claude/plugins/installed_plugins.json"),
        &json!({
            "version": 2,
            "plugins": {
                "frontend@official": [{ "scope": "user", "installPath": plugin }],
                "other@official": [{ "scope": "project", "projectPath": project, "installPath": project_plugin }],
            },
        })
        .to_string(),
    );

    // MCP servers, one config per agent the catalog names a format for.
    put(
        &h(".claude.json"),
        &json!({
            "mcpServers": {
                "airtable": { "command": "npx", "args": ["-y", "airtable-mcp-server"], "env": { "AIRTABLE_API_KEY": SECRET } },
                "notion": { "type": "http", "url": "https://mcp.notion.com/mcp" },
                "wsp": { "command": format!("{root}/wsp-bin/wsp"), "args": ["mcp"] },
            },
            "projects": { (home.clone()): { "mcpServers": { "local": { "command": "node", "args": ["server.js"] } } } },
        })
        .to_string(),
    );
    put(
        &h(".codex/config.toml"),
        &format!("model = \"gpt-5\"\n\n[mcp_servers.linear]\nurl = \"https://mcp.linear.app/sse\"\nhttp_headers = {{ Authorization = \"Bearer {SECRET}\" }}\n\n[mcp_servers.old]\ncommand = \"uvx\"\nargs = [\"old-server\"]\nenabled = false\n"),
    );
    put(
        &h(".gemini/settings.json"),
        &json!({ "mcpServers": { "fs": { "command": "npx", "args": ["-y", "@example/fs", format!("--token={SECRET}")] } } }).to_string(),
    );
    put(
        &h(".config/opencode/opencode.json"),
        &json!({ "mcp": { "ctx": { "type": "remote", "url": "https://ctx.example/mcp", "enabled": false } } }).to_string(),
    );

    // Login files a reader may stat and never read.
    put(&h(".codex/auth.json"), &json!({ "tokens": { "access_token": SECRET } }).to_string());
    put(&h(".claude/.credentials.json"), &json!({ "claudeAiOauth": { "accessToken": SECRET } }).to_string());
    put(&h(".gemini/oauth_creds.json"), &json!({ "access_token": SECRET }).to_string());

    // The project a workspace holds.
    put(
        &format!("{project}/.mcp.json"),
        &json!({ "mcpServers": { "project-db": { "command": "npx", "args": ["-y", "db-mcp"] } } }).to_string(),
    );
    put(&format!("{project}/.claude/skills/deploy/SKILL.md"), &skill("deploy", "Deploy the app", ""));
    put(&format!("{project}/.agents/skills/lint/SKILL.md"), &skill("lint", "Lint the app", ""));

    // Agents on the login's PATH, and the runners a server command would start, which say so if they ever run.
    let script = |name: &str, body: &str| {
        fs::create_dir_all(&bin).unwrap();
        write_stub(&format!("{bin}/{name}"), &format!("#!/bin/sh\n{body}\n"));
    };
    // Its plugin switch answers as 2.1.296's does.
    script(
        "claude",
        r#"case "$1" in --version) echo "2.1.281 (Claude Code)";; auth) echo '{"loggedIn": true, "authMethod": "claude.ai"}';; plugin) echo "{\"command\":\"$2\",\"outcome\":\"ok\",\"pluginId\":\"$3\",\"scope\":\"user\"}";; *) exit 2;; esac"#,
    );
    // Its app server answers a plugin read as 0.162.1 does with no plugins: the requests' ids in order, then it waits.
    script(
        "codex",
        r#"case "$1" in --version) echo "codex-cli 0.155.1";; login) echo "Not logged in"; exit 1;; app-server) printf '%s\n' '{"id":1,"result":{}}' '{"id":2,"result":{"marketplaces":[]}}' '{"id":3,"result":{"config":{}}}'; cat >/dev/null;; *) exit 2;; esac"#,
    );
    script(
        "hermes",
        r#"case "$1" in --version) echo "Hermes Agent v0.20.0 (2026.8.3)";; auth) echo "openrouter (1 credentials):";; *) exit 2;; esac"#,
    );
    for runner in ["npx", "uvx", "node", "wsp"] {
        script(runner, "touch \"$HOME/SPAWNED\"; exit 0");
    }
    AgentHome { home, bin, project }
}
