// SPDX-License-Identifier: AGPL-3.0-only
//! Port of the detectMcp cases of packages/collect/test/mcp.test.ts.
mod common;

use common::fake_host::*;
use md5::{Digest, Md5};
use serde_json::{json, Value};
use wsp_catalog::catalog::{catalog_agents, AgentEntry};
use wsp_catalog::mcp::MCP_SERVERS_JSON;
use wsp_catalog::mcp_config::McpConfig;
use wsp_collect::detect::mcp::detect_mcp;
use wsp_collect::manifest::ManifestEntry;

const NOTION: &str = "https://mcp.notion.com/mcp";

fn hash(text: &str) -> String {
    format!("{:x}", Md5::digest(text.as_bytes()))
}

fn claude_json(projects: Option<Value>) -> String {
    let home = HOME;
    let mono = format!("{HOME}/code/mono");
    let projects = projects.unwrap_or_else(|| {
        json!({
            (home): { "allowedTools": [], "mcpServers": { "zomato": { "type": "stdio", "command": "npx", "args": ["mcp-remote", "https://mcp.zomato.com/mcp"] } } },
            (mono): { "mcpServers": { "linear": { "type": "http", "url": "https://mcp.linear.app/sse" } } },
        })
    });
    json!({
        "numStartups": 3,
        "mcpServers": {
            "github": { "type": "stdio", "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"], "env": { "GITHUB_TOKEN": format!("{:x<40}", "ghp_x"), "GITHUB_HOST": "github.com" } },
            "notion": { "type": "http", "url": NOTION },
            "gsc": { "command": "uvx", "args": ["mcp-search-console"], "env": { "GSC_CREDENTIALS_PATH": format!("{HOME}/.config/gsc/creds.json") } },
            "notes": { "command": format!("{HOME}/Library/Application Support/Notes/mcp"), "args": [] },
        },
        "projects": projects,
    })
    .to_string()
}

fn with_claude(config: &str) -> FakeHost {
    fake_host(FakeLaptop::new().file("~/.claude.json", config))
}

fn rows_of(host: &FakeHost) -> Vec<ManifestEntry> {
    block_on(detect_mcp(host, None))
}

fn json_of(rows: &[ManifestEntry]) -> Value {
    serde_json::to_value(rows).unwrap()
}

/// The named fields of each row, a field the row does not carry as null (the TS `undefined`).
fn pick(rows: &[ManifestEntry], fields: &[&str]) -> Value {
    let all = json_of(rows);
    Value::Array(
        all.as_array()
            .unwrap()
            .iter()
            .map(|row| Value::Array(fields.iter().map(|f| row.get(*f).cloned().unwrap_or(Value::Null)).collect()))
            .collect(),
    )
}

fn details(rows: &[ManifestEntry]) -> Value {
    Value::Array(pick(rows, &["detail"]).as_array().unwrap().iter().map(|r| r[0].clone()).collect())
}

fn assert_hides(rows: &[ManifestEntry], secrets: &[&str]) {
    let text = serde_json::to_string(rows).unwrap();
    for secret in secrets {
        assert!(!text.contains(secret), "{secret} leaked into the rows: {text}");
    }
}

fn gemini_clone(id: &str, name: &str, files: &[&str], http_auth: Option<&str>) -> AgentEntry {
    let mut entry = catalog_agents().into_iter().find(|a| a.id == "gemini").unwrap().clone();
    entry.id = id.to_string();
    entry.name = name.to_string();
    entry.mcp = Some(McpConfig {
        format: MCP_SERVERS_JSON,
        files: files.iter().map(|f| f.to_string()).collect(),
        project_files: None,
        scope: "user scope".to_string(),
        http_auth: http_auth.map(|a| a.to_string()),
        check: None,
        login: None,
        oauth: None,
        headless: None,
        trusts: None,
    });
    entry
}

mod mcp_servers {
    use super::*;

