// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/mcp.test.ts.
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_catalog::catalog::{catalog_agents, catalog_entry, mcp_agents, mcp_switch, EntryKind};
use wsp_catalog::jsonc::parse_jsonc;
use wsp_catalog::mcp::{
    EditOutcome, McpEditResult, McpEditScope, McpFormat, McpMergeResult, McpMergeScope, McpReads, McpServer, McpServerScope, McpTransport,
    MergeOutcome, CODEX_TOML, GEMINI_SETTINGS_JSON, MCP_SERVERS_JSON, OPENCODE_JSON,
};
use wsp_catalog::mcp_travelled::FromTo;
use wsp_words::exit::HostError;

const HOME: &str = "/Users/dev";

fn strs(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|x| x.to_string()).collect()
}

fn map(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn lines(ls: &[&str]) -> String {
    ls.join("\n")
}

fn kv(key: &str, value: Value) -> Value {
    let mut m = Map::new();
    m.insert(key.to_string(), value);
    Value::Object(m)
}

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap()
}

fn keys(value: &Value) -> Vec<String> {
    value.as_object().unwrap().keys().cloned().collect()
}

fn stdio(command: &str, args: &[&str]) -> McpTransport {
    McpTransport::Stdio { command: command.to_string(), args: strs(args), env: IndexMap::new(), cwd: None, tool_timeout_sec: None }
}

fn stdio_env(command: &str, args: &[&str], env: &[(&str, &str)]) -> McpTransport {
    McpTransport::Stdio { command: command.to_string(), args: strs(args), env: map(env), cwd: None, tool_timeout_sec: None }
}

fn http(url: &str, headers: &[(&str, &str)]) -> McpTransport {
    McpTransport::Http { url: url.to_string(), headers: map(headers) }
}

fn server() -> McpTransport {
    stdio("/usr/local/bin/node", &["/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json"])
}

fn user(name: &str, transport: McpTransport, env_refs: &[&str]) -> McpServer {
    McpServer { name: name.to_string(), scope: McpServerScope::User, transport, env_refs: strs(env_refs), reads: None, disabled: None }
}

fn refs(servers: &[McpServer]) -> Vec<(String, Vec<String>)> {
    servers.iter().map(|s| (s.name.clone(), s.env_refs.clone())).collect()
}

fn flags(servers: &[McpServer]) -> Vec<(String, bool)> {
    servers.iter().map(|s| (s.name.clone(), s.disabled == Some(true))).collect()
}

fn names(servers: &[McpServer]) -> Vec<String> {
    servers.iter().map(|s| s.name.clone()).collect()
}

fn assert_refused(r: Result<String, HostError>, message: &str) {
    let e = r.unwrap_err();
    assert!(e.message.contains(message), "{:?} does not contain {:?}", e.message, message);
}

fn enable(format: McpFormat, text: &str, name: &str, on: bool) -> String {
    format.enable(text, name, on, None).unwrap().unwrap()
}

fn place(format: McpFormat, text: Option<&str>, name: &str, transport: &McpTransport) -> String {
    format.place(text, name, transport).unwrap()
}

fn remove(format: McpFormat, text: &str, names: &[&str]) -> String {
    format.remove(text, &strs(names), None).unwrap()
}

fn done(name: &str, outcome: EditOutcome, command: Option<&str>) -> McpEditResult {
    McpEditResult { name: name.to_string(), outcome, command: command.map(|c| c.to_string()) }
}

fn merged(name: &str, outcome: MergeOutcome, command: Option<&str>) -> McpMergeResult {
    McpMergeResult { name: name.to_string(), outcome, command: command.map(|c| c.to_string()) }
}

fn outcomes(results: &[McpMergeResult]) -> Vec<MergeOutcome> {
    results.iter().map(|r| r.outcome).collect()
}

fn merge_scope(keep: &[&str], drop: &[&str], replace: &[&str], project: Option<(&str, &str)>) -> McpMergeScope {
    McpMergeScope {
        keep: strs(keep),
        drop: strs(drop),
        project: project.map(|(from, to)| FromTo { from: from.to_string(), to: to.to_string() }),
        replace: strs(replace),
    }
}

fn edit_scope(keep: &[&str], drop: &[&str], project: Option<(&str, &str)>) -> McpEditScope {
    McpEditScope {
        keep: strs(keep),
        drop: strs(drop),
        project: project.map(|(from, to)| FromTo { from: from.to_string(), to: to.to_string() }),
    }
}

fn with_env() -> McpTransport {
    stdio_env("npx", &["-y", "@acme/mcp", "--flag"], &[("ACME_KEY", "sk-acme-x"), ("ACME_ORG", "o 1")])
}

fn remote() -> McpTransport {
    http("https://mcp.acme.example/mcp", &[("Authorization", "Bearer tok-x"), ("X-Org", "acme")])
}

fn bare_remote() -> McpTransport {
    http("https://mcp.plain.example/mcp", &[])
}

fn remote_b() -> McpTransport {
    http("https://b.example/mcp", &[])
}

fn codex_own() -> String {
    [
        r#"model = "gpt-5""#,
        "",
        r#"[projects."/private/tmp/proof-907/repo"]"#,
        r#"trust_level = "trusted""#,
        "",
        "[hooks.state]",
        "enabled = true",
        "",
        "[mcp_servers.mine]",
        r#"command = "/usr/local/bin/mine""#,
        "",
        "[mcp_servers.mine.env]",
        r#"MINE_KEY = "abc""#,
        "",
    ]
    .join("\n")
}

fn codex_travelled(args: &[&str]) -> String {
    let list = args.iter().map(|a| format!("\"{a}\"")).collect::<Vec<_>>().join(", ");
    [
        "[mcp_servers.context7]".to_string(),
        format!("command = \"{HOME}/.local/bin/context7\""),
        format!("args = [{list}]"),
        "".to_string(),
        "[mcp_servers.context7.env]".to_string(),
        r#"KEY = "k""#.to_string(),
        "".to_string(),
        "[mcp_servers.mine]".to_string(),
        format!("command = \"{HOME}/other\""),
        "".to_string(),
        "[mcp_servers.gone]".to_string(),
        r#"command = "x""#.to_string(),
        "".to_string(),
    ]
    .join("\n")
}

// The machine's own words for a string of this computer's: the home moves, and a command right under a bin
// directory is on the machine's PATH by name.
fn lib(s: &str, command: bool) -> String {
    if command && s == format!("{HOME}/.local/bin/bare") {
        "bare".to_string()
    } else if let Some(rest) = s.strip_prefix(&format!("{HOME}/")) {
        format!("/root/{rest}")
    } else {
        s.to_string()
    }
}

fn claude_own() -> String {
    let value = json!({
        "numStartups": 41,
        "oauthAccount": { "emailAddress": "he@example.com" },
        "mcpServers": { "mine": { "command": "/usr/local/bin/mine", "args": [] } },
        "projects": { "/root/work": { "history": ["his own turn"] } },
    });
    format!("{}\n", pretty(&value))
}

fn claude_travelled(gsc_args: &[&str]) -> String {
    let value = json!({
        "numStartups": 3,
        "mcpServers": {
            "gsc": { "command": format!("{HOME}/.local/bin/bare"), "args": gsc_args },
            "notion": { "command": "npx", "args": ["-y", "notion-mcp"] },
            "mine": { "command": format!("{HOME}/other"), "args": [] },
        },
        "projects": kv(HOME, json!({ "mcpServers": { "zed": { "command": format!("{HOME}/.local/bin/zed"), "args": [] } } })),
    });
    format!("{}\n", pretty(&value))
}

// A jsonc config as a person keeps one: comments above, beside and below its keys and servers.
const COMMENTS: [&str; 7] = [
    "// top of the file",
    "// beside the theme",
    "/* before the servers */",
    "// the person's own server",
    "// end of mine",
    "/* his */",
    "// the end",
];

