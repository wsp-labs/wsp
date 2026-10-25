// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/mcp-launch.test.ts, with the helper of packages/catalog/test/turn-files.ts.
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};
use wsp_catalog::mcp::{McpFormat, McpServer, CODEX_TOML, MCP_SERVERS_JSON, OPENCODE_JSON};
use wsp_catalog::mcp_launch::{claude_launch_servers, codex_launch_config, launch_misses, LaunchConfigs, TurnFile};

fn own(text: &str) -> TurnFile {
    TurnFile { path: "/root/.claude.json".to_string(), text: text.to_string() }
}

fn proj(text: &str) -> TurnFile {
    proj_at(text, "/root/spoo-ts/.mcp.json")
}

fn proj_at(text: &str, path: &str) -> TurnFile {
    TurnFile { path: path.to_string(), text: text.to_string() }
}

fn values() -> IndexMap<String, String> {
    IndexMap::from([
        ("LINEAR_TOKEN".to_string(), "lin_TESTONLY".to_string()),
        ("GH_TOKEN".to_string(), "ghp_TESTONLY".to_string()),
        ("NOTION_TOKEN".to_string(), "ntn_TESTONLY".to_string()),
    ])
}

fn configs(user: Option<TurnFile>, projects: Option<Vec<TurnFile>>, folder: &str) -> LaunchConfigs {
    LaunchConfigs { user, projects, folder: folder.to_string(), key: None }
}