    #[test]
    fn claude_code_one_row_per_user_scope_server_and_per_server_local_to_the_home() {
        let zomato = hash("https://mcp.zomato.com/mcp");
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.claude.json", claude_json(None))
                .file("~/.config/gsc/creds.json", 2100)
                .file(&format!("~/.mcp-auth/mcp-remote-v1/{zomato}_tokens.json"), 1400)
                .file(&format!("~/.mcp-auth/mcp-remote-v1/{zomato}_client_info.json"), 300)
                .file(&format!("~/.mcp-auth/mcp-remote-v1/{zomato}_lock.json"), 60)
                .file(&format!("~/.mcp-auth/mcp-remote-v1/{zomato}_code_verifier_2f1.txt"), 100)
                .file("~/.mcp-auth/mcp-remote-0.1.37/abc_tokens.json", 900),
        );
        let rows = rows_of(&host);
        let all = json_of(&rows);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "agents/mcp/claude/github",
                "agents/mcp/claude/notion",
                "agents/mcp/claude/gsc",
                "agents/mcp/claude/notes",
                "agents/mcp/claude/home/zomato",
                "agents/mcp/mcp-remote"
            ]
        );
        assert_eq!(
            all[0],
            json!({
                "rung": "agents", "id": "agents/mcp/claude/github", "label": "github", "group": "Claude Code MCP servers", "paths": [], "bytes": 0, "default": "bring", "consent": true, "keys": ["GITHUB_TOKEN"],
                "detail": "stdio: npx @modelcontextprotocol/server-github; runs via npx; carries a secret: env GITHUB_TOKEN (40 B)", "signIn": "token",
            })
        );
        assert_eq!(
            all[1],
            json!({
                "rung": "agents", "id": "agents/mcp/claude/notion", "label": "notion", "group": "Claude Code MCP servers", "paths": [], "bytes": 0, "default": "bring",
                "detail": "http: mcp.notion.com/mcp; nothing to install; its sign-in is kept with the Claude Code login", "signIn": "oauth",
            })
        );
        // The file a secret-named env value points at travels on the row; the path itself is rewritten on the machine.
        assert_eq!(
            all[2],
            json!({
                "rung": "agents", "id": "agents/mcp/claude/gsc", "label": "gsc", "group": "Claude Code MCP servers", "paths": ["~/.config/gsc/creds.json"], "bytes": 2100, "default": "bring", "consent": true, "keys": ["GSC_CREDENTIALS_PATH"],
                "detail": "stdio: uvx mcp-search-console; needs uv, installed on the machine when missing; carries a secret: the file GSC_CREDENTIALS_PATH points at (2 KB)", "signIn": "token",
            })
        );
        assert_eq!(
            all[3],
            json!({
                "rung": "agents", "id": "agents/mcp/claude/notes", "label": "notes", "group": "Claude Code MCP servers", "paths": [], "bytes": 0, "default": "skip",
                "reason": "command ~/Library/Application Support/Notes/mcp is macOS-only, will not run",
                "detail": "stdio: ~/Library/Application Support/Notes/mcp; carries no secret", "signIn": "none",
            })
        );
        assert_eq!(
            all[4],
            json!({
                "rung": "agents", "id": "agents/mcp/claude/home/zomato", "label": "zomato", "group": "Claude Code MCP servers", "paths": [], "bytes": 0, "default": "bring",
                "detail": "local to ~; stdio: npx mcp-remote (mcp.zomato.com/mcp); runs via npx; its saved sign-in (1 KB) travels on the mcp-remote sign-ins row", "signIn": "oauth",
            })
        );
        // The whole store is claimed; only what the current bridge reads travels.
        assert_eq!(
            all[5],
            json!({
                "rung": "agents", "id": "agents/mcp/mcp-remote", "label": "mcp-remote sign-ins", "group": "MCP sign-ins", "paths": ["~/.mcp-auth"],
                "excludes": ["~/.mcp-auth/mcp-remote-0.1.37", format!("~/.mcp-auth/mcp-remote-v1/{zomato}_lock.json")],
                "bytes": 1800, "default": "bring", "consent": true,
                "detail": "browser sign-ins saved by mcp-remote for remote servers: 1 token (1 KB), for zomato; older bridge versions' folders stay here",
            })
        );
        // The config itself is never listed here; it travels with the agent's row. Each agent's own file is read, the
        // first of its candidates that is there, and no token file.
        let reads: Vec<String> = host.calls().into_iter().filter(|c| c.starts_with("read ")).collect();
        let want: Vec<String> = [
            ".claude.json",
            ".codex/config.toml",
            ".gemini/settings.json",
            ".config/opencode/opencode.json",
            ".config/opencode/opencode.jsonc",
        ]
        .iter()
        .map(|f| format!("read {HOME}/{f}"))
        .collect();
        assert_eq!(reads, want);
    }

    #[test]
    fn a_server_local_to_another_project_stays_behind_its_repo_is_not_on_the_machine() {
        let rows = rows_of(&with_claude(&claude_json(None)));
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert!(!labels.contains(&"linear"));
        assert!(labels.contains(&"zomato"));
    }

    #[test]
    fn an_mcp_remote_server_without_a_saved_sign_in_says_the_browser_sign_in_runs_again() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.claude.json", claude_json(Some(json!({}))))
                .file("~/.mcp-auth/mcp-remote-0.1.37/abc_tokens.json", 900),
        );
        let rows = rows_of(&host);
        let remote =
            json_of(&rows).as_array().unwrap().iter().find(|r| r["id"] == "agents/mcp/mcp-remote").cloned().expect("the mcp-remote row");
        assert_eq!(
            remote,
            json!({
                "rung": "agents", "id": "agents/mcp/mcp-remote", "label": "mcp-remote sign-ins", "group": "MCP sign-ins", "paths": ["~/.mcp-auth"], "excludes": ["~/.mcp-auth/mcp-remote-0.1.37"], "bytes": 0,
                "default": "skip", "reason": "no saved sign-in the current bridge reads; older versions' folders are left here",
            })
        );
        let config =
            json!({ "mcpServers": { "z": { "command": "npx", "args": ["-y", "mcp-remote", "https://z.example/mcp"] } } }).to_string();
        let via_remote = rows_of(&with_claude(&config));
        assert_eq!(
            details(&via_remote),
            json!(["stdio: npx mcp-remote (z.example/mcp); runs via npx; no saved sign-in; the browser sign-in runs again on the machine"])
        );
    }

    #[test]
    fn codex_mcp_servers_tables_in_config_toml_env_sub_tables_included_with_the_bearer() {
        let toml = [
            "model = \"gpt-5\"",
            "",
            "[mcp_servers.grafana]",
            "command = \"/opt/homebrew/bin/uvx\"",
            "args = [\"mcp-grafana\"]",
            "",
            "[mcp_servers.grafana.env]",
            "GRAFANA_SERVICE_ACCOUNT_TOKEN = \"glsa_abcdefghij\"",
            "GRAFANA_URL = \"https://g.example\"",
            "",
            "# a remote one",
            "[mcp_servers.sentry]",
            "url = \"https://mcp.sentry.dev/mcp?x=1\"",
            "bearer_token_env_var = \"SENTRY_TOKEN\"",
            "",
            "[mcp_servers.\"my server\"]",
            "command = \"node\"",
            "args = [",
            "  \"/Applications/Tool.app/Contents/mcp.js\",",
            "]",
            "env = { A_KEY = \"1234\" }",
            "",
            "[[hooks.SessionStart]]",
            "matcher = \"x\"",
        ]
        .join("\n");
        let rows = rows_of(&fake_host(FakeLaptop::new().file("~/.codex/config.toml", toml)));
        assert_eq!(
            pick(&rows, &["id", "default", "reason", "detail"]),
            json!([
                ["agents/mcp/codex/grafana", "bring", null, "stdio: /opt/homebrew/bin/uvx mcp-grafana; needs uv, installed on the machine when missing; carries a secret: env GRAFANA_SERVICE_ACCOUNT_TOKEN (15 B)"],
                ["agents/mcp/codex/sentry", "bring", null, "http: mcp.sentry.dev/mcp; nothing to install; reads SENTRY_TOKEN from the environment, set it on the machine; carries no secret"],
                ["agents/mcp/codex/my server", "skip", "path /Applications/Tool.app/Contents/mcp.js is macOS-only, will not run", "stdio: node /Applications/Tool.app/Contents/mcp.js; carries a secret: env A_KEY (4 B)"],
            ])
        );
    }

    #[test]
    fn gemini_cli_and_opencode_mcp_servers_and_mcp_comments_and_trailing_commas_allowed() {
        let gemini = "{\n  // my servers\n  \"mcpServers\": { \"memory\": { \"command\": \"~/.local/bin/codebase-memory-mcp\" }, \"docs\": { \"httpUrl\": \"https://docs.example/mcp\", \"headers\": { \"Authorization\": \"Bearer abcdef\" } }, },\n}\n";
        let opencode = r#"{ "mcp": { "memory": { "type": "local", "command": ["/Users/dev/.local/bin/codebase-memory-mcp", "--v"], "environment": { "MEM_KEY": "k" }, "enabled": true }, "ctx": { "type": "remote", "url": "https://ctx.example/mcp" } } }"#;
        let host = fake_host(FakeLaptop::new().file("~/.gemini/settings.json", gemini).file("~/.config/opencode/opencode.jsonc", opencode));
        let rows = rows_of(&host);
        assert_eq!(
            pick(&rows, &["id", "group", "detail"]),
            json!([
                ["agents/mcp/gemini/memory", "Gemini CLI MCP servers", "stdio: ~/.local/bin/codebase-memory-mcp; needs codebase-memory-mcp on the machine; carries no secret"],
                ["agents/mcp/gemini/docs", "Gemini CLI MCP servers", "http: docs.example/mcp; nothing to install; carries a secret: header Authorization (13 B)"],
                ["agents/mcp/opencode/memory", "OpenCode MCP servers", "stdio: ~/.local/bin/codebase-memory-mcp --v; needs codebase-memory-mcp on the machine; carries a secret: env MEM_KEY (1 B)"],
                ["agents/mcp/opencode/ctx", "OpenCode MCP servers", "http: ctx.example/mcp; nothing to install; carries no secret"],
            ])
        );
    }

    #[test]
    fn a_value_after_a_secret_named_flag_or_inside_a_secret_named_assignment_is_a() {
        let config = json!({ "mcpServers": { "s": { "command": "npx", "args": ["-y", "some-server", "--api-key", "sk-123", "--token=abcdef", "API_KEY=zzz"] } } }).to_string();
        let rows = rows_of(&with_claude(&config));
        assert_eq!(
            details(&rows),
            json!(["stdio: npx some-server --api-key … --token=… API_KEY=…; runs via npx; carries secrets: flag --api-key (6 B), flag --token (6 B), arg API_KEY (3 B)"])
        );
        assert_hides(&rows, &["sk-123", "abcdef", "zzz"]);
    }

    #[test]
    fn an_argument_the_definition_does_not_name_is_a_value_hidden_and_counted_so_the() {
        let config = json!({
            "mcpServers": {
                "bare": { "command": "npx", "args": ["-y", "some-server", "sk-test-bare-token"] },
                "port": { "command": "npx", "args": ["-y", "some-server", "--port", "3000"] },
                "verbose": { "command": "npx", "args": ["-y", "some-server", "--verbose", "sk-test-token"] },
                "own": { "command": format!("{HOME}/bin/server"), "args": ["sk-test-token"] },
                "docker": { "command": "docker", "args": ["run", "-i", "--rm", "-e", "GITHUB_TOKEN=ghp-test-flag-secret", "ghcr.io/x/y"] },
                "dockerEq": { "command": "docker", "args": ["run", "--env=GITHUB_TOKEN=ghp-test-eq-secret", "ghcr.io/x/y"] },
                "env": { "command": "npx", "args": ["-y", "some-server", "--env", "API_KEY=sk-test-flag-secret"] },
                "dashes": { "command": "npx", "args": ["-y", "some-server", "--", "--verbose", "sk-after-dashes"] },
            }
        })
        .to_string();
        let rows = rows_of(&with_claude(&config));
        assert_eq!(
            pick(&rows, &["detail", "consent"]),
            json!([
                ["stdio: npx some-server …; runs via npx; carries a secret: arg 3 (18 B)", true],
                ["stdio: npx some-server --port 3000; runs via npx; carries no secret", null],
                // A flag's value is the flag's word: the definition names it, so it is shown whatever it holds.
                ["stdio: npx some-server --verbose sk-test-token; runs via npx; carries no secret", null],
                ["stdio: ~/bin/server …; needs ~/bin/server on the machine; carries a secret: arg 1 (13 B)", true],
                // A flag's value is read like any other argument, so a variable set behind -e or --env is a variable.
                ["stdio: docker … -i --rm -e GITHUB_TOKEN=… …; needs docker on the machine; carries secrets: arg 1 (3 B), arg GITHUB_TOKEN (20 B), arg 6 (11 B)", true],
                // The short spelling of the same argument reads the same as the spaced one.
                ["stdio: docker … --env=GITHUB_TOKEN=… …; needs docker on the machine; carries secrets: arg 1 (3 B), arg GITHUB_TOKEN (18 B), arg 3 (11 B)", true],
                ["stdio: npx some-server --env API_KEY=…; runs via npx; carries a secret: arg API_KEY (19 B)", true],
                // Past the two dashes there are no flags left: what follows is the program's own, whatever it is spelled like.
                ["stdio: npx some-server -- … …; runs via npx; carries secrets: arg 4 (9 B), arg 5 (15 B)", true],
            ])
        );
        assert_hides(
            &rows,
            &["sk-test-bare-token", "ghp-test-flag-secret", "ghp-test-eq-secret", "sk-test-flag-secret", "sk-after-dashes"],
        );
    }

    #[test]
    fn a_positional_address_a_path_and_a_name_value_whose_name_is_no_secrets_are_the() {
        let config = json!({
            "mcpServers": {
                "aws-mcp": { "command": "uvx", "args": ["mcp-proxy-for-aws@latest", "https://aws-mcp.us-east-1.api.aws/mcp", "--metadata", "AWS_REGION=us-east-1", "--metadata", "INSTALL_SOURCE=claude-code"] },
                "gsc": { "command": format!("{HOME}/mcp-gsc/.venv/bin/python"), "args": [format!("{HOME}/mcp-gsc/gsc_server.py")], "env": { "GSC_CREDENTIALS_PATH": format!("{HOME}/.config/gsc/creds.json"), "GSC_SKIP_OAUTH": "true" } },
                "login": { "command": "npx", "args": ["-y", "some-server", "https://acme:pw-test-login@x.example/mcp"] },
                "query": { "command": "npx", "args": ["-y", "some-server", "https://x.example/mcp?api_key=sk-test-query"] },
                "named": { "command": "npx", "args": ["-y", "some-server", "--api-key", "sk-test-flag", "-e", "SENTRY_ACCESS_TOKEN=sntrys-test"], "env": { "SENTRY_HOST": "sentry.io" } },
                "dburl": { "command": "docker", "args": ["run", "-e", "DATABASE_URL=postgres://app:pw-test-db@db.example/app", "ghcr.io/acme/lab"] },
                "dsn": { "command": "npx", "args": ["-y", "some-server", "--metadata", "DSN=https://pk-test-dsn@o1.ingest.example/1"] },
                "zapier": { "command": "npx", "args": ["-y", "mcp-remote", "https://mcp.zapier.com/api/mcp/s/ZmFrZS16YXBpZXItdGVzdC1wYXRoLXNlY3JldA==/mcp"] },
                "azure": { "command": "npx", "args": ["-y", "mcp-remote", "https://fn.example.net/runtime/webhooks/mcp/sse?code=az-test-code"] },
                "plaindb": { "command": "npx", "args": ["-y", "some-server", "--metadata", "DATABASE_URL=postgres://db.example/app"] },
            }
        })
        .to_string();
        let host = fake_host(
            FakeLaptop::new().file("~/.claude.json", config).file("~/.config/gsc/creds.json", 2100).file("~/mcp-gsc/gsc_server.py", 900),
        );
        let rows = rows_of(&host);
        assert_eq!(
            pick(&rows, &["label", "consent", "keys", "detail"]),
            json!([
                ["aws-mcp", null, null, "stdio: uvx mcp-proxy-for-aws@latest (aws-mcp.us-east-1.api.aws/mcp) --metadata AWS_REGION=us-east-1 --metadata …; needs uv, installed on the machine when missing; carries no secret"],
                ["gsc", true, ["GSC_CREDENTIALS_PATH"], "stdio: ~/mcp-gsc/.venv/bin/python ~/mcp-gsc/gsc_server.py; needs ~/mcp-gsc/.venv/bin/python on the machine; depends on ~/mcp-gsc/gsc_server.py, which comes along only if a row carries it; carries a secret: the file GSC_CREDENTIALS_PATH points at (2 KB)"],
                ["login", true, ["argument 3"], "stdio: npx some-server …; runs via npx; carries a secret: arg 3 (40 B)"],
                ["query", true, ["argument 3"], "stdio: npx some-server …; runs via npx; carries a secret: arg 3 (43 B)"],
                ["named", true, ["--api-key", "SENTRY_ACCESS_TOKEN"], "stdio: npx some-server --api-key … -e SENTRY_ACCESS_TOKEN=…; runs via npx; carries secrets: flag --api-key (12 B), arg SENTRY_ACCESS_TOKEN (11 B)"],
                // An address with a login is a value under any name and any scheme, the same rule a positional one keeps.
                ["dburl", true, ["argument 1", "DATABASE_URL", "argument 4"], "stdio: docker … -e DATABASE_URL=… …; needs docker on the machine; carries secrets: arg 1 (3 B), arg DATABASE_URL (40 B), arg 4 (16 B)"],
                ["dsn", true, ["DSN"], "stdio: npx some-server --metadata DSN=…; runs via npx; carries a secret: arg DSN (39 B)"],
                // A key in an address's path or under a query name that says nothing asks too: asking costs one question.
                ["zapier", true, ["argument 3"], "stdio: npx mcp-remote …; runs via npx; carries a secret: arg 3 (77 B); no saved sign-in; the browser sign-in runs again on the machine"],
                ["azure", true, ["argument 3"], "stdio: npx mcp-remote …; runs via npx; carries a secret: arg 3 (65 B); no saved sign-in; the browser sign-in runs again on the machine"],
                ["plaindb", null, null, "stdio: npx some-server --metadata DATABASE_URL=postgres://db.example/app; runs via npx; carries no secret"],
            ])
        );
        assert_hides(
            &rows,
            &["pw-test-login", "sk-test-query", "sk-test-flag", "sntrys-test", "pw-test-db", "pk-test-dsn", "ZmFrZS16YXBp", "az-test-code"],
        );
    }

    #[test]
    fn a_secret_named_env_value_pointing_outside_home_is_named_without_its_value_a_home() {
        let config = json!({
            "mcpServers": {
                "outside": { "command": "npx", "args": ["x"], "env": { "CREDENTIALS_PATH": "/etc/creds.json" } },
                "whatsapp": { "command": format!("{HOME}/.local/bin/uv"), "args": ["--directory", format!("{HOME}/whatsapp-mcp/server"), "run", "main.py"] },
                "memory": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-memory"], "env": { "MEMORY_FILE_PATH": format!("{HOME}/.claude/memory.json") } },
                "sqlite": { "command": "uvx", "args": ["mcp-server-sqlite", "--db-path", "~/notes.db"] },
            }
        })
        .to_string();
        let host = fake_host(
            FakeLaptop::new().file("~/.claude.json", config).file("/etc/creds.json", 10).file("~/whatsapp-mcp/server/main.py", 20),
        );
        let rows = rows_of(&host);
        assert_eq!(
            pick(&rows, &["default", "reason", "detail"]),
            json!([
                ["bring", null, "stdio: npx x; runs via npx; the file CREDENTIALS_PATH points at is outside your home and is not copied"],
                ["bring", null, "stdio: ~/.local/bin/uv --directory ~/whatsapp-mcp/server run …; needs uv, installed on the machine when missing; depends on ~/whatsapp-mcp/server, which comes along only if a row carries it; carries a secret: arg 4 (7 B)"],
                ["skip", null, "stdio: npx @modelcontextprotocol/server-memory; runs via npx; ~/.claude/memory.json is not on this computer; unticked, tick it if the server creates it on first start; carries no secret"],
                ["skip", null, "stdio: uvx mcp-server-sqlite --db-path ~/notes.db; needs uv, installed on the machine when missing; ~/notes.db is not on this computer; unticked, tick it if the server creates it on first start; carries no secret"],
            ])
        );
        assert_hides(&rows, &["/etc/creds.json"]);
    }

    #[test]
    fn mcp_remotes_static_oauth_client_info_blob_is_a_secret_whatever_its_name_says_a() {
        let config = json!({
            "mcpServers": {
                "remote": { "command": "npx", "args": ["mcp-remote", NOTION, "--static-oauth-client-info", r#"{"client_id":"abc","client_secret":"shh-secret"}"#] },
                "sw": { "command": "npx", "args": ["some-server", "--auth", "--transport", "http-only"] },
            }
        })
        .to_string();
        let rows = rows_of(&with_claude(&config));
        assert_eq!(
            details(&rows),
            json!([
                "stdio: npx mcp-remote (mcp.notion.com/mcp) --static-oauth-client-info …; runs via npx; carries a secret: flag --static-oauth-client-info (48 B); no saved sign-in; the browser sign-in runs again on the machine",
                "stdio: npx some-server --auth --transport http-only; runs via npx; carries no secret",
            ])
        );
        assert_hides(&rows, &["shh-secret"]);
    }

    #[test]
    fn a_config_that_does_not_parse_or_has_no_servers_adds_no_row() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.claude.json", "{not json")
                .file("~/.gemini/settings.json", "{}")
                .file("~/.codex/config.toml", "model = \"x\"\n"),
        );
        assert_eq!(json_of(&rows_of(&host)), json!([]));
        assert_eq!(json_of(&rows_of(&empty_host())), json!([]));
    }

    // Adapted: the TS builds an McpFormat of functions that reads a one-server-per-line text file. McpFormat is an
    // enum here, so the new agent is a clone of a catalog agent under the mcpServers JSON format, its file the same
    // two servers in that format.
    #[test]
    fn an_agent_the_catalog_gains_with_a_format_of_its_own_is_read_through_its_module() {
        let mut entry = catalog_agents().into_iter().find(|a| a.id == "pi").unwrap().clone();
        entry.id = "lines".to_string();
        entry.name = "Lines".to_string();
        entry.mcp = Some(McpConfig {
            format: MCP_SERVERS_JSON,
            files: vec!["~/.lines/servers.json".to_string()],
            project_files: None,
            scope: "one file".to_string(),
            http_auth: None,
            check: None,
            login: None,
            oauth: None,
            headless: None,
            trusts: None,
        });
        let servers = json!({ "mcpServers": { "alpha": { "command": "npx", "args": ["-y", "pkg"] }, "beta": { "command": "/Applications/B.app/b" } } }).to_string();
        let host = fake_host(FakeLaptop::new().file("~/.lines/servers.json", servers).file("~/.claude.json", claude_json(None)));
        let rows = block_on(detect_mcp(&host, Some(&[&entry])));
        assert_eq!(
            pick(&rows, &["id", "group", "default", "detail"]),
            json!([
                ["agents/mcp/lines/alpha", "Lines MCP servers", "bring", "stdio: npx pkg; runs via npx; carries no secret"],
                ["agents/mcp/lines/beta", "Lines MCP servers", "skip", "stdio: /Applications/B.app/b; carries no secret"],
            ])
        );
    }

    #[test]
    fn an_http_servers_sign_in_note_is_what_the_agents_mcp_module_says_an_entry_that() {
        let notion = json!({ "mcpServers": { "notion": { "type": "http", "url": NOTION } } }).to_string();
        let host = fake_host(FakeLaptop::new().file("~/.remote.json", notion));
        let said = gemini_clone("remote", "Remote", &["~/.remote.json"], Some("its sign-in is kept with the Remote login"));
        let rows = block_on(detect_mcp(&host, Some(&[&said])));
        assert_eq!(
            rows[0].detail.as_deref(),
            Some("http: mcp.notion.com/mcp; nothing to install; its sign-in is kept with the Remote login")
        );
        let silent = gemini_clone("remote", "Remote", &["~/.remote.json"], None);
        let rows = block_on(detect_mcp(&host, Some(&[&silent])));
        assert_eq!(rows[0].detail.as_deref(), Some("http: mcp.notion.com/mcp; nothing to install; carries no secret"));
    }

    #[test]
    fn says_how_each_server_signs_in_nothing_a_key_or_a_token_it_carries_or_a_browser() {
        let config = json!({
            "mcpServers": {
                "context7": { "type": "http", "url": "https://mcp.context7.com/mcp", "headers": { "CONTEXT7_API_KEY": "ctx7sk-x" } },
                "linear": { "type": "http", "url": "https://mcp.linear.app/mcp" },
                "firecrawl": { "command": "npx", "args": ["-y", "firecrawl-mcp"], "env": { "FIRECRAWL_API_KEY": "fc-x" } },
                "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"], "env": { "GITHUB_TOKEN": "ghp_x" } },
                "playwright": { "command": "npx", "args": ["@playwright/mcp@latest"] },
                "zomato": { "command": "npx", "args": ["mcp-remote", "https://mcp.zomato.com/mcp"] },
            }
        })
        .to_string();
        let rows = rows_of(&with_claude(&config));
        let kinds: serde_json::Map<String, Value> = rows
            .iter()
            .filter(|r| r.label != "mcp-remote sign-ins")
            .map(|r| (r.label.clone(), serde_json::to_value(r.sign_in).unwrap()))
            .collect();
        assert_eq!(
            Value::Object(kinds),
            json!({ "context7": "key", "linear": "oauth", "firecrawl": "key", "github": "token", "playwright": "none", "zomato": "oauth" })
        );
        // An http server on an agent that keeps no sign-in for it carries none.
        let remote = gemini_clone("remote", "Remote", &["~/.remote.json"], None);
        let notion = json!({ "mcpServers": { "notion": { "type": "http", "url": NOTION } } }).to_string();
        let bare = fake_host(FakeLaptop::new().file("~/.remote.json", notion));
        let rows = block_on(detect_mcp(&bare, Some(&[&remote])));
        assert_eq!(serde_json::to_value(rows[0].sign_in).unwrap(), json!("none"));
    }
}