fn commented(key: &str, entry: &dyn Fn(&str) -> Value) -> String {
    lines(&[
        "{",
        "  // top of the file",
        r#"  "theme": "x", // beside the theme"#,
        "  /* before the servers */",
        &format!("  {}: {{", Value::String(key.to_string())),
        "    // the person's own server",
        &format!("    \"mine\": {}, // end of mine", entry("/usr/local/bin/mine")),
        &format!("    \"old\": {}", entry("x")),
        "  },",
        r#"  "projects": { "/root/work": { /* his */ "history": [] } }"#,
        "}",
        "// the end",
        "",
    ])
}

type Entry = fn(&str) -> Value;

fn claude_entry(command: &str) -> Value {
    json!({ "command": command, "args": [] })
}

fn opencode_entry(command: &str) -> Value {
    json!({ "type": "local", "command": [command] })
}

fn json_formats() -> Vec<(McpFormat, &'static str, Entry)> {
    vec![
        (MCP_SERVERS_JSON, "mcpServers", claude_entry as Entry),
        (GEMINI_SETTINGS_JSON, "mcpServers", claude_entry as Entry),
        (OPENCODE_JSON, "mcp", opencode_entry as Entry),
    ]
}

mod the_catalogs_mcp_configs {
    use super::*;

    #[test]
    fn are_registered_from_the_agent_entries_that_have_one_each_naming_its_format_modul() {
        let agents = mcp_agents();
        assert_eq!(agents.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["claude", "codex", "gemini", "opencode"]);
        assert_eq!(
            agents.iter().map(|a| a.mcp.as_ref().unwrap().format).collect::<Vec<_>>(),
            [MCP_SERVERS_JSON, CODEX_TOML, GEMINI_SETTINGS_JSON, OPENCODE_JSON]
        );
        for a in &agents {
            for f in &a.mcp.as_ref().unwrap().files {
                assert!(a.config_paths.contains(f), "{}", a.id);
            }
        }
        let with_mcp: Vec<_> = catalog_agents().into_iter().filter(|a| a.mcp.is_some()).collect();
        assert_eq!(with_mcp, agents);
        let pi = catalog_entry("pi").unwrap();
        assert_eq!(pi.kind(), EntryKind::Agent);
        assert!(pi.as_agent().unwrap().mcp.is_none());
    }
}

mod read {
    use super::*;

    #[test]
    fn mcpservers_json_command_or_url_entries_at_the_root_are_user_scope_the_home_folde() {
        let text = json!({
            "mcpServers": {
                "a": { "command": "x" },
                "b": { "url": "https://b" },
                "c": { "type": "sse", "url": "https://c" },
                "d": {},
            },
            "projects": {
                HOME: { "mcpServers": { "z": { "type": "stdio", "command": "npx", "args": ["mcp-remote", "https://z"] } } },
                format!("{HOME}/code"): { "mcpServers": { "other": { "command": "o" } } },
            },
        })
        .to_string();
        assert_eq!(
            MCP_SERVERS_JSON.read(&text, HOME),
            vec![
                user("a", stdio("x", &[]), &[]),
                user("b", http("https://b", &[]), &[]),
                user("c", http("https://c", &[]), &[]),
                McpServer {
                    name: "z".to_string(),
                    scope: McpServerScope::Home,
                    transport: stdio("npx", &["mcp-remote", "https://z"]),
                    env_refs: vec![],
                    reads: None,
                    disabled: None,
                },
            ]
        );
        let gemini = lines(&[
            "{",
            "  // my servers",
            r#"  "mcpServers": { "docs": { "httpUrl": "https://docs.example/mcp", "headers": { "Authorization": "Bearer abcdef" } }, },"#,
            "}",
            "",
        ]);
        assert_eq!(
            MCP_SERVERS_JSON.read(&gemini, HOME),
            vec![user("docs", http("https://docs.example/mcp", &[("Authorization", "Bearer abcdef")]), &[])]
        );
        assert_eq!(MCP_SERVERS_JSON.read("nope", HOME), vec![]);
        assert_eq!(MCP_SERVERS_JSON.read("[]", HOME), vec![]);
    }

    #[test]
    fn opencodes_json_local_entries_carry_the_command_as_one_array_with_their_environme() {
        let text = r#"{ "mcp": { "memory": { "type": "local", "command": ["/Users/dev/.local/bin/mem", "--v"], "environment": { "MEM_KEY": "k" }, "enabled": true }, "ctx": { "type": "remote", "url": "https://ctx.example/mcp" }, "none": { "type": "local", "command": [] } } }"#;
        assert_eq!(
            OPENCODE_JSON.read(text, HOME),
            vec![
                user("memory", stdio_env("/Users/dev/.local/bin/mem", &["--v"], &[("MEM_KEY", "k")]), &[]),
                user("ctx", http("https://ctx.example/mcp", &[]), &[]),
            ]
        );
    }

    #[test]
    fn a_header_that_takes_its_value_from_the_environment_names_the_variable_it_reads_x() {
        let claude = MCP_SERVERS_JSON.read(
            &json!({ "mcpServers": {
                "a": { "type": "http", "url": "https://a.example/mcp", "headers": { "Authorization": "Bearer ${A_TOKEN}", "X-Org": "${ORG:-acme}", "Plain": "static" } },
                "b": { "type": "http", "url": "https://b.example/mcp", "headers": { "X": "no $REF here" } },
            } })
            .to_string(),
            HOME,
        );
        assert_eq!(refs(&claude), vec![("a".to_string(), strs(&["A_TOKEN", "ORG"])), ("b".to_string(), vec![])]);
        let opencode = OPENCODE_JSON.read(
            &json!({ "mcp": { "c": { "type": "remote", "url": "https://c.example/mcp", "headers": { "Authorization": "Bearer {env:C_TOKEN}" } } } }).to_string(),
            HOME,
        );
        assert_eq!(refs(&opencode), vec![("c".to_string(), strs(&["C_TOKEN"]))]);
    }

    #[test]
    fn a_url_that_takes_part_of_itself_from_the_environment_names_the_variable_and_gemi() {
        let servers = json!({
            "u": { "url": "https://${MCP_HOST}/mcp" },
            "h": { "httpUrl": "https://h.example/mcp", "headers": { "Authorization": "Bearer $TOK", "X-Org": "${ORG}" } },
        });
        let claude = MCP_SERVERS_JSON.read(&json!({ "mcpServers": servers }).to_string(), HOME);
        assert_eq!(refs(&claude), vec![("u".to_string(), strs(&["MCP_HOST"])), ("h".to_string(), strs(&["ORG"]))]);
        let gemini = GEMINI_SETTINGS_JSON.read(&json!({ "mcpServers": servers }).to_string(), HOME);
        assert_eq!(refs(&gemini), vec![("u".to_string(), strs(&["MCP_HOST"])), ("h".to_string(), strs(&["TOK", "ORG"]))]);
        let plain = GEMINI_SETTINGS_JSON
            .read(&json!({ "mcpServers": { "p": { "url": "https://p.example/mcp", "headers": { "X": "costs $5" } } } }).to_string(), HOME);
        assert_eq!(plain[0].env_refs, Vec::<String>::new());
    }

