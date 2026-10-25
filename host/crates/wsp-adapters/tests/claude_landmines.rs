// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/adapter-claude/test/landmines.test.ts (45 tests) and test/sign-in-order.test.ts (7 tests) at
//! a3b870a6e: the Claude launch's environment, its sign-in order and its command line.

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use futures::future::{BoxFuture, FutureExt};
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::claude::landmines::{
    broker_door_url, build_command, build_env, forwards_subagent_text, saved_spend_command, sign_in_guard, user_message_line, BaseEnv, BrokerKind, BrokeredSignIn, BuildCommandOptions, ClaudeEnvOptions, FIRST_BYTE_ENV, OFF_SERVERS_FILE, PROJECT_DIR_ENV, SAVED_SPEND_TAIL_BYTES,
};
use wsp_adapters::pending::McpServerSpec;
use wsp_adapters::port::{agent_word, AgentLaunch, DraftAsk, Env, ExecFailure, ExecStream, ExecStreamFactory, ExecOptions, HarnessExec, HarnessStartOptions, TitleTurn, TurnImage};
use wsp_frames::shell_quote;

use claude_exec::{FakeExec, StreamOpts};

// INTERFACE GAP: PLUGIN_SEED_ENV (packages/protocol env.ts:113) has no Rust constant; this is its value.
const PLUGIN_SEED_ENV: &str = "CLAUDE_CODE_PLUGIN_SEED_DIR";

fn base_env(pairs: &[(&str, &str)]) -> BaseEnv {
    pairs.iter().map(|(k, v)| (k.to_string(), Some(v.to_string()))).collect()
}

fn with(base: &BaseEnv, more: &[(&str, &str)]) -> BaseEnv {
    let mut out = base.clone();
    for (k, v) in more {
        out.insert(k.to_string(), Some(v.to_string()));
    }
    out
}

fn env_of(base: &BaseEnv) -> ClaudeEnvOptions {
    ClaudeEnvOptions { base: Some(base.clone()), ..Default::default() }
}

fn brokered(token: &str, kind: BrokerKind) -> Option<BrokeredSignIn> {
    Some(BrokeredSignIn { token: token.to_string(), kind })
}

fn get<'a>(env: &'a Env, key: &str) -> Option<&'a str> {
    env.get(key).map(String::as_str)
}

fn assert_matches_object(env: &Env, expected: &[(&str, &str)]) {
    for (k, v) in expected {
        assert_eq!(get(env, k), Some(*v), "{k}");
    }
}

fn command(o: BuildCommandOptions) -> String {
    build_command(&o).unwrap()
}

fn err_of(o: BuildCommandOptions) -> String {
    build_command(&o).expect_err("expected the command to be refused").0
}

fn spec(command: &str, args: &[&str]) -> McpServerSpec {
    McpServerSpec { command: command.to_string(), args: args.iter().map(|a| a.to_string()).collect(), no_slate: None }
}