/// Rules of spec/mcp.md the TS cases leave unpinned.
mod pins {
    use super::*;
    use indexmap::IndexMap;
    use wsp_collect::detect::mcp::{mcp_remote_token_file, mcp_remote_tokens};

    #[test]
    fn mcp_remote_token_file_follows_mcp_remote_config_dir_unless_it_is_empty() {
        let host = empty_host();
        let env = |v: Option<&str>| -> IndexMap<String, String> {
            v.map(|d| ("MCP_REMOTE_CONFIG_DIR".to_string(), d.to_string())).into_iter().collect()
        };
        assert_eq!(mcp_remote_token_file(&host, "abc", &env(None)), "/Users/dev/.mcp-auth/mcp-remote-v1/abc_tokens.json");
        assert_eq!(mcp_remote_token_file(&host, "abc", &env(Some(""))), "/Users/dev/.mcp-auth/mcp-remote-v1/abc_tokens.json");
        assert_eq!(mcp_remote_token_file(&host, "abc", &env(Some("/cfg"))), "/cfg/mcp-remote-v1/abc_tokens.json");
    }

    #[test]
    fn mcp_remote_tokens_is_empty_without_a_store_and_never_reads_a_token() {
        let a = "0123456789abcdef0123456789abcdef";
        let b = "fedcba9876543210fedcba9876543210";
        let host = fake_host(
            FakeLaptop::new()
                .file(&format!("~/.mcp-auth/mcp-remote-v1/{b}_tokens.json"), "{\"access_token\":\"x\"}")
                .file(&format!("~/.mcp-auth/mcp-remote-v1/{a}_tokens.json"), 700)
                .file(&format!("~/.mcp-auth/mcp-remote-v1/{a}_lock.json"), 5)
                .file(&format!("~/.mcp-auth/mcp-remote-v1/{a}_client_info.json"), 90),
        );
        let got = block_on(mcp_remote_tokens(&host));
        // Listing order, which the fake sorts; each value the file's size.
        assert_eq!(got.into_iter().collect::<Vec<_>>(), vec![(a.to_string(), 700), (b.to_string(), 20)]);
        assert_eq!(host.calls(), Vec::<String>::new());
        assert!(block_on(mcp_remote_tokens(&empty_host())).is_empty());
        // A file where the store's folder should be is no store.
        let flat = fake_host(FakeLaptop::new().file("~/.mcp-auth", 3));
        assert!(block_on(mcp_remote_tokens(&flat)).is_empty());
    }
}

