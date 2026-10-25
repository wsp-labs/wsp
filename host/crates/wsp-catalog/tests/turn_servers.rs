// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/turn-servers.test.ts.
use serde_json::{json, Value};
use wsp_catalog::catalog::mcp_agents;
use wsp_catalog::mcp::{CODEX_TOML, MCP_SERVERS_JSON};
use wsp_catalog::mcp_codex::codex_trusts;
use wsp_catalog::mcp_launch::{
    checkout_of, started_servers, turn_server_files, turn_servers, Checkout, LaunchConfigs, ServerFiles, TurnAgent, TurnFile, TurnScope,
    TurnServer,
};

fn agent(id: &str) -> TurnAgent {
    let entry = mcp_agents().into_iter().find(|a| a.id == id).unwrap();
    let mcp = entry.mcp.as_ref().unwrap();
    TurnAgent { id: entry.id.clone(), format: mcp.format, files: mcp.files.clone(), project_files: mcp.project_files.clone() }
}

fn stdio(name: &str) -> Value {
    json!({ "command": "node", "args": [format!("/srv/{name}.mjs")] })
}

fn scope_name(scope: TurnScope) -> &'static str {
    match scope {
        TurnScope::User => "user",
        TurnScope::Local => "local",
        TurnScope::Project => "project",
    }
}

type Row = [String; 4];

fn row(name: &str, scope: &str, file: &str, state: &str) -> Row {
    [name.to_string(), scope.to_string(), file.to_string(), state.to_string()]
}

fn said(servers: &[TurnServer]) -> Vec<Row> {
    servers
        .iter()
        .map(|s| row(&s.server.name, scope_name(s.scope), &s.file, if s.server.disabled == Some(true) { "off" } else { "on" }))
        .collect()
}

fn name_scope(servers: &[TurnServer]) -> Vec<(String, String)> {
    servers.iter().map(|s| (s.server.name.clone(), scope_name(s.scope).to_string())).collect()
}

fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

fn names(servers: &[TurnServer]) -> Vec<String> {
    servers.iter().map(|s| s.server.name.clone()).collect()
}