fn servers(entries: Vec<(&str, McpServerSpec)>) -> Option<IndexMap<String, McpServerSpec>> {
    Some(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn mcp_flag_json(cmd: &str) -> Value {
    let flag = Regex::new(r"--mcp-config '(.+?)' ").unwrap().captures(cmd).unwrap_or_else(|| panic!("no --mcp-config in {cmd}"))[1].to_string();
    serde_json::from_str(&flag).unwrap()
}

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempRoot(dir)
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_stub(path: &Path, script: &str) {
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

mod build_env_cases {
    use super::*;

    fn base() -> BaseEnv {
        base_env(&[("CLAUDECODE", "1"), ("CLAUDE_CODE_ENTRYPOINT", "cli"), ("CLAUDE_CODE_SSE_PORT", "12345"), ("FORCE_CODE_TERMINAL", "true"), ("PATH", "/root/.local/bin:/usr/bin"), ("HOME", "/Users/z")])
    }

    // TS: landmines.test.ts:21
    #[test]
    fn strips_inherited_claude_code_claudecode_and_force_code_terminal() {
        let env = build_env(&env_of(&base()));
        assert_eq!(get(&env, "CLAUDECODE"), None);
        assert_eq!(get(&env, "CLAUDE_CODE_ENTRYPOINT"), None);
        assert_eq!(get(&env, "CLAUDE_CODE_SSE_PORT"), None);
        assert_eq!(get(&env, "FORCE_CODE_TERMINAL"), None);
        assert_eq!(get(&env, "PATH"), Some("/root/.local/bin:/usr/bin"));
    }

    // TS: landmines.test.ts:30
    #[test]
    fn keeps_the_plugin_seed_a_guests_login_names_through_the_strip_on_every_road() {
        assert_eq!(PLUGIN_SEED_ENV, "CLAUDE_CODE_PLUGIN_SEED_DIR");
        let guest = with(&base(), &[("CLAUDE_CONFIG_DIR", "/root/.claude-cfg"), ("CLAUDE_CODE_PLUGIN_SEED_DIR", "/root/.claude-plugin-seed")]);
        assert_eq!(get(&build_env(&env_of(&guest)), "CLAUDE_CODE_PLUGIN_SEED_DIR"), Some("/root/.claude-plugin-seed"));
        let through_broker = build_env(&ClaudeEnvOptions { base: Some(guest.clone()), brokered: brokered("turn-token-TESTONLY", BrokerKind::Token), ..Default::default() });
        assert_eq!(get(&through_broker, "CLAUDE_CODE_PLUGIN_SEED_DIR"), Some("/root/.claude-plugin-seed"));
        assert_eq!(get(&build_env(&env_of(&guest)), "CLAUDE_CODE_ENTRYPOINT"), None);
        assert_eq!(get(&build_env(&env_of(&base())), "CLAUDE_CODE_PLUGIN_SEED_DIR"), None);
    }

    // TS: landmines.test.ts:39
    #[test]
    fn turns_on_the_task_list_tools_a_headless_run_leaves_off() {
        assert_eq!(get(&build_env(&env_of(&base())), "CLAUDE_CODE_ENABLE_TODO_TOOLS"), Some("1"));
    }

    // TS: landmines.test.ts:43
    #[test]
    fn drops_an_inherited_project_key_and_sets_the_one_the_caller_named_after_the_strip() {
        assert_eq!(get(&build_env(&env_of(&with(&base(), &[(PROJECT_DIR_ENV, "-Users-z-repo")]))), PROJECT_DIR_ENV), None);
        let keyed = build_env(&ClaudeEnvOptions { base: Some(base()), project_dir_name: Some("-Users-z-repo".into()), ..Default::default() });
        assert_eq!(get(&keyed, PROJECT_DIR_ENV), Some("-Users-z-repo"));
        let stale = build_env(&ClaudeEnvOptions { base: Some(with(&base(), &[(PROJECT_DIR_ENV, "-stale")])), project_dir_name: Some("-Users-z-repo".into()), ..Default::default() });
        assert_eq!(get(&stale, PROJECT_DIR_ENV), Some("-Users-z-repo"));
        assert!(!build_env(&env_of(&base())).contains_key(PROJECT_DIR_ENV));
    }

    // TS: landmines.test.ts:55
    #[test]
    fn never_sets_claude_config_dir_or_is_sandbox_of_its_own() {
        let own = build_env(&env_of(&base()));
        assert_eq!(get(&own, "CLAUDE_CONFIG_DIR"), None);
        assert_eq!(get(&own, "IS_SANDBOX"), None);
        assert_eq!(get(&own, "HOME"), Some("/Users/z"));
        let theirs = build_env(&env_of(&with(&base(), &[("CLAUDE_CONFIG_DIR", "/Users/z/elsewhere")])));
        assert_eq!(get(&theirs, "CLAUDE_CONFIG_DIR"), Some("/Users/z/elsewhere"));
        let guest = build_env(&env_of(&base_env(&[("HOME", "/root"), ("CLAUDE_CONFIG_DIR", "/root/.claude-cfg"), ("IS_SANDBOX", "1")])));
        assert_eq!(get(&guest, "CLAUDE_CONFIG_DIR"), Some("/root/.claude-cfg"));
        assert_eq!(get(&guest, "IS_SANDBOX"), Some("1"));
    }

    // TS: landmines.test.ts:69
    #[test]
    fn sets_the_headless_flags_ide_discovery_suppression() {
        let env = build_env(&env_of(&base()));
        assert_eq!(get(&env, "CLAUDE_CODE_AUTO_CONNECT_IDE"), Some("0"));
        assert_eq!(get(&env, "CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL"), Some("1"));
        let allowed = ["CLAUDE_CODE_AUTO_CONNECT_IDE", "CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL", "CLAUDE_CODE_ENABLE_TODO_TOOLS", "CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS", "CLAUDE_CODE_MCP_STARTUP_WAIT_MS"];
        let leaked: Vec<&String> = env.keys().filter(|k| (k.starts_with("CLAUDE_CODE_") || k.as_str() == "CLAUDECODE") && !allowed.contains(&k.as_str())).collect();
        assert_eq!(leaked, Vec::<&String>::new());
    }

    // TS: landmines.test.ts:85
    #[test]
    fn lets_a_background_subagent_run_as_long_as_the_turn_holds_its_reply() {
        assert_eq!(get(&build_env(&env_of(&base())), "CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS"), Some("0"));
        let set = with(&base(), &[("CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS", "600000")]);
        assert_eq!(get(&build_env(&env_of(&set)), "CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS"), Some("0"));
    }

    // TS: landmines.test.ts:90
    #[test]
    fn starts_the_first_turn_without_waiting_on_any_mcp_server_on_every_road() {
        assert_eq!(get(&build_env(&env_of(&base())), "CLAUDE_CODE_MCP_STARTUP_WAIT_MS"), Some("0"));
        let set = with(&base(), &[("CLAUDE_CODE_MCP_STARTUP_WAIT_MS", "30000")]);
        assert_eq!(get(&build_env(&env_of(&set)), "CLAUDE_CODE_MCP_STARTUP_WAIT_MS"), Some("0"));
        let through_broker = build_env(&ClaudeEnvOptions { base: Some(base()), brokered: brokered("wsp-turn-x", BrokerKind::Token), ..Default::default() });
        assert_eq!(get(&through_broker, "CLAUDE_CODE_MCP_STARTUP_WAIT_MS"), Some("0"));
    }

    // TS: landmines.test.ts:96
    #[test]
    fn gives_up_on_a_model_call_no_reply_has_started_for_in_30_s_unless_the_person_set_their_own_wait() {
        assert_eq!(get(&build_env(&env_of(&base())), FIRST_BYTE_ENV), Some("30000"));
        let with_token = build_env(&ClaudeEnvOptions { base: Some(base()), oauth_token: Some("sk-ant-oat01-TESTONLY".into()), ..Default::default() });
        assert_eq!(get(&with_token, FIRST_BYTE_ENV), Some("30000"));
        assert_eq!(get(&build_env(&env_of(&with(&base(), &[(FIRST_BYTE_ENV, "120000")]))), FIRST_BYTE_ENV), Some("120000"));
        assert!(!build_env(&env_of(&base())).contains_key("API_TIMEOUT_MS"));
        let through_broker = build_env(&ClaudeEnvOptions { base: Some(base()), brokered: brokered("wsp-turn-x", BrokerKind::Token), ..Default::default() });
        assert!(!through_broker.contains_key(FIRST_BYTE_ENV));
    }

    // TS: landmines.test.ts:104
    #[test]
    fn passes_the_api_key_through_when_given() {
        let env = build_env(&ClaudeEnvOptions { base: Some(BaseEnv::new()), api_key: Some("sk-ant-x".into()), ..Default::default() });
        assert_eq!(get(&env, "ANTHROPIC_API_KEY"), Some("sk-ant-x"));
    }

    // TS: landmines.test.ts:109
    #[test]
    fn sets_our_own_long_lived_token_after_the_strip_so_an_inherited_one_never_rides_along() {
        let inherited = with(&base(), &[("CLAUDE_CODE_OAUTH_TOKEN", "inherited")]);
        let env = build_env(&ClaudeEnvOptions { base: Some(inherited.clone()), oauth_token: Some("sk-ant-oat01-TESTONLY".into()), ..Default::default() });
        assert_eq!(get(&env, "CLAUDE_CODE_OAUTH_TOKEN"), Some("sk-ant-oat01-TESTONLY"));
        let with_key = build_env(&ClaudeEnvOptions { base: Some(inherited), api_key: Some("sk-ant-x".into()), ..Default::default() });
        assert_eq!(get(&with_key, "CLAUDE_CODE_OAUTH_TOKEN"), None);
        assert_eq!(get(&build_env(&env_of(&BaseEnv::new())), "CLAUDE_CODE_OAUTH_TOKEN"), None);
    }
}

mod brokered_launch {
    use super::*;

    const TOKEN: &str = "turn-token-TESTONLY";
    const REAL: [(&str, &str); 7] = [
        ("ANTHROPIC_API_KEY", "sk-ant-api03-REAL"),
        ("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-REAL"),
        ("ANTHROPIC_AUTH_TOKEN", "gateway-REAL"),
        ("ANTHROPIC_CUSTOM_HEADERS", "x-gateway-key: REAL"),
        ("ANTHROPIC_FOUNDRY_API_KEY", "foundry-REAL"),
        ("AWS_BEARER_TOKEN_BEDROCK", "bedrock-REAL"),
        ("ANTHROPIC_BASE_URL", "https://gateway.acme.example"),
    ];

    fn base() -> BaseEnv {
        let mut b = base_env(&REAL);
        for (k, v) in [("HOME", "/root"), ("PATH", "/usr/bin"), ("IS_SANDBOX", "1"), ("DISABLE_AUTOUPDATER", "1"), ("WSP_HOST_TOKEN", TOKEN)] {
            b.insert(k.to_string(), Some(v.to_string()));
        }
        b
    }

    fn shared() -> Vec<(&'static str, String)> {
        vec![
            ("ANTHROPIC_BASE_URL", broker_door_url()),
            ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".to_string()),
            ("CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST", "1".to_string()),
            ("DISABLE_UPDATES", "1".to_string()),
            ("API_TIMEOUT_MS", "3600000".to_string()),
        ]
    }

    fn launch(kind: BrokerKind) -> Env {
        build_env(&ClaudeEnvOptions { base: Some(base()), brokered: brokered(TOKEN, kind), ..Default::default() })
    }

    fn assert_shared(env: &Env) {
        for (k, v) in shared() {
            assert_eq!(get(env, k), Some(v.as_str()), "{k}");
        }
    }

    // TS: landmines.test.ts:137
    #[test]
    fn carries_the_placeholder_of_the_credentials_kind_and_every_variable_of_the_brokered_road() {
        let token = launch(BrokerKind::Token);
        assert_shared(&token);
        assert_matches_object(&token, &[("CLAUDE_CODE_OAUTH_TOKEN", TOKEN), ("ENABLE_TOOL_SEARCH", "true")]);
        assert_eq!(launch(BrokerKind::Login), launch(BrokerKind::Token));
        let key = launch(BrokerKind::Key);
        assert_shared(&key);
        assert_matches_object(&key, &[("ANTHROPIC_API_KEY", TOKEN), ("ENABLE_TOOL_SEARCH", "true")]);
        let gateway = launch(BrokerKind::Gateway);
        assert_shared(&gateway);
        assert_matches_object(&gateway, &[("ANTHROPIC_AUTH_TOKEN", TOKEN), ("ANTHROPIC_API_KEY", "")]);
    }

    // TS: landmines.test.ts:144
    #[test]
    fn sends_one_placeholder_per_kind() {
        let token = launch(BrokerKind::Token);
        assert!(!token.contains_key("ANTHROPIC_API_KEY"));
        assert!(!token.contains_key("ANTHROPIC_AUTH_TOKEN"));
        assert_eq!(get(&token, "CLAUDE_CODE_OAUTH_TOKEN"), Some(TOKEN));
        let key = launch(BrokerKind::Key);
        assert!(!key.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));
        assert!(!key.contains_key("ANTHROPIC_AUTH_TOKEN"));
        assert_eq!(get(&key, "ANTHROPIC_API_KEY"), Some(TOKEN));
        let gateway = launch(BrokerKind::Gateway);
        assert!(!gateway.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));
        assert_eq!(get(&gateway, "ANTHROPIC_AUTH_TOKEN"), Some(TOKEN));
    }

    // TS: landmines.test.ts:152
    #[test]
    fn turns_tool_search_on_only_where_the_upstream_is_first_party() {
        assert!(!launch(BrokerKind::Gateway).contains_key("ENABLE_TOOL_SEARCH"));
        assert_eq!(get(&launch(BrokerKind::Token), "ENABLE_TOOL_SEARCH"), Some("true"));
        assert_eq!(get(&launch(BrokerKind::Key), "ENABLE_TOOL_SEARCH"), Some("true"));
    }

    // TS: landmines.test.ts:156
    #[test]
    fn holds_no_real_credential_whatever_the_login_or_the_vault_carried() {
        for kind in [BrokerKind::Login, BrokerKind::Token, BrokerKind::Key, BrokerKind::Gateway] {
            let env = build_env(&ClaudeEnvOptions { base: Some(base()), api_key: Some("sk-ant-api03-VAULT".into()), oauth_token: Some("sk-ant-oat01-VAULT".into()), brokered: brokered(TOKEN, kind), ..Default::default() });
            let values: Vec<&str> = env.values().map(String::as_str).collect();
            let mut secrets: Vec<&str> = REAL.iter().map(|(_, v)| *v).collect();
            secrets.extend(["sk-ant-api03-VAULT", "sk-ant-oat01-VAULT"]);
            for value in secrets {
                assert!(!values.contains(&value), "{kind:?} carries {value}");
            }
            assert!(!env.contains_key("ANTHROPIC_CUSTOM_HEADERS"), "{kind:?}");
            assert!(!env.contains_key("ANTHROPIC_FOUNDRY_API_KEY"), "{kind:?}");
            assert!(!env.contains_key("AWS_BEARER_TOKEN_BEDROCK"), "{kind:?}");
        }
    }

    // TS: landmines.test.ts:167
    #[test]
    fn keeps_the_rest_of_the_launch_the_logins_own_variables_the_headless_flags_and_the_project_key() {
        let env = build_env(&ClaudeEnvOptions { base: Some(base()), brokered: brokered(TOKEN, BrokerKind::Token), project_dir_name: Some("-root-lab".into()), ..Default::default() });
        assert_matches_object(&env, &[("HOME", "/root"), ("PATH", "/usr/bin"), ("IS_SANDBOX", "1"), ("DISABLE_AUTOUPDATER", "1"), ("WSP_HOST_TOKEN", TOKEN)]);
        assert_matches_object(&env, &[("CLAUDE_CODE_AUTO_CONNECT_IDE", "0"), ("CLAUDE_CODE_ENABLE_TODO_TOOLS", "1"), (PROJECT_DIR_ENV, "-root-lab")]);
    }

    // TS: landmines.test.ts:173
    #[test]
    fn asks_the_cli_about_no_other_login_since_the_placeholder_is_the_only_sign_in_it_may_use() {
        let placeholder = ClaudeEnvOptions { base: Some(base()), brokered: brokered(TOKEN, BrokerKind::Token), ..Default::default() };
        assert_eq!(sign_in_guard(&placeholder, None), "");
        assert_ne!(sign_in_guard(&env_of(&base()), None), "");
    }

    // TS: landmines.test.ts:178
    #[test]
    fn leaves_a_launch_that_is_not_brokered_as_it_was() {
        let vaulted = build_env(&ClaudeEnvOptions { base: Some(base()), oauth_token: Some("sk-ant-oat01-VAULT".into()), ..Default::default() });
        let inherited = build_env(&env_of(&base()));
        for env in [&vaulted, &inherited] {
            for name in ["CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST", "DISABLE_UPDATES", "API_TIMEOUT_MS", "ENABLE_TOOL_SEARCH"] {
                assert!(!env.contains_key(name), "{name}");
            }
            assert_matches_object(env, &[("ANTHROPIC_BASE_URL", "https://gateway.acme.example"), ("ANTHROPIC_AUTH_TOKEN", "gateway-REAL")]);
        }
        assert_eq!(get(&vaulted, "CLAUDE_CODE_OAUTH_TOKEN"), Some("sk-ant-oat01-VAULT"));
        assert_eq!(get(&inherited, "CLAUDE_CODE_OAUTH_TOKEN"), Some("sk-ant-oat01-REAL"));
    }
}

mod build_command_cases {
    use super::*;

    const ID: &str = "11111111-2222-4333-8444-555555555555";
    const SESSION: &str = "e16ed170-8257-4668-879e-fe836341633c";

    fn fresh() -> BuildCommandOptions {
        BuildCommandOptions { session_id: Some(SESSION.to_string()), ..Default::default() }
    }

    fn launch_args(args: &[&str]) -> Option<AgentLaunch> {
        Some(AgentLaunch { program: None, args: Some(args.iter().map(|a| a.to_string()).collect()) })
    }

    // TS: landmines.test.ts:191
    #[test]
    fn a_fast_turn_turns_fast_mode_on_through_the_settings_and_a_turn_that_is_not_fast_carries_none() {
        let fast = command(BuildCommandOptions { session_id: Some(ID.into()), fast: Some(true), ..Default::default() });
        assert!(fast.contains(r#"--settings '{"fastMode":true}'"#), "{fast}");
        assert!(!command(BuildCommandOptions { session_id: Some(ID.into()), ..Default::default() }).contains("--settings"));
    }

    // TS: landmines.test.ts:197
    #[test]
    fn leaves_out_each_server_through_the_clis_denylist_beside_the_persons_own_settings() {
        let cmd = command(BuildCommandOptions {
            session_id: Some(ID.into()),
            off_servers: Some(vec!["cloudflare".into(), "posthog".into()]),
            launch: launch_args(&["--settings", &json!({"theme": "dark"}).to_string()]),
            ..Default::default()
        });
        assert!(cmd.contains(r#"--settings '{"theme":"dark","deniedMcpServers":[{"serverName":"cloudflare"},{"serverName":"posthog"}]}'"#), "{cmd}");
        assert!(!command(BuildCommandOptions { session_id: Some(ID.into()), off_servers: Some(vec![]), ..Default::default() }).contains("--settings"));
    }

    // TS: landmines.test.ts:203
    #[test]
    fn writes_the_off_servers_into_a_managed_drop_in_before_the_launch_and_a_write_that_fails_still_launches() {
        let root = TempRoot::new("wsp-off");
        let bin = root.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let ran = root.join("ran");
        write_stub(&bin.join("claude"), &format!("#!/bin/sh\npwd > {}\n", shell_quote(ran.to_str().unwrap())));
        let managed_dir = Path::new(OFF_SERVERS_FILE).parent().unwrap().to_str().unwrap().to_string();
        let file_name = Path::new(OFF_SERVERS_FILE).file_name().unwrap().to_str().unwrap().to_string();
        let run = |cmd: &str, managed: &Path| {
            let status = Command::new("/bin/bash")
                .arg("-c")
                .arg(cmd.replace(&managed_dir, managed.to_str().unwrap()))
                .env_clear()
                .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
                .stdin(Stdio::null())
                .status()
                .unwrap();
            assert!(status.success(), "{cmd}");
        };
        let cwd = root.path().to_str().unwrap().to_string();
        let managed = root.join("managed-settings.d");
        run(&command(BuildCommandOptions { session_id: Some(ID.into()), cwd: Some(cwd.clone()), off_servers: Some(vec!["cloudflare".into(), "posthog".into()]), ..Default::default() }), &managed);
        let written: Value = serde_json::from_str(&std::fs::read_to_string(managed.join(&file_name)).unwrap()).unwrap();
        assert_eq!(written, json!({"deniedMcpServers": [{"serverName": "cloudflare"}, {"serverName": "posthog"}]}));
        assert_eq!(std::fs::read_to_string(&ran).unwrap().trim(), cwd);
        std::fs::remove_file(&ran).unwrap();
        std::fs::write(root.join("blocked"), "").unwrap();
        run(&command(BuildCommandOptions { session_id: Some(ID.into()), cwd: Some(cwd.clone()), off_servers: Some(vec!["cloudflare".into()]), ..Default::default() }), &root.join("blocked").join("managed-settings.d"));
        assert!(ran.exists());
        run(&command(BuildCommandOptions { session_id: Some(ID.into()), cwd: Some(cwd), off_servers: Some(vec![]), ..Default::default() }), &managed);
        assert!(!managed.join(&file_name).exists());
        assert!(!command(BuildCommandOptions { session_id: Some(ID.into()), ..Default::default() }).contains(OFF_SERVERS_FILE));
    }

    // TS: landmines.test.ts:228
    #[test]
    fn carries_every_required_flag_for_a_fresh_session_and_reads_its_messages_from_stdin_as_stream_json() {
        let cmd = command(fresh());
        assert!(cmd.contains("--output-format stream-json"));
        assert!(cmd.contains("--input-format stream-json"));
        assert!(cmd.contains("--verbose"));
        assert!(cmd.contains("--dangerously-skip-permissions"));
        assert!(cmd.contains(&format!("--session-id {SESSION}")));
        assert!(!cmd.contains("</dev/null"));
        assert!(Regex::new(r"claude -p --").unwrap().is_match(&cmd), "{cmd}");
        assert!(!cmd.contains("--resume"));
    }

    // TS: landmines.test.ts:241
    #[test]
    fn forwards_a_subagents_own_text_and_thinking_only_where_asked() {
        assert!(command(BuildCommandOptions { subagent_text: true, ..fresh() }).contains("--forward-subagent-text"));
        assert!(!command(fresh()).contains("--forward-subagent-text"));
    }

    // TS: landmines.test.ts:246
    #[test]
    fn takes_the_subagent_text_flag_from_2_1_270_on() {
        for version in ["2.1.270", "2.1.288", "2.2.0", "3.0.0"] {
            assert!(forwards_subagent_text(Some(version)), "{version}");
        }
        for version in ["2.1.269", "2.0.99", "1.9.300", "", "not a version"] {
            assert!(!forwards_subagent_text(Some(version)), "{version}");
        }
        assert!(!forwards_subagent_text(None));
    }

    // TS: landmines.test.ts:252
    #[test]
    fn uses_resume_instead_of_session_id_when_resuming() {
        let cmd = command(BuildCommandOptions { resume: Some(SESSION.into()), ..Default::default() });
        assert!(cmd.contains(&format!("--resume {SESSION}")));
        assert!(!cmd.contains("--session-id"));
    }

    // TS: landmines.test.ts:258
    #[test]
    fn prefixes_cd_when_a_cwd_is_given() {
        let cmd = command(BuildCommandOptions { cwd: Some("/root/app".into()), ..fresh() });
        assert!(cmd.starts_with(&format!("cd '/root/app' && {} -p", agent_word("claude", None))), "{cmd}");
    }

    // TS: landmines.test.ts:263
    #[test]
    fn maps_the_picked_model_effort_and_permission_mode_to_the_clis_flags() {
        let cmd = command(BuildCommandOptions { model: Some("claude-opus-5".into()), effort: Some("high".into()), permission_mode: Some("acceptEdits".into()), ..fresh() });
        assert!(cmd.contains("--model 'claude-opus-5'"));
        assert!(cmd.contains("--effort 'high'"));
        assert!(cmd.contains("--permission-mode 'acceptEdits'"));
        assert!(!cmd.contains("--dangerously-skip-permissions"));
    }

    // TS: landmines.test.ts:271
    #[test]
    fn no_permission_mode_and_bypass_permissions_both_skip_permissions_and_still_route_the_question_tool_here() {
        for cmd in [command(fresh()), command(BuildCommandOptions { permission_mode: Some("bypassPermissions".into()), ..fresh() })] {
            assert!(cmd.contains("--dangerously-skip-permissions"));
            assert!(!cmd.contains("--permission-mode"));
            assert!(cmd.contains("--permission-prompt-tool stdio"));
        }
    }

    // TS: landmines.test.ts:281
    #[test]
    fn default_names_itself_and_routes_its_prompts_to_this_host() {
        let cmd = command(BuildCommandOptions { permission_mode: Some("default".into()), ..fresh() });
        assert!(cmd.contains("--permission-mode 'default'"));
        assert!(cmd.contains("--permission-prompt-tool stdio"));
        assert!(!cmd.contains("--dangerously-skip-permissions"));
    }

    // TS: landmines.test.ts:288
    #[test]
    fn every_mode_that_can_prompt_routes_its_prompts_here() {
        for mode in ["acceptEdits", "manual", "dontAsk", "auto"] {
            let cmd = command(BuildCommandOptions { permission_mode: Some(mode.into()), ..fresh() });
            assert!(cmd.contains("--permission-prompt-tool stdio"), "{mode}");
            assert!(cmd.contains(&format!("--permission-mode '{mode}'")), "{mode}");
        }
    }

    // TS: landmines.test.ts:294
    #[test]
    fn a_context_window_rides_the_model_as_the_clis_own_suffix_and_200k_is_the_plain_slug() {
        let with_window = |window: &str| BuildCommandOptions { model: Some("claude-opus-5".into()), context_window: Some(window.into()), ..fresh() };
        assert!(command(with_window("1m")).contains("--model 'claude-opus-5[1m]'"));
        assert!(command(with_window("200k")).contains("--model 'claude-opus-5'"));
        assert!(err_of(with_window("2m")).contains("contextWindow"));
        assert!(err_of(BuildCommandOptions { context_window: Some("1m".into()), ..fresh() }).contains("contextWindow"));
    }

    // TS: landmines.test.ts:301
    #[test]
    fn servers_handed_to_a_turn_ride_one_mcp_config_as_the_json_a_config_file_holds() {
        let args = ["/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json"];
        let cmd = command(BuildCommandOptions { mcp_servers: servers(vec![("wsp", spec("/usr/local/bin/node", &args))]), ..fresh() });
        assert_eq!(mcp_flag_json(&cmd), json!({"mcpServers": {"wsp": {"command": "/usr/local/bin/node", "args": args, "alwaysLoad": true}}}));
        let docs = command(BuildCommandOptions { mcp_servers: servers(vec![("docs", spec("npx", &[]))]), ..fresh() });
        assert_eq!(mcp_flag_json(&docs), json!({"mcpServers": {"docs": {"command": "npx", "args": []}}}));
        assert!(!cmd.contains("--strict-mcp-config"));
        assert!(!command(fresh()).contains("--mcp-config"));
        assert!(!command(BuildCommandOptions { mcp_servers: servers(vec![]), ..fresh() }).contains("--mcp-config"));
    }

    // TS: landmines.test.ts:314
    #[test]
    fn a_launch_with_no_slate_hands_the_wsp_server_without_always_load() {
        let wsp = spec("/usr/local/bin/node", &["/opt/wsp/bin.js", "mcp"]);
        let no_slate = McpServerSpec { no_slate: Some(true), ..wsp.clone() };
        let cmd = command(BuildCommandOptions { mcp_servers: servers(vec![("wsp", no_slate)]), ..fresh() });
        assert_eq!(mcp_flag_json(&cmd), json!({"mcpServers": {"wsp": {"command": "/usr/local/bin/node", "args": ["/opt/wsp/bin.js", "mcp"]}}}));
        let slate = command(BuildCommandOptions { mcp_servers: servers(vec![("wsp", wsp)]), ..fresh() });
        assert_eq!(mcp_flag_json(&slate)["mcpServers"]["wsp"]["alwaysLoad"], json!(true));
    }

    // The word that follows `--append-system-prompt `, its single quotes undone.
    fn brief_of(cmd: &str) -> Option<String> {
        let flag = "--append-system-prompt ";
        let rest = &cmd[cmd.find(flag)? + flag.len()..];
        let mut out = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match c {
                '\'' => {
                    for q in chars.by_ref() {
                        if q == '\'' {
                            break;
                        }
                        out.push(q);
                    }
                }
                '\\' => out.push(chars.next()?),
                ' ' => break,
                other => out.push(other),
            }
        }
        Some(out)
    }

    // TS: landmines.test.ts:322
    #[test]
    fn a_launch_whose_wsp_server_gives_the_thread_a_slate_brings_the_slates_brief_and_no_other_launch_does() {
        let scoped = command(BuildCommandOptions { mcp_servers: servers(vec![("wsp", spec("/opt/wsp", &["mcp", "--scoped"]))]), ..fresh() });
        // INTERFACE GAP: SLATE_BRIEF (packages/protocol wsp-tools.ts:42) has no Rust constant, so the brief is read back
        // off the launch line and checked for the sentences the TS test names.
        let brief = brief_of(&scoped).expect("the launch carries --append-system-prompt");
        assert!(brief.starts_with("wsp slate. This session is a wsp thread with a slate"), "{brief}");
        assert!(brief.contains("your first tool call is mcp__wsp__slate_catalog"));
        assert!(brief.contains("Never read that file or put its value in a tool call."));
        assert!(scoped.contains(&format!("--append-system-prompt {}", shell_quote(&brief))));
        let on_a_box = command(BuildCommandOptions { mcp_servers: servers(vec![("wsp", spec("wsp", &["mcp"]))]), ..fresh() });
        assert!(on_a_box.contains(&format!("--append-system-prompt {}", shell_quote(&brief))));
        let child = McpServerSpec { no_slate: Some(true), ..spec("/opt/wsp", &["mcp", "--scoped", "--no-slate"]) };
        assert!(!command(BuildCommandOptions { mcp_servers: servers(vec![("wsp", child)]), ..fresh() }).contains("--append-system-prompt"));
        assert!(!command(fresh()).contains("--append-system-prompt"));
    }

    // TS: landmines.test.ts:334
    #[test]
    fn sends_no_model_or_effort_flag_when_none_was_picked() {
        let cmd = command(fresh());
        assert!(!cmd.contains("--model"));
        assert!(!cmd.contains("--effort"));
    }

    // TS: landmines.test.ts:340
    #[test]
    fn rejects_a_picked_value_that_is_not_a_plain_slug() {
        assert_eq!(err_of(BuildCommandOptions { model: Some("opus; rm -rf /".into()), ..fresh() }), r#"model must be a plain slug, got "opus; rm -rf /""#);
        assert_eq!(err_of(BuildCommandOptions { effort: Some("".into()), ..fresh() }), r#"effort must be a plain slug, got """#);
        assert_eq!(err_of(BuildCommandOptions { permission_mode: Some("accept edits".into()), ..fresh() }), r#"permissionMode must be a plain slug, got "accept edits""#);
        assert!(command(BuildCommandOptions { model: Some("claude-opus-5[1m]".into()), ..fresh() }).contains("--model 'claude-opus-5[1m]'"));
    }

    // TS: landmines.test.ts:347
    #[test]
    fn rejects_zero_or_two_session_identifiers() {
        assert!(err_of(BuildCommandOptions::default()).contains("exactly one"));
        assert!(err_of(BuildCommandOptions { session_id: Some(SESSION.into()), resume: Some(SESSION.into()), ..Default::default() }).contains("exactly one"));
    }

    // TS: landmines.test.ts:354
    #[test]
    fn rejects_a_session_identifier_that_is_not_a_uuid() {
        assert!(err_of(BuildCommandOptions { resume: Some("$(rm -rf /)".into()), ..Default::default() }).contains("UUID"));
        assert!(err_of(BuildCommandOptions { session_id: Some("abc".into()), ..Default::default() }).contains("UUID"));
    }
}

mod user_message_line_cases {
    use super::*;

    const SESSION: &str = "e16ed170-8257-4668-879e-fe836341633c";

    fn parsed(line: &str) -> Value {
        serde_json::from_str(line).unwrap()
    }

    fn image(media_type: &str, bytes: &str) -> TurnImage {
        TurnImage { media_type: media_type.into(), bytes: bytes.into(), path: None }
    }

    // TS: landmines.test.ts:363
    #[test]
    fn is_one_stream_json_user_line_the_cli_takes_on_stdin_with_the_text_intact() {
        let text = "don't run $(reboot) `id`\nsecond line with \"quotes\"";
        let line = user_message_line(text, SESSION, &[], None);
        assert!(!line.contains('\n'));
        assert_eq!(
            parsed(&line),
            json!({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": text}]}, "parent_tool_use_id": null, "session_id": SESSION})
        );
    }

    // TS: landmines.test.ts:375
    #[test]
    fn carries_an_image_as_a_base64_content_block_ahead_of_the_text() {
        // base64 of "not really a png"
        let bytes = "bm90IHJlYWxseSBhIHBuZw==";
        let line = user_message_line("what does this show?", SESSION, &[image("image/png", bytes)], None);
        assert!(!line.contains('\n'));
        assert_eq!(
            parsed(&line),
            json!({
                "type": "user",
                "message": {"role": "user", "content": [
                    {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": bytes}},
                    {"type": "text", "text": "what does this show?"},
                ]},
                "parent_tool_use_id": null,
                "session_id": SESSION,
            })
        );
    }

    // TS: landmines.test.ts:393
    #[test]
    fn carries_every_image_of_the_message_in_the_order_the_person_added_them_each_with_its_own_type() {
        let line = user_message_line("these three", SESSION, &[image("image/png", "AAA="), image("image/jpeg", "BBB="), image("image/webp", "CCC=")], None);
        let content = parsed(&line)["message"]["content"].as_array().unwrap().clone();
        let types: Vec<&str> = content.iter().map(|b| b["type"].as_str().unwrap()).collect();
        assert_eq!(types, ["image", "image", "image", "text"]);
        let media: Vec<&str> = content[..3].iter().map(|b| b["source"]["media_type"].as_str().unwrap()).collect();
        assert_eq!(media, ["image/png", "image/jpeg", "image/webp"]);
        let data: Vec<&str> = content[..3].iter().map(|b| b["source"]["data"].as_str().unwrap()).collect();
        assert_eq!(data, ["AAA=", "BBB=", "CCC="]);
        assert_eq!(content[3], json!({"type": "text", "text": "these three"}));
    }

    // TS: landmines.test.ts:405
    #[test]
    fn a_message_with_no_image_is_the_one_block_line_it_always_was() {
        let plain = parsed(&user_message_line("plain", SESSION, &[], None));
        assert_eq!(plain, parsed(&user_message_line("plain", SESSION, &[], None)));
        assert_eq!(plain["message"]["content"], json!([{"type": "text", "text": "plain"}]));
    }
}

mod saved_spend {
    use super::*;

    const SESSION: &str = "e16ed170-8257-4668-879e-fe836341633c";

    fn saved(total: Value) -> String {
        json!({"type": "cost-state", "sessionId": SESSION, "totalCostUSD": total, "modelUsage": {}}).to_string()
    }

    fn bash_out(script: &str) -> String {
        let out = Command::new("bash").arg("-c").arg(script).stdin(Stdio::null()).output().unwrap();
        String::from_utf8(out.stdout).unwrap()
    }

    fn printed(test: &str, lines: &[String]) -> String {
        let dir = TempRoot::new(&format!("wsp-saved-spend-{test}"));
        std::fs::create_dir_all(dir.join("projects/-w")).unwrap();
        std::fs::write(dir.join(&format!("projects/-w/{SESSION}.jsonl")), format!("{}\n", lines.join("\n"))).unwrap();
        bash_out(&format!("{}echo launched", saved_spend_command(dir.path().to_str().unwrap(), SESSION)))
    }

    // TS: landmines.test.ts:424
    #[test]
    fn is_the_last_cost_state_line_of_the_sessions_file() {
        let out = printed("last", &[saved(json!(1)), r#"{"type":"user"}"#.to_string(), saved(json!(352.3)), r#"{"type":"last-prompt"}"#.to_string()]);
        assert_eq!(out, format!("{}\nlaunched\n", saved(json!(352.3))));
    }

    // TS: landmines.test.ts:428
    #[test]
    fn is_read_off_the_files_last_bytes_alone() {
        let filler = json!({"type": "user", "text": "x".repeat(SAVED_SPEND_TAIL_BYTES as usize)}).to_string();
        assert_eq!(printed("tail", &[saved(json!(352.3)), filler]), "launched\n");
    }

    // TS: landmines.test.ts:433
    #[test]
    fn prints_nothing_for_a_session_with_no_file() {
        assert_eq!(bash_out(&format!("{}echo launched", saved_spend_command("/nonexistent-config", SESSION))), "launched\n");
    }
}

mod sign_in_order {
    use super::*;

    const RC_KEY: &str = "sk-ant-x-from-zshrc";
    const RC_TOKEN: &str = "sk-ant-oat01-x-from-zshrc";
    const WSP_TOKEN: &str = "sk-ant-oat01-x-from-wsp-vault";
    const PATH_TAIL: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

    #[derive(Clone, Copy)]
    enum Own {
        None,
        Login,
        Helper,
    }

    /// A stand-in claude: `auth status` answers as 2.1.296 prints it, signed in by whatever credential its environment
    /// holds, else by the person's own login or by an apiKeyHelper; any other line reports the credentials it ran with.
    fn stand_in(test: &str, own: Own) -> TempRoot {
        let root = TempRoot::new(&format!("wsp-claude-signin-{test}"));
        let login = root.join("own-login");
        let helper = root.join("helper");
        match own {
            Own::Login => std::fs::write(&login, "").unwrap(),
            Own::Helper => std::fs::write(&helper, "").unwrap(),
            Own::None => {}
        }
        std::fs::write(root.join("asked"), "a diff\n").unwrap();
        let script = [
            "#!/bin/sh".to_string(),
            r#"if [ "$1" = auth ]; then"#.to_string(),
            r#"  if [ -n "$ANTHROPIC_API_KEY$CLAUDE_CODE_OAUTH_TOKEN" ]; then printf '{\n  "loggedIn": true,\n  "authMethod": "api_key"\n}\n';"#.to_string(),
            format!(r#"  elif [ -f "{}" ]; then printf '{{\n  "loggedIn": true,\n  "authMethod": "claude.ai"\n}}\n';"#, login.display()),
            format!(r#"  elif [ -f "{}" ]; then printf '{{\n  "loggedIn": true,\n  "authMethod": "api_key_helper"\n}}\n';"#, helper.display()),
            r#"  else printf '{\n  "loggedIn": false,\n  "authMethod": "none"\n}\n'; fi"#.to_string(),
            "  exit 0".to_string(),
            "fi".to_string(),
            r#"cat > /dev/null; printf '{"type":"result","is_error":false,"result":"key=%s token=%s"}\n' "${ANTHROPIC_API_KEY:-none}" "${CLAUDE_CODE_OAUTH_TOKEN:-none}""#.to_string(),
            String::new(),
        ]
        .join("\n");
        write_stub(&root.join("claude"), &script);
        root
    }

    /// The environment a host on this computer hands a turn: its own, with whatever the login shell lent it.
    fn inherited(bin: &Path, lent: &[(&str, &str)]) -> IndexMap<String, String> {
        let mut env = IndexMap::new();
        env.insert("PATH".to_string(), format!("{}:{PATH_TAIL}", bin.display()));
        env.insert("HOME".to_string(), bin.display().to_string());
        for (k, v) in lent {
            env.insert(k.to_string(), v.to_string());
        }
        env
    }

    fn as_base(env: &IndexMap<String, String>) -> BaseEnv {
        env.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect()
    }

    #[derive(Default, Clone)]
    struct Vault {
        oauth_token: Option<String>,
        api_key: Option<String>,
        brokered: Option<BrokeredSignIn>,
    }

    fn token_vault() -> Vault {
        Vault { oauth_token: Some(WSP_TOKEN.into()), ..Default::default() }
    }

    fn options(env: &IndexMap<String, String>, vault: &Vault) -> ClaudeEnvOptions {
        ClaudeEnvOptions { base: Some(as_base(env)), api_key: vault.api_key.clone(), oauth_token: vault.oauth_token.clone(), brokered: vault.brokered.clone(), project_dir_name: None }
    }

    fn deps(exec: Arc<dyn ExecStreamFactory>, base: &IndexMap<String, String>, vault: &Vault) -> AdapterDeps {
        let mut deps = AdapterDeps::new(exec, format!("{}/.claude", base["HOME"]));
        deps.base_env = Some(as_base(base));
        deps.api_key = vault.api_key.clone();
        deps.oauth_token = vault.oauth_token.clone();
        deps.brokered = vault.brokered.clone();
        deps
    }

    fn reported(stdout: &str, stderr: &str) -> String {
        Regex::new(r#"key=[^ "]+ token=[^ "]+"#).unwrap().find(stdout).map(|m| m.as_str().to_string()).unwrap_or_else(|| format!("no result: {stdout} {stderr}"))
    }

    /// The turn's launch line as the adapter hands it to the exec, run the way this computer runs one: its environment
    /// as the process's own. Answers what the stand-in signed in with.
    fn launch_signs_in_with(base: &IndexMap<String, String>, vault: &Vault) -> String {
        let exec = FakeExec::scripted(vec![], StreamOpts::default());
        let mut d = deps(exec.clone(), base, vault);
        d.result_exit_ms = Some(5);
        d.interrupt_grace_ms = Some(5);
        let adapter = create_claude_adapter(d).unwrap();
        let _session = adapter.start(HarnessStartOptions { prompt: "go".into(), on_event: Some(Arc::new(|_| {})), ..Default::default() }).unwrap();
        let call = exec.calls().into_iter().next().expect("the adapter launched nothing");
        let mut child = Command::new("bash").arg("-c").arg(&call.command).env_clear().envs(call.env.iter()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        drop(child.stdin.take());
        let ran = child.wait_with_output().unwrap();
        reported(&String::from_utf8_lossy(&ran.stdout), &String::from_utf8_lossy(&ran.stderr))
    }

    struct NoTurns;

    impl ExecStreamFactory for NoTurns {
        fn launch(&self, _command: &str, _options: ExecOptions) -> Arc<dyn ExecStream> {
            panic!("no turns here")
        }
    }

    /// bash under the host's own environment, with the question's variables on its input as NAME=value NUL entries.
    struct BashExec {
        base: IndexMap<String, String>,
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl HarnessExec for BashExec {
        fn exec(&self, command: &str, env: Option<&Env>) -> BoxFuture<'static, Result<String, ExecFailure>> {
            let (command, base, seen) = (command.to_string(), self.base.clone(), self.seen.clone());
            let input: String = env.map(|e| e.iter().map(|(k, v)| format!("{k}={v}\0")).collect()).unwrap_or_default();
            async move {
                let out = tokio::task::spawn_blocking(move || {
                    let mut child = Command::new("bash").arg("-c").arg(&command).env_clear().envs(base.iter()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
                    let mut stdin = child.stdin.take().unwrap();
                    let _ = stdin.write_all(input.as_bytes());
                    drop(stdin);
                    String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout).to_string()
                })
                .await
                .unwrap();
                seen.lock().unwrap().push(Regex::new(r#"key=[^ "]+ token=[^ "]+"#).unwrap().find(&out).map(|m| m.as_str().to_string()).unwrap_or_else(|| format!("no result: {out}")));
                Ok(out)
            }
            .boxed()
        }
    }

    /// A title question and a commit-message draft as this computer's exec runs them. Answers what the stand-in signed
    /// in with, which must be the same for both.
    async fn title_signs_in_with(base: &IndexMap<String, String>, vault: &Vault) -> String {
        let adapter = create_claude_adapter(deps(Arc::new(NoTurns), base, &Vault { api_key: None, ..vault.clone() })).unwrap();
        let seen: Arc<Mutex<Vec<String>>> = Default::default();
        let exec: Arc<dyn HarnessExec> = Arc::new(BashExec { base: base.clone(), seen: seen.clone() });
        let _ = adapter.title_for(TitleTurn { opening: "name it".into(), ..Default::default() }, exec.clone()).await;
        let _ = adapter.draft_for(DraftAsk { prompt_file: format!("{}/asked", base["HOME"]), model: None }, exec).await;
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert_eq!(seen[1], seen[0], "the draft signs in as the title does");
        seen[0].clone()
    }

    // TS: sign-in-order.test.ts:105
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wsps_own_token_stands_above_a_key_from_the_login_shell_and_the_two_never_ride_together() {
        let root = stand_in("token-over-key", Own::None);
        let base = inherited(root.path(), &[("ANTHROPIC_API_KEY", RC_KEY)]);
        let vault = token_vault();
        assert_eq!(build_env(&options(&base, &vault)).get("ANTHROPIC_API_KEY"), None);
        assert_eq!(sign_in_guard(&options(&base, &vault), None), "");
        assert_eq!(launch_signs_in_with(&base, &vault), format!("key=none token={WSP_TOKEN}"));
        assert_eq!(title_signs_in_with(&base, &vault).await, format!("key=none token={WSP_TOKEN}"));
    }

    // TS: sign-in-order.test.ts:114
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_brokered_launch_signs_in_with_the_turn_token_alone_above_the_vault_the_persons_own_login_and_the_login_shell() {
        let root = stand_in("brokered", Own::Login);
        let base = inherited(root.path(), &[("CLAUDE_CODE_OAUTH_TOKEN", RC_TOKEN), ("ANTHROPIC_API_KEY", RC_KEY)]);
        let turn = "wsp-turn-x-placeholder";
        let sign_in = Vault { oauth_token: Some(WSP_TOKEN.into()), brokered: brokered(turn, BrokerKind::Login), ..Default::default() };
        assert_eq!(launch_signs_in_with(&base, &sign_in), format!("key=none token={turn}"));
        assert_eq!(title_signs_in_with(&base, &sign_in).await, format!("key=none token={turn}"));
        assert_eq!(launch_signs_in_with(&base, &Vault { brokered: brokered(turn, BrokerKind::Key), ..Default::default() }), format!("key={turn} token=none"));
    }

    // TS: sign-in-order.test.ts:124
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_persons_own_claude_sign_in_stands_above_a_key_or_a_token_from_the_login_shell_which_are_dropped() {
        let root = stand_in("own-login", Own::Login);
        let lents: [Vec<(&str, &str)>; 2] = [vec![("ANTHROPIC_API_KEY", RC_KEY)], vec![("CLAUDE_CODE_OAUTH_TOKEN", RC_TOKEN), ("ANTHROPIC_API_KEY", RC_KEY)]];
        for lent in lents {
            let base = inherited(root.path(), &lent);
            assert_eq!(launch_signs_in_with(&base, &Vault::default()), "key=none token=none");
            assert_eq!(title_signs_in_with(&base, &Vault::default()).await, "key=none token=none");
        }
    }

    // TS: sign-in-order.test.ts:134
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn with_neither_the_key_from_the_login_shell_signs_the_launch_in() {
        let root = stand_in("rc-key", Own::None);
        let base = inherited(root.path(), &[("ANTHROPIC_API_KEY", RC_KEY)]);
        assert_eq!(launch_signs_in_with(&base, &Vault::default()), format!("key={RC_KEY} token=none"));
        assert_eq!(title_signs_in_with(&base, &Vault::default()).await, format!("key={RC_KEY} token=none"));
    }

    // TS: sign-in-order.test.ts:141
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn with_neither_a_token_from_the_login_shell_reaches_the_cli_alone_since_a_key_beside_it_would_win() {
        let root = stand_in("rc-token", Own::None);
        let base = inherited(root.path(), &[("CLAUDE_CODE_OAUTH_TOKEN", RC_TOKEN), ("ANTHROPIC_API_KEY", RC_KEY)]);
        let env = build_env(&options(&base, &Vault::default()));
        assert_eq!(get(&env, "CLAUDE_CODE_OAUTH_TOKEN"), Some(RC_TOKEN));
        assert_eq!(get(&env, "ANTHROPIC_API_KEY"), None);
        assert_eq!(launch_signs_in_with(&base, &Vault::default()), format!("key=none token={RC_TOKEN}"));
        assert_eq!(title_signs_in_with(&base, &Vault::default()).await, format!("key=none token={RC_TOKEN}"));
    }

    // TS: sign-in-order.test.ts:150
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_api_key_helper_leaves_the_key_from_the_login_shell_in_place_since_claude_code_ranks_that_key_above_a_helper() {
        let root = stand_in("helper", Own::Helper);
        let base = inherited(root.path(), &[("ANTHROPIC_API_KEY", RC_KEY)]);
        assert_eq!(launch_signs_in_with(&base, &Vault::default()), format!("key={RC_KEY} token=none"));
        assert_eq!(title_signs_in_with(&base, &Vault::default()).await, format!("key={RC_KEY} token=none"));
    }

    // TS: sign-in-order.test.ts:157
    #[test]
    fn asks_nothing_where_nothing_was_inherited() {
        let root = stand_in("nothing", Own::None);
        assert_eq!(sign_in_guard(&options(&inherited(root.path(), &[]), &Vault::default()), None), "");
    }
}