/// Every case of tests/fixtures/oracle/w2/mcp.json (oracle/w2-mcp.oracle.ts): spec mcp's wave-2 rules.
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, host, w2};
    use indexmap::IndexMap;
    use wsp_catalog::mcp::McpServer;
    use wsp_collect::detect::mcp::{mcp_remote_token_file, mcp_remote_tokens, mcp_server_row};

    fn agent(id: &str) -> &'static AgentEntry {
        catalog_agents().into_iter().find(|a| a.id == id).unwrap()
    }

    fn rows(r: &[ManifestEntry]) -> Value {
        Value::Array(r.iter().map(|r| serde_json::to_value(r).unwrap()).collect())
    }

    #[test]
    fn detect_mcp_writes_the_rows_the_ts_wrote_with_the_same_reads() {
        for case in cases(&w2("mcp"), "detect_mcp") {
            let h = host(&case["laptop"]);
            let ids: Option<Vec<&'static AgentEntry>> =
                case["args"][0].as_array().map(|a| a.iter().map(|id| agent(id.as_str().unwrap())).collect());
            assert_eq!(rows(&block_on(detect_mcp(&h, ids.as_deref()))), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }

    #[test]
    fn tokens_token_files_and_one_row_answer_what_the_ts_answered() {
        let f = w2("mcp");
        for case in cases(&f, "mcp_remote_tokens") {
            let h = host(&case["laptop"]);
            let got = block_on(mcp_remote_tokens(&h));
            let got = if case["out"].is_array() { Value::Array(got.iter().map(|(k, v)| json!([k, v])).collect()) } else { json!(got) };
            assert_eq!(got, case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
        for case in cases(&f, "mcp_remote_token_file") {
            let h = host(&case["laptop"]);
            let vars: IndexMap<String, String> =
                case["args"][1].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
            assert_eq!(json!(mcp_remote_token_file(&h, case["args"][0].as_str().unwrap(), &vars)), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "mcp_server_row") {
            let h = host(&case["laptop"]);
            let a = &case["args"];
            let server: McpServer = serde_json::from_value(a[1].clone()).unwrap();
            let tokens: IndexMap<String, u64> =
                a[2].as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.as_u64().unwrap())).collect();
            let row = block_on(mcp_server_row(&h, agent(a[0].as_str().unwrap()), &server, &tokens));
            assert_eq!(serde_json::to_value(&row).unwrap(), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }
}
