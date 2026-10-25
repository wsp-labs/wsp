// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/mcp-refer.test.ts.
use std::fmt::Debug;

use base64::Engine;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::jsonc::parse_jsonc;
use wsp_catalog::mcp::{McpFormat, McpReferred, McpResolved, McpTransport, ReferredServer, CODEX_TOML, GEMINI_SETTINGS_JSON, MCP_SERVERS_JSON, OPENCODE_JSON};
use wsp_catalog::mcp_refs::{mcp_header_variable, oauth_secrets_in, refers, servers_by_name, still_stands, ByName, Dropped, HeldValue, KnownEntry};
use wsp_words::exit::HostError;

const HOME: &str = "/Users/dev";
const TOKEN: &str = "lin_api_TESTONLY";
const NOTION: &str = "ntn_TESTONLY";

type Held = IndexMap<String, HeldValue>;
type Known = IndexMap<String, KnownEntry>;

fn set(names: &[&str]) -> IndexSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

fn refer(format: McpFormat, text: &str) -> Result<McpReferred, HostError> {
    format.refer(text, None, &[], &IndexSet::new())
}

fn refer_only(format: McpFormat, text: &str, only: &str) -> Result<McpReferred, HostError> {
    format.refer(text, Some(only), &[], &IndexSet::new())
}