    #[test]
    fn codexs_toml_mcp_servers_tables_with_their_env_header_and_bearer_sub_keys_quoted() {
        let toml = lines(&[
            r#"model = "gpt-5""#,
            "",
            "[mcp_servers.grafana]",
            r#"command = "/opt/homebrew/bin/uvx""#,
            r#"args = ["mcp-grafana", "a\tb\u00e9"]"#,
            "",
            "[mcp_servers.grafana.env]",
            r#"GRAFANA_SERVICE_ACCOUNT_TOKEN = "glsa_abcdefghij""#,
            r#"GRAFANA_URL = "https://g.example" # keep"#,
            "",
            "# a remote one",
            "[mcp_servers.sentry]",
            r#"url = "https://mcp.sentry.dev/mcp?x=1""#,
            r#"bearer_token_env_var = "SENTRY_TOKEN""#,
            "",
            "[mcp_servers.sentry.env_http_headers]",
            r#"X-Org = "SENTRY_ORG""#,
            "",
            r#"[mcp_servers."my server"]"#,
            "command = 'node'",
            "args = [",
            r#"  "/Applications/Tool.app/Contents/mcp.js","#,
            "]",
            r#"env = { A_KEY = "1234" }"#,
            "",
            "[[hooks.SessionStart]]",
            r#"matcher = "x""#,
        ]);
        assert_eq!(
            CODEX_TOML.read(&toml, HOME),
            vec![
                user(
                    "grafana",
                    stdio_env(
                        "/opt/homebrew/bin/uvx",
                        &["mcp-grafana", "a\tb\u{e9}"],
                        &[("GRAFANA_SERVICE_ACCOUNT_TOKEN", "glsa_abcdefghij"), ("GRAFANA_URL", "https://g.example")]
                    ),
                    &[]
                ),
                McpServer {
                    name: "sentry".to_string(),
                    scope: McpServerScope::User,
                    transport: http("https://mcp.sentry.dev/mcp?x=1", &[]),
                    env_refs: strs(&["SENTRY_TOKEN", "SENTRY_ORG"]),
                    reads: Some(McpReads {
                        env: vec![],
                        headers: map(&[("X-Org", "SENTRY_ORG")]),
                        bearer: Some("SENTRY_TOKEN".to_string())
                    }),
                    disabled: None,
                },
                user("my server", stdio_env("node", &["/Applications/Tool.app/Contents/mcp.js"], &[("A_KEY", "1234")]), &[]),
            ]
        );
        assert_eq!(CODEX_TOML.read("model = \"x\"\n", HOME), vec![]);
    }
}

mod place_group {
    use super::*;

    #[test]
    fn mcpservers_json_claude_code_gemini_cli_a_fresh_file_holds_the_server_an_existing() {
        let s = server();
        let (command, args) = match &s {
            McpTransport::Stdio { command, args, .. } => (command.clone(), args.clone()),
            _ => unreachable!(),
        };
        let fresh = place(MCP_SERVERS_JSON, None, "wsp", &s);
        assert_eq!(json(&fresh), json!({ "mcpServers": { "wsp": { "command": command, "args": args } } }));
        assert!(fresh.ends_with('\n'));
        let existing = json!({
            "numStartups": 4,
            "mcpServers": { "other": { "command": "x", "args": [] }, "wsp": { "command": "old", "args": [] } },
            "projects": { "/a": {} },
        })
        .to_string();
        let placed = json(&place(MCP_SERVERS_JSON, Some(&existing), "wsp", &s));
        assert_eq!(
            placed,
            json!({
                "numStartups": 4,
                "mcpServers": { "other": { "command": "x", "args": [] }, "wsp": { "command": command, "args": args } },
                "projects": { "/a": {} },
            })
        );
        assert_eq!(keys(&placed), ["numStartups", "mcpServers", "projects"]);
        assert_eq!(place(MCP_SERVERS_JSON, Some(""), "wsp", &s), fresh);
        assert_refused(MCP_SERVERS_JSON.place(Some("{ not json"), "wsp", &s), "the file is not valid JSON; add the server by hand");
        assert_refused(MCP_SERVERS_JSON.place(Some("[]"), "wsp", &s), "the file is not a JSON object; add the server by hand");
    }