fn sorted_keys<V>(map: &IndexMap<String, V>) -> Vec<String> {
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort();
    keys
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn held(names: &[&str]) -> IndexSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

fn one(format: McpFormat, text: &str) -> McpServer {
    format.read(text, "/root").remove(0)
}

mod a_claude_code_launchs_servers {
    use super::*;

    fn user() -> String {
        json!({
            "mcpServers": {
                "tracker": {
                    "type": "http",
                    "url": "https://mcp.linear.app/mcp",
                    "headers": { "Authorization": "Bearer ${LINEAR_TOKEN}" },
                    "oauth": { "clientId": "abc", "callbackPort": 8080 },
                },
                "gh": {
                    "command": "gh-mcp",
                    "args": ["--token=${GH_TOKEN:-}"],
                    "env": { "GH_TOKEN": "${GH_TOKEN:-none}", "LOG": "${LOG:-info}", "HOME_AT": "${HOME}/x" },
                    "timeout": 60000,
                },
                "mine": { "command": "mine-mcp", "args": [], "env": { "OTHER": "${NOT_HELD}" } },
                "off": { "command": "off-mcp", "env": { "T": "${GH_TOKEN}" } },
            },
            "projects": {
                "/root": { "mcpServers": { "homeonly": { "command": "h", "env": { "N": "${NOTION_TOKEN}" } } } },
                "/root/spoo-ts": {
                    "disabledMcpServers": ["off"],
                    "mcpServers": { "local": { "command": "l", "env": { "N": "${NOTION_TOKEN}" } } },
                },
            },
        })
        .to_string()
    }

    #[test]
    fn is_each_server_that_reads_a_held_value_whole_the_value_in_place_and_every_other() {
        let got = claude_launch_servers(&configs(Some(own(&user())), None, "/root/spoo-ts"), &values());
        assert_eq!(sorted_keys(&got), strings(&["gh", "local", "tracker"]));
        assert_eq!(
            got["tracker"],
            json!({
                "type": "http",
                "url": "https://mcp.linear.app/mcp",
                "headers": { "Authorization": "Bearer lin_TESTONLY" },
                "oauth": { "clientId": "abc", "callbackPort": 8080 },
            })
        );
        // A reference with a default is filled like a plain one; one the vault does not hold is left for the agent.
        assert_eq!(
            got["gh"],
            json!({
                "command": "gh-mcp",
                "args": ["--token=ghp_TESTONLY"],
                "env": { "GH_TOKEN": "ghp_TESTONLY", "LOG": "${LOG:-info}", "HOME_AT": "${HOME}/x" },
                "timeout": 60000,
            })
        );
    }

    #[test]
    fn fills_the_home_folders_own_servers_for_a_turn_there_and_the_projects_mcp_json_be() {
        let at_home = claude_launch_servers(&configs(Some(own(&user())), None, "/root"), &values());
        assert_eq!(sorted_keys(&at_home), strings(&["gh", "homeonly", "off", "tracker"]));
        let project = json!({
            "mcpServers": {
                "shared": { "command": "s", "env": { "T": "${LINEAR_TOKEN}" } },
                "local": { "command": "from-project", "env": { "N": "${NOTION_TOKEN}" } },
            },
        })
        .to_string();
        let got = claude_launch_servers(&configs(Some(own(&user())), Some(vec![proj(&project)]), "/root/spoo-ts"), &values());
        assert_eq!(got["shared"], json!({ "command": "s", "env": { "T": "lin_TESTONLY" } }));
        assert_eq!(got["local"], json!({ "command": "l", "env": { "N": "ntn_TESTONLY" } }));
    }

    #[test]
    fn fills_a_server_of_a_mcp_json_above_the_folder_under_the_nearer_one_of_the_same_n() {
        let repo = proj(&json!({ "mcpServers": { "shared": { "command": "near", "env": { "T": "${LINEAR_TOKEN}" } } } }).to_string());
        let parent = proj_at(
            &json!({
                "mcpServers": {
                    "shared": { "command": "far", "env": { "T": "${LINEAR_TOKEN}" } },
                    "above": { "command": "a", "env": { "G": "${GH_TOKEN}" } },
                },
            })
            .to_string(),
            "/root/.mcp.json",
        );
        let got = claude_launch_servers(&configs(Some(own(&user())), Some(vec![repo, parent]), "/root/spoo-ts"), &values());
        assert_eq!(got["shared"], json!({ "command": "near", "env": { "T": "lin_TESTONLY" } }));
        assert_eq!(got["above"], json!({ "command": "a", "env": { "G": "ghp_TESTONLY" } }));
    }

    #[test]
    fn is_nothing_with_no_value_held_or_no_file() {
        let nothing: IndexMap<String, Value> = IndexMap::new();
        assert_eq!(claude_launch_servers(&configs(Some(own(&user())), None, "/root"), &IndexMap::new()), nothing);
        assert_eq!(claude_launch_servers(&configs(None, None, "/root"), &values()), nothing);
        assert_eq!(claude_launch_servers(&configs(Some(own("not json")), None, "/root"), &values()), nothing);
    }
}

mod a_codex_thread_starts_config {
    use super::*;

    fn user() -> String {
        [
            "model = \"gpt\"",
            "[mcp_servers.notion]",
            "command = \"npx\"",
            "args = [\"notion-mcp\"]",
            "env_vars = [\"NOTION_TOKEN\", \"HTTPS_PROXY\"]",
            "disabled_tools = [\"delete_page\"]",
            "startup_timeout_sec = 40",
            "[mcp_servers.tracker]",
            "url = \"https://mcp.linear.app/mcp\"",
            "env_http_headers = { \"Authorization\" = \"LINEAR_TOKEN\" }",
            "[mcp_servers.bearer]",
            "url = \"https://b.example/mcp\"",
            "bearer_token_env_var = \"GH_TOKEN\"",
            "[mcp_servers.off]",
            "command = \"x\"",
            "env_vars = [\"GH_TOKEN\"]",
            "enabled = false",
            "",
        ]
        .join("\n")
    }

    fn project() -> String {
        ["[mcp_servers.proj]", "command = \"p\"", "env_vars = [\"GH_TOKEN\"]", ""].join("\n")
    }

    #[test]
    fn sets_only_the_held_names_on_each_server_that_passes_them_through_so_codex_lays_t() {
        assert_eq!(
            codex_launch_config(&configs(Some(own(&user())), None, "/root/spoo-ts"), &values()),
            IndexMap::from([
                ("mcp_servers.notion.env.NOTION_TOKEN".to_string(), "ntn_TESTONLY".to_string()),
                ("mcp_servers.tracker.http_headers.Authorization".to_string(), "lin_TESTONLY".to_string()),
            ])
        );
    }

    #[test]
    fn reads_the_projects_file_only_where_codexs_own_marks_the_folder_trusted() {
        let got = codex_launch_config(&configs(Some(own(&user())), Some(vec![proj(&project())]), "/root/spoo-ts"), &values());
        assert!(!got.contains_key("mcp_servers.proj.env.GH_TOKEN"));
        let trusted = format!("{}[projects.\"/root/spoo-ts\"]\ntrust_level = \"trusted\"\n", user());
        let got = codex_launch_config(&configs(Some(own(&trusted)), Some(vec![proj(&project())]), "/root/spoo-ts"), &values());
        assert_eq!(got.get("mcp_servers.proj.env.GH_TOKEN").map(String::as_str), Some("ghp_TESTONLY"));
    }

    #[test]
    fn reads_a_folders_trust_however_the_toml_spells_it_a_dotted_key_and_an_inline_tabl() {
        for trust in [
            "projects.\"/root/spoo-ts\".trust_level = \"trusted\"",
            "projects = { \"/root/spoo-ts\" = { trust_level = \"trusted\" } }",
        ] {
            let text = format!("{trust}\n{}", user());
            let got = codex_launch_config(&configs(Some(own(&text)), Some(vec![proj(&project())]), "/root/spoo-ts"), &values());
            assert_eq!(got.get("mcp_servers.proj.env.GH_TOKEN").map(String::as_str), Some("ghp_TESTONLY"), "{trust}");
        }
        let untrusted = format!("projects.\"/root/spoo-ts\".trust_level = \"untrusted\"\n{}", user());
        let got = codex_launch_config(&configs(Some(own(&untrusted)), Some(vec![proj(&project())]), "/root/spoo-ts"), &values());
        assert!(!got.contains_key("mcp_servers.proj.env.GH_TOKEN"));
    }
}

mod the_held_names_a_launch_on_a_computer_the_person_owns_cannot_hand_a_server {
    use super::*;

    #[test]
    fn are_none_for_claude_code_which_takes_every_server_whole_with_its_values_in_place() {
        let server = one(
            MCP_SERVERS_JSON,
            &json!({ "mcpServers": { "gh": { "command": "gh-mcp", "env": { "GH_TOKEN": "${GH_TOKEN}" } } } }).to_string(),
        );
        assert_eq!(launch_misses("claude", MCP_SERVERS_JSON, &server, &held(&["LINEAR_TOKEN", "GH_TOKEN"])), Vec::<String>::new());
    }

    #[test]
    fn are_the_one_a_codex_server_reads_through_its_bearer_variable_and_none_it_passes() {
        let servers = CODEX_TOML.read(
            &[
                "[mcp_servers.web]",
                "url = \"https://b.example/mcp\"",
                "bearer_token_env_var = \"LINEAR_TOKEN\"",
                "[mcp_servers.gh]",
                "command = \"gh-mcp\"",
                "env_vars = [\"GH_TOKEN\"]",
                "[mcp_servers.hdr]",
                "url = \"https://h.example\"",
                "env_http_headers = { \"X-Key\" = \"LINEAR_TOKEN\" }",
                "",
            ]
            .join("\n"),
            "/root",
        );
        let held = held(&["LINEAR_TOKEN", "GH_TOKEN"]);
        let got: Vec<(String, Vec<String>)> = servers.iter().map(|s| (s.name.clone(), launch_misses("codex", CODEX_TOML, s, &held))).collect();
        assert_eq!(
            got,
            vec![
                ("web".to_string(), strings(&["LINEAR_TOKEN"])),
                ("gh".to_string(), vec![]),
                ("hdr".to_string(), vec![]),
            ]
        );
    }

    #[test]
    fn are_every_held_name_the_server_reads_for_an_agent_whose_launch_takes_no_servers() {
        let held = held(&["LINEAR_TOKEN", "GH_TOKEN"]);
        let remote = one(
            OPENCODE_JSON,
            &json!({
                "mcp": {
                    "tracker": {
                        "type": "remote",
                        "url": "https://mcp.linear.app/mcp",
                        "headers": { "Authorization": "Bearer {env:LINEAR_TOKEN}" },
                    },
                },
            })
            .to_string(),
        );
        let local = one(
            OPENCODE_JSON,
            &json!({
                "mcp": {
                    "gh": {
                        "type": "local",
                        "command": ["gh-mcp"],
                        "environment": { "GH_TOKEN": "{env:GH_TOKEN}", "LOG": "{env:LOG}" },
                    },
                },
            })
            .to_string(),
        );
        assert_eq!(launch_misses("opencode", OPENCODE_JSON, &remote, &held), strings(&["LINEAR_TOKEN"]));
        assert_eq!(launch_misses("opencode", OPENCODE_JSON, &local, &held), strings(&["GH_TOKEN"]));
        let cursor = one(
            MCP_SERVERS_JSON,
            &json!({ "mcpServers": { "gh": { "command": "gh-mcp", "env": { "GH_TOKEN": "${GH_TOKEN}" } } } }).to_string(),
        );
        assert_eq!(launch_misses("cursor", MCP_SERVERS_JSON, &cursor, &held), strings(&["GH_TOKEN"]));
    }
}
