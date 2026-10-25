// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

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
            McpFormat::McpServersJson => js(
                json!({ "mcpServers": { "s": if w == Where::Env { json!({ "command": "x", "env": { "K": v } }) } else { remote(json!({ "type": "http", "url": "https://s.example" })) } } }),
            ),
            McpFormat::GeminiSettingsJson => js(
                json!({ "mcpServers": { "s": if w == Where::Env { json!({ "command": "x", "env": { "K": v } }) } else { remote(json!({ "httpUrl": "https://s.example" })) } } }),
            ),
            McpFormat::OpencodeJson => js(
                json!({ "mcp": { "s": if w == Where::Env { json!({ "type": "local", "command": ["x"], "environment": { "K": v } }) } else { remote(json!({ "type": "remote", "url": "https://s.example" })) } } }),
            ),
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
                (
                    "$ecret123".to_string(),
                    Box::new(|w| if w == Where::Oauth { Outcome::Refused } else { Outcome::Moved("$ecret123".to_string()) }),
                ),
                ("${notheld}".to_string(), Box::new(|_| Outcome::Dropped)),
                ("Bearer ${notheld}".to_string(), Box::new(|w| if w == Where::Oauth { Outcome::Mixed } else { Outcome::Dropped })),
                (
                    format!("${{notheld:-{D}}}"),
                    Box::new(|w| if w == Where::Oauth { Outcome::Refused } else { Outcome::Moved(D.to_string()) }),
                ),
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
                    Outcome::Mixed => throws(
                        by_name_known(format, &text, &vault()),
                        "s's oauth.clientSecret mixes a value with a variable, so it cannot travel by name",
                    ),
                    Outcome::Refused => throws(
                        by_name_known(format, &text, &vault()),
                        &format!(
                            "s keeps an OAuth secret in {}, and its agent reads no variable there, so the file stays on this computer",
                            key_path(format, w)
                        ),
                    ),
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
        let line =
            "a server still carries an OAuth secret in oauth.clientSecret after it was written by name, so the file stays on this computer";
        let entry = |secret: &str| vec![json!({ "oauth": { "clientSecret": secret } })];
        for (format, unheld, own_ref, vault_ref) in [
            (GEMINI_SETTINGS_JSON, vec!["$ecret123", "${notheld}"], "${WSP_MCP_S_OAUTH_CLIENTSECRET}", "${ACME_TOKEN}"),
            (OPENCODE_JSON, vec!["{env:notheld}"], "{env:WSP_MCP_S_OAUTH_CLIENTSECRET}", "{env:ACME_TOKEN}"),
        ] {
            for v in unheld {
                throws(lax(format, entry(v), &vault()), line);
            }
            assert!(last_check(
                format,
                entry(own_ref),
                vec![server("s", &[("WSP_MCP_S_OAUTH_CLIENTSECRET", "cs_TESTONLY")])],
                &Known::new()
            )
            .is_ok());
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
            (
                MCP_SERVERS_JSON,
                json!({ "mcpServers": { "s": { "command": "x", "env": { "K": d } } } }),
                json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "${ACME_TOKEN}" } } } }),
            ),
            (
                GEMINI_SETTINGS_JSON,
                json!({ "mcpServers": { "s": { "httpUrl": "https://s.example", "headers": { "X": d }, "oauth": { "clientSecret": d } } } }),
                json!({ "mcpServers": { "s": { "httpUrl": "https://s.example", "headers": { "X": "${ACME_TOKEN}" }, "oauth": { "clientSecret": "${ACME_TOKEN}" } } } }),
            ),
            (
                GEMINI_SETTINGS_JSON,
                json!({ "mcpServers": { "s": { "command": "x", "env": { "K": d } } } }),
                json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "${ACME_TOKEN}" } } } }),
            ),
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
            (
                MCP_SERVERS_JSON,
                js(json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example/?token=${NOTHELD}" } } })),
                "mcpServers.s.url",
            ),
            (
                MCP_SERVERS_JSON,
                js(json!({ "mcpServers": { "s": { "type": "http", "url": format!("https://s.example/?token=${{NOTHELD:-{DFLT}}}") } } })),
                "mcpServers.s.url",
            ),
            (
                GEMINI_SETTINGS_JSON,
                js(json!({ "mcpServers": { "s": { "httpUrl": "https://s.example/$ecret123" } } })),
                "mcpServers.s.httpUrl",
            ),
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
            (
                "[mcp_servers.s]\nurl = \"https://s.example\"\nenv_http_headers = { \"X-Key\" = \"NOTHELD\" }\n",
                "mcp_servers.s.env_http_headers.X-Key",
            ),
        ] {
            let out = by_name_known(CODEX_TOML, text, &vault()).unwrap();
            assert_eq!(out.dropped, dropped_for(path), "{text}");
            assert!(!out.text.contains("NOTHELD"), "{text}");
        }
        let kept = "[mcp_servers.s]\nurl = \"https://s.example\"\nbearer_token_env_var = \"ACME_TOKEN\"\nenv_http_headers = { \"X-Key\" = \"ACME_TOKEN\" }\n";
        assert_eq!(by_name_known(CODEX_TOML, kept, &vault()).unwrap().text, kept);
        // A header the copy itself moves names what it writes.
        let moved = by_name(
            CODEX_TOML,
            "[mcp_servers.s]\nurl = \"https://s.example\"\nhttp_headers = { Authorization = \"Bearer tok_TESTONLY\" }\n",
        )
        .unwrap();
        assert_eq!(moved.dropped, vec![]);
    }

    #[test]
    fn whether_a_reference_names_what_the_file_writes_does_not_rest_on_the_order_of_its() {
        for env in [json!({ "K": "${TOKEN}", "TOKEN": "sk_TESTONLY_x" }), json!({ "TOKEN": "sk_TESTONLY_x", "K": "${TOKEN}" })] {
            let mut held = Held::new();
            let out = by_name_held(
                MCP_SERVERS_JSON,
                &js(json!({ "mcpServers": { "s": { "command": "x", "env": env } } })),
                &mut held,
                &Known::new(),
            )
            .unwrap();
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
        let standing =
            file(format, Some(&format!("{}/bin/x", named(format, "ACME_TOKEN"))), &[&format!("--token={}", named(format, "ACME_TOKEN"))]);
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
            let standing = make(
                vec![format!("{}/code", reference("HOME")), format!("--user={}", reference("USER"))],
                json!({ "K": reference("HOME"), "P": reference("PATH") }),
            );
            let mut held = Held::new();
            let out = by_name_held(format, &standing, &mut held, &Known::new()).unwrap();
            assert_eq!(out.dropped, vec![], "{root}");
            assert_eq!(out.text, standing, "{root}");
            assert_eq!(held.len(), 0, "{root}");
            let unknown = by_name(format, &make(vec![format!("{}/code", reference("HOMEDIR"))], json!({}))).unwrap();
            assert_eq!(unknown.dropped.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), vec!["s"], "{root}");
        }
        let text = js(
            json!({ "mcpServers": { "s": { "command": "x", "args": ["${HOME:-/x}/code"], "env": { "K": format!("${{HOME:-{D}}}") } } } }),
        );
        let bared = by_name(MCP_SERVERS_JSON, &text).unwrap();
        assert_eq!(
            parse(&bared.text),
            json!({ "mcpServers": { "s": { "command": "x", "args": ["${HOME}/code"], "env": { "K": "${HOME}" } } } })
        );
    }

    fn mixes(format: McpFormat, text: String, key: &str) {
        assert_eq!(
            said(by_name(format, &text)),
            format!("s's {key} mixes a value with a variable, so it cannot travel by name; make it one or the other")
        );
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_a_header_or_1() {
        mixes(
            MCP_SERVERS_JSON,
            js(
                json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example", "headers": { "Authorization": "Bearer sk_TESTONLY_x ${HOME}" } } } }),
            ),
            "Authorization",
        );
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_a_header_or_2() {
        mixes(
            MCP_SERVERS_JSON,
            js(json!({ "mcpServers": { "s": { "command": "x", "env": { "TOKEN": "sk_TESTONLY_x${USER}" } } } })),
            "TOKEN",
        );
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_a_header_or_3() {
        mixes(
            GEMINI_SETTINGS_JSON,
            js(json!({ "mcpServers": { "s": { "httpUrl": "https://s.example", "headers": { "X-Key": "sk_TESTONLY_x$HOME" } } } })),
            "X-Key",
        );
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_a_header_or_4() {
        mixes(
            OPENCODE_JSON,
            js(
                json!({ "mcp": { "s": { "type": "remote", "url": "https://s.example", "headers": { "X-Key": "sk_TESTONLY_x{env:LANG}" } } } }),
            ),
            "X-Key",
        );
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_a_header_or_5() {
        mixes(MCP_SERVERS_JSON, js(json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "${HOME}/code" } } } })), "K");
    }

    #[test]
    fn a_name_the_copy_is_said_to_write_is_one_it_holds_so_a_reference_to_a_variable_se() {
        let text = js(
            json!({ "mcpServers": { "s": { "command": "x", "env": { "K": "${HOME}/code" } }, "t": { "type": "http", "url": "https://t.example", "headers": { "X": "${K}" } } } }),
        );
        throws(by_name(MCP_SERVERS_JSON, &text), "s's K mixes a value with a variable");
    }

    #[test]
    fn a_name_every_machine_defines_lets_no_text_beside_it_travel_in_an_address_its_ser() {
        let cases: Vec<(McpFormat, String, &str)> = vec![
            (
                MCP_SERVERS_JSON,
                js(json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example/mcp?k=sk_TESTONLY_x${HOME}" } } })),
                "mcpServers.s.url",
            ),
            (
                GEMINI_SETTINGS_JSON,
                js(json!({ "mcpServers": { "s": { "httpUrl": "https://s.example/mcp?k=sk_TESTONLY_x$HOME" } } })),
                "mcpServers.s.httpUrl",
            ),
            (
                OPENCODE_JSON,
                js(json!({ "mcp": { "s": { "type": "remote", "url": "https://s.example/mcp?k=sk_TESTONLY_x{env:LANG}" } } })),
                "mcp.s.url",
            ),
        ];
        for (format, text, p) in &cases {
            let out = by_name(*format, text).unwrap();
            assert_eq!(out.dropped, vec![dropped("s", &unread_line(p))], "{text}");
            assert!(!out.text.contains("sk_TESTONLY_x"), "{text}");
        }
    }

    #[test]
    fn a_name_the_copy_moves_out_of_an_argument_s_default_or_a_variable_s_default_is_on() {
        let cases = [
            (json!({ "command": "x", "args": [format!("${{A:-{D}}}")] }), "WSP_MCP_S_ARGS_0"),
            (json!({ "command": "x", "env": { "K": format!("${{X:-{D}}}") } }), "K"),
        ];
        for (s, name) in cases {
            let mut held = Held::new();
            let text = js(
                json!({ "mcpServers": { "s": s, "t": { "type": "http", "url": "https://t.example", "headers": { "X": dollar(name) } } } }),
            );
            let out = by_name_held(MCP_SERVERS_JSON, &text, &mut held, &Known::new()).unwrap();
            assert_eq!(out.dropped, vec![], "{name}");
            assert_eq!(parse(&out.text)["mcpServers"]["t"]["headers"]["X"], json!(dollar(name)), "{name}");
            assert_eq!(held.get(name).map(|h| h.value.as_str()), Some(D), "{name}");
        }
    }

    #[test]
    fn refuses_an_argument_s_default_whose_name_would_be_one_the_server_sets_itself_nam() {
        let text = js(
            json!({ "mcpServers": { "s": { "command": "x", "args": [format!("${{A:-{D}}}")], "env": { "WSP_MCP_S_ARGS_0": "other_TESTONLY" } } } }),
        );
        assert_eq!(
            said(by_name(MCP_SERVERS_JSON, &text)),
            "s sets WSP_MCP_S_ARGS_0, the name the default in mcpServers.s.args.0 would travel under; rename it"
        );
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
        for def in [
            json!({ "type": "remote", "url": "https://s.example", "headers": { "Authorization": format!("Bearer {H}") } }),
            json!({ "type": "local", "command": ["x"], "environment": { "K": H } }),
        ] {
            let text = js(json!({ "mcp": { "top": { "type": "local", "command": ["y"] }, "servers": { "s": def } } }));
            assert_eq!(
                said(by_name(OPENCODE_JSON, &text)),
                "the file keeps servers under mcp.servers, whose values wsp does not write by name, so the file stays on this computer"
            );
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
        let text = js(
            json!({ "mcp": { "top": { "type": "local", "command": ["y"] }, "servers": { "s": { "type": "local", "command": ["x"], "environment": { "K": H } } } } }),
        );
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
        let gemini = js(
            json!({ "mcpServers": { "s": { "httpUrl": "https://s.example", "headers": { "X": "$X_KEY" }, "oauth": { "clientSecret": "$S_SECRET" } } } }),
        );
        assert_eq!(refs(GEMINI_SETTINGS_JSON, &gemini), vec!["X_KEY".to_string(), "S_SECRET".to_string()]);
        let claude_text = js(
            json!({ "mcpServers": { "s": { "type": "http", "url": "https://s.example", "oauth": { "clientSecret": "${S_SECRET}" } } } }),
        );
        assert_eq!(refs(MCP_SERVERS_JSON, &claude_text), Vec::<String>::new());
    }
}