    #[test]
    fn opencodes_json_the_server_is_a_local_mcp_entry_whose_command_is_one_array_enable() {
        let placed =
            json(&place(OPENCODE_JSON, Some(r#"{ "$schema": "https://opencode.ai/config.json", "theme": "x" }"#), "wsp", &server()));
        assert_eq!(
            placed,
            json!({
                "$schema": "https://opencode.ai/config.json",
                "theme": "x",
                "mcp": { "wsp": { "type": "local", "command": ["/usr/local/bin/node", "/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json"], "enabled": true } },
            })
        );
    }

    #[test]
    fn a_settings_file_with_comments_and_a_trailing_comma_opencodes_jsonc_gemini_clis_s() {
        let s = server();
        let command = json!(["/usr/local/bin/node", "/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json"]);
        let jsonc = lines(&[
            "{",
            "  // servers I use",
            r#"  "$schema": "https://opencode.ai/config.json","#,
            r#"  "mcp": {"#,
            r#"    /* memory */ "other": { "type": "local", "command": ["x"], "enabled": true },"#,
            "  },",
            "}",
            "",
        ]);
        let placed = place(OPENCODE_JSON, Some(&jsonc), "wsp", &s);
        assert_eq!(
            parse_jsonc(&placed).unwrap(),
            json!({
                "$schema": "https://opencode.ai/config.json",
                "mcp": {
                    "other": { "type": "local", "command": ["x"], "enabled": true },
                    "wsp": { "type": "local", "command": command, "enabled": true },
                },
            })
        );
        assert!(placed.contains("// servers I use\n"));
        assert!(placed.contains(r#"/* memory */ "other": {"#));
        let gemini = place(
            MCP_SERVERS_JSON,
            Some("{ \"theme\": \"dark\", // the look\n \"mcpServers\": { \"docs\": { \"url\": \"https://docs.example/mcp\" }, }, }"),
            "wsp",
            &s,
        );
        assert!(gemini.contains("// the look\n"));
        assert_eq!(
            parse_jsonc(&gemini).unwrap(),
            json!({
                "theme": "dark",
                "mcpServers": {
                    "docs": { "url": "https://docs.example/mcp" },
                    "wsp": { "command": "/usr/local/bin/node", "args": ["/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json"] },
                },
            })
        );
        let url =
            place(OPENCODE_JSON, Some(r#"{ "mcp": { "ctx": { "type": "remote", "url": "https://ctx.example/*/mcp" } }, }"#), "wsp", &s);
        assert_eq!(parse_jsonc(&url).unwrap()["mcp"]["ctx"]["url"], "https://ctx.example/*/mcp");
    }

    #[test]
    fn an_add_goes_on_a_line_of_its_own_after_the_last_server_so_the_comment_at_the_end() {
        let text = lines(&["{", r#"  "mcpServers": { // header"#, r#"    "a": { "command": "x" } // note on a"#, "  }", "}", ""]);
        let placed = place(MCP_SERVERS_JSON, Some(&text), "w", &stdio("n", &[]));
        assert_eq!(
            placed,
            lines(&[
                "{",
                r#"  "mcpServers": { // header"#,
                r#"    "a": { "command": "x" }, // note on a"#,
                r#"    "w": {"#,
                r#"      "command": "n","#,
                r#"      "args": []"#,
                "    }",
                "  }",
                "}",
                "",
            ])
        );
        let empty = remove(MCP_SERVERS_JSON, &placed, &["a", "w"]);
        assert_eq!(empty, lines(&["{", r#"  "mcpServers": { // header"#, "  }", "}", ""]));
        assert_eq!(
            place(MCP_SERVERS_JSON, Some(&empty), "w", &stdio("n", &[])),
            lines(&[
                "{",
                r#"  "mcpServers": { // header"#,
                r#"    "w": {"#,
                r#"      "command": "n","#,
                r#"      "args": []"#,
                "    }",
                "  }",
                "}",
                "",
            ])
        );
    }

    #[test]
    fn a_file_that_holds_the_servers_key_twice_is_refused_rather_than_edited_into_somet() {
        let twice = "{ \"mcpServers\": { \"a\": { \"command\": \"x\" } }, // mine\n \"mcpServers\": { \"b\": { \"command\": \"y\" } } }";
        let refusal = "the file could not be changed in place, which keeps its comments; change it by hand";
        assert_refused(MCP_SERVERS_JSON.place(Some(twice), "wsp", &server()), refusal);
        assert_refused(MCP_SERVERS_JSON.remove(twice, &strs(&["b"]), None), refusal);
    }

    #[test]
    fn codexs_toml_a_fresh_file_is_one_table_an_existing_file_gets_the_table_appended_a() {
        let table = "[mcp_servers.wsp]\ncommand = \"/usr/local/bin/node\"\nargs = [\"/opt/wsp/bin.js\", \"mcp\", \"--state\", \"/Users/me/.wsp/state.json\"]\n";
        assert_eq!(place(CODEX_TOML, None, "wsp", &server()), table);
        let existing = "model = \"gpt-5\"\n\n[projects.\"/Users/me/proj\"]\ntrust_level = \"trusted\"\n";
        let appended = place(CODEX_TOML, Some(existing), "wsp", &server());
        assert_eq!(appended, format!("{existing}\n{table}"));
        let rerun =
            place(CODEX_TOML, Some(&format!("{appended}\n[mcp_servers.other]\ncommand = \"x\"\n")), "wsp", &stdio("/new/node", &["a"]));
        assert_eq!(
            rerun,
            format!("{existing}\n[mcp_servers.wsp]\ncommand = \"/new/node\"\nargs = [\"a\"]\n\n[mcp_servers.other]\ncommand = \"x\"\n")
        );
        assert_eq!(place(CODEX_TOML, Some(&rerun), "wsp", &stdio("/new/node", &["a"])), rerun);
    }

    #[test]
    fn codexs_toml_carries_a_servers_tool_timeout_which_codex_reads_as_tool_timeout_sec() {
        let timed = McpTransport::Stdio {
            command: "/usr/local/bin/node".to_string(),
            args: strs(&["/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json"]),
            env: IndexMap::new(),
            cwd: None,
            tool_timeout_sec: Some(3600),
        };
        let placed = place(CODEX_TOML, None, "wsp", &timed);
        assert_eq!(
            placed,
            "[mcp_servers.wsp]\ncommand = \"/usr/local/bin/node\"\nargs = [\"/opt/wsp/bin.js\", \"mcp\", \"--state\", \"/Users/me/.wsp/state.json\"]\ntool_timeout_sec = 3600\n"
        );
        let parsed = toml::from_str::<toml::Value>(&placed).unwrap();
        assert_eq!(parsed["mcp_servers"]["wsp"]["tool_timeout_sec"].as_integer(), Some(3600));
        assert_eq!(names(&CODEX_TOML.read(&placed, "/home/u")), ["wsp"]);
        assert!(!place(MCP_SERVERS_JSON, None, "wsp", &timed).to_lowercase().contains("timeout"));
    }

    #[test]
    fn codexs_toml_quotes_a_path_with_a_quote_or_a_backslash_in_it_as_a_basic_string() {
        let placed = place(CODEX_TOML, None, "wsp", &stdio("C:\\node \"x\".exe", &[]));
        assert!(placed.contains(r#"command = "C:\\node \"x\".exe""#));
    }

    #[test]
    fn codexs_toml_quotes_a_server_name_that_is_not_a_bare_key_in_the_table_header_and() {
        let placed = place(CODEX_TOML, None, "my server]\n[mcp_servers.other", &server());
        assert_eq!(placed.split('\n').filter(|l| l.starts_with('[')).count(), 1);
        assert!(placed.contains(r#"[mcp_servers."my server]\n[mcp_servers.other"]"#));
        let spaced = place(CODEX_TOML, Some("[other]\nx = 1\n"), "my server", &server());
        assert!(spaced.contains("[mcp_servers.\"my server\"]\n"));
        assert_eq!(names(&CODEX_TOML.read(&spaced, "/home/u")), ["my server"]);
        let rerun = place(CODEX_TOML, Some(&spaced), "my server", &stdio("/new/node", &[]));
        assert_eq!(rerun.matches("[mcp_servers.").count(), 1);
        assert!(rerun.contains("command = \"/new/node\""));
    }
}

mod place_every_transport_a_person_adds {
    use super::*;

    #[test]
    fn each_format_reads_back_what_it_placed_a_command_with_its_arguments_and_variables() {
        let formats =
            [("Claude Code", MCP_SERVERS_JSON), ("Gemini CLI", GEMINI_SETTINGS_JSON), ("OpenCode", OPENCODE_JSON), ("Codex", CODEX_TOML)];
        for (agent, format) in formats {
            for t in [with_env(), remote(), bare_remote(), server()] {
                let placed = place(format, None, "acme", &t);
                let read: Vec<McpTransport> = format.read(&placed, HOME).into_iter().map(|s| s.transport).collect();
                assert_eq!(read, vec![t.clone()], "{agent} {t:?}");
            }
        }
    }

    #[test]
    fn claude_code_writes_an_address_as_an_http_entry_gemini_cli_as_httpurl_each_with_i() {
        let url = "https://mcp.acme.example/mcp";
        let headers = json!({ "Authorization": "Bearer tok-x", "X-Org": "acme" });
        assert_eq!(
            json(&place(MCP_SERVERS_JSON, None, "acme", &remote())),
            json!({ "mcpServers": { "acme": { "type": "http", "url": url, "headers": headers } } })
        );
        assert_eq!(
            json(&place(MCP_SERVERS_JSON, None, "acme", &bare_remote())),
            json!({ "mcpServers": { "acme": { "type": "http", "url": "https://mcp.plain.example/mcp" } } })
        );
        assert_eq!(
            json(&place(GEMINI_SETTINGS_JSON, None, "acme", &remote())),
            json!({ "mcpServers": { "acme": { "httpUrl": url, "headers": headers } } })
        );
        assert_eq!(
            json(&place(MCP_SERVERS_JSON, None, "acme", &with_env())),
            json!({ "mcpServers": { "acme": { "command": "npx", "args": ["-y", "@acme/mcp", "--flag"], "env": { "ACME_KEY": "sk-acme-x", "ACME_ORG": "o 1" } } } })
        );
        assert_eq!(
            json(&place(GEMINI_SETTINGS_JSON, None, "wsp", &server())),
            json!({ "mcpServers": { "wsp": { "command": "/usr/local/bin/node", "args": ["/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json"] } } })
        );
    }

    #[test]
    fn opencode_writes_a_commands_variables_as_its_environment_and_an_address_as_a_remo() {
        assert_eq!(
            json(&place(OPENCODE_JSON, None, "acme", &with_env())),
            json!({ "mcp": { "acme": { "type": "local", "command": ["npx", "-y", "@acme/mcp", "--flag"], "enabled": true, "environment": { "ACME_KEY": "sk-acme-x", "ACME_ORG": "o 1" } } } })
        );
        assert_eq!(
            json(&place(OPENCODE_JSON, None, "acme", &remote())),
            json!({ "mcp": { "acme": { "type": "remote", "url": "https://mcp.acme.example/mcp", "enabled": true, "headers": { "Authorization": "Bearer tok-x", "X-Org": "acme" } } } })
        );
    }

    #[test]
    fn codex_writes_a_commands_variables_and_an_addresss_headers_as_inline_tables_every() {
        assert_eq!(
            place(CODEX_TOML, None, "acme", &with_env()),
            "[mcp_servers.acme]\ncommand = \"npx\"\nargs = [\"-y\", \"@acme/mcp\", \"--flag\"]\nenv = { \"ACME_KEY\" = \"sk-acme-x\", \"ACME_ORG\" = \"o 1\" }\n"
        );
        assert_eq!(
            place(CODEX_TOML, None, "acme", &remote()),
            "[mcp_servers.acme]\nurl = \"https://mcp.acme.example/mcp\"\nhttp_headers = { \"Authorization\" = \"Bearer tok-x\", \"X-Org\" = \"acme\" }\n"
        );
        let tricky = stdio_env("x", &[], &[("K", "a \"quoted\" \\ value")]);
        let placed = place(CODEX_TOML, None, "t", &tricky);
        assert_eq!(CODEX_TOML.read(&placed, HOME)[0].transport, tricky);
    }

    #[test]
    fn a_server_placed_beside_others_leaves_every_other_server_in_the_file_as_it_was() {
        let claude = json!({ "numStartups": 2, "mcpServers": { "mine": { "command": "m", "args": [] } } }).to_string();
        let placed = json(&place(MCP_SERVERS_JSON, Some(&claude), "acme", &remote()));
        assert_eq!(placed["mcpServers"]["mine"], json!({ "command": "m", "args": [] }));
        let codex = "[mcp_servers.mine]\ncommand = \"m\"\n";
        let placed = place(CODEX_TOML, Some(codex), "acme", &remote());
        assert_eq!(names(&CODEX_TOML.read(&placed, HOME)), ["mine", "acme"]);
    }
}

mod enable_group {
    use super::*;

    #[test]
    fn opencode_turns_a_server_off_and_on_by_its_enabled_field_and_the_read_says_so() {
        let first = place(OPENCODE_JSON, None, "a", &server());
        let text = place(OPENCODE_JSON, Some(&first), "b", &remote_b());
        let off = enable(OPENCODE_JSON, &text, "a", false);
        assert_eq!(flags(&OPENCODE_JSON.read(&off, HOME)), [("a".to_string(), true), ("b".to_string(), false)]);
        let on = enable(OPENCODE_JSON, &off, "a", true);
        assert_eq!(flags(&OPENCODE_JSON.read(&on, HOME)), [("a".to_string(), false), ("b".to_string(), false)]);
    }

    #[test]
    fn codex_turns_a_server_off_with_an_enabled_line_in_its_own_table_and_on_by_taking() {
        let own = codex_own();
        let text =
            format!("{own}\n[mcp_servers.a]\ncommand = \"x\"\n\n[mcp_servers.a.env]\nK = \"v\"\n\n[mcp_servers.b]\ncommand = \"y\"\n");
        let off = enable(CODEX_TOML, &text, "a", false);
        assert_eq!(
            off,
            format!("{own}\n[mcp_servers.a]\nenabled = false\ncommand = \"x\"\n\n[mcp_servers.a.env]\nK = \"v\"\n\n[mcp_servers.b]\ncommand = \"y\"\n")
        );
        assert_eq!(flags(&CODEX_TOML.read(&off, HOME)), [("mine".to_string(), false), ("a".to_string(), true), ("b".to_string(), false)]);
        assert_eq!(enable(CODEX_TOML, &off, "a", true), text);
        let written = text.replacen("[mcp_servers.a]\ncommand = \"x\"", "[mcp_servers.a]\ncommand = \"x\"\nenabled = true # mine", 1);
        assert_eq!(enable(CODEX_TOML, &written, "a", false), written.replacen("enabled = true # mine", "enabled = false", 1));
    }

    #[test]
    fn gemini_cli_turns_a_server_off_by_its_name_in_mcp_excluded_and_on_by_taking_it_ou() {
        let text = json!({
            "theme": "dark",
            "mcp": { "allowed": ["b"] },
            "mcpServers": { "a": { "command": "x" }, "b": { "command": "y" } },
        })
        .to_string();
        let off = enable(GEMINI_SETTINGS_JSON, &text, "a", false);
        assert_eq!(
            json(&off),
            json!({
                "theme": "dark",
                "mcp": { "allowed": ["b"], "excluded": ["a"] },
                "mcpServers": { "a": { "command": "x" }, "b": { "command": "y" } },
            })
        );
        assert_eq!(flags(&GEMINI_SETTINGS_JSON.read(&off, HOME)), [("a".to_string(), true), ("b".to_string(), false)]);
        assert_eq!(
            json(&enable(GEMINI_SETTINGS_JSON, &off, "a", true)),
            json!({
                "theme": "dark",
                "mcp": { "allowed": ["b"], "excluded": [] },
                "mcpServers": { "a": { "command": "x" }, "b": { "command": "y" } },
            })
        );
    }

    #[test]
    fn opencode_and_gemini_cli_flip_a_switch_in_place_so_a_jsonc_file_keeps_its_comment() {
        let opencode = lines(&["{", "  // mine", r#"  "mcp": { "a": { "type": "local", "command": ["x"], "enabled": true } }"#, "}", ""]);
        let off = enable(OPENCODE_JSON, &opencode, "a", false);
        assert_eq!(off, opencode.replacen(r#""enabled": true"#, r#""enabled": false"#, 1));
        let gemini = lines(&["{", "  // mine", r#"  "mcpServers": { "a": { "command": "x" } }"#, "}", ""]);
        let excluded = enable(GEMINI_SETTINGS_JSON, &gemini, "a", false);
        assert!(excluded.contains("// mine\n"));
        assert_eq!(parse_jsonc(&excluded).unwrap(), json!({ "mcpServers": { "a": { "command": "x" } }, "mcp": { "excluded": ["a"] } }));
        assert_eq!(
            parse_jsonc(&enable(GEMINI_SETTINGS_JSON, &excluded, "a", true)).unwrap(),
            json!({ "mcpServers": { "a": { "command": "x" } }, "mcp": { "excluded": [] } })
        );
    }

    #[test]
    fn claude_code_keeps_no_switch_a_person_turns_per_server_so_its_format_has_none_and() {
        assert!(!MCP_SERVERS_JSON.has_enable());
        assert!(MCP_SERVERS_JSON.enable("{}", "a", false, None).is_none());
        assert!(CODEX_TOML.has_enable());
        assert!(GEMINI_SETTINGS_JSON.has_enable());
        assert!(OPENCODE_JSON.has_enable());
        assert_eq!(
            mcp_agents().iter().map(|a| (a.id.clone(), mcp_switch(&a.id))).collect::<Vec<_>>(),
            [("claude".to_string(), false), ("codex".to_string(), true), ("gemini".to_string(), true), ("opencode".to_string(), true)]
        );
        assert!(!mcp_switch("pi"));
    }
}

mod edit {
    use super::*;

    #[test]
    fn every_json_format_edits_a_jsonc_file_in_place_each_comment_outside_what_it_took() {
        for (format, key, entry) in json_formats() {
            let own = commented(key, &entry).replacen("\"/usr/local/bin/mine\"", &Value::String(format!("{HOME}/mine")).to_string(), 1);
            let edited = format.edit(&lib, &edit_scope(&["mine"], &["old"], None), &own).unwrap();
            assert_eq!(
                edited.results,
                vec![done("mine", EditOutcome::Written, Some("/root/mine")), done("old", EditOutcome::Dropped, None)],
                "{key}"
            );
            for c in COMMENTS {
                assert!(edited.text.contains(c), "{c}");
            }
            assert_eq!(parse_jsonc(&edited.text).unwrap()[key], json!({ "mine": entry("/root/mine") }));
            assert_eq!(format.edit(&lib, &edit_scope(&["nobody"], &[], None), &own).unwrap().text, own);
        }
        let home = commented("mcpServers", &|c| json!({ "command": c })).replacen(
            "\"projects\": { \"/root/work\"",
            &format!(
                "\"projects\": {{ {}: {{ \"mcpServers\": {{ \"zed\": {{ \"command\": {} }} }} }}, \"/root/work\"",
                Value::String(HOME.to_string()),
                Value::String(format!("{HOME}/zed"))
            ),
            1,
        );
        let moved = MCP_SERVERS_JSON.edit(&lib, &edit_scope(&["zed"], &[], Some((HOME, "/root"))), &home).unwrap();
        for c in COMMENTS {
            assert!(moved.text.contains(c), "{c}");
        }
        let mut want = Map::new();
        want.insert(HOME.to_string(), json!({ "mcpServers": {} }));
        want.insert("/root/work".to_string(), json!({ "history": [] }));
        want.insert("/root".to_string(), json!({ "mcpServers": { "zed": { "command": "/root/zed" } } }));
        assert_eq!(parse_jsonc(&moved.text).unwrap()["projects"], Value::Object(want));
    }
}

mod merge {
    use super::*;

    #[test]
    fn mcpservers_json_the_agents_own_keys_and_the_server_the_person_has_stand_wsps_are() {
        let own = claude_own();
        let travelled = claude_travelled(&["--stdio"]);
        let m = MCP_SERVERS_JSON.merge(&lib, &merge_scope(&["gsc", "notion", "mine"], &[], &[], None), Some(&own), &travelled).unwrap();
        assert_eq!(
            m.results,
            vec![
                merged("gsc", MergeOutcome::Added, Some("bare")),
                merged("notion", MergeOutcome::Added, Some("npx")),
                merged("mine", MergeOutcome::Theirs, Some("/root/other")),
            ]
        );
        let root = json(&m.text);
        // Every key of the agent's own stands, the number it keeps for itself included, and its servers keep their order.
        assert_eq!(root["numStartups"], 41);
        assert_eq!(root["oauthAccount"], json!({ "emailAddress": "he@example.com" }));
        assert_eq!(root["projects"], json!({ "/root/work": { "history": ["his own turn"] } }));
        assert_eq!(root["mcpServers"]["mine"], json!({ "command": "/usr/local/bin/mine", "args": [] }));
        assert_eq!(keys(&root["mcpServers"]), ["mine", "gsc", "notion"]);
        assert_eq!(root["mcpServers"]["gsc"], json!({ "command": "bare", "args": ["--stdio"] }));

        let again = MCP_SERVERS_JSON
            .merge(&lib, &merge_scope(&["gsc", "notion", "mine"], &[], &["gsc", "notion"], None), Some(&m.text), &travelled)
            .unwrap();
        assert_eq!(outcomes(&again.results), [MergeOutcome::Same, MergeOutcome::Same, MergeOutcome::Theirs]);
        assert_eq!(again.text, m.text);
    }

    #[test]
    fn mcpservers_json_wsps_own_entry_is_written_over_when_this_computers_copy_changed() {
        let landed = MCP_SERVERS_JSON
            .merge(&lib, &merge_scope(&["gsc"], &[], &[], None), Some(&claude_own()), &claude_travelled(&["--stdio"]))
            .unwrap()
            .text;
        let changed = MCP_SERVERS_JSON
            .merge(&lib, &merge_scope(&["gsc"], &[], &["gsc"], None), Some(&landed), &claude_travelled(&["--stdio", "--verbose"]))
            .unwrap();
        assert_eq!(outcomes(&changed.results), [MergeOutcome::Replaced]);
        assert_eq!(json(&changed.text)["mcpServers"]["gsc"]["args"], json!(["--stdio", "--verbose"]));

        // The entry as the agent wrote it back, with a field of its own on it: its digest is not the one the list holds,
        // so it is not in `replace` and the merge leaves it.
        let mut theirs = json(&landed);
        theirs["mcpServers"]["gsc"]["disabled"] = json!(false);
        let left = MCP_SERVERS_JSON
            .merge(&lib, &merge_scope(&["gsc"], &[], &[], None), Some(&pretty(&theirs)), &claude_travelled(&["--stdio", "--verbose"]))
            .unwrap();
        assert_eq!(outcomes(&left.results), [MergeOutcome::Theirs]);
        assert_eq!(left.text, pretty(&theirs));
    }

    #[test]
    fn mcpservers_json_a_keys_shape_is_read_whatever_order_the_agent_wrote_it_back_in_a() {
        let landed = MCP_SERVERS_JSON
            .merge(&lib, &merge_scope(&["gsc"], &[], &[], None), Some(&claude_own()), &claude_travelled(&["--stdio"]))
            .unwrap()
            .text;
        let rewritten = pretty(&json!({ "mcpServers": { "gsc": { "args": ["--stdio"], "command": "bare" } } }));
        let entry = MCP_SERVERS_JSON.entry_of(&landed, "gsc", None);
        assert!(entry.is_some());
        assert_eq!(MCP_SERVERS_JSON.entry_of(&rewritten, "gsc", None), entry);
        assert_eq!(MCP_SERVERS_JSON.entry_of(&landed, "nobody", None), None);
        assert_eq!(MCP_SERVERS_JSON.entry_of("{ not json", "gsc", None), None);
    }

    #[test]
    fn mcpservers_json_a_server_of_the_persons_home_folder_lands_under_the_machines_own() {
        let m = MCP_SERVERS_JSON
            .merge(&lib, &merge_scope(&["zed"], &[], &[], Some((HOME, "/root"))), Some(&claude_own()), &claude_travelled(&["--stdio"]))
            .unwrap();
        assert_eq!(m.results.iter().map(|r| (r.name.as_str(), r.outcome)).collect::<Vec<_>>(), [("zed", MergeOutcome::Added)]);
        let root = json(&m.text);
        assert_eq!(root["projects"]["/root"]["mcpServers"], json!({ "zed": { "command": "/root/.local/bin/zed", "args": [] } }));
        assert_eq!(root["projects"].get(HOME), None);
        assert_eq!(root["projects"]["/root/work"], json!({ "history": ["his own turn"] }));
        assert_eq!(keys(&root["mcpServers"]), ["mine"]);
        let entry = MCP_SERVERS_JSON.entry_of(&m.text, "zed", Some("/root"));
        assert!(entry.is_some());
        assert_eq!(entry, MCP_SERVERS_JSON.entry_of(&m.text, "zed", Some("/root")));
        assert_eq!(MCP_SERVERS_JSON.entry_of(&m.text, "zed", None), None);
    }

    #[test]
    fn mcpservers_json_a_file_that_is_not_there_yet_is_made_from_wsps_servers_alone_and() {
        let travelled = claude_travelled(&["--stdio"]);
        let made = MCP_SERVERS_JSON.merge(&lib, &merge_scope(&["notion"], &[], &[], None), None, &travelled).unwrap();
        assert_eq!(json(&made.text), json!({ "mcpServers": { "notion": { "command": "npx", "args": ["-y", "notion-mcp"] } } }));
        assert_eq!(MCP_SERVERS_JSON.merge(&lib, &merge_scope(&["nobody"], &[], &[], None), None, &travelled).unwrap().text, "");
        let e = MCP_SERVERS_JSON.merge(&lib, &merge_scope(&[], &[], &[], None), Some("[]"), &travelled).unwrap_err();
        assert!(e.message.contains("the file is not a JSON object"));
    }

    #[test]
    fn every_json_format_merges_into_a_jsonc_file_in_place_each_comment_it_did_not_take() {
        for (format, key, entry) in json_formats() {
            let own = commented(key, &entry);
            let mut servers = Map::new();
            servers.insert("gsc".to_string(), entry(&format!("{HOME}/.local/bin/bare")));
            servers.insert("old".to_string(), entry("x"));
            let mut travelled = Map::new();
            travelled.insert(key.to_string(), Value::Object(servers));
            travelled.insert("projects".to_string(), kv(HOME, kv(key, json!({ "zed": entry(&format!("{HOME}/zed")) }))));
            let travelled = Value::Object(travelled).to_string();
            let m = format.merge(&lib, &merge_scope(&["gsc"], &["old"], &["old"], None), Some(&own), &travelled).unwrap();
            assert_eq!(outcomes(&m.results), [MergeOutcome::Added, MergeOutcome::Dropped], "{key}");
            for c in COMMENTS {
                assert!(m.text.contains(c), "{c}");
            }
            let mut want = parse_jsonc(&own).unwrap();
            want[key] = json!({ "mine": entry("/usr/local/bin/mine"), "gsc": entry("bare") });
            assert_eq!(parse_jsonc(&m.text).unwrap(), want);
            if format == OPENCODE_JSON {
                continue;
            }
            let home = format.merge(&lib, &merge_scope(&["zed"], &[], &[], Some((HOME, "/root"))), Some(&m.text), &travelled).unwrap();
            for c in COMMENTS {
                assert!(home.text.contains(c), "{c}");
            }
            assert_eq!(
                parse_jsonc(&home.text).unwrap()["projects"],
                json!({ "/root/work": { "history": [] }, "/root": kv(key, json!({ "zed": entry("/root/zed") })) })
            );
        }
    }

    #[test]
    fn mcpservers_json_many_servers_merged_at_once_land_in_order_after_the_persons_own() {
        let names: Vec<String> = (0..20).map(|i| format!("s{i}")).collect();
        let all = |names: &[String]| -> Value {
            Value::Object(names.iter().map(|n| (n.clone(), json!({ "command": "npx", "args": [n] }))).collect())
        };
        let travelled = json!({ "mcpServers": all(&names), "projects": kv(HOME, json!({ "mcpServers": all(&names) })) }).to_string();
        let mut want: Vec<(String, Value)> = vec![("mine".to_string(), json!({ "command": "mine", "args": [] }))];
        for n in &names {
            want.push((n.clone(), json!({ "command": "npx", "args": [n] })));
        }
        let owns = [
            "{\n  // mine\n  \"mcpServers\": {\n    \"mine\": { \"command\": \"mine\", \"args\": [] } // his\n  }\n}\n",
            "{ /* mine */ \"mcpServers\": { \"mine\": { \"command\": \"mine\", \"args\": [] } } }",
        ];
        let keep: Vec<&str> = names.iter().map(|n| n.as_str()).collect();
        for own in owns {
            let m = MCP_SERVERS_JSON.merge(&lib, &merge_scope(&keep, &[], &[], None), Some(own), &travelled).unwrap();
            assert!(m.results.iter().all(|r| r.outcome == MergeOutcome::Added));
            let got: Vec<(String, Value)> =
                parse_jsonc(&m.text).unwrap()["mcpServers"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            assert_eq!(got, want);
            for c in ["// mine", "// his", "/* mine */"] {
                if own.contains(c) {
                    assert!(m.text.contains(c));
                }
            }
            let home =
                MCP_SERVERS_JSON.merge(&lib, &merge_scope(&keep, &[], &[], Some((HOME, "/root"))), Some(&m.text), &travelled).unwrap();
            assert_eq!(keys(&parse_jsonc(&home.text).unwrap()["projects"]["/root"]["mcpServers"]), names);
        }
    }

    #[test]
    fn codexs_toml_its_trust_tables_its_hooks_state_and_the_persons_own_server_stand_li() {
        let own = codex_own();
        let m = CODEX_TOML
            .merge(&lib, &merge_scope(&["context7", "mine"], &["gone"], &[], None), Some(&own), &codex_travelled(&["--stdio"]))
            .unwrap();
        assert_eq!(
            m.results,
            vec![
                merged("context7", MergeOutcome::Added, Some("/root/.local/bin/context7")),
                merged("mine", MergeOutcome::Theirs, Some("/root/other")),
                merged("gone", MergeOutcome::Left, None),
            ]
        );
        // Every line the agent and the person wrote is where it was, the table Codex trusts a folder by included.
        assert!(m.text.starts_with(&own));
        assert_eq!(
            &m.text[own.len()..],
            lines(&[
                "",
                "[mcp_servers.context7]",
                r#"command = "/root/.local/bin/context7""#,
                r#"args = ["--stdio"]"#,
                "",
                "[mcp_servers.context7.env]",
                r#"KEY = "k""#,
                ""
            ])
        );
        assert_eq!(names(&CODEX_TOML.read(&m.text, "/root")), ["mine", "context7"]);

        let again = CODEX_TOML
            .merge(&lib, &merge_scope(&["context7", "mine"], &[], &["context7"], None), Some(&m.text), &codex_travelled(&["--stdio"]))
            .unwrap();
        assert_eq!(outcomes(&again.results), [MergeOutcome::Same, MergeOutcome::Theirs]);
        assert_eq!(again.text, m.text);
    }

    #[test]
    fn codexs_toml_wsps_own_table_is_written_over_where_it_stands_and_taken_out_with_it() {
        let own = codex_own();
        let landed =
            CODEX_TOML.merge(&lib, &merge_scope(&["context7"], &[], &[], None), Some(&own), &codex_travelled(&["--stdio"])).unwrap().text;
        let changed = CODEX_TOML
            .merge(&lib, &merge_scope(&["context7"], &[], &["context7"], None), Some(&landed), &codex_travelled(&["--stdio", "--verbose"]))
            .unwrap();
        assert_eq!(outcomes(&changed.results), [MergeOutcome::Replaced]);
        assert_eq!(changed.text, landed.replacen("args = [\"--stdio\"]", "args = [\"--stdio\", \"--verbose\"]", 1));

        let dropped = CODEX_TOML
            .merge(&lib, &merge_scope(&[], &["context7", "mine"], &["context7"], None), Some(&landed), &codex_travelled(&["--stdio"]))
            .unwrap();
        assert_eq!(dropped.results, vec![merged("context7", MergeOutcome::Dropped, None), merged("mine", MergeOutcome::Left, None)]);
        assert_eq!(dropped.text, own);
    }

    #[test]
    fn codexs_toml_a_table_reads_the_same_however_codex_spaced_and_commented_it_and_a_f() {
        let landed = CODEX_TOML
            .merge(&lib, &merge_scope(&["context7"], &[], &[], None), Some(&codex_own()), &codex_travelled(&["--stdio"]))
            .unwrap()
            .text;
        let rewritten = landed.replacen(
            "command = \"/root/.local/bin/context7\"",
            "  command = \"/root/.local/bin/context7\"   # codex wrote this back",
            1,
        );
        let entry = CODEX_TOML.entry_of(&landed, "context7", None);
        assert!(entry.is_some());
        assert_eq!(CODEX_TOML.entry_of(&rewritten, "context7", None), entry);
        assert_ne!(CODEX_TOML.entry_of(&format!("{landed}startup_timeout_sec = 30\n"), "context7", None), entry);
        assert_eq!(CODEX_TOML.entry_of(&landed, "nobody", None), None);
    }

    #[test]
    fn codexs_toml_a_file_that_is_not_there_yet_is_one_table_with_no_blank_line_in_fron() {
        let travelled = codex_travelled(&["--stdio"]);
        let made = CODEX_TOML.merge(&lib, &merge_scope(&["mine"], &[], &[], None), None, &travelled).unwrap();
        assert_eq!(made.text, "[mcp_servers.mine]\ncommand = \"/root/other\"\n");
        assert_eq!(CODEX_TOML.merge(&lib, &merge_scope(&["nobody"], &[], &[], None), None, &travelled).unwrap().text, "");
    }
}

mod remove_group {
    use super::*;

    #[test]
    fn mcpservers_json_the_named_keys_go_under_the_user_scope_and_the_home_folder_alike() {
        let travelled = claude_travelled(&["--stdio"]);
        let m =
            MCP_SERVERS_JSON.merge(&lib, &merge_scope(&["gsc", "notion"], &[], &[], None), Some(&claude_own()), &travelled).unwrap().text;
        let with_home =
            MCP_SERVERS_JSON.merge(&lib, &merge_scope(&["zed"], &[], &[], Some((HOME, "/root"))), Some(&m), &travelled).unwrap().text;

        let gone = remove(MCP_SERVERS_JSON, &with_home, &["gsc", "notion"]);
        let root = json(&gone);
        assert_eq!(keys(&root["mcpServers"]), ["mine"]);
        assert_eq!(root["numStartups"], 41);
        assert_eq!(root["oauthAccount"], json!({ "emailAddress": "he@example.com" }));
        assert_eq!(root["projects"]["/root/work"], json!({ "history": ["his own turn"] }));
        // The home folder's servers are the person's own scope and are named there, not under the root key.
        assert_eq!(root["projects"]["/root"]["mcpServers"], json!({ "zed": { "command": "/root/.local/bin/zed", "args": [] } }));
        let both = MCP_SERVERS_JSON.remove(&gone, &strs(&["zed"]), Some("/root")).unwrap();
        assert_eq!(json(&both)["projects"]["/root"]["mcpServers"], json!({}));

        // A name the file does not define, a scope it has nothing under, and a file with no servers at all: the text
        // stands as it is rather than being rewritten for nothing.
        assert_eq!(remove(MCP_SERVERS_JSON, &with_home, &["nobody"]), with_home);
        assert_eq!(MCP_SERVERS_JSON.remove(&with_home, &strs(&["gsc"]), Some("/root/elsewhere")).unwrap(), with_home);
        assert_eq!(remove(MCP_SERVERS_JSON, r#"{ "theme": "x" }"#, &["gsc"]), r#"{ "theme": "x" }"#);
        assert_refused(MCP_SERVERS_JSON.remove("[]", &strs(&["gsc"]), None), "the file is not a JSON object");
    }

    #[test]
    fn opencodes_json_the_named_keys_go_from_under_its_own_key_every_other_byte_of_the() {
        let own = lines(&[
            "{",
            r#"  "theme": "x","#,
            r#"  "mcp": { "wsp": { "type": "local", "command": ["wsp", "mcp"] }, "mine": { "type": "local", "command": ["mine"] } }"#,
            "}",
            "",
        ]);
        let gone = remove(OPENCODE_JSON, &own, &["wsp"]);
        assert_eq!(json(&gone), json!({ "theme": "x", "mcp": { "mine": { "type": "local", "command": ["mine"] } } }));
        assert_eq!(gone, lines(&["{", r#"  "theme": "x","#, r#"  "mcp": { "mine": { "type": "local", "command": ["mine"] } }"#, "}", ""]));
        assert_eq!(remove(OPENCODE_JSON, &own, &["nobody"]), own);
    }

    #[test]
    fn a_jsonc_file_keeps_its_comments_when_a_key_comes_out_of_it() {
        let own = lines(&[
            "{",
            "  // my own servers",
            r#"  "theme": "x","#,
            r#"  "mcp": { "wsp": { "type": "local", "command": ["wsp", "mcp"] } } // the rest"#,
            "}",
            "",
        ]);
        let gone = remove(OPENCODE_JSON, &own, &["wsp"]);
        assert!(gone.contains("// my own servers\n"));
        assert!(gone.contains("// the rest\n"));
        assert_eq!(parse_jsonc(&gone).unwrap(), json!({ "theme": "x", "mcp": {} }));
        assert_eq!(remove(OPENCODE_JSON, &own, &["nobody"]), own);
    }

    #[test]
    fn a_remove_takes_out_the_servers_own_lines_its_comment_lines_above_it_and_the_comm() {
        let claude = lines(&[
            "{ // my settings",
            r#"  "mcpServers": { // header for all my servers"#,
            "    // about a",
            r#"    "a": { "command": "x" }, // note on a"#,
            "    // about b",
            r#"    "b": { "command": "y" } // note on b"#,
            "  } // end of servers",
            "}",
            "",
        ]);
        assert_eq!(
            remove(MCP_SERVERS_JSON, &claude, &["b"]),
            lines(&[
                "{ // my settings",
                r#"  "mcpServers": { // header for all my servers"#,
                "    // about a",
                r#"    "a": { "command": "x" } // note on a"#,
                "  } // end of servers",
                "}",
                "",
            ])
        );
        assert_eq!(
            remove(MCP_SERVERS_JSON, &claude, &["a"]),
            lines(&[
                "{ // my settings",
                r#"  "mcpServers": { // header for all my servers"#,
                "    // about b",
                r#"    "b": { "command": "y" } // note on b"#,
                "  } // end of servers",
                "}",
                "",
            ])
        );
        let middle = "{\n  \"mcpServers\": {\n    \"a\": { \"command\": \"x\" }, // note on a\n\n    // section two\n    \"b\": { \"command\": \"y\" }, /* note on b */\n    \"c\": { \"command\": \"z\" } // note on c\n  }\n}\n";
        assert_eq!(
            remove(MCP_SERVERS_JSON, middle, &["b"]),
            "{\n  \"mcpServers\": {\n    \"a\": { \"command\": \"x\" }, // note on a\n\n    \"c\": { \"command\": \"z\" } // note on c\n  }\n}\n"
        );
        let blank = "{\n  \"mcpServers\": {\n    // my servers\n\n    \"a\": { \"command\": \"x\" }\n  }\n}\n";
        assert_eq!(remove(MCP_SERVERS_JSON, blank, &["a"]), "{\n  \"mcpServers\": {\n    // my servers\n\n  }\n}\n");
    }

    #[test]
    fn gemini_cli_turns_a_server_on_and_takes_a_removed_one_out_of_mcp_excluded_by_that() {
        let text = lines(&[
            "{",
            r#"  "mcp": {"#,
            r#"    "excluded": [ // switched off"#,
            r#"      "s", // trying it"#,
            r#"      "other" // keep off"#,
            "    ]",
            "  },",
            r#"  "mcpServers": { "s": { "command": "x" }, "other": { "command": "y" }, "t": { "command": "z" } }"#,
            "}",
            "",
        ]);
        let kept = lines(&[r#"    "excluded": [ // switched off"#, r#"      "other" // keep off"#, "    ]"]);
        assert!(enable(GEMINI_SETTINGS_JSON, &text, "s", true).contains(&kept));
        assert!(remove(GEMINI_SETTINGS_JSON, &text, &["s"]).contains(&kept));
        let off = enable(GEMINI_SETTINGS_JSON, &text, "t", false);
        assert!(off.contains(&lines(&[r#"      "s", // trying it"#, r#"      "other", // keep off"#, r#"      "t""#, "    ]"])));
        assert_eq!(
            enable(GEMINI_SETTINGS_JSON, r#"{ "mcp": { "excluded": ["s"] }, "mcpServers": { "t": {} } }"#, "t", false),
            r#"{ "mcp": { "excluded": ["s", "t"] }, "mcpServers": { "t": {} } }"#
        );
        assert_eq!(
            flags(&GEMINI_SETTINGS_JSON.read(&off, HOME)),
            [("s".to_string(), true), ("other".to_string(), true), ("t".to_string(), true)]
        );
    }

    #[test]
    fn gemini_cli_takes_a_removed_servers_name_out_of_mcp_excluded_too_so_a_later_serve() {
        let text = lines(&[
            "{",
            "  // mine",
            r#"  "mcp": { "excluded": ["s", "t"] },"#,
            r#"  "mcpServers": { "s": { "command": "x" }, "t": { "command": "y" } }"#,
            "}",
            "",
        ]);
        let gone = remove(GEMINI_SETTINGS_JSON, &text, &["s"]);
        assert_eq!(parse_jsonc(&gone).unwrap(), json!({ "mcp": { "excluded": ["t"] }, "mcpServers": { "t": { "command": "y" } } }));
        assert!(gone.contains("// mine\n"));
        let again = place(GEMINI_SETTINGS_JSON, Some(&gone), "s", &server());
        assert_eq!(flags(&GEMINI_SETTINGS_JSON.read(&again, HOME)), [("t".to_string(), true), ("s".to_string(), false)]);
        assert_eq!(remove(GEMINI_SETTINGS_JSON, &text, &["nobody"]), text);
    }

    #[test]
    fn codexs_toml_the_named_tables_go_with_their_sub_tables_and_the_persons_trust_tabl() {
        let own = codex_own();
        let landed =
            CODEX_TOML.merge(&lib, &merge_scope(&["context7"], &[], &[], None), Some(&own), &codex_travelled(&["--stdio"])).unwrap().text;
        assert!(landed.contains("[mcp_servers.context7.env]"));
        assert_eq!(remove(CODEX_TOML, &landed, &["context7"]), own);
        // A name no table stands for leaves the file exactly as it was, comments and spacing with it.
        assert_eq!(remove(CODEX_TOML, &landed, &["nobody"]), landed);
        assert!(!remove(CODEX_TOML, &landed, &["mine"]).contains("[mcp_servers.mine]"));
        assert!(remove(CODEX_TOML, &landed, &["mine"]).contains(r#"[projects."/private/tmp/proof-907/repo"]"#));
    }
}