fn file(path: &str, text: String) -> TurnFile {
    TurnFile { path: path.to_string(), text }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

mod a_claude_code_turns_servers {
    use super::*;

    fn user() -> TurnFile {
        file(
            "/root/.claude-cfg/.claude.json",
            json!({
                "mcpServers": { "userone": stdio("userone"), "shared": stdio("user-shared") },
                "projects": {
                    "/root": { "mcpServers": { "homeonly": stdio("homeonly") } },
                    "/root/acme": {
                        "mcpServers": { "localone": stdio("localone"), "shared": stdio("local-shared") },
                        "disabledMcpServers": ["userone"],
                        "disabledMcpjsonServers": ["leftout"],
                    },
                },
            })
            .to_string(),
        )
    }

    fn repo() -> TurnFile {
        file("/root/acme/.mcp.json", json!({ "mcpServers": { "inrepo": stdio("inrepo"), "shared": stdio("repo-shared") } }).to_string())
    }

    fn parent() -> TurnFile {
        file(
            "/root/.mcp.json",
            json!({ "mcpServers": { "parentone": stdio("parentone"), "parenttwo": stdio("parenttwo"), "leftout": stdio("leftout") } }).to_string(),
        )
    }

    fn configs() -> LaunchConfigs {
        LaunchConfigs { user: Some(user()), projects: Some(vec![repo(), parent()]), folder: "/root/acme".to_string(), key: None }
    }

    #[test]
    fn is_the_folders_local_entry_then_each_mcp_json_from_the_folder_up_then_the_user_f() {
        let user = user();
        let repo = repo();
        let parent = parent();
        assert_eq!(
            said(&turn_servers(&agent("claude"), &configs())),
            vec![
                row("localone", "local", &user.path, "on"),
                row("shared", "local", &user.path, "on"),
                row("inrepo", "project", &repo.path, "on"),
                row("shared", "project", &repo.path, "on"),
                row("parentone", "project", &parent.path, "on"),
                row("parenttwo", "project", &parent.path, "on"),
                row("userone", "user", &user.path, "off"),
                row("shared", "user", &user.path, "on"),
            ]
        );
    }

    #[test]
    fn starts_the_first_of_each_name_and_none_switched_off_and_leaves_the_homes_own_ent() {
        let started = started_servers(&turn_servers(&agent("claude"), &configs()));
        assert_eq!(
            name_scope(&started),
            pairs(&[("localone", "local"), ("shared", "local"), ("inrepo", "project"), ("parentone", "project"), ("parenttwo", "project")])
        );
        let home = started_servers(&turn_servers(
            &agent("claude"),
            &LaunchConfigs { user: Some(user()), projects: Some(vec![parent()]), folder: "/root".to_string(), key: None },
        ));
        assert_eq!(
            name_scope(&home),
            pairs(&[
                ("homeonly", "local"),
                ("parentone", "project"),
                ("parenttwo", "project"),
                ("leftout", "project"),
                ("userone", "user"),
                ("shared", "user"),
            ])
        );
    }

    #[test]
    fn keys_a_worktrees_local_entry_by_the_projects_folder_as_claude_code_keys_it_by_th() {
        let started = started_servers(&turn_servers(
            &agent("claude"),
            &LaunchConfigs {
                user: Some(user()),
                projects: Some(vec![]),
                folder: "/root/.wsp/worktrees/acme-fix".to_string(),
                key: Some("/root/acme".to_string()),
            },
        ));
        assert_eq!(names(&started), strings(&["localone", "shared"]));
    }

    #[test]
    fn keys_a_subfolders_local_entry_by_its_checkouts_top_as_git_answers_it_there() {
        assert_eq!(
            checkout_of(Some("/root/acme\n/root/acme/.git\n"), "/root/acme/web"),
            Checkout { top: Some("/root/acme".to_string()), key: "/root/acme".to_string() }
        );
        assert_eq!(
            checkout_of(Some("/root/.wsp/worktrees/acme-fix\n/root/acme/.git\n"), "/root/.wsp/worktrees/acme-fix"),
            Checkout { top: Some("/root/.wsp/worktrees/acme-fix".to_string()), key: "/root/acme".to_string() }
        );
        assert_eq!(checkout_of(None, "/root/notes"), Checkout { top: None, key: "/root/notes".to_string() });
        let Checkout { key, .. } = checkout_of(Some("/root/acme\n/root/acme/.git\n"), "/root/acme/web");
        let started = started_servers(&turn_servers(
            &agent("claude"),
            &LaunchConfigs { user: Some(user()), projects: Some(vec![]), folder: "/root/acme/web".to_string(), key: Some(key) },
        ));
        assert_eq!(names(&started), strings(&["localone", "shared"]));
    }

    #[test]
    fn reads_every_mcp_json_from_the_folder_up_to_the_root() {
        assert_eq!(
            turn_server_files(&agent("claude"), "/root/acme/web", "/root", Some("/root/.claude-cfg"), None),
            ServerFiles {
                user: strings(&["/root/.claude-cfg/.claude.json"]),
                base: "/root".to_string(),
                projects: strings(&["/root/acme/web/.mcp.json", "/root/acme/.mcp.json", "/root/.mcp.json", "/.mcp.json"]),
            }
        );
    }
}

mod a_codex_turns_servers {
    use super::*;

    fn toml(names: &[&str]) -> String {
        let mut lines: Vec<String> = Vec::new();
        for n in names {
            lines.push(format!("[mcp_servers.{n}]"));
            lines.push("command = \"node\"".to_string());
            lines.push(format!("args = [\"/srv/{n}.mjs\"]"));
            lines.push(String::new());
        }
        lines.join("\n")
    }

    fn project() -> TurnFile {
        file("/root/acme/.codex/config.toml", toml(&["cproj", "shared"]))
    }

    fn own(extra: &str) -> TurnFile {
        file(
            "/wsp/logins/codex/config.toml",
            format!("{}\n[mcp_servers.off]\ncommand = \"x\"\nenabled = false\n{}", toml(&["cuser", "shared"]), extra),
        )
    }

    #[test]
    fn reads_the_projects_file_before_its_own_where_its_own_file_trusts_the_folder() {
        let user = own("[projects.\"/root/acme\"]\ntrust_level = \"trusted\"\n");
        let project = project();
        let servers = turn_servers(
            &agent("codex"),
            &LaunchConfigs { user: Some(user.clone()), projects: Some(vec![project.clone()]), folder: "/root/acme".to_string(), key: None },
        );
        assert_eq!(
            said(&servers),
            vec![
                row("cproj", "project", &project.path, "on"),
                row("shared", "project", &project.path, "on"),
                row("cuser", "user", &user.path, "on"),
                row("shared", "user", &user.path, "on"),
                row("off", "user", &user.path, "off"),
            ]
        );
        assert_eq!(
            name_scope(&started_servers(&servers)),
            pairs(&[("cproj", "project"), ("shared", "project"), ("cuser", "user")])
        );
    }

    #[test]
    fn reads_no_project_file_in_a_folder_its_own_file_does_not_trust() {
        for trust in ["", "[projects.\"/root/acme\"]\ntrust_level = \"untrusted\"\n", "[projects.\"/root/other\"]\ntrust_level = \"trusted\"\n"] {
            let servers = turn_servers(
                &agent("codex"),
                &LaunchConfigs { user: Some(own(trust)), projects: Some(vec![project()]), folder: "/root/acme".to_string(), key: None },
            );
            assert!(!servers.iter().any(|s| s.scope == TurnScope::Project), "trust {trust:?}");
        }
    }

    #[test]
    fn reads_the_folders_own_codex_config_toml_under_the_store_its_turns_read_and_each() {
        assert_eq!(
            turn_server_files(&agent("codex"), "/root/acme", "/root", Some("/wsp/logins/codex"), None),
            ServerFiles {
                user: strings(&["/wsp/logins/codex/config.toml"]),
                base: "/wsp/logins/codex".to_string(),
                projects: strings(&["/root/acme/.codex/config.toml"]),
            }
        );
        assert_eq!(
            turn_server_files(&agent("codex"), "/root/acme/web/src", "/root", Some("/wsp/logins/codex"), Some("/root/acme")).projects,
            strings(&["/root/acme/web/src/.codex/config.toml", "/root/acme/web/.codex/config.toml", "/root/acme/.codex/config.toml"])
        );
        // A worktree outside the main checkout reads its own folder's file alone.
        assert_eq!(
            turn_server_files(&agent("codex"), "/root/.wsp/worktrees/acme-fix", "/root", None, Some("/root/acme")).projects,
            strings(&["/root/.wsp/worktrees/acme-fix/.codex/config.toml"])
        );
    }

    #[test]
    fn reads_a_subfolders_servers_from_the_checkouts_top_its_files_up_to_there_trusted() {
        let user = own("[projects.\"/root/acme\"]\ntrust_level = \"trusted\"\n");
        let top = file("/root/acme/.codex/config.toml", toml(&["ctop"]));
        let servers = turn_servers(
            &agent("codex"),
            &LaunchConfigs {
                user: Some(user.clone()),
                projects: Some(vec![top.clone()]),
                folder: "/root/acme/web".to_string(),
                key: Some("/root/acme".to_string()),
            },
        );
        assert_eq!(name_scope(&started_servers(&servers)), pairs(&[("ctop", "project"), ("cuser", "user"), ("shared", "user")]));
        let untrusted_here = turn_servers(
            &agent("codex"),
            &LaunchConfigs { user: Some(user), projects: Some(vec![top]), folder: "/root/acme/web".to_string(), key: None },
        );
        assert!(!untrusted_here.iter().any(|s| s.scope == TurnScope::Project));
    }
}

mod an_agent_with_no_road_of_its_own {
    use super::*;

    #[test]
    fn reads_the_first_of_its_project_files_in_the_folder_before_its_own_file() {
        let opencode = agent("opencode");
        assert_eq!(
            turn_server_files(&opencode, "/root/acme", "/root", None, None).projects,
            strings(&["/root/acme/opencode.json", "/root/acme/opencode.jsonc"])
        );
        let servers = turn_servers(
            &opencode,
            &LaunchConfigs {
                user: Some(file(
                    "/root/.config/opencode/opencode.json",
                    json!({ "mcp": { "mine": { "type": "local", "command": ["m"] } } }).to_string(),
                )),
                projects: Some(vec![file(
                    "/root/acme/opencode.json",
                    json!({ "mcp": { "theirs": { "type": "local", "command": ["t"] } } }).to_string(),
                )]),
                folder: "/root/acme".to_string(),
                key: None,
            },
        );
        assert_eq!(name_scope(&servers), pairs(&[("theirs", "project"), ("mine", "user")]));
    }
}

mod what_a_projects_servers_travel_in {
    use super::*;

    #[test]
    fn is_a_copy_of_the_file_holding_the_named_servers_alone_moved_under_the_folder_the() {
        let own = json!({
            "mcpServers": { "userone": stdio("userone") },
            "projects": { "/Users/dev/acme": { "mcpServers": { "localone": stdio("localone"), "other": stdio("other") }, "allowedTools": [] } },
        })
        .to_string();
        let only = MCP_SERVERS_JSON.only(&own, &strings(&["localone", "gone"]), Some("/Users/dev/acme"), Some("/Users/dev/acme")).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&only).unwrap(),
            json!({ "projects": { "/Users/dev/acme": { "mcpServers": { "localone": stdio("localone") } } } })
        );
        let parent_only = MCP_SERVERS_JSON
            .only(
                &json!({ "mcpServers": { "parentone": stdio("parentone"), "b": stdio("b") } }).to_string(),
                &strings(&["parentone"]),
                None,
                Some("/Users/dev/acme"),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&parent_only).unwrap(),
            json!({ "projects": { "/Users/dev/acme": { "mcpServers": { "parentone": stdio("parentone") } } } })
        );
        let toml = "model = \"gpt\"\n\n[mcp_servers.cproj]\ncommand = \"npx\"\n\n[mcp_servers.cproj.env]\nLOG = \"info\"\n\n[mcp_servers.other]\ncommand = \"o\"\n";
        assert_eq!(
            CODEX_TOML.only(toml, &strings(&["cproj"]), None, None).unwrap(),
            "[mcp_servers.cproj]\ncommand = \"npx\"\n\n[mcp_servers.cproj.env]\nLOG = \"info\"\n"
        );
    }

    #[test]
    fn marks_a_folder_trusted_in_codexs_own_file_in_place_where_its_table_is_there_and() {
        let trust = |text: Option<&str>| CODEX_TOML.trust(text, "/root/acme").unwrap();
        assert_eq!(trust(None).unwrap(), "[projects.\"/root/acme\"]\ntrust_level = \"trusted\"\n");
        assert_eq!(trust(Some("model = \"gpt\"\n")).unwrap(), "model = \"gpt\"\n\n[projects.\"/root/acme\"]\ntrust_level = \"trusted\"\n");
        assert_eq!(
            trust(Some("[projects.\"/root/acme\"]\ntrust_level = \"untrusted\"\n[mcp_servers.x]\ncommand = \"x\"\n")).unwrap(),
            "[projects.\"/root/acme\"]\ntrust_level = \"trusted\"\n[mcp_servers.x]\ncommand = \"x\"\n"
        );
        let trusted = "projects.\"/root/acme\".trust_level = \"trusted\"\n";
        assert_eq!(trust(Some(trusted)).unwrap(), trusted);
        assert!(codex_trusts(&trust(Some("model = \"gpt\"\n")).unwrap(), "/root/acme"));
        let refused = trust(Some("projects.\"/root/acme\".trust_level = \"untrusted\"\n")).unwrap_err();
        assert!(refused.message.contains("shape wsp does not edit"), "{}", refused.message);
        // A file that writes its projects as an inline table takes no table beside it: Codex could not read the result.
        let inline = trust(Some("projects = { \"/root/other\" = { trust_level = \"trusted\" } }\n")).unwrap_err();
        assert!(inline.message.contains("shape wsp does not edit"), "{}", inline.message);
    }
}