fn values(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn server(name: &str, pairs: &[(&str, &str)]) -> ReferredServer {
    ReferredServer { name: name.to_string(), project: None, values: values(pairs), unread: None }
}

fn server_in(name: &str, project: &str, pairs: &[(&str, &str)]) -> ReferredServer {
    ReferredServer { name: name.to_string(), project: Some(project.to_string()), values: values(pairs), unread: None }
}

fn said<T: Debug>(r: Result<T, HostError>) -> String {
    match r {
        Ok(_) => String::new(),
        Err(e) => e.message,
    }
}

fn throws<T: Debug>(r: Result<T, HostError>, said: &str) {
    match r {
        Ok(v) => panic!("expected an error containing {said:?}, got {v:?}"),
        Err(e) => assert!(e.message.contains(said), "expected {said:?} in {:?}", e.message),
    }
}

fn known(pairs: &[(&str, &str)], by: &[&str]) -> Known {
    pairs.iter().map(|(n, v)| (n.to_string(), KnownEntry { value: v.to_string(), by: by.iter().map(|b| b.to_string()).collect() })).collect()
}

fn no_rows(_: &str) -> Option<String> {
    None
}

fn rows(name: &str) -> Option<String> {
    (name == "GEMINI_API_KEY").then(|| "Gemini CLI".to_string())
}

fn by_name(format: McpFormat, text: &str) -> Result<ByName, HostError> {
    servers_by_name(format, text, &mut Held::new(), &no_rows, &Known::new())
}

fn by_name_known(format: McpFormat, text: &str, known: &Known) -> Result<ByName, HostError> {
    servers_by_name(format, text, &mut Held::new(), &no_rows, known)
}

fn by_name_held(format: McpFormat, text: &str, held: &mut Held, known: &Known) -> Result<ByName, HostError> {
    servers_by_name(format, text, held, &no_rows, known)
}

fn held_list(held: &Held) -> Vec<(String, HeldValue)> {
    held.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn held_value(value: &str, by: &str) -> HeldValue {
    HeldValue { value: value.to_string(), by: by.to_string() }
}

fn dropped(name: &str, reason: &str) -> Dropped {
    Dropped { name: name.to_string(), reason: reason.to_string() }
}

fn js(v: Value) -> String {
    serde_json::to_string(&v).unwrap()
}

fn js_pretty(v: Value) -> String {
    serde_json::to_string_pretty(&v).unwrap()
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn lines(l: &[&str]) -> String {
    l.join("\n")
}

fn btoa(v: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(v)
}

fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn dollar(name: &str) -> String {
    format!("${{{name}}}")
}

fn envref(name: &str) -> String {
    format!("{{env:{name}}}")
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v.clone());
    }
    Value::Object(m)
}

fn claude(servers: Value) -> String {
    js_pretty(json!({ "mcpServers": servers }))
}

fn unread_line(path: &str) -> String {
    format!("reads a variable servers.env does not hold in {path}, so it could not start there")
}

fn first_server(format: McpFormat, text: &str) -> wsp_catalog::mcp::McpServer {
    format.read(text, HOME).into_iter().next().unwrap()
}

/// What servers_by_name says of a file whose reference writer hands back these servers and entries: the TS tests'
/// stand-in format, `{ ...FORMAT, refer: async text => ({ text, servers, entries }) }`.
fn last_check(format: McpFormat, entries: Vec<Value>, servers: Vec<ReferredServer>, known: &Known) -> Result<(), HostError> {
    let refer = |text: &str, _: Option<&str>, _: &[(String, String)], _: &indexmap::IndexSet<String>| Ok::<_, HostError>(wsp_catalog::mcp::McpReferred { text: text.to_string(), servers: servers.clone(), entries: entries.clone() });
    let text = if format == McpFormat::CodexToml { "[mcp_servers.s]\n" } else { "{}" };
    let mut held = IndexMap::new();
    let out = wsp_catalog::mcp_refs::servers_by_name_with(format, &refer, text, &mut held, &|_| None, known)?;
    assert_eq!(out.dropped, vec![], "a stand-in writer that drops nothing leaves nothing dropped");
    Ok(())
}

fn lax(format: McpFormat, entries: Vec<Value>, known: &Known) -> Result<(), HostError> {
    last_check(format, entries, vec![server("s", &[])], known)
}

mod the_name_a_header_s_value_travels_under {
    use super::*;

    #[test]
    fn is_wsp_mcp_server_header_in_capitals_every_other_character_an_underscore() {
        assert_eq!(mcp_header_variable("linear", "Authorization"), "WSP_MCP_LINEAR_AUTHORIZATION");
        assert_eq!(mcp_header_variable("my-docs.v2", "X-Api-Key"), "WSP_MCP_MY_DOCS_V2_X_API_KEY");
    }
}

mod claude_code_s_reference_writer {
    use super::*;

    fn file() -> String {
        js_pretty(json!({
            "numStartups": 3,
            "mcpServers": {
                "linear": { "type": "http", "url": "https://mcp.linear.app/mcp", "headers": { "Authorization": format!("Bearer {TOKEN}"), "X-Team": "eng" } },
                "notion": { "command": "npx", "args": ["-y", "notion-mcp"], "env": { "NOTION_TOKEN": NOTION, "LOG": "${LOG_LEVEL:-info}" } },
                "mine": { "type": "http", "url": "https://x.example/mcp", "headers": { "Authorization": "Bearer ${MY_TOKEN}" } }
            },
            "projects": { "/Users/dev": { "mcpServers": { "zomato": { "type": "http", "url": "https://z.example/mcp", "headers": { "X-Key": "zk_TESTONLY" } } } } }
        }))
    }

    #[test]
    fn writes_name_in_headers_and_env_the_bearer_s_token_alone_in_the_vault_and_leaves() {
        let out = MCP_SERVERS_JSON.refer(&file(), None, &[], &set(&["LOG_LEVEL", "MY_TOKEN"])).unwrap();
        let root = parse_jsonc(&out.text).unwrap();
        assert_eq!(root["mcpServers"]["linear"]["headers"], json!({ "Authorization": "Bearer ${WSP_MCP_LINEAR_AUTHORIZATION}", "X-Team": "${WSP_MCP_LINEAR_X_TEAM}" }));
        // A held name's default goes: the value is servers.env's.
        assert_eq!(root["mcpServers"]["notion"]["env"], json!({ "NOTION_TOKEN": "${NOTION_TOKEN}", "LOG": "${LOG_LEVEL}" }));
        assert_eq!(root["mcpServers"]["mine"]["headers"], json!({ "Authorization": "Bearer ${MY_TOKEN}" }));
        assert_eq!(root["projects"]["/Users/dev"]["mcpServers"]["zomato"]["headers"], json!({ "X-Key": "${WSP_MCP_ZOMATO_X_KEY}" }));
        assert_eq!(
            out.servers,
            vec![
                server("linear", &[("WSP_MCP_LINEAR_AUTHORIZATION", TOKEN), ("WSP_MCP_LINEAR_X_TEAM", "eng")]),
                server("notion", &[("NOTION_TOKEN", NOTION)]),
                server("mine", &[]),
                server_in("zomato", HOME, &[("WSP_MCP_ZOMATO_X_KEY", "zk_TESTONLY")]),
            ]
        );
        for secret in [TOKEN, NOTION, "zk_TESTONLY"] {
            assert!(!out.text.contains(secret), "{secret} still in the text");
        }
        // The agent's own reader sees the names the definition now reads.
        let read = MCP_SERVERS_JSON.read(&out.text, HOME);
        let linear = read.iter().find(|s| s.name == "linear").unwrap();
        assert_eq!(linear.env_refs, vec!["WSP_MCP_LINEAR_AUTHORIZATION".to_string(), "WSP_MCP_LINEAR_X_TEAM".to_string()]);
    }

    #[test]
    fn keeps_every_other_key_and_comment_of_the_file_where_it_was() {
        let text = "{\n  // mine\n  \"theme\": \"dark\",\n  \"mcpServers\": {\n    \"a\": { \"command\": \"x\", \"env\": { \"K\": \"v1\" } } // the a server\n  }\n}\n";
        let out = refer(MCP_SERVERS_JSON, text).unwrap();
        assert_eq!(out.text, "{\n  // mine\n  \"theme\": \"dark\",\n  \"mcpServers\": {\n    \"a\": { \"command\": \"x\", \"env\": { \"K\": \"${K}\" } } // the a server\n  }\n}\n");
    }

    #[test]
    fn names_one_server_alone_when_asked_and_stands_byte_for_byte_where_nothing_holds_a() {
        let notion = refer_only(MCP_SERVERS_JSON, &file(), "notion").unwrap();
        assert_eq!(notion.servers.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["notion"]);
        assert!(refer_only(MCP_SERVERS_JSON, &file(), "notion").unwrap().text.contains(TOKEN));
        let plain = "{ \"mcpServers\": { \"a\": { \"command\": \"x\" } } }";
        assert_eq!(refer(MCP_SERVERS_JSON, plain).unwrap().text, plain);
    }
}

mod the_other_json_formats_write_their_own_syntax {
    use super::*;

    #[test]
    fn gemini_cli_writes_name_opencode_env_name() {
        let gemini = refer(GEMINI_SETTINGS_JSON, &js(json!({ "mcpServers": { "l": { "httpUrl": "https://l.example", "headers": { "Authorization": format!("Bearer {TOKEN}") } } } }))).unwrap();
        assert!(gemini.text.contains("\"Bearer ${WSP_MCP_L_AUTHORIZATION}\""));
        let opencode = refer(
            OPENCODE_JSON,
            &js(json!({ "mcp": {
                "l": { "type": "remote", "url": "https://l.example", "headers": { "Authorization": format!("Bearer {TOKEN}") } },
                "n": { "type": "local", "command": ["npx"], "environment": { "NOTION_TOKEN": NOTION } }
            } })),
        )
        .unwrap();
        let root = parse_jsonc(&opencode.text).unwrap();
        assert_eq!(root["mcp"]["l"]["headers"], json!({ "Authorization": "Bearer {env:WSP_MCP_L_AUTHORIZATION}" }));
        assert_eq!(root["mcp"]["n"]["environment"], json!({ "NOTION_TOKEN": "{env:NOTION_TOKEN}" }));
        assert_eq!(opencode.servers, vec![server("l", &[("WSP_MCP_L_AUTHORIZATION", TOKEN)]), server("n", &[("NOTION_TOKEN", NOTION)])]);
    }
}

mod codex_s_reference_writer {
    use super::*;

    #[test]
    fn writes_env_http_headers_and_env_vars_in_place_of_the_values_every_other_line_as() {
        let text = lines(&[
            r#"model = "gpt-5""#,
            "",
            "# linear, added by hand",
            "[mcp_servers.linear]",
            r#"url = "https://mcp.linear.app/mcp""#,
            &format!(r#"http_headers = {{ "Authorization" = "Bearer {TOKEN}", "X-Team" = "eng" }}"#),
            "startup_timeout_sec = 20",
            "",
            "[mcp_servers.notion]",
            r#"command = "npx""#,
            r#"args = ["-y", "notion-mcp"]"#,
            "",
            "[mcp_servers.notion.env]",
            &format!(r#"NOTION_TOKEN = "{NOTION}""#),
            "",
            r#"[projects."/Users/dev"]"#,
            r#"trust_level = "trusted""#,
            "",
        ]);
        let out = refer(CODEX_TOML, &text).unwrap();
        assert_eq!(
            out.text,
            lines(&[
                r#"model = "gpt-5""#,
                "",
                "# linear, added by hand",
                "[mcp_servers.linear]",
                r#"env_http_headers = { "Authorization" = "WSP_MCP_LINEAR_AUTHORIZATION_BEARER", "X-Team" = "WSP_MCP_LINEAR_X_TEAM" }"#,
                r#"url = "https://mcp.linear.app/mcp""#,
                "startup_timeout_sec = 20",
                "",
                "[mcp_servers.notion]",
                r#"env_vars = ["NOTION_TOKEN"]"#,
                r#"command = "npx""#,
                r#"args = ["-y", "notion-mcp"]"#,
                "",
                r#"[projects."/Users/dev"]"#,
                r#"trust_level = "trusted""#,
                "",
            ])
        );
        assert_eq!(out.servers, vec![server("linear", &[("WSP_MCP_LINEAR_AUTHORIZATION", TOKEN), ("WSP_MCP_LINEAR_X_TEAM", "eng")]), server("notion", &[("NOTION_TOKEN", NOTION)])]);
        let read = CODEX_TOML.read(&out.text, HOME);
        let linear = read.iter().find(|s| s.name == "linear").unwrap();
        assert!(matches!(&linear.transport, McpTransport::Http { headers, .. } if headers.is_empty()), "{:?}", linear.transport);
        assert_eq!(linear.env_refs, vec!["WSP_MCP_LINEAR_AUTHORIZATION_BEARER".to_string(), "WSP_MCP_LINEAR_X_TEAM".to_string()]);
        let notion = read.iter().find(|s| s.name == "notion").unwrap();
        assert!(matches!(&notion.transport, McpTransport::Stdio { env, .. } if env.is_empty()), "{:?}", notion.transport);
    }

    #[test]
    fn reads_a_name_another_server_s_bearer_header_is_read_by_as_held_the_token_alone_k() {
        let reads = |name: &str| {
            lines(&[
                "[mcp_servers.acme]",
                r#"url = "https://a.example""#,
                r#"http_headers = { "Authorization" = "Bearer tok_TESTONLY" }"#,
                "[mcp_servers.beta]",
                r#"url = "https://b.example""#,
                &format!(r#"env_http_headers = {{ "Authorization" = "{name}" }}"#),
                "",
            ])
        };
        for name in ["WSP_MCP_ACME_AUTHORIZATION_BEARER", "WSP_MCP_ACME_AUTHORIZATION"] {
            assert_eq!(refer(CODEX_TOML, &reads(name)).unwrap().servers, vec![server("acme", &[("WSP_MCP_ACME_AUTHORIZATION", "tok_TESTONLY")]), server("beta", &[])], "{name}");
        }
        assert!(refer(CODEX_TOML, &reads("WSP_MCP_ELSE_AUTHORIZATION_BEARER")).unwrap().servers[1].unread.is_some());
    }

    #[test]
    fn refuses_two_headers_that_would_travel_under_one_name_naming_the_headers_as_the_p() {
        let text = lines(&["[mcp_servers.acme]", r#"url = "https://a.example""#, r#"http_headers = { "Authorization" = "Bearer tok_TESTONLY", "Authorization-Bearer" = "x" }"#, ""]);
        throws(refer(CODEX_TOML, &text), "acme sends Authorization and Authorization-Bearer, which would both travel as WSP_MCP_ACME_AUTHORIZATION_BEARER; rename one");
    }

    #[test]
    fn adds_to_the_names_a_table_already_reads_rather_than_writing_a_key_twice() {
        let text = lines(&[
            "[mcp_servers.a]",
            r#"url = "https://a.example""#,
            r#"bearer_token_env_var = "A_TOKEN""#,
            r#"env_http_headers = { "X-One" = "ONE" }"#,
            r#"http_headers = { "Authorization" = "Bearer x1", "X-Two" = "two" }"#,
            "",
        ]);
        let out = CODEX_TOML.refer(&text, None, &[], &set(&["A_TOKEN", "ONE"])).unwrap();
        assert_eq!(
            out.text,
            lines(&[
                "[mcp_servers.a]",
                r#"url = "https://a.example""#,
                r#"bearer_token_env_var = "A_TOKEN""#,
                r#"env_http_headers = { "X-One" = "ONE", "Authorization" = "WSP_MCP_A_AUTHORIZATION_BEARER", "X-Two" = "WSP_MCP_A_X_TWO" }"#,
                "",
            ])
        );
        assert_eq!(out.servers[0].values, values(&[("WSP_MCP_A_AUTHORIZATION", "x1"), ("WSP_MCP_A_X_TWO", "two")]));
        let stdio = lines(&["[mcp_servers.b]", r#"command = "b""#, r#"env_vars = ["HOME_DIR"]"#, r#"env = { K = "v" }"#, ""]);
        assert_eq!(refer(CODEX_TOML, &stdio).unwrap().text, lines(&["[mcp_servers.b]", r#"command = "b""#, r#"env_vars = ["HOME_DIR", "K"]"#, ""]));
    }
}

mod a_whole_file_s_servers_by_name {
    use super::*;

    #[test]
    fn hands_every_value_to_the_vault_once_and_takes_out_a_server_that_sets_a_name_anot() {
        let mut held = Held::new();
        let first = servers_by_name(MCP_SERVERS_JSON, &claude(json!({ "notion": { "command": "a", "env": { "NOTION_TOKEN": NOTION } } })), &mut held, &rows, &Known::new()).unwrap();
        assert_eq!(first.dropped, vec![]);
        let text = lines(&[
            "[mcp_servers.other]",
            r#"command = "b""#,
            r#"env = { NOTION_TOKEN = "ntn_OTHER" }"#,
            "",
            "[mcp_servers.same]",
            r#"command = "c""#,
            &format!(r#"env = {{ NOTION_TOKEN = "{NOTION}" }}"#),
            "",
        ]);
        let second = servers_by_name(CODEX_TOML, &text, &mut held, &rows, &Known::new()).unwrap();
        assert_eq!(second.dropped, vec![dropped("other", "sets NOTION_TOKEN, which notion already sets to another value")]);
        assert!(!second.text.contains("ntn_OTHER"));
        assert!(second.text.contains("[mcp_servers.same]\nenv_vars = [\"NOTION_TOKEN\"]"), "{}", second.text);
        assert_eq!(held_list(&held), vec![("NOTION_TOKEN".to_string(), held_value(NOTION, "notion"))]);
    }

    #[test]
    fn takes_out_of_the_copy_a_server_that_sets_a_catalog_row_s_own_variable_or_a_value() {
        let mut held = Held::new();
        let text = claude(json!({
            "gem": { "command": "gem-mcp", "env": { "GEMINI_API_KEY": "gem_TESTONLY" } },
            "pem": { "command": "p", "env": { "KEY": "-----BEGIN\nx\n-----END" } }
        }));
        let out = servers_by_name(MCP_SERVERS_JSON, &text, &mut held, &rows, &Known::new()).unwrap();
        assert_eq!(
            out.dropped,
            vec![
                dropped("gem", "GEMINI_API_KEY belongs to the Gemini CLI key, so set it there or give the variable another name"),
                dropped("pem", "sets KEY to a value on more than one line, which cannot travel by name"),
            ]
        );
        assert!(!out.text.contains("gem_TESTONLY"));
        assert!(!out.text.contains("BEGIN"));
        assert_eq!(held.len(), 0);
    }

    #[test]
    fn throws_the_format_s_own_words_where_a_file_is_not_the_format_so_a_caller_never_c() {
        for f in [MCP_SERVERS_JSON, GEMINI_SETTINGS_JSON, OPENCODE_JSON] {
            throws(servers_by_name(f, "not json", &mut Held::new(), &rows, &Known::new()), "the file is not valid JSON");
        }
    }
}

mod what_a_reference_writer_refuses_rather_than_guess {
    use super::*;

    const SHAPE: &str = "is written in a shape wsp does not read, so its values cannot be written by name";

    #[test]
    fn every_format_a_server_entry_or_server_table_that_is_not_an_object_is_refused_by() {
        let secret = "tok_TESTONLY_bare";
        let vals = [json!(format!("Bearer {secret}")), json!([secret]), json!(42)];
        let mut cases: Vec<(McpFormat, String, String)> = Vec::new();
        for v in &vals {
            cases.push((MCP_SERVERS_JSON, js(json!({ "mcpServers": { "ok": { "command": "a" }, "linear": v } })), "mcpServers.linear".to_string()));
            cases.push((MCP_SERVERS_JSON, js(json!({ "projects": { "/Users/dev": { "mcpServers": { "linear": v } } } })), format!("projects.{HOME}.mcpServers.linear")));
            cases.push((GEMINI_SETTINGS_JSON, js(json!({ "mcpServers": { "linear": v } })), "mcpServers.linear".to_string()));
            cases.push((OPENCODE_JSON, js(json!({ "mcp": { "linear": v } })), "mcp.linear".to_string()));
            cases.push((MCP_SERVERS_JSON, js(json!({ "mcpServers": v })), "mcpServers".to_string()));
            cases.push((MCP_SERVERS_JSON, js(json!({ "projects": { "/Users/dev": { "mcpServers": v } } })), format!("projects.{HOME}.mcpServers")));
            cases.push((OPENCODE_JSON, js(json!({ "mcp": v })), "mcp".to_string()));
        }
        cases.push((CODEX_TOML, format!("[mcp_servers]\nlinear = \"Bearer {secret}\"\n"), "mcp_servers.linear".to_string()));
        cases.push((CODEX_TOML, format!("[mcp_servers]\nlinear = [\"{secret}\"]\n"), "mcp_servers.linear".to_string()));
        cases.push((CODEX_TOML, "[mcp_servers]\nlinear = 42\n".to_string(), "mcp_servers.linear".to_string()));
        for (format, text, at) in &cases {
            assert_eq!(said(by_name(*format, text)), format!("{at} {SHAPE}"), "{text}");
        }
        // A table written as null holds nothing to carry, so the file travels.
        let empty = js(json!({ "mcpServers": null, "projects": { "/Users/dev": { "mcpServers": null } } }));
        assert_eq!(by_name(MCP_SERVERS_JSON, &empty).unwrap().text, empty);
    }

    #[test]
    fn codex_every_toml_spelling_of_a_server_s_headers_and_variables_reads_as_one_tree() {
        let linear = |secret: &str| vec![("WSP_MCP_LINEAR_AUTHORIZATION", secret.to_string())];
        let spellings: Vec<(&str, &str, Vec<(&str, String)>)> = vec![
            ("mcp_servers.linear = { url = \"https://l.example\", http_headers = { Authorization = \"Bearer tok_INLINE\" } }\n", "tok_INLINE", linear("tok_INLINE")),
            ("[mcp_servers]\nlinear = { url = \"https://l.example\", http_headers = { Authorization = \"Bearer tok_BARE\" } }\n", "tok_BARE", linear("tok_BARE")),
            ("[mcp_servers]\nlinear.url = \"https://l.example\"\nlinear.http_headers.Authorization = \"Bearer tok_DOTTEDBARE\"\n", "tok_DOTTEDBARE", linear("tok_DOTTEDBARE")),
            ("[mcp_servers.linear]\nurl = \"https://l.example\"\nhttp_headers.Authorization = \"Bearer tok_DOTTED\"\n", "tok_DOTTED", linear("tok_DOTTED")),
            ("[mcp_servers.notion]\ncommand = \"npx\"\nenv.NOTION_TOKEN = \"ntn_DOTTED\"\n", "ntn_DOTTED", vec![("NOTION_TOKEN", "ntn_DOTTED".to_string())]),
            ("[mcp_servers.linear]\nurl = \"https://l.example\"\n\n[mcp_servers.linear.\"http_headers\"]\nAuthorization = \"Bearer tok_QUOTED\"\n", "tok_QUOTED", linear("tok_QUOTED")),
            ("[mcp_servers.linear]\nurl = \"https://l.example\"\nhttp_headers = { Authorization = \"\"\"Bearer tok_MULTILINE\"\"\" }\n", "tok_MULTILINE", linear("tok_MULTILINE")),
        ];
        for (text, secret, pairs) in &spellings {
            let out = refer(CODEX_TOML, text).unwrap();
            assert!(!out.text.contains(secret), "{text}");
            let expected: IndexMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
            assert_eq!(out.servers.iter().map(|s| s.values.clone()).collect::<Vec<_>>(), vec![expected], "{text}");
            let read: toml::Value = toml::from_str(&out.text).unwrap();
            let def = read["mcp_servers"].as_table().unwrap().values().next().unwrap();
            assert!(def.get("http_headers").is_none(), "{text}");
            assert!(def.get("env").is_none(), "{text}");
            let want = if pairs[0].0 == "NOTION_TOKEN" { json!(["NOTION_TOKEN"]) } else { json!({ "Authorization": "WSP_MCP_LINEAR_AUTHORIZATION_BEARER" }) };
            let want = toml::Value::try_from(want).unwrap();
            assert_eq!(def.get("env_http_headers").or(def.get("env_vars")), Some(&want), "{text}");
        }
    }

    #[test]
    fn codex_refuses_a_server_shape_it_cannot_map_so_the_file_stays_on_this_computer_ra() {
        let shapes = [
            ("[[mcp_servers.linear]]\nurl = \"https://l.example\"\nhttp_headers = { Authorization = \"Bearer tok_ARRAYTABLE\" }\n", "mcp_servers.linear"),
            ("[[mcp_servers]]\nurl = \"https://l.example\"\nhttp_headers = { Authorization = \"Bearer tok_ARRAYROOT\" }\n", "mcp_servers"),
            ("[mcp_servers.linear]\nurl = \"https://l.example\"\n\n[mcp_servers.linear.http_headers.Authorization]\nvalue = \"Bearer tok_NESTED\"\n", "mcp_servers.linear"),
        ];
        for (text, at) in shapes {
            throws(refer(CODEX_TOML, text), &format!("{at} {SHAPE}"));
        }
    }

    #[test]
    fn codex_reads_no_variable_for_an_oauth_client_secret_so_the_file_stays_on_this_com() {
        let said_line = "linear keeps an OAuth secret in mcp_servers.linear.oauth.client_secret, and Codex reads no variable there, so the file stays on this computer";
        let spellings = [
            "[mcp_servers.linear]\nurl = \"https://l.example\"\n\n[mcp_servers.linear.oauth]\nclient_id = \"cid\"\nclient_secret = \"cs_TESTONLY_table\"\n",
            "[mcp_servers.linear]\nurl = \"https://l.example\"\noauth = { client_id = \"cid\", client_secret = \"cs_TESTONLY_inline\" }\n",
            "[mcp_servers.linear]\nurl = \"https://l.example\"\noauth.client_id = \"cid\"\noauth.client_secret = \"cs_TESTONLY_dotted\"\nhttp_headers = { Authorization = \"Bearer tok_TESTONLY\" }\n",
            "mcp_servers.linear = { url = \"https://l.example\", oauth = { client_id = \"cid\", client_secret = \"cs_TESTONLY_root\" } }\n",
        ];
        for text in spellings {
            throws(refer(CODEX_TOML, text), said_line);
            throws(by_name(CODEX_TOML, text), said_line);
        }
        throws(refer(CODEX_TOML, "[mcp_servers.linear]\nurl = \"https://l.example\"\noauth = \"cs_TESTONLY_string\"\n"), &format!("mcp_servers.linear {SHAPE}"));
        let client_only = "[mcp_servers.linear]\nurl = \"https://l.example\"\noauth = { client_id = \"cid\", callback_port = 5555 }\n";
        assert_eq!(by_name(CODEX_TOML, client_only).unwrap().text, client_only);
        let beside = format!("{}\n[mcp_servers.other]\ncommand = \"o\"\nenv = {{ K = \"k_TESTONLY\" }}\n", spellings[0]);
        let one = refer_only(CODEX_TOML, &beside, "other").unwrap();
        assert_eq!(one.servers, vec![server("other", &[("K", "k_TESTONLY")])]);
    }

    #[test]
    fn the_copy_s_last_check_refuses_a_server_definition_that_still_carries_an_oauth_cl() {
        let dirty = json!({ "url": "https://s.example", "oauth": { "client_id": "cid", "client_secret": "cs_TESTONLY" } });
        throws(lax(CODEX_TOML, vec![dirty], &Known::new()), "a server still carries an OAuth secret in oauth.client_secret after it was written by name, so the file stays on this computer");
        let clean = json!({ "url": "https://s.example", "oauth": { "client_id": "cid" } });
        assert!(lax(CODEX_TOML, vec![clean], &Known::new()).is_ok());
    }

    #[test]
    fn copies_an_ordinary_config_whose_unrelated_keys_and_history_hold_the_same_short_w() {
        let text = js_pretty(json!({
            "verbose": true,
            "numStartups": 1,
            "theme": "dev",
            "history": ["deploy to production", "run dev with debug on", "true"],
            "projects": { "/Users/dev/app": { "lastSessionId": "1", "allowedTools": ["debug"] } },
            "mcpServers": { "notion": { "command": "npx", "env": { "DEBUG": "1", "STAGE": "dev", "NODE_ENV": "production", "FLAG": "true" } } }
        }));
        let out = refer(MCP_SERVERS_JSON, &text).unwrap();
        assert_eq!(out.servers, vec![server("notion", &[("DEBUG", "1"), ("STAGE", "dev"), ("NODE_ENV", "production"), ("FLAG", "true")])]);
        assert!(out.text.contains("\"history\": ["));
        let codex = "# production box, dev notes\nmodel = \"gpt-5\"\n\n[mcp_servers.n]\ncommand = \"npx\"\nenv = { NODE_ENV = \"production\", DEBUG = \"1\" }\n";
        assert_eq!(refer(CODEX_TOML, codex).unwrap().servers[0].values, values(&[("NODE_ENV", "production"), ("DEBUG", "1")]));
    }

    #[test]
    fn refuses_the_copy_where_a_value_still_stands_in_a_server_s_entry_whole_or_after_i() {
        let line = "s's value still stands in the file after it was written by name, so the file stays on this computer";
        throws(refer(CODEX_TOML, "[mcp_servers.s]\nurl = \"https://s.example\"\nnote = \"Bearer tok1\"\nhttp_headers = { Authorization = \"Bearer tok1\" }\n"), line);
        throws(refer(CODEX_TOML, "[mcp_servers.s]\ncommand = \"x\"\nargs = [\"tok1\"]\nenv = { K = \"tok1\" }\n"), line);
        throws(refer(MCP_SERVERS_JSON, &js(json!({ "mcpServers": { "s": { "command": "x", "args": ["--auth", "token sk_TESTONLY_long_value"], "env": { "K": "sk_TESTONLY_long_value" } } } }))), line);
        throws(refer(MCP_SERVERS_JSON, &js(json!({ "mcpServers": { "s": { "url": "https://s.example", "headers": { "Authorization": "Bearer t1" } }, "o": { "command": "x", "args": ["Basic t1"] } } }))), "s's value still stands");
        // A header whose whole value keeps its scheme word, and the credential standing bare elsewhere.
        throws(refer(MCP_SERVERS_JSON, &js(json!({ "mcpServers": { "s": { "url": "https://s.example", "headers": { "X-Auth": "Basic cred123" } }, "o": { "command": "x", "args": ["cred123"] } } }))), "s's value still stands");
    }

    #[derive(Default)]
    struct Leak {
        args: Option<Vec<String>>,
        url: Option<String>,
    }

    fn leak_file(format: McpFormat, l: &Leak) -> String {
        let v = "sk_TESTONLY_arg";
        let bearer = json!({ "Authorization": format!("Bearer {v}") });
        let args = l.args.clone().unwrap_or_default();
        match format {
            McpFormat::McpServersJson => js(json!({ "mcpServers": { "s": match &l.url {
                Some(url) => json!({ "type": "http", "url": url, "headers": bearer }),
                None => json!({ "command": "x", "args": args, "env": { "TOKEN": v } }),
            } } })),
            McpFormat::GeminiSettingsJson => js(json!({ "mcpServers": { "s": match &l.url {
                Some(url) => json!({ "httpUrl": url, "headers": bearer }),
                None => json!({ "command": "x", "args": args, "env": { "TOKEN": v } }),
            } } })),
            McpFormat::OpencodeJson => {
                let mut command = vec!["x".to_string()];
                command.extend(args);
                js(json!({ "mcp": { "s": match &l.url {
                    Some(url) => json!({ "type": "remote", "url": url, "headers": bearer }),
                    None => json!({ "type": "local", "command": command, "environment": { "TOKEN": v } }),
                } } }))
            }
            McpFormat::CodexToml => match &l.url {
                Some(url) => format!("[mcp_servers.s]\nurl = {}\nhttp_headers = {{ Authorization = \"Bearer {v}\" }}\n", serde_json::to_string(url).unwrap()),
                None => format!("[mcp_servers.s]\ncommand = \"x\"\nargs = {}\nenv = {{ TOKEN = \"{v}\" }}\n", serde_json::to_string(&args).unwrap()),
            },
        }
    }

    #[test]
    fn refuses_the_copy_where_a_value_still_stands_in_an_argument_or_an_address_as_a_fl() {
        let v = "sk_TESTONLY_arg";
        let b64 = btoa(v);
        let leaks: Vec<(Leak, &str)> = vec![
            (Leak { args: Some(vec![format!("--token={v}")]), url: None }, "the --token argument"),
            (Leak { args: Some(vec!["-H".into(), format!("Authorization: Bearer {v}")]), url: None }, "the Authorization header"),
            (Leak { args: None, url: Some(format!("https://s.example/mcp?team=eng&key={v}")) }, "the key parameter of an address"),
            (Leak { args: Some(vec!["https://s.example/sse".into(), "--header".into(), format!("Authorization: Basic {b64}")]), url: None }, "the Authorization header, as base64"),
        ];
        let agents = [("Claude Code", MCP_SERVERS_JSON), ("Gemini CLI", GEMINI_SETTINGS_JSON), ("OpenCode", OPENCODE_JSON), ("Codex", CODEX_TOML)];
        for (agent, format) in agents {
            for (leak, at) in &leaks {
                let r = refer(format, &leak_file(format, leak));
                match r {
                    Ok(_) => panic!("{agent}: {at}: expected a refusal"),
                    Err(e) => assert!(e.message.contains(&format!("s's value still stands in {at} after it was written by name, so the file stays on this computer")), "{agent}: {at}: {}", e.message),
                }
            }
        }
        // The same flags, keys and header lines carrying no value that left stand.
        let text = js(json!({ "mcpServers": { "s": { "command": "x", "args": ["--token=other", "-H", "Accept: text/plain", "https://s.example/?key=other"], "env": { "TOKEN": v } } } }));
        assert_eq!(refer(MCP_SERVERS_JSON, &text).unwrap().servers, vec![server("s", &[("TOKEN", v)])]);
    }

    #[test]
    fn two_headers_of_one_server_that_would_travel_under_one_name() {
        let text = js(json!({ "mcpServers": { "s": { "url": "https://s.example", "headers": { "X-Api-Key": "a", "X_Api_Key": "b" } } } }));
        let line = "s sends X-Api-Key and X_Api_Key, which would both travel as WSP_MCP_S_X_API_KEY; rename one";
        throws(refer(MCP_SERVERS_JSON, &text), line);
        throws(refer(CODEX_TOML, "[mcp_servers.s]\nurl = \"https://s.example\"\nhttp_headers = { \"X-Api-Key\" = \"a\", \"X_Api_Key\" = \"b\" }\n"), line);
    }

    #[test]
    fn a_value_that_mixes_a_literal_with_a_variable_which_cannot_travel_whole_by_name() {
        throws(refer(MCP_SERVERS_JSON, &js(json!({ "mcpServers": { "s": { "url": "https://s.example", "headers": { "Authorization": "Bearer lit_${X}" } } } }))), "s's Authorization mixes a value with a variable, so it cannot travel by name; make it one or the other");
        throws(refer(GEMINI_SETTINGS_JSON, &js(json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "ab$cd" } } } }))), "s's K mixes a value with a variable, so it cannot travel by name; make it one or the other");
        // A whole reference to a name servers.env holds, a bearer's included, hands nothing to the vault.
        let text = js(json!({ "mcpServers": { "s": { "url": "https://s.example", "headers": { "Authorization": "Bearer ${X}", "Y": "${Y:-d}" } } } }));
        let out = MCP_SERVERS_JSON.refer(&text, None, &[], &set(&["X", "Y"])).unwrap();
        assert_eq!(out.servers, vec![server("s", &[])]);
    }
}

mod an_mcp_server_s_oauth_secrets {
    use super::*;

    const CS: &str = "cs_TESTONLY_oauth";

    #[test]
    fn gemini_cli_writes_name_for_oauth_clientsecret_which_it_expands_and_hands_the_val() {
        let text = js_pretty(json!({ "mcpServers": { "linear": { "httpUrl": "https://l.example/mcp", "oauth": { "enabled": true, "clientId": "cid", "clientSecret": CS } } } }));
        let out = refer(GEMINI_SETTINGS_JSON, &text).unwrap();
        let root = parse_jsonc(&out.text).unwrap();
        assert_eq!(root["mcpServers"]["linear"]["oauth"], json!({ "enabled": true, "clientId": "cid", "clientSecret": "${WSP_MCP_LINEAR_OAUTH_CLIENTSECRET}" }));
        assert_eq!(out.servers, vec![server("linear", &[("WSP_MCP_LINEAR_OAUTH_CLIENTSECRET", CS)])]);
        let copied = by_name(GEMINI_SETTINGS_JSON, &text).unwrap();
        assert!(!copied.text.contains(CS));
        assert!(copied.text.contains("${WSP_MCP_LINEAR_OAUTH_CLIENTSECRET}"));
    }

    #[test]
    fn opencode_writes_env_name_for_oauth_clientsecret_and_for_the_snake_case_client_se() {
        for key in ["clientSecret", "client_secret"] {
            let n = mcp_header_variable("linear", &format!("oauth.{key}"));
            let text = js_pretty(json!({ "mcp": { "linear": { "type": "remote", "url": "https://l.example/mcp", "oauth": obj(&[("clientId", json!("cid")), (key, json!(CS))]) } } }));
            let out = refer(OPENCODE_JSON, &text).unwrap();
            let root = parse_jsonc(&out.text).unwrap();
            assert_eq!(root["mcp"]["linear"]["oauth"], obj(&[("clientId", json!("cid")), (key, json!(envref(&n)))]), "{key}");
            assert_eq!(out.servers, vec![server("linear", &[(n.as_str(), CS)])], "{key}");
            assert!(!by_name(OPENCODE_JSON, &text).unwrap().text.contains(CS), "{key}");
        }
        assert_eq!(mcp_header_variable("linear", "oauth.client_secret"), "WSP_MCP_LINEAR_OAUTH_CLIENT_SECRET");
        let off = js(json!({ "mcp": { "linear": { "type": "remote", "url": "https://l.example/mcp", "oauth": false } } }));
        assert_eq!(by_name(OPENCODE_JSON, &off).unwrap().text, off);
    }

    #[test]
    fn refuses_an_oauth_secret_the_agent_reads_no_variable_for_naming_the_key_so_the_fi() {
        let cases: Vec<(McpFormat, String, String)> = vec![
            (MCP_SERVERS_JSON, js(json!({ "mcpServers": { "linear": { "type": "http", "url": "https://l.example", "oauth": { "clientId": "cid", "clientSecret": CS } } } })), "mcpServers.linear.oauth.clientSecret".to_string()),
            (MCP_SERVERS_JSON, js(json!({ "projects": { "/Users/dev": { "mcpServers": { "linear": { "type": "http", "url": "https://l.example", "oauth": { "client_secret": CS } } } } } })), format!("projects.{HOME}.mcpServers.linear.oauth.client_secret")),
            (GEMINI_SETTINGS_JSON, js(json!({ "mcpServers": { "linear": { "httpUrl": "https://l.example", "oauth": { "clientSecret": "${OK}", "refreshToken": CS } } } })), "mcpServers.linear.oauth.refreshToken".to_string()),
            (GEMINI_SETTINGS_JSON, js(json!({ "mcpServers": { "linear": { "httpUrl": "https://l.example", "oauth": { "client_secret": CS } } } })), "mcpServers.linear.oauth.client_secret".to_string()),
            (OPENCODE_JSON, js(json!({ "mcp": { "linear": { "type": "remote", "url": "https://l.example", "oauth": { "access_token": CS } } } })), "mcp.linear.oauth.access_token".to_string()),
        ];
        for (format, text, at) in &cases {
            let line = format!("linear keeps an OAuth secret in {at}, and its agent reads no variable there, so the file stays on this computer");
            throws(refer(*format, text), &line);
            throws(by_name(*format, text), &line);
        }
        throws(refer(GEMINI_SETTINGS_JSON, &js(json!({ "mcpServers": { "linear": { "httpUrl": "https://l.example", "oauth": { "clientSecret": 7 } } } }))), "linear keeps an OAuth secret in mcpServers.linear.oauth.clientSecret");
        throws(refer(GEMINI_SETTINGS_JSON, &js(json!({ "mcpServers": { "linear": { "httpUrl": "https://l.example", "oauth": CS } } }))), "mcpServers.linear is written in a shape wsp does not read, so its values cannot be written by name");
        throws(refer(GEMINI_SETTINGS_JSON, &js(json!({ "mcpServers": { "linear": { "httpUrl": "https://l.example", "oauth": { "clientSecret": "x_${Y}" } } } }))), "linear's oauth.clientSecret mixes a value with a variable");
        throws(refer(CODEX_TOML, &format!("[mcp_servers.linear]\nurl = \"https://l.example\"\noauth = {{ refresh_token = \"{CS}\" }}\n")), "linear keeps an OAuth secret in mcp_servers.linear.oauth.refresh_token, and Codex reads no variable there, so the file stays on this computer");
    }

    #[test]
    fn leaves_a_secret_that_already_names_a_variable_as_written_and_another_server_s_se() {
        let named = js(json!({ "mcpServers": { "linear": { "type": "http", "url": "https://l.example", "oauth": { "clientSecret": "${LINEAR_SECRET}" } } } }));
        assert_eq!(by_name_known(MCP_SERVERS_JSON, &named, &known(&[("LINEAR_SECRET", CS)], &["linear"])).unwrap().text, named);
        let beside = js(json!({ "mcpServers": { "linear": { "httpUrl": "https://l.example", "oauth": { "refreshToken": CS } }, "other": { "command": "o", "env": { "K": "k_TESTONLY" } } } }));
        assert_eq!(refer_only(GEMINI_SETTINGS_JSON, &beside, "other").unwrap().servers, vec![server("other", &[("K", "k_TESTONLY")])]);
    }

    #[test]
    fn the_copy_s_last_check_reads_every_spelling_of_an_oauth_secret_at_any_depth_of_a() {
        let spellings: Vec<(Value, &str)> = vec![
            (json!({ "oauth": { "clientSecret": CS } }), "oauth.clientSecret"),
            (json!({ "oauth": { "client_secret": CS } }), "oauth.client_secret"),
            (json!({ "oauth": { "Client-Secret": CS } }), "oauth.Client-Secret"),
            (json!({ "OAuth": { "CLIENTSECRET": CS } }), "OAuth.CLIENTSECRET"),
            (json!({ "oauth": { "refresh_token": CS } }), "oauth.refresh_token"),
            (json!({ "oauth": { "refreshToken": CS } }), "oauth.refreshToken"),
            (json!({ "oauth": { "accessToken": CS } }), "oauth.accessToken"),
            (json!({ "oauth": { "id_token": CS } }), "oauth.id_token"),
            (json!({ "servers": { "x": { "oauth": { "client_secret": CS } } } }), "servers.x.oauth.client_secret"),
            (json!({ "list": [{ "oauth": { "clientSecret": 7 } }] }), "list.0.oauth.clientSecret"),
        ];
        for format in [MCP_SERVERS_JSON, GEMINI_SETTINGS_JSON, OPENCODE_JSON, CODEX_TOML] {
            for (entry, at) in &spellings {
                throws(lax(format, vec![entry.clone()], &Known::new()), &format!("a server still carries an OAuth secret in {at} after it was written by name, so the file stays on this computer"));
            }
            assert!(lax(format, vec![json!({ "oauth": { "clientId": "cid", "clientSecret": "" } })], &Known::new()).is_ok());
        }
        let x = known(&[("X", CS)], &["s"]);
        assert!(lax(GEMINI_SETTINGS_JSON, vec![json!({ "oauth": { "clientSecret": "${X}" } })], &x).is_ok());
        assert!(lax(OPENCODE_JSON, vec![json!({ "oauth": { "client_secret": "{env:X}" } })], &x).is_ok());
        throws(lax(OPENCODE_JSON, vec![json!({ "oauth": { "clientSecret": "${X}" } })], &Known::new()), "oauth.clientSecret");
        throws(lax(CODEX_TOML, vec![json!({ "oauth": { "client_secret": "${X}" } })], &Known::new()), "oauth.client_secret");
    }

    #[test]
    fn opencode_a_secret_under_the_mcp_servers_envelope_it_also_reads_keeps_the_file_on() {
        for oauth in [json!({ "client_secret": CS }), json!([{ "client_secret": CS }])] {
            let text = js(json!({ "mcp": { "servers": { "linear": { "type": "remote", "url": "https://l.example", "oauth": oauth } } } }));
            throws(by_name(OPENCODE_JSON, &text), "the file keeps servers under mcp.servers, whose values wsp does not write by name, so the file stays on this computer");
        }
    }

    #[test]
    fn no_agent_row_copies_the_file_gemini_cli_or_opencode_keeps_its_mcp_sign_in_tokens() {
        let stores = [("gemini", "~/.gemini/gemini-credentials.json"), ("opencode", "~/.local/share/opencode/mcp-auth.json")];
        for (id, store) in stores {
            let agents = catalog_agents();
            let a = agents.iter().find(|x| x.id == id).unwrap();
            let mut paths: Vec<String> = a.config_paths.clone();
            if let Some(m) = &a.mcp {
                paths.extend(m.files.clone());
            }
            for p in paths {
                let covered = store == p || store.starts_with(&format!("{}/", p.trim_end_matches('/')));
                assert!(!covered, "{id}: {p}");
            }
        }
    }
}

mod a_definition_s_references_read_back_as_the_agent_starts_the_server {
    use super::*;

    fn value(n: &str) -> Option<String> {
        match n {
            "LINEAR" => Some(TOKEN.to_string()),
            "NOTION_TOKEN" => Some(NOTION.to_string()),
            _ => None,
        }
    }

    fn one(format: McpFormat, text: &str) -> McpResolved {
        format.resolve(&first_server(format, text), &value)
    }

    fn resolved(transport: McpTransport, vals: &[&str]) -> McpResolved {
        McpResolved::Resolved { transport, values: vals.iter().map(|v| v.to_string()).collect() }
    }

    fn stdio(command: &str, args: &[&str], env: &[(&str, &str)]) -> McpTransport {
        McpTransport::Stdio { command: command.to_string(), args: args.iter().map(|a| a.to_string()).collect(), env: values(env), cwd: None, tool_timeout_sec: None }
    }

    fn http(url: &str, headers: &[(&str, &str)]) -> McpTransport {
        McpTransport::Http { url: url.to_string(), headers: values(headers) }
    }

    #[test]
    fn fills_each_format_s_own_syntax_in_headers_command_variables_and_the_address_and() {
        assert_eq!(
            one(MCP_SERVERS_JSON, &js(json!({ "mcpServers": { "l": { "type": "http", "url": "https://l.example/${LINEAR}", "headers": { "Authorization": "Bearer ${LINEAR}" } } } }))),
            resolved(http(&format!("https://l.example/{TOKEN}"), &[("Authorization", &format!("Bearer {TOKEN}"))]), &[TOKEN, TOKEN])
        );
        assert_eq!(
            one(MCP_SERVERS_JSON, &js(json!({ "mcpServers": { "n": { "command": "npx", "args": ["--t", "${NOTION_TOKEN}"], "env": { "NOTION_TOKEN": "${NOTION_TOKEN}" } } } }))),
            resolved(stdio("npx", &["--t", "${NOTION_TOKEN}"], &[("NOTION_TOKEN", NOTION)]), &[NOTION])
        );
        match one(GEMINI_SETTINGS_JSON, &js(json!({ "mcpServers": { "n": { "command": "npx", "env": { "A": "$NOTION_TOKEN", "B": "${LINEAR}" } } } }))) {
            McpResolved::Resolved { transport: McpTransport::Stdio { env, .. }, .. } => assert_eq!(env, values(&[("A", NOTION), ("B", TOKEN)])),
            other => panic!("{other:?}"),
        }
        match one(OPENCODE_JSON, &js(json!({ "mcp": { "l": { "type": "remote", "url": "https://l.example", "headers": { "Authorization": "Bearer {env:LINEAR}" } } } }))) {
            McpResolved::Resolved { transport: McpTransport::Http { headers, .. }, .. } => assert_eq!(headers, values(&[("Authorization", &format!("Bearer {TOKEN}"))])),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn takes_a_written_default_for_a_variable_with_no_value_and_names_the_first_one_wit() {
        assert_eq!(one(MCP_SERVERS_JSON, &js(json!({ "mcpServers": { "n": { "command": "npx", "env": { "A": "${UNSET:-dflt}" } } } }))), resolved(stdio("npx", &[], &[("A", "dflt")]), &[]));
        assert_eq!(one(MCP_SERVERS_JSON, &js(json!({ "mcpServers": { "n": { "command": "npx", "env": { "A": "${UNSET}", "B": "${ALSO_UNSET}" } } } }))), McpResolved::Missing("UNSET".to_string()));
        assert_eq!(one(OPENCODE_JSON, &js(json!({ "mcp": { "l": { "type": "remote", "url": "https://l.example", "headers": { "K": "{env:UNSET}" } } } }))), McpResolved::Missing("UNSET".to_string()));
    }

    #[test]
    fn gives_a_codex_command_its_env_vars_that_have_a_value_and_sends_env_http_headers() {
        assert_eq!(one(CODEX_TOML, "[mcp_servers.n]\ncommand = \"npx\"\nenv_vars = [\"NOTION_TOKEN\", \"UNSET\"]\n"), resolved(stdio("npx", &[], &[("NOTION_TOKEN", NOTION)]), &[NOTION]));
        assert_eq!(
            one(CODEX_TOML, "[mcp_servers.l]\nurl = \"https://l.example\"\nbearer_token_env_var = \"LINEAR\"\nenv_http_headers = { \"X-Notion\" = \"NOTION_TOKEN\" }\n"),
            resolved(http("https://l.example", &[("X-Notion", NOTION), ("Authorization", &format!("Bearer {TOKEN}"))]), &[NOTION, TOKEN])
        );
        assert_eq!(one(CODEX_TOML, "[mcp_servers.l]\nurl = \"https://l.example\"\nbearer_token_env_var = \"UNSET\"\n"), McpResolved::Missing("UNSET".to_string()));
        assert_eq!(one(CODEX_TOML, "[mcp_servers.l]\nurl = \"https://l.example\"\n\n[mcp_servers.l.env_http_headers]\nX-Notion = \"UNSET\"\n"), McpResolved::Missing("UNSET".to_string()));
    }
}

mod a_value_servers_env_holds_written_back_as_its_reference_on_the_copy {
    use super::*;

    const V: &str = "sk_TESTONLY_local";

    fn vault() -> Known {
        known(&[("ACME_TOKEN", V)], &[])
    }

    fn address() -> String {
        format!("https://s.example/mcp?team=eng&key={V}")
    }

    #[derive(Default)]
    struct Local {
        args: Option<Vec<String>>,
        env: Option<Vec<(&'static str, String)>>,
        url: Option<String>,
    }

    fn with_args(args: &[String]) -> Local {
        Local { args: Some(args.to_vec()), ..Local::default() }
    }

    fn with_env(args: &[String], env: &[(&'static str, &str)]) -> Local {
        Local { args: Some(args.to_vec()), env: Some(env.iter().map(|(k, v)| (*k, v.to_string())).collect()), url: None }
    }

    fn with_url(url: &str) -> Local {
        Local { url: Some(url.to_string()), ..Local::default() }
    }

    fn file(format: McpFormat, l: &Local) -> String {
        let args = l.args.clone().unwrap_or_default();
        let env: Option<Value> = l.env.as_ref().map(|e| obj(&e.iter().map(|(k, v)| (*k, json!(v))).collect::<Vec<_>>()));
        let with = |mut base: Map<String, Value>, key: &str| {
            if let Some(e) = &env {
                base.insert(key.to_string(), e.clone());
            }
            Value::Object(base)
        };
        let m = |v: Value| v.as_object().unwrap().clone();
        match format {
            McpFormat::McpServersJson => js(json!({ "mcpServers": { "s": match &l.url {
                Some(url) => json!({ "type": "http", "url": url }),
                None => with(m(json!({ "command": "x", "args": args })), "env"),
            } } })),
            McpFormat::GeminiSettingsJson => js(json!({ "mcpServers": { "s": match &l.url {
                Some(url) => json!({ "httpUrl": url }),
                None => with(m(json!({ "command": "x", "args": args })), "env"),
            } } })),
            McpFormat::OpencodeJson => {
                let mut command = vec!["x".to_string()];
                command.extend(args);
                js(json!({ "mcp": { "s": match &l.url {
                    Some(url) => json!({ "type": "remote", "url": url }),
                    None => with(m(json!({ "type": "local", "command": command })), "environment"),
                } } }))
            }
            McpFormat::CodexToml => unreachable!(),
        }
    }

    fn reference(format: McpFormat) -> String {
        if format == OPENCODE_JSON {
            "{env:ACME_TOKEN}".to_string()
        } else {
            "${ACME_TOKEN}".to_string()
        }
    }

    fn token_arg() -> Vec<String> {
        vec![format!("--token={V}")]
    }

    fn travels_as_reference(format: McpFormat) {
        let vault = vault();
        for local in [with_args(&token_arg()), with_env(&token_arg(), &[("ACME_TOKEN", V)]), with_url(&address())] {
            let mut held = Held::new();
            let text = file(format, &local);
            let out = servers_by_name(format, &text, &mut held, &no_rows, &vault).unwrap();
            assert!(!out.text.contains(V), "{text}");
            let want = match &local.url {
                Some(_) => format!("key={}", reference(format)),
                None => format!("--token={}", reference(format)),
            };
            assert!(out.text.contains(&want), "{}", out.text);
            assert_eq!(out.dropped, vec![]);
            assert_eq!(held_list(&held), vec![("ACME_TOKEN".to_string(), held_value(V, "s"))]);
            let read = first_server(format, &out.text);
            if local.url.is_some() {
                match format.resolve(&read, &|n| vault.get(n).map(|k| k.value.clone())) {
                    McpResolved::Resolved { transport: McpTransport::Http { url, .. }, .. } => assert_eq!(url, address()),
                    other => panic!("{other:?}"),
                }
            }
        }
    }

    fn still_refuses(format: McpFormat) {
        let line = "s's value still stands in the --token argument after it was written by name, so the file stays on this computer";
        let text = file(format, &with_env(&token_arg(), &[("ACME_TOKEN", V)]));
        throws(servers_by_name(format, &text, &mut Held::new(), &no_rows, &Known::new()), line);
        throws(servers_by_name(format, &text, &mut Held::new(), &no_rows, &known(&[("OTHER", "sk_TESTONLY_other")], &[])), "s's value still stands in the --token argument");
        let plain_args = vec!["--workers=10".to_string(), "run1".to_string()];
        let plain = servers_by_name(format, &file(format, &with_args(&plain_args)), &mut Held::new(), &no_rows, &known(&[("N", "1")], &[])).unwrap();
        assert_eq!(plain.text, file(format, &with_args(&plain_args)));
    }

    fn refuses_other_value(format: McpFormat) {
        let text = file(format, &with_env(&token_arg(), &[("ACME_TOKEN", "sk_TESTONLY_other")]));
        throws(servers_by_name(format, &text, &mut Held::new(), &no_rows, &vault()), "s passes the value servers.env holds as ACME_TOKEN in the --token argument, which s sets to another value, so the file stays on this computer");
    }

    fn final_guard(format: McpFormat) {
        let args = vec![format!("--token={V}"), "--header".to_string(), format!("Authorization: Basic {}", btoa(V))];
        throws(servers_by_name(format, &file(format, &with_args(&args)), &mut Held::new(), &no_rows, &vault()), "s's value still stands in the Authorization header, as base64 after it was written by name");
    }

    #[test]
    fn claude_code_a_value_an_argument_or_the_address_holds_as_servers_env_does_travels() {
        travels_as_reference(MCP_SERVERS_JSON);
    }

    #[test]
    fn claude_code_a_value_servers_env_does_not_hold_still_refuses_the_copy_where_it_st() {
        still_refuses(MCP_SERVERS_JSON);
    }

    #[test]
    fn claude_code_refuses_where_the_file_sets_the_variable_servers_env_holds_the_value() {
        refuses_other_value(MCP_SERVERS_JSON);
    }

    #[test]
    fn claude_code_the_final_guard_still_runs_after_the_rewrite_so_a_value_left_standin() {
        final_guard(MCP_SERVERS_JSON);
    }

    #[test]
    fn gemini_cli_a_value_an_argument_or_the_address_holds_as_servers_env_does_travels() {
        travels_as_reference(GEMINI_SETTINGS_JSON);
    }

    #[test]
    fn gemini_cli_a_value_servers_env_does_not_hold_still_refuses_the_copy_where_it_sta() {
        still_refuses(GEMINI_SETTINGS_JSON);
    }

    #[test]
    fn gemini_cli_refuses_where_the_file_sets_the_variable_servers_env_holds_the_value() {
        refuses_other_value(GEMINI_SETTINGS_JSON);
    }

    #[test]
    fn gemini_cli_the_final_guard_still_runs_after_the_rewrite_so_a_value_left_standing() {
        final_guard(GEMINI_SETTINGS_JSON);
    }

    #[test]
    fn opencode_a_value_an_argument_or_the_address_holds_as_servers_env_does_travels_as() {
        travels_as_reference(OPENCODE_JSON);
    }

    #[test]
    fn opencode_a_value_servers_env_does_not_hold_still_refuses_the_copy_where_it_stand() {
        still_refuses(OPENCODE_JSON);
    }

    #[test]
    fn opencode_refuses_where_the_file_sets_the_variable_servers_env_holds_the_value_un() {
        refuses_other_value(OPENCODE_JSON);
    }

    #[test]
    fn opencode_the_final_guard_still_runs_after_the_rewrite_so_a_value_left_standing_a() {
        final_guard(OPENCODE_JSON);
    }

    #[test]
    fn writes_the_outer_place_where_two_known_values_nest_so_a_header_line_s_whole_cred() {
        let text = js(json!({ "mcpServers": { "s": { "command": "x", "args": ["-H", "Authorization: Basic cred_TESTONLY"] } } }));
        let out = by_name_known(MCP_SERVERS_JSON, &text, &known(&[("WHOLE", "Basic cred_TESTONLY"), ("PART", "cred_TESTONLY")], &[])).unwrap();
        let root = parse_jsonc(&out.text).unwrap();
        assert_eq!(root["mcpServers"]["s"]["args"], json!(["-H", "Authorization: ${WHOLE}"]));
    }

    #[test]
    fn refuses_a_value_servers_env_holds_that_stands_only_percent_encoded_or_as_base64() {
        let odd = "sk TESTONLY/plus+";
        let encoded = format!("https://s.example/mcp?key={}", encode_uri_component(odd));
        let line = "s's value still stands in the key parameter of an address after it was written by name, so the file stays on this computer";
        let agents = [MCP_SERVERS_JSON, GEMINI_SETTINGS_JSON, OPENCODE_JSON];
        for format in agents {
            throws(by_name_known(format, &file(format, &with_url(&encoded)), &known(&[("ODD", odd)], &[])), line);
        }
        throws(by_name_known(CODEX_TOML, &format!("[mcp_servers.s]\nurl = \"{encoded}\"\n"), &known(&[("ODD", odd)], &[])), line);
        for format in agents {
            throws(by_name_known(format, &file(format, &with_args(&["--auth".to_string(), btoa(V)])), &vault()), "s's value still stands in the file, as base64");
        }
        throws(by_name_known(CODEX_TOML, &format!("[mcp_servers.s]\ncommand = \"x\"\nargs = [\"{}\"]\n", btoa(V)), &vault()), "s's value still stands in the file, as base64");
    }

    #[test]
    fn writes_the_name_the_server_itself_sets_where_servers_env_holds_its_value_under_t() {
        let text = js(json!({ "mcpServers": { "s": { "command": "x", "args": [format!("--token={V}")], "env": { "MINE": V } } } }));
        let out = by_name_known(MCP_SERVERS_JSON, &text, &known(&[("OTHER", V), ("MINE", V)], &[])).unwrap();
        let root = parse_jsonc(&out.text).unwrap();
        assert_eq!(root["mcpServers"]["s"]["args"], json!(["--token=${MINE}"]));
    }

    #[test]
    fn refuses_a_name_servers_env_holds_with_another_value_for_another_server_or_for_no() {
        let file = |value: &str| js(json!({ "mcpServers": { "s": { "command": "x", "env": { "TOKEN": value } } } }));
        let other = by_name_known(MCP_SERVERS_JSON, &file("sk_TESTONLY_second"), &known(&[("TOKEN", "sk_TESTONLY_first")], &["a"])).unwrap();
        assert_eq!(other.dropped, vec![dropped("s", "sets TOKEN, which a already sets to another value")]);
        assert!(!other.text.contains("sk_TESTONLY_second"));
        let nobody = by_name_known(MCP_SERVERS_JSON, &file("sk_TESTONLY_second"), &known(&[("TOKEN", "sk_TESTONLY_first")], &[])).unwrap();
        assert_eq!(nobody.dropped, vec![dropped("s", "sets TOKEN, which servers.env already holds with another value and no server recorded for it")]);
        let shared = by_name_known(MCP_SERVERS_JSON, &file("sk_TESTONLY_second"), &known(&[("TOKEN", "sk_TESTONLY_first")], &["s", "a"])).unwrap();
        assert_eq!(shared.dropped, vec![dropped("s", "sets TOKEN, which a already sets to another value")]);
        let mut held = Held::new();
        let rotated = by_name_held(MCP_SERVERS_JSON, &file("sk_TESTONLY_second"), &mut held, &known(&[("TOKEN", "sk_TESTONLY_first")], &["s"])).unwrap();
        assert_eq!(rotated.dropped, vec![]);
        assert_eq!(held_list(&held), vec![("TOKEN".to_string(), held_value("sk_TESTONLY_second", "s"))]);
    }

    #[test]
    fn codex_reads_no_variable_in_an_argument_or_an_address_so_the_copy_refuses_and_nam() {
        let line = |at: &str| format!("s passes the value of ACME_TOKEN in {at}, and Codex reads no variable there, so the file stays on this computer");
        let vault = vault();
        throws(by_name_known(CODEX_TOML, &format!("[mcp_servers.s]\ncommand = \"x\"\nargs = [\"--token={V}\"]\n"), &vault), &line("the --token argument"));
        throws(by_name_known(CODEX_TOML, &format!("[mcp_servers.s]\ncommand = \"x\"\nargs = [\"--token={V}\"]\nenv = {{ ACME_TOKEN = \"{V}\" }}\n"), &vault), &line("the --token argument"));
        throws(by_name_known(CODEX_TOML, &format!("[mcp_servers.s]\nurl = \"{}\"\n", address()), &vault), &line("the key parameter of an address"));
        throws(by_name_known(CODEX_TOML, &format!("[mcp_servers.s]\ncommand = \"x\"\nargs = [\"{V}\"]\n"), &vault), &line("an argument"));
    }
}

mod a_literal_that_reads_like_a_reference {
    use super::*;

    const HELD: &str = "sk_TESTONLY_held";
    const D: &str = "sk_TESTONLY_default";

    #[derive(Clone, Copy, PartialEq)]
    enum Where {
        Header,
        Env,
        Oauth,
    }

    // What a value comes to: moved to servers.env as the string, the server dropped, or the file refused.
    enum Outcome {
        Moved(String),
        Dropped,
        Refused,
        Mixed,
    }

    fn vault() -> Known {
        known(&[("ACME_TOKEN", HELD)], &["s"])
    }

    fn file(format: McpFormat, w: Where, v: &str) -> String {
        let remote = |base: Value| -> Value {
            let mut m = base.as_object().unwrap().clone();
            match w {
                Where::Header => m.insert("headers".to_string(), json!({ "Authorization": v })),
                _ => m.insert("oauth".to_string(), json!({ "clientSecret": v })),
            };
            Value::Object(m)
        };
        match format {
            McpFormat::McpServersJson => js(json!({ "mcpServers": { "s": if w == Where::Env { json!({ "command": "x", "env": { "K": v } }) } else { remote(json!({ "type": "http", "url": "https://s.example" })) } } })),
            McpFormat::GeminiSettingsJson => js(json!({ "mcpServers": { "s": if w == Where::Env { json!({ "command": "x", "env": { "K": v } }) } else { remote(json!({ "httpUrl": "https://s.example" })) } } })),
            McpFormat::OpencodeJson => js(json!({ "mcp": { "s": if w == Where::Env { json!({ "type": "local", "command": ["x"], "environment": { "K": v } }) } else { remote(json!({ "type": "remote", "url": "https://s.example" })) } } })),
            McpFormat::CodexToml => unreachable!(),
        }
    }

    fn root_of(format: McpFormat) -> (&'static str, &'static str) {
        if format == OPENCODE_JSON {
            ("mcp", "environment")
        } else {
            ("mcpServers", "env")
        }
    }

    fn key_path(format: McpFormat, w: Where) -> String {
        let (root, env_key) = root_of(format);
        let tail = match w {
            Where::Header => "headers.Authorization".to_string(),
            Where::Env => format!("{env_key}.K"),
            Where::Oauth => "oauth.clientSecret".to_string(),
        };
        format!("{root}.s.{tail}")
    }

    fn reference(format: McpFormat) -> &'static str {
        match format {
            McpFormat::McpServersJson => "${ACME_TOKEN}",
            McpFormat::GeminiSettingsJson => "$ACME_TOKEN",
            _ => "{env:ACME_TOKEN}",
        }
    }

    fn cases(format: McpFormat) -> Vec<(String, Box<dyn Fn(Where) -> Outcome>)> {
        match format {
            McpFormat::McpServersJson => vec![
                ("$ecret123".to_string(), Box::new(|w| if w == Where::Oauth { Outcome::Refused } else { Outcome::Moved("$ecret123".to_string()) })),
                ("${notheld}".to_string(), Box::new(|_| Outcome::Dropped)),
                ("Bearer ${notheld}".to_string(), Box::new(|w| if w == Where::Oauth { Outcome::Mixed } else { Outcome::Dropped })),
                (format!("${{notheld:-{D}}}"), Box::new(|w| if w == Where::Oauth { Outcome::Refused } else { Outcome::Moved(D.to_string()) })),
                (
                    format!("Bearer ${{notheld:-{D}}}"),
                    Box::new(|w| match w {
                        Where::Oauth => Outcome::Mixed,
                        Where::Header => Outcome::Moved(D.to_string()),
                        Where::Env => Outcome::Moved(format!("Bearer {D}")),
                    }),
                ),
            ],
            McpFormat::GeminiSettingsJson => vec![
                ("$ecret123".to_string(), Box::new(|_| Outcome::Dropped)),
                ("${notheld}".to_string(), Box::new(|_| Outcome::Dropped)),
                (format!("${{notheld:-{D}}}"), Box::new(|_| Outcome::Moved(D.to_string()))),
            ],
            McpFormat::OpencodeJson => vec![
                ("$ecret123".to_string(), Box::new(|_| Outcome::Moved("$ecret123".to_string()))),
                ("${notheld}".to_string(), Box::new(|_| Outcome::Moved("${notheld}".to_string()))),
                ("{env:notheld}".to_string(), Box::new(|_| Outcome::Dropped)),
            ],
            McpFormat::CodexToml => unreachable!(),
        }
    }

    fn nobody_holds(format: McpFormat) {
        for w in [Where::Header, Where::Env, Where::Oauth] {
            for (v, outcome_of) in cases(format) {
                let text = file(format, w, &v);
                let label = format!("{} {v}", key_path(format, w));
                match outcome_of(w) {
                    Outcome::Mixed => throws(by_name_known(format, &text, &vault()), "s's oauth.clientSecret mixes a value with a variable, so it cannot travel by name"),
                    Outcome::Refused => throws(by_name_known(format, &text, &vault()), &format!("s keeps an OAuth secret in {}, and its agent reads no variable there, so the file stays on this computer", key_path(format, w))),
                    outcome => {
                        let mut held = Held::new();
                        let out = by_name_held(format, &text, &mut held, &vault()).unwrap();
                        assert!(!out.text.contains(&v), "{label}");
                        match outcome {
                            Outcome::Dropped => {
                                assert_eq!(out.dropped, vec![dropped("s", &unread_line(&key_path(format, w)))], "{label}");
                                assert_eq!(held.len(), 0, "{label}");
                            }
                            Outcome::Moved(moved) => {
                                assert_eq!(out.dropped, vec![], "{label}");
                                assert!(!out.text.contains(&moved), "{label}");
                                assert_eq!(held.values().map(|h| h.value.clone()).collect::<Vec<_>>(), vec![moved], "{label}");
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
    }

    fn held_stands(format: McpFormat) {
        for w in [Where::Header, Where::Env, Where::Oauth] {
            if w == Where::Oauth && format == MCP_SERVERS_JSON {
                continue;
            }
            let text = file(format, w, reference(format));
            let mut held = Held::new();
            assert_eq!(by_name_held(format, &text, &mut held, &vault()).unwrap().text, text);
            assert_eq!(held.len(), 0);
        }
    }

    #[test]
    fn claude_code_a_reference_to_a_name_nobody_holds_drops_its_server_its_default_trav() {
        nobody_holds(MCP_SERVERS_JSON);
    }

    #[test]
    fn claude_code_a_reference_to_a_name_servers_env_holds_stands_as_written() {
        held_stands(MCP_SERVERS_JSON);
    }

    #[test]
    fn gemini_cli_a_reference_to_a_name_nobody_holds_drops_its_server_its_default_trave() {
        nobody_holds(GEMINI_SETTINGS_JSON);
    }

    #[test]
    fn gemini_cli_a_reference_to_a_name_servers_env_holds_stands_as_written() {
        held_stands(GEMINI_SETTINGS_JSON);
    }

    #[test]
    fn opencode_a_reference_to_a_name_nobody_holds_drops_its_server_its_default_travels() {
        nobody_holds(OPENCODE_JSON);
    }

    #[test]
    fn opencode_a_reference_to_a_name_servers_env_holds_stands_as_written() {
        held_stands(OPENCODE_JSON);
    }

    #[test]
    fn a_variable_that_names_itself_which_servers_env_does_not_hold_drops_its_server_wi() {
        for format in [MCP_SERVERS_JSON, GEMINI_SETTINGS_JSON, OPENCODE_JSON] {
            let own = reference(format).replace("ACME_TOKEN", "K");
            let (root, env_key) = root_of(format);
            let out = by_name_known(format, &file(format, Where::Env, &own), &vault()).unwrap();
            assert_eq!(out.dropped, vec![dropped("s", &unread_line(&format!("{root}.s.{env_key}.K")))], "{own}");
            let text = file(format, Where::Env, &own);
            assert_eq!(by_name_known(format, &text, &known(&[("K", HELD)], &["s"])).unwrap().text, text, "{own}");
        }
        let mut held = Held::new();
        let text = js(json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "${K:-info}" } } } }));
        let out = by_name_held(MCP_SERVERS_JSON, &text, &mut held, &Known::new()).unwrap();
        assert_eq!(parse(&out.text), json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "${K}" } } } }));
        assert_eq!(held.get("K").map(|h| h.value.as_str()), Some("info"));
    }

    #[test]
    fn the_copy_s_last_check_lets_an_oauth_secret_through_only_as_a_reference_to_a_name() {
        let line = "a server still carries an OAuth secret in oauth.clientSecret after it was written by name, so the file stays on this computer";
        let entry = |secret: &str| vec![json!({ "oauth": { "clientSecret": secret } })];
        for (format, unheld, own_ref, vault_ref) in [
            (GEMINI_SETTINGS_JSON, vec!["$ecret123", "${notheld}"], "${WSP_MCP_S_OAUTH_CLIENTSECRET}", "${ACME_TOKEN}"),
            (OPENCODE_JSON, vec!["{env:notheld}"], "{env:WSP_MCP_S_OAUTH_CLIENTSECRET}", "{env:ACME_TOKEN}"),
        ] {
            for v in unheld {
                throws(lax(format, entry(v), &vault()), line);
            }
            assert!(last_check(format, entry(own_ref), vec![server("s", &[("WSP_MCP_S_OAUTH_CLIENTSECRET", "cs_TESTONLY")])], &Known::new()).is_ok());
            assert!(lax(format, entry(vault_ref), &vault()).is_ok());
        }
    }
}

mod a_reference_with_a_default_in_an_address_or_by_name_in_codex {
    use super::*;

    const DFLT: &str = "sk_TESTONLY_dflt";

    fn vault() -> Known {
        known(&[("ACME_TOKEN", "sk_TESTONLY_held")], &["s"])
    }

    fn dropped_for(path: &str) -> Vec<Dropped> {
        vec![dropped("s", &unread_line(path))]
    }

    #[test]
    fn a_held_name_s_default_never_travels_the_reference_is_written_bare_the_value_comi() {
        let d = format!("${{ACME_TOKEN:-{DFLT}}}");
        let cases: Vec<(McpFormat, Value, Value)> = vec![
            (
                MCP_SERVERS_JSON,
                json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example", "headers": { "Authorization": format!("Bearer {d}"), "X": d } } } }),
                json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example", "headers": { "Authorization": "Bearer ${ACME_TOKEN}", "X": "${ACME_TOKEN}" } } } }),
            ),
            (MCP_SERVERS_JSON, json!({ "mcpServers": { "s": { "command": "x", "env": { "K": d } } } }), json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "${ACME_TOKEN}" } } } })),
            (
                GEMINI_SETTINGS_JSON,
                json!({ "mcpServers": { "s": { "httpUrl": "https://s.example", "headers": { "X": d }, "oauth": { "clientSecret": d } } } }),
                json!({ "mcpServers": { "s": { "httpUrl": "https://s.example", "headers": { "X": "${ACME_TOKEN}" }, "oauth": { "clientSecret": "${ACME_TOKEN}" } } } }),
            ),
            (GEMINI_SETTINGS_JSON, json!({ "mcpServers": { "s": { "command": "x", "env": { "K": d } } } }), json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "${ACME_TOKEN}" } } } })),
            (
                MCP_SERVERS_JSON,
                json!({ "mcpServers": { "s": { "type": "http", "url": format!("https://s.example/?k={d}") } } }),
                json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example/?k=${ACME_TOKEN}" } } }),
            ),
        ];
        for (format, from, to) in cases {
            let mut held = Held::new();
            let out = by_name_held(format, &js(from), &mut held, &vault()).unwrap();
            assert!(!out.text.contains(DFLT));
            assert_eq!(parse(&out.text), to);
            assert_eq!(out.dropped, vec![]);
            assert_eq!(held.len(), 0);
        }
        // OpenCode's {env:X} carries no default, so the same text is a value and moves.
        let opencode = by_name_known(OPENCODE_JSON, &js(json!({ "mcp": { "s": { "type": "remote", "url": "https://s.example", "oauth": { "clientSecret": format!("{{env:ACME_TOKEN:-{DFLT}}}") } } } })), &vault()).unwrap();
        assert!(!opencode.text.contains(DFLT));
    }

    #[test]
    fn the_copy_s_last_check_refuses_a_held_name_s_reference_that_still_carries_its_def() {
        let entries = vec![json!({ "oauth": { "clientSecret": format!("${{ACME_TOKEN:-{DFLT}}}") } })];
        throws(lax(GEMINI_SETTINGS_JSON, entries, &vault()), "a server still carries an OAuth secret in oauth.clientSecret");
    }

    #[test]
    fn a_reference_in_an_address_to_a_name_nobody_holds_drops_its_server() {
        let cases: Vec<(McpFormat, String, &str)> = vec![
            (MCP_SERVERS_JSON, js(json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example/?token=${NOTHELD}" } } })), "mcpServers.s.url"),
            (MCP_SERVERS_JSON, js(json!({ "mcpServers": { "s": { "type": "http", "url": format!("https://s.example/?token=${{NOTHELD:-{DFLT}}}") } } })), "mcpServers.s.url"),
            (GEMINI_SETTINGS_JSON, js(json!({ "mcpServers": { "s": { "httpUrl": "https://s.example/$ecret123" } } })), "mcpServers.s.httpUrl"),
            (OPENCODE_JSON, js(json!({ "mcp": { "s": { "type": "remote", "url": "https://s.example/?t={env:NOTHELD}" } } })), "mcp.s.url"),
        ];
        for (format, text, path) in &cases {
            let out = by_name_known(*format, text, &vault()).unwrap();
            assert_eq!(out.dropped, dropped_for(path), "{text}");
            assert!(!out.text.contains("NOTHELD"), "{text}");
            assert!(!out.text.contains("ecret123"), "{text}");
        }
        let kept = js(json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example/?token=${ACME_TOKEN}" } } }));
        assert_eq!(by_name_known(MCP_SERVERS_JSON, &kept, &vault()).unwrap().text, kept);
    }

    #[test]
    fn codex_a_bearer_or_header_variable_by_name_that_nobody_holds_drops_its_server_one() {
        for (text, path) in [
            ("[mcp_servers.s]\nurl = \"https://s.example\"\nbearer_token_env_var = \"NOTHELD\"\n", "mcp_servers.s.bearer_token_env_var"),
            ("[mcp_servers.s]\nurl = \"https://s.example\"\nenv_http_headers = { \"X-Key\" = \"NOTHELD\" }\n", "mcp_servers.s.env_http_headers.X-Key"),
        ] {
            let out = by_name_known(CODEX_TOML, text, &vault()).unwrap();
            assert_eq!(out.dropped, dropped_for(path), "{text}");
            assert!(!out.text.contains("NOTHELD"), "{text}");
        }
        let kept = "[mcp_servers.s]\nurl = \"https://s.example\"\nbearer_token_env_var = \"ACME_TOKEN\"\nenv_http_headers = { \"X-Key\" = \"ACME_TOKEN\" }\n";
        assert_eq!(by_name_known(CODEX_TOML, kept, &vault()).unwrap().text, kept);
        // A header the copy itself moves names what it writes.
        let moved = by_name(CODEX_TOML, "[mcp_servers.s]\nurl = \"https://s.example\"\nhttp_headers = { Authorization = \"Bearer tok_TESTONLY\" }\n").unwrap();
        assert_eq!(moved.dropped, vec![]);
    }

    #[test]
    fn whether_a_reference_names_what_the_file_writes_does_not_rest_on_the_order_of_its() {
        for env in [json!({ "K": "${TOKEN}", "TOKEN": "sk_TESTONLY_x" }), json!({ "TOKEN": "sk_TESTONLY_x", "K": "${TOKEN}" })] {
            let mut held = Held::new();
            let out = by_name_held(MCP_SERVERS_JSON, &js(json!({ "mcpServers": { "s": { "command": "x", "env": env } } })), &mut held, &Known::new()).unwrap();
            assert_eq!(out.dropped, vec![], "{env}");
            assert_eq!(parse(&out.text)["mcpServers"]["s"]["env"]["K"], json!("${TOKEN}"));
            assert_eq!(held.get("TOKEN").map(|h| h.value.as_str()), Some("sk_TESTONLY_x"));
        }
        // A reference to what a dropped server wrote reads nothing, so its server goes too, never the whole file.
        let text = js(json!({ "mcpServers": {
            "a": { "command": "x", "env": { "K": "${TOKEN}", "L": "sk_TESTONLY_a" } },
            "b": { "command": "y", "env": { "TOKEN": "sk_TESTONLY_b" } }
        } }));
        let row_of = |n: &str| (n == "TOKEN").then(|| "Acme".to_string());
        let after = servers_by_name(MCP_SERVERS_JSON, &text, &mut Held::new(), &row_of, &Known::new()).unwrap();
        assert_eq!(after.dropped.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
        assert!(!after.text.contains("sk_TESTONLY"));
        let early = js(json!({ "mcpServers": {
            "a": { "command": "x", "env": { "K": "${WSP_MCP_B_AUTHORIZATION}" } },
            "b": { "type": "http", "url": "https://b.example", "headers": { "Authorization": "tok_TESTONLY" } }
        } }));
        assert_eq!(by_name(MCP_SERVERS_JSON, &early).unwrap().dropped, vec![]);
    }
}

mod a_server_s_arguments_and_command_by_the_same_rule_as_its_headers {
    use super::*;

    const D: &str = "sk_TESTONLY_dflt";

    fn vault() -> Known {
        known(&[("ACME_TOKEN", "sk_TESTONLY_held")], &["s"])
    }

    fn all(command: Option<&str>, args: &[&str]) -> Vec<String> {
        let mut out = vec![command.unwrap_or("x").to_string()];
        out.extend(args.iter().map(|a| a.to_string()));
        out
    }

    fn file(format: McpFormat, command: Option<&str>, args: &[&str]) -> String {
        match format {
            McpFormat::OpencodeJson => js(json!({ "mcp": { "s": { "type": "local", "command": all(command, args) } } })),
            _ => js(json!({ "mcpServers": { "s": { "command": command.unwrap_or("x"), "args": args } } })),
        }
    }

    fn root_of(format: McpFormat) -> &'static str {
        if format == OPENCODE_JSON {
            "mcp"
        } else {
            "mcpServers"
        }
    }

    fn path(format: McpFormat, is_command: bool, i: usize) -> String {
        match (format == OPENCODE_JSON, is_command) {
            (_, true) if format != OPENCODE_JSON => "command".to_string(),
            (true, true) => "command.0".to_string(),
            (true, false) => format!("command.{}", i + 1),
            (false, _) => format!("args.{i}"),
        }
    }

    fn named(format: McpFormat, n: &str) -> String {
        if format == OPENCODE_JSON {
            envref(n)
        } else {
            dollar(n)
        }
    }

    fn strings(format: McpFormat, text: &str) -> Vec<String> {
        match first_server(format, text).transport {
            McpTransport::Stdio { command, args, .. } => {
                let mut out = vec![command];
                out.extend(args);
                out
            }
            McpTransport::Http { .. } => Vec::new(),
        }
    }

    fn held_name_stands(format: McpFormat) {
        let standing = file(format, Some(&format!("{}/bin/x", named(format, "ACME_TOKEN"))), &[&format!("--token={}", named(format, "ACME_TOKEN"))]);
        assert_eq!(by_name_known(format, &standing, &vault()).unwrap().text, standing);
        if format == OPENCODE_JSON {
            return;
        }
        let mut held = Held::new();
        let text = file(format, Some(&format!("${{ACME_TOKEN:-{D}}}")), &[&format!("--token=${{ACME_TOKEN:-{D}}}")]);
        let out = by_name_held(format, &text, &mut held, &vault()).unwrap();
        assert!(!out.text.contains(D));
        assert_eq!(strings(format, &out.text), all(Some("${ACME_TOKEN}"), &["--token=${ACME_TOKEN}"]));
        assert_eq!(held.len(), 0);
    }

    fn nobody_holds(format: McpFormat) {
        let unheld = if format == GEMINI_SETTINGS_JSON { "$ecret123".to_string() } else { named(format, "NOTHELD") };
        let root = root_of(format);
        let shapes: Vec<(String, String)> = vec![
            (file(format, None, &["-y", &format!("--token={unheld}")]), path(format, false, 1)),
            (file(format, Some(&format!("{unheld}/bin/x")), &[]), path(format, true, 0)),
        ];
        for (text, p) in shapes {
            let out = by_name_known(format, &text, &vault()).unwrap();
            assert_eq!(out.dropped, vec![dropped("s", &unread_line(&format!("{root}.s.{p}")))], "{text}");
            assert!(!out.text.contains(&unheld));
        }
    }

    fn default_for_unheld(format: McpFormat) {
        let mut held = Held::new();
        let text = file(format, None, &[&format!("--token=${{NOTHELD:-{D}}}")]);
        let out = by_name_held(format, &text, &mut held, &vault()).unwrap();
        assert_eq!(out.dropped, vec![]);
        assert!(!out.text.contains(D));
        assert_eq!(strings(format, &out.text), all(None, &["--token=${WSP_MCP_S_ARGS_0}"]));
        assert_eq!(held_list(&held), vec![("WSP_MCP_S_ARGS_0".to_string(), held_value(D, "s"))]);
    }

    fn held_in_command(format: McpFormat) {
        let out = by_name_known(format, &file(format, Some("sk_TESTONLY_held"), &[]), &vault()).unwrap();
        assert!(!out.text.contains("sk_TESTONLY_held"));
        assert_eq!(strings(format, &out.text)[0], named(format, "ACME_TOKEN"));
    }

    #[test]
    fn claude_code_a_reference_to_a_held_name_stands_written_bare_where_it_carries_a_de() {
        held_name_stands(MCP_SERVERS_JSON);
    }

    #[test]
    fn claude_code_a_reference_to_a_name_nobody_holds_drops_its_server_naming_the_key_p() {
        nobody_holds(MCP_SERVERS_JSON);
    }

    #[test]
    fn claude_code_a_default_for_a_name_nobody_holds_is_a_literal_moved_to_servers_env() {
        default_for_unheld(MCP_SERVERS_JSON);
    }

    #[test]
    fn claude_code_a_value_servers_env_holds_in_the_command_travels_as_its_reference() {
        held_in_command(MCP_SERVERS_JSON);
    }

    #[test]
    fn gemini_cli_a_reference_to_a_held_name_stands_written_bare_where_it_carries_a_def() {
        held_name_stands(GEMINI_SETTINGS_JSON);
    }

    #[test]
    fn gemini_cli_a_reference_to_a_name_nobody_holds_drops_its_server_naming_the_key_pa() {
        nobody_holds(GEMINI_SETTINGS_JSON);
    }

    #[test]
    fn gemini_cli_a_default_for_a_name_nobody_holds_is_a_literal_moved_to_servers_env_a() {
        default_for_unheld(GEMINI_SETTINGS_JSON);
    }

    #[test]
    fn gemini_cli_a_value_servers_env_holds_in_the_command_travels_as_its_reference() {
        held_in_command(GEMINI_SETTINGS_JSON);
    }

    #[test]
    fn opencode_a_reference_to_a_held_name_stands_written_bare_where_it_carries_a_defau() {
        held_name_stands(OPENCODE_JSON);
    }

    #[test]
    fn opencode_a_reference_to_a_name_nobody_holds_drops_its_server_naming_the_key_path() {
        nobody_holds(OPENCODE_JSON);
    }

    #[test]
    fn opencode_a_value_servers_env_holds_in_the_command_travels_as_its_reference() {
        held_in_command(OPENCODE_JSON);
    }

    #[test]
    fn a_name_every_machine_defines_stands_in_an_argument_without_servers_env_holding_i() {
        for format in [MCP_SERVERS_JSON, GEMINI_SETTINGS_JSON, OPENCODE_JSON] {
            let reference = |n: &str| match format {
                McpFormat::McpServersJson => dollar(n),
                McpFormat::GeminiSettingsJson => format!("${n}"),
                _ => envref(n),
            };
            let make = |args: Vec<String>, env: Value| match format {
                McpFormat::OpencodeJson => {
                    let mut command = vec!["x".to_string()];
                    command.extend(args);
                    js(json!({ "mcp": { "s": { "type": "local", "command": command, "environment": env } } }))
                }
                _ => js(json!({ "mcpServers": { "s": { "command": "x", "args": args, "env": env } } })),
            };
            let root = root_of(format);
            let standing = make(vec![format!("{}/code", reference("HOME")), format!("--user={}", reference("USER"))], json!({ "K": reference("HOME"), "P": reference("PATH") }));
            let mut held = Held::new();
            let out = by_name_held(format, &standing, &mut held, &Known::new()).unwrap();
            assert_eq!(out.dropped, vec![], "{root}");
            assert_eq!(out.text, standing, "{root}");
            assert_eq!(held.len(), 0, "{root}");
            let unknown = by_name(format, &make(vec![format!("{}/code", reference("HOMEDIR"))], json!({}))).unwrap();
            assert_eq!(unknown.dropped.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), vec!["s"], "{root}");
        }
        let text = js(json!({ "mcpServers": { "s": { "command": "x", "args": ["${HOME:-/x}/code"], "env": { "K": format!("${{HOME:-{D}}}") } } } }));
        let bared = by_name(MCP_SERVERS_JSON, &text).unwrap();
        assert_eq!(parse(&bared.text), json!({ "mcpServers": { "s": { "command": "x", "args": ["${HOME}/code"], "env": { "K": "${HOME}" } } } }));
    }

    fn mixes(format: McpFormat, text: String, key: &str) {
        assert_eq!(said(by_name(format, &text)), format!("s's {key} mixes a value with a variable, so it cannot travel by name; make it one or the other"));
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_a_header_or_1() {
        mixes(MCP_SERVERS_JSON, js(json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example", "headers": { "Authorization": "Bearer sk_TESTONLY_x ${HOME}" } } } })), "Authorization");
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_a_header_or_2() {
        mixes(MCP_SERVERS_JSON, js(json!({ "mcpServers": { "s": { "command": "x", "env": { "TOKEN": "sk_TESTONLY_x${USER}" } } } })), "TOKEN");
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_a_header_or_3() {
        mixes(GEMINI_SETTINGS_JSON, js(json!({ "mcpServers": { "s": { "httpUrl": "https://s.example", "headers": { "X-Key": "sk_TESTONLY_x$HOME" } } } })), "X-Key");
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_a_header_or_4() {
        mixes(OPENCODE_JSON, js(json!({ "mcp": { "s": { "type": "remote", "url": "https://s.example", "headers": { "X-Key": "sk_TESTONLY_x{env:LANG}" } } } })), "X-Key");
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_a_header_or_5() {
        mixes(MCP_SERVERS_JSON, js(json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "${HOME}/code" } } } })), "K");
    }

    #[test]
    fn a_name_the_copy_is_said_to_write_is_one_it_holds_so_a_reference_to_a_variable_se() {
        let text = js(json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "${HOME}/code" } }, "t": { "type": "http", "url": "https://t.example", "headers": { "X": "${K}" } } } }));
        throws(by_name(MCP_SERVERS_JSON, &text), "s's K mixes a value with a variable");
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_an_address_its_ser() {
        let cases: Vec<(McpFormat, String, &str)> = vec![
            (MCP_SERVERS_JSON, js(json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example/mcp?k=sk_TESTONLY_x${HOME}" } } })), "mcpServers.s.url"),
            (GEMINI_SETTINGS_JSON, js(json!({ "mcpServers": { "s": { "httpUrl": "https://s.example/mcp?k=sk_TESTONLY_x$HOME" } } })), "mcpServers.s.httpUrl"),
            (OPENCODE_JSON, js(json!({ "mcp": { "s": { "type": "remote", "url": "https://s.example/mcp?k=sk_TESTONLY_x{env:LANG}" } } })), "mcp.s.url"),
        ];
        for (format, text, p) in &cases {
            let out = by_name(*format, text).unwrap();
            assert_eq!(out.dropped, vec![dropped("s", &unread_line(p))], "{text}");
            assert!(!out.text.contains("sk_TESTONLY_x"), "{text}");
        }
    }

    #[test]
    fn a_name_the_copy_moves_out_of_an_argument_s_default_or_a_variable_s_default_is_on() {
        let cases = [(json!({ "command": "x", "args": [format!("${{A:-{D}}}")] }), "WSP_MCP_S_ARGS_0"), (json!({ "command": "x", "env": { "K": format!("${{X:-{D}}}") } }), "K")];
        for (s, name) in cases {
            let mut held = Held::new();
            let text = js(json!({ "mcpServers": { "s": s, "t": { "type": "http", "url": "https://t.example", "headers": { "X": dollar(name) } } } }));
            let out = by_name_held(MCP_SERVERS_JSON, &text, &mut held, &Known::new()).unwrap();
            assert_eq!(out.dropped, vec![], "{name}");
            assert_eq!(parse(&out.text)["mcpServers"]["t"]["headers"]["X"], json!(dollar(name)), "{name}");
            assert_eq!(held.get(name).map(|h| h.value.as_str()), Some(D), "{name}");
        }
    }

    #[test]
    fn refuses_an_argument_s_default_whose_name_would_be_one_the_server_sets_itself_nam() {
        let text = js(json!({ "mcpServers": { "s": { "command": "x", "args": [format!("${{A:-{D}}}")], "env": { "WSP_MCP_S_ARGS_0": "other_TESTONLY" } } } }));
        assert_eq!(said(by_name(MCP_SERVERS_JSON, &text)), "s sets WSP_MCP_S_ARGS_0, the name the default in mcpServers.s.args.0 would travel under; rename it");
    }

    #[test]
    fn codex_a_value_servers_env_holds_in_the_command_refuses_the_copy_since_codex_read() {
        throws(
            by_name_known(CODEX_TOML, "[mcp_servers.s]\ncommand = \"sk_TESTONLY_held\"\n", &vault()),
            "s passes the value of ACME_TOKEN in the command, and Codex reads no variable there, so the file stays on this computer",
        );
    }
}

mod opencode_s_mcp_servers_envelope {
    use super::*;

    const H: &str = "sk_TESTONLY_envelope";

    #[test]
    fn refuses_the_file_whole_naming_the_key_path_and_never_a_value_since_its_servers_a() {
        for def in [json!({ "type": "remote", "url": "https://s.example", "headers": { "Authorization": format!("Bearer {H}") } }), json!({ "type": "local", "command": ["x"], "environment": { "K": H } })] {
            let text = js(json!({ "mcp": { "top": { "type": "local", "command": ["y"] }, "servers": { "s": def } } }));
            assert_eq!(said(by_name(OPENCODE_JSON, &text)), "the file keeps servers under mcp.servers, whose values wsp does not write by name, so the file stays on this computer");
        }
    }

    #[test]
    fn reads_a_table_under_mcp_that_holds_a_type_or_an_enabled_switch_as_one_server_cal() {
        let text = js(json!({ "mcp": { "servers": { "type": "local", "command": ["x"], "environment": { "K": H } } } }));
        let out = by_name(OPENCODE_JSON, &text).unwrap();
        assert!(!out.text.contains(H));
    }

    #[test]
    fn leaves_the_envelope_alone_when_one_other_server_is_asked_for_as_an_add_on_the_ma() {
        let text = js(json!({ "mcp": { "top": { "type": "local", "command": ["y"] }, "servers": { "s": { "type": "local", "command": ["x"], "environment": { "K": H } } } } }));
        assert_eq!(refer_only(OPENCODE_JSON, &text, "top").unwrap().servers, vec![server("top", &[])]);
    }
}

mod an_oauth_secret_s_reference_read_as_one_the_definition_reads {
    use super::*;

    #[test]
    fn counts_where_the_agent_expands_a_variable_in_it_and_not_where_it_reads_none() {
        let remote = |oauth: Value| js(json!({ "mcp": { "s": { "type": "remote", "url": "https://s.example", "oauth": oauth } } }));
        let refs = |format: McpFormat, text: &str| first_server(format, text).env_refs;
        assert_eq!(refs(OPENCODE_JSON, &remote(json!({ "clientSecret": "{env:S_SECRET}" }))), vec!["S_SECRET".to_string()]);
        assert_eq!(refs(OPENCODE_JSON, &remote(json!({ "client_secret": "{env:S_SECRET}" }))), vec!["S_SECRET".to_string()]);
        let gemini = js(json!({ "mcpServers": { "s": { "httpUrl": "https://s.example", "headers": { "X": "$X_KEY" }, "oauth": { "clientSecret": "$S_SECRET" } } } }));
        assert_eq!(refs(GEMINI_SETTINGS_JSON, &gemini), vec!["X_KEY".to_string(), "S_SECRET".to_string()]);
        let claude_text = js(json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example", "oauth": { "clientSecret": "${S_SECRET}" } } } }));
        assert_eq!(refs(MCP_SERVERS_JSON, &claude_text), Vec::<String>::new());
    }
}
