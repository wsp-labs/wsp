// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-codex/test/command.test.ts and placeholder-login.test.ts, ported. The placeholder-login cases run a
//! brokered launch in a real shell, the program in place of codex a stand-in that keeps the words it was handed and the
//! environment it ran under, so what is read is the auth.json the launch itself wrote.
//!
//! Not ported here (other files own them): command.test.ts "the probe, a title and a draft run the program and none of the
//! launch words"; placeholder-login.test.ts "a revert or a fork's turn lookup reads no login...", "a revert or a fork's
//! turn lookup names no model address...", "the adapter hands a title its brokered word...", "titles, drafts, renames,
//! probes and conversation reads each run codex through it".

#[path = "common/codex_shell.rs"]
mod codex_shell;

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::time::{SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_adapters::codex::catalog::catalog_probe_command;
use wsp_adapters::codex::command::*;
use wsp_adapters::codex::rpc::{thread_resume_line, thread_start_line, ServiceTier, ThreadOptions};
use wsp_adapters::codex::session_title::title_for_command;
use wsp_adapters::pending::{CodexClaims, McpServerSpec};
use wsp_adapters::port::{agent_word, program_word, AgentLaunch, Env};
use wsp_wire::types::daemon_contract::BROKER_PORT;
use wsp_wire::types::env::{HOST_KEY_ENV, HOST_TOKEN_ENV, HOST_URL_ENV, LAUNCH_ENV, THREAD_ENV, TURN_TOKEN_ENV};
use wsp_wire::types::placeholders::{PLACEHOLDER_KEY_PREFIX, PLACEHOLDER_REFRESH_PREFIX, PLACEHOLDER_SIGNATURE, TURN_CLAIM};
use wsp_wire::types::wsp_tools::WSP_TOOL_TIMEOUT_SEC;

use codex_shell::{run_shell, write_stub, Ran, TempDir};

const NOW: i64 = 1_791_900_000_000;
const TOKEN: &str = "AbC-123_xyz0AbC-123_xyz0AbC-123_x";
const YEAR_S: i64 = 365 * 24 * 60 * 60;
const TOKEN_WORD: &str = "$WSP_HOST_TOKEN";
const DEFAULT_ENV: &[(&str, &str)] = &[(HOST_TOKEN_ENV, TOKEN)];

fn key() -> String {
    format!("{PLACEHOLDER_KEY_PREFIX}{TOKEN}")
}

fn door() -> String {
    format!("http://127.0.0.1:{BROKER_PORT}")
}

fn second_origin() -> String {
    format!("chatgpt_base_url=\"http://localhost:{BROKER_PORT}/backend-api/\"")
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

fn claims() -> CodexClaims {
    CodexClaims {
        account_id: "11111111-2222-4333-8444-555555555555".into(),
        plan_type: Some("plus".into()),
        user_id: Some("user-acme0001".into()),
        fedramp: Some(false),
    }
}

fn chatgpt_with(claims: CodexClaims) -> CodexBroker {
    CodexBroker { mode: BrokerMode::Chatgpt { claims, models: None }, providers: None }
}

fn chatgpt() -> CodexBroker {
    chatgpt_with(claims())
}

fn apikey() -> CodexBroker {
    CodexBroker { mode: BrokerMode::Apikey, providers: None }
}

fn is_apikey(broker: &CodexBroker) -> bool {
    matches!(broker.mode, BrokerMode::Apikey)
}

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => panic!("not an object"),
    }
}

fn providers(items: &[(&str, Value)]) -> Option<IndexMap<String, Map<String, Value>>> {
    Some(items.iter().map(|(k, v)| (k.to_string(), obj(v.clone()))).collect())
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn base(items: &[(&str, &str)]) -> Option<IndexMap<String, Option<String>>> {
    Some(items.iter().map(|(k, v)| (k.to_string(), Some(v.to_string()))).collect())
}

fn turn_options() -> BuildCommandOptions {
    BuildCommandOptions::default()
}

fn servers(items: &[(&str, McpServerSpec)]) -> Option<IndexMap<String, McpServerSpec>> {
    Some(items.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

fn spec(command: &str, args: &[&str]) -> McpServerSpec {
    McpServerSpec { command: command.into(), args: strs(args), no_slate: None }
}

fn program_launch(program: &str) -> AgentLaunch {
    AgentLaunch { program: Some(program.into()), args: None }
}

fn path_env() -> String {
    std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())
}

/// A folder holding the stand-in for codex, which keeps its words and its environment there.
fn stand_in() -> (TempDir, String) {
    let dir = TempDir::new("codex-broker-");
    let d = dir.s();
    let program = write_stub(&dir.join("codex"), &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{d}/args\"\nenv > \"{d}/env\"\n"));
    (dir, program)
}

/// A line run in bash under build_env's environment, as the machine runs it. A key turn is handed the placeholder key as
/// the runtime shapes it from the turn token.
fn run_ran(dir: &TempDir, line: &str, broker: &CodexBroker, env: &[(&str, &str)]) -> Ran {
    let built = build_env(&CodexEnvOptions {
        base: base(env),
        home: dir.join("home").to_string_lossy().into_owned(),
        api_key: if is_apikey(broker) { Some(key()) } else { None },
        broker: Some(broker.clone()),
    })
    .unwrap();
    let mut shell_env: Vec<(String, String)> = vec![("PATH".into(), path_env())];
    shell_env.extend(built);
    run_shell("bash", line, &shell_env, "")
}

fn run(dir: &TempDir, line: &str, broker: &CodexBroker, env: &[(&str, &str)]) -> i32 {
    run_ran(dir, line, broker, env).status
}

struct Launched {
    dir: TempDir,
    command: String,
    status: i32,
}

/// A turn's launch run, returning the folder it ran against.
fn launch(broker: &CodexBroker, env: &[(&str, &str)]) -> Launched {
    let (dir, program) = stand_in();
    let options = BuildCommandOptions {
        cwd: Some(dir.s()),
        broker: Some(broker.clone()),
        launch: Some(program_launch(&program)),
       
        ..Default::default()
    };
    let command = build_command(&options, now_ms()).unwrap();
    let status = run(&dir, &command, broker, env);
    Launched { dir, command, status }
}

/// The launch with the overrides ahead of whatever follows, as the terminal's codex runs it.
fn ahead(program: &str, broker: &CodexBroker) -> String {
    let l = codex_launch(Some(&program_launch(program)), Some(broker), now_ms()).unwrap();
    let mut words = vec![l.word];
    words.extend(l.flags);
    words.join(" ")
}

fn b64url_decode(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => panic!("not base64url: {s}"),
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

/// A JWT read the way codex-cli 0.162.1 reads one (decode_jwt_payload, codex-rs/login/src/token_data.rs): three parts, none
/// empty, the payload base64url with no padding.
fn payload_of(jwt: &str) -> Value {
    let parts: Vec<&str> = jwt.split('.').collect();
    assert_eq!(parts.len(), 3, "{jwt}");
    for part in &parts {
        assert_ne!(*part, "");
    }
    assert_eq!(parts[2], PLACEHOLDER_SIGNATURE);
    assert!(parts[1].chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "{}", parts[1]);
    serde_json::from_slice(&b64url_decode(parts[1])).unwrap()
}

fn login_of(dir: &TempDir) -> Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("home").join("auth.json")).unwrap()).unwrap()
}

fn token_of(login: &Value, name: &str) -> String {
    login["tokens"][name].as_str().unwrap().to_string()
}

fn args_of(dir: &TempDir) -> Vec<String> {
    std::fs::read_to_string(dir.join("args")).unwrap().trim().split('\n').map(String::from).collect()
}

fn env_of(dir: &TempDir) -> HashMap<String, String> {
    std::fs::read_to_string(dir.join("env"))
        .unwrap()
        .trim()
        .split('\n')
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect()
}

fn exists(dir: &TempDir, name: &str) -> bool {
    dir.join(name).exists()
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` as epoch milliseconds.
fn iso_ms(s: &str) -> i64 {
    let n = |a: usize, b: usize| s[a..b].parse::<i64>().unwrap();
    assert!(s.ends_with('Z') && s.len() == 24, "{s}");
    let (y, m, d) = (n(0, 4), n(5, 7), n(8, 10));
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    ((days * 24 + n(11, 13)) * 60 + n(14, 16)) * 60_000 + n(17, 19) * 1000 + n(20, 23)
}

mod build_env_cases {
    use super::*;

    #[test]
    fn sets_codex_home_to_the_home_named_and_keeps_the_base_environment() {
        let env = build_env(&CodexEnvOptions {
            base: Some(IndexMap::from([
                ("PATH".to_string(), Some("/usr/bin".to_string())),
                ("HOME".to_string(), Some("/Users/z".to_string())),
                ("GONE".to_string(), None),
            ])),
            home: "/root/.codex".into(),
            ..Default::default()
        })
        .unwrap();
        let expected: Env = IndexMap::from([
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("HOME".to_string(), "/Users/z".to_string()),
            ("CODEX_HOME".to_string(), "/root/.codex".to_string()),
        ]);
        assert_eq!(env, expected);
    }

    // CODEX_API_KEY is the variable this CLI's own login reads; OPENAI_API_KEY is the catalog's name for the key
    // and what a provider a person configured with env_key reads. A turn that got no key is set neither.
    #[test]
    fn sets_the_api_key_the_vault_handed_it_under_both_names_and_neither_when_it_handed() {
        let env = build_env(&CodexEnvOptions { home: "/root/.codex".into(), api_key: Some("sk-x-fake-openai".into()), ..Default::default() }).unwrap();
        assert_eq!(env.get("CODEX_API_KEY").map(String::as_str), Some("sk-x-fake-openai"));
        assert_eq!(env.get("OPENAI_API_KEY").map(String::as_str), Some("sk-x-fake-openai"));
        let none = build_env(&CodexEnvOptions { home: "/root/.codex".into(), ..Default::default() }).unwrap();
        assert_eq!(none.get("CODEX_API_KEY"), None);
        assert_eq!(none.get("OPENAI_API_KEY"), None);
    }

    #[test]
    fn refuses_a_relative_home() {
        let err = build_env(&CodexEnvOptions { home: ".codex".into(), ..Default::default() }).unwrap_err();
        assert!(err.contains("home must be an absolute path"), "{err}");
    }
}

mod build_command_cases {
    use super::*;

    const UPDATE_PLAN: &str = "-c tools.update_plan.enabled='true'";

    fn build(options: BuildCommandOptions) -> String {
        build_command(&options, NOW).unwrap()
    }

    // The plan tool is off in a headless 0.155.1 unless its config turns it on, and its updates are the steps card.
    #[test]
    fn runs_the_app_server_on_stdio_in_the_guest_home_with_no_prompt_and_no_turn_flags() {
        assert_eq!(build(turn_options()), format!("cd ~ && {} app-server {UPDATE_PLAN}", agent_word("codex", None)));
    }

    #[test]
    fn never_names_a_listener_the_server_speaks_on_the_process_s_own_stdio_and_nothing() {
        let lines = [
            build(turn_options()),
            build(BuildCommandOptions { cwd: Some("/root/w".into()), ..turn_options() }),
            build(BuildCommandOptions { mcp_servers: servers(&[("wsp", spec("/opt/wsp/bin/wsp", &["mcp"]))]), ..turn_options() }),
        ];
        for line in lines {
            assert!(!line.contains("--listen"), "{line}");
            assert!(!line.contains("ws://"), "{line}");
            assert!(!line.contains("unix://"), "{line}");
            // The exec in agentWord's prefix is the sh becoming codex, no codex subcommand.
            assert!(!line.replace(&agent_word("codex", None), "codex").contains("exec"), "{line}");
        }
    }

    #[test]
    fn hands_each_server_named_on_the_launch_to_codex_whole_and_the_wsp_one_the_launch() {
        let command = build(BuildCommandOptions {
            mcp_servers: servers(&[("wsp", spec("/opt/wsp/bin/wsp", &["mcp"])), ("docs", spec("npx", &["-y", "docs-mcp"]))]),
            ..turn_options()
        });
        // A whole entry per server: codex refuses to start on an override that names env_vars for a server its config
        // does not hold (measured on codex-cli 0.155.1: "invalid transport in mcp_servers.wsp").
        assert!(command.contains(r#"-c mcp_servers.wsp.command='"/opt/wsp/bin/wsp"'"#), "{command}");
        assert!(command.contains(r#"-c mcp_servers.wsp.args='["mcp"]'"#), "{command}");
        // Codex clears a server's environment down to its own short list, so the pair is named for it to pass through.
        assert!(command.contains(&format!("-c mcp_servers.wsp.env_vars='{}'", serde_json::to_string(LAUNCH_ENV).unwrap())), "{command}");
        let mut launch_env: Vec<&str> = LAUNCH_ENV.to_vec();
        launch_env.sort();
        let mut named = vec![HOST_KEY_ENV, HOST_TOKEN_ENV, HOST_URL_ENV, TURN_TOKEN_ENV, THREAD_ENV];
        named.sort();
        assert_eq!(launch_env, named);
        assert!(command.contains(r#"-c mcp_servers.docs.args='["-y","docs-mcp"]'"#), "{command}");
        assert!(!command.contains("mcp_servers.docs.env_vars"), "{command}");
        // Codex ends a tool call past its own limit, and a send can wait through a paused Boat's wake before its turn.
        assert!(command.contains(&format!("-c mcp_servers.wsp.tool_timeout_sec='{WSP_TOOL_TIMEOUT_SEC}'")), "{command}");
        assert!(!command.contains("mcp_servers.docs.tool_timeout_sec"), "{command}");
        assert!(command.starts_with(&format!("cd ~ && {} app-server {UPDATE_PLAN} -c ", agent_word("codex", None))), "{command}");
    }

    #[test]
    fn serves_the_slate_tools_again_as_a_server_of_their_own_that_codex_lists_up_front() {
        let command = build(BuildCommandOptions { mcp_servers: servers(&[("wsp", spec("/opt/wsp/bin/wsp", &["mcp"]))]), ..turn_options() });
        let slate = serde_json::to_string(&["slate_catalog", "slate_write", "slate_state", "slate_read"]).unwrap();
        // Codex 0.155.1 keeps every MCP tool behind its search when the model has one, unless the server omits "deferred".
        assert!(command.contains(&format!("-c mcp_servers.wsp.disabled_tools='{slate}'")), "{command}");
        assert!(command.contains(r#"-c mcp_servers.wsp_slate.command='"/opt/wsp/bin/wsp"'"#), "{command}");
        assert!(command.contains(r#"-c mcp_servers.wsp_slate.args='["mcp"]'"#), "{command}");
        assert!(command.contains(&format!("-c mcp_servers.wsp_slate.env_vars='{}'", serde_json::to_string(LAUNCH_ENV).unwrap())), "{command}");
        assert!(command.contains(&format!("-c mcp_servers.wsp_slate.enabled_tools='{slate}'")), "{command}");
        assert!(command.contains(r#"-c mcp_servers.wsp_slate.omit_tools_from='["deferred"]'"#), "{command}");
        let docs = build(BuildCommandOptions { mcp_servers: servers(&[("docs", spec("npx", &[]))]), ..turn_options() });
        assert!(!docs.contains("wsp_slate"), "{docs}");
        // The four slate tools are called without an ask in read-only and workspace-write, each named; no other tool is.
        for tool in ["slate_catalog", "slate_write", "slate_state", "slate_read"] {
            assert!(command.contains(&format!(r#"-c mcp_servers.wsp_slate.tools.{tool}.approval_mode='"approve"'"#)), "{command}");
        }
        assert_eq!(command.matches("approval_mode").count(), 4, "{command}");
        assert!(!command.contains("default_tools_approval_mode"), "{command}");
    }

    #[test]
    fn serves_a_thread_another_thread_started_no_slate_server_and_leaves_its_wsp_one_as() {
        let wsps = [
            McpServerSpec { command: "/opt/wsp/bin/wsp".into(), args: strs(&["mcp", "--scoped", "--no-slate"]), no_slate: Some(true) },
            McpServerSpec { command: "wsp".into(), args: strs(&["mcp"]), no_slate: Some(true) },
        ];
        for wsp in wsps {
            let command = build(BuildCommandOptions { mcp_servers: servers(&[("wsp", wsp)]), ..turn_options() });
            // The server told no slate lists none of the slate's tools, so there is nothing here to hide.
            assert!(!command.contains("disabled_tools"), "{command}");
            assert!(!command.contains("wsp_slate"), "{command}");
            // The mark is the adapter's to read, never a word on the line a box's wsp would have to take.
            assert!(!command.contains("noSlate"), "{command}");
        }
    }

    #[test]
    fn refuses_a_server_name_that_is_not_one_plain_word_of_a_config_key() {
        let err = build_command(&BuildCommandOptions { mcp_servers: servers(&[("a.b", spec("x", &[]))]), ..turn_options() }, NOW).unwrap_err();
        assert!(err.contains("server name"), "{err}");
    }

    #[test]
    fn asks_nothing_of_codex_login_a_provider_configured_on_the_machine_needs_none() {
        assert!(!build(turn_options()).contains("codex login"));
    }

    #[test]
    fn leaves_out_each_off_server_its_config_under_codex_home_still_names() {
        let root = TempDir::new("wsp-codex-off-");
        let bin = root.join("bin");
        std::fs::create_dir(&bin).unwrap();
        std::fs::create_dir(root.join("home")).unwrap();
        write_stub(&bin.join("codex"), "#!/bin/sh\nprintf '%s\\n' \"$@\"\n");
        std::fs::write(
            root.join("home").join("config.toml"),
            "[mcp_servers.cloudflare]\nurl = \"https://mcp.cloudflare.com/mcp\"\n\nmcp_servers.posthog.url = \"https://mcp.posthog.com/mcp\"\n\n# [mcp_servers.gone]\n",
        )
        .unwrap();
        let words = |off_servers: Option<Vec<String>>| -> Vec<String> {
            let line = build_command(&BuildCommandOptions { cwd: Some(root.s()), off_servers, ..turn_options() }, NOW).unwrap();
            let env = vec![
                ("PATH".to_string(), format!("{}:/usr/bin:/bin", bin.to_string_lossy())),
                ("CODEX_HOME".to_string(), root.join("home").to_string_lossy().into_owned()),
            ];
            let ran = run_shell("/bin/bash", &line, &env, "");
            assert_eq!(ran.status, 0, "{}", ran.stderr);
            ran.stdout.trim().split('\n').map(String::from).collect()
        };
        let off = |w: &[String]| -> Vec<String> {
            w.iter().enumerate().filter(|(i, word)| *i > 0 && w[i - 1] == "-c" && word.ends_with(".enabled=false")).map(|(_, word)| word.clone()).collect()
        };
        let named = words(Some(strs(&["cloudflare", "posthog", "gone", "a.b"])));
        assert_eq!(off(&named), strs(&["mcp_servers.cloudflare.enabled=false", "mcp_servers.posthog.enabled=false"]));
        assert_eq!(off(&words(Some(vec![]))), Vec::<String>::new());
        assert_eq!(words(None), words(Some(vec![])));
    }

    #[test]
    fn starts_in_the_folder_named_quoted() {
        let line = build(BuildCommandOptions { cwd: Some("/root/my project".into()), ..turn_options() });
        assert_eq!(line, format!("cd '/root/my project' && {} app-server {UPDATE_PLAN}", agent_word("codex", None)));
    }
}

mod access_params_cases {
    use super::*;

    #[test]
    fn the_two_sandboxed_modes_keep_their_sandbox_and_ask_the_person_when_the_agent_wan() {
        assert_eq!(access_params(Some("read-only")).unwrap(), AccessParams { sandbox: Sandbox::ReadOnly, approval_policy: ApprovalPolicy::OnRequest });
        assert_eq!(access_params(Some("workspace-write")).unwrap(), AccessParams { sandbox: Sandbox::WorkspaceWrite, approval_policy: ApprovalPolicy::OnRequest });
    }

    #[test]
    fn full_access_and_a_turn_that_names_no_mode_runs_with_no_sandbox_and_asks_nobody() {
        assert_eq!(access_params(Some("danger-full-access")).unwrap(), AccessParams { sandbox: Sandbox::DangerFullAccess, approval_policy: ApprovalPolicy::Never });
        assert_eq!(access_params(None).unwrap(), AccessParams { sandbox: Sandbox::DangerFullAccess, approval_policy: ApprovalPolicy::Never });
    }

    #[test]
    fn refuses_any_other_mode_by_the_three_it_takes() {
        let yolo = access_params(Some("yolo")).unwrap_err();
        assert!(yolo.contains("permissionMode must be one of read-only, workspace-write, danger-full-access"), "{yolo}");
        let plan = access_params(Some("plan")).unwrap_err();
        assert!(plan.contains(r#"permissionMode must be one of read-only, workspace-write, danger-full-access, got "plan""#), "{plan}");
    }

    #[test]
    fn a_fast_thread_asks_the_server_for_the_fast_service_tier_and_one_that_is_not_name() {
        let access = access_params(None).unwrap();
        let fast = ThreadOptions { cwd: None, model: None, service_tier: Some(ServiceTier::Fast), access, config: None };
        let plain = ThreadOptions { service_tier: None, ..fast.clone() };
        let start: Value = serde_json::from_str(&thread_start_line(&fast)).unwrap();
        assert_eq!(start["params"]["serviceTier"], json!("fast"));
        let resume: Value = serde_json::from_str(&thread_resume_line(&fast, "t1")).unwrap();
        assert_eq!(resume["params"]["serviceTier"], json!("fast"));
        let none: Value = serde_json::from_str(&thread_start_line(&plain)).unwrap();
        assert!(none["params"].as_object().unwrap().get("serviceTier").is_none(), "{none}");
    }
}

mod a_person_s_setup_for_codex_on_a_computer {
    use super::*;

    fn setup() -> AgentLaunch {
        AgentLaunch { program: Some("/opt/codex".into()), args: Some(strs(&["-c", "model_provider=\"ollama\""])) }
    }

    #[test]
    fn runs_the_program_in_place_of_codex_on_the_server_line_the_launch_words_after_app() {
        let launch = setup();
        let with = build_command(&BuildCommandOptions { launch: Some(launch.clone()), ..turn_options() }, NOW).unwrap();
        let expected = format!("&& {} app-server '-c' 'model_provider=\"ollama\"' -c tools.update_plan.enabled=", agent_word("codex", Some(&launch)));
        assert!(with.contains(&expected), "{with}");
        let without = build_command(&turn_options(), NOW).unwrap();
        assert!(without.contains(&format!("&& {} app-server -c tools.update_plan.enabled=", agent_word("codex", None))), "{without}");
    }
}

mod a_brokered_codex_launch_on_a_chatgpt_login {
    use super::*;

    #[test]
    fn writes_auth_json_as_codex_parses_a_chatgpt_login_the_real_account_id_a_far_exp() {
        let before = now_ms();
        let Launched { dir, command, status } = launch(&chatgpt(), DEFAULT_ENV);
        assert_eq!(status, 0);
        let file = login_of(&dir);
        let c = claims();
        assert_eq!(file["auth_mode"], json!("chatgpt"));
        assert_eq!(file["OPENAI_API_KEY"], Value::Null);
        // Without the account id codex skips reload and refresh on a 401, and a pinned workspace logs the file out.
        assert_eq!(file["tokens"]["account_id"], json!(c.account_id));
        assert_eq!(file["tokens"]["refresh_token"], json!(format!("{PLACEHOLDER_REFRESH_PREFIX}{TOKEN}")));
        let refreshed = iso_ms(file["last_refresh"].as_str().unwrap());
        assert!(refreshed >= before - 1000, "{refreshed} {before}");
        assert!(refreshed <= now_ms(), "{refreshed}");
        for jwt in [token_of(&file, "id_token"), token_of(&file, "access_token")] {
            let claims_in = payload_of(&jwt);
            assert_eq!(
                claims_in["https://api.openai.com/auth"],
                json!({ "chatgpt_account_id": c.account_id, "chatgpt_plan_type": "plus", "chatgpt_user_id": c.user_id, "chatgpt_account_is_fedramp": false })
            );
            assert_eq!(claims_in[TURN_CLAIM], json!(TOKEN));
            assert!(claims_in["exp"].as_i64().unwrap() >= before / 1000 + YEAR_S - 1, "{claims_in}");
        }
        let mode = std::fs::metadata(dir.join("home").join("auth.json")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        // The token is read off the turn's environment where the line runs, so the line names the variable.
        assert!(command.contains(&format!("\"{TOKEN_WORD}\"")), "{command}");
    }

    #[test]
    fn writes_the_account_id_alone_where_the_host_s_login_names_no_plan_user_or_fedramp() {
        let c = CodexClaims { account_id: claims().account_id, ..Default::default() };
        let Launched { dir, .. } = launch(&chatgpt_with(c.clone()), DEFAULT_ENV);
        let payload = payload_of(&token_of(&login_of(&dir), "access_token"));
        assert_eq!(payload["https://api.openai.com/auth"], json!({ "chatgpt_account_id": c.account_id, "chatgpt_account_is_fedramp": false }));
    }

    #[test]
    fn carries_a_fedramp_account_s_flag_in_the_claim_codex_reads_it_from() {
        let c = CodexClaims { fedramp: Some(true), plan_type: Some("enterprise".into()), ..claims() };
        let Launched { dir, .. } = launch(&chatgpt_with(c), DEFAULT_ENV);
        let payload = payload_of(&token_of(&login_of(&dir), "access_token"));
        let auth = &payload["https://api.openai.com/auth"];
        assert_eq!(auth["chatgpt_account_is_fedramp"], json!(true));
        assert_eq!(auth["chatgpt_plan_type"], json!("enterprise"));
    }

    #[test]
    fn points_both_base_urls_at_the_door_under_two_origins_and_sends_refresh_and_revoke() {
        let Launched { dir, .. } = launch(&chatgpt(), DEFAULT_ENV);
        let args = args_of(&dir);
        let d = door();
        assert!(args.contains(&format!("openai_base_url=\"{d}/backend-api/codex\"")), "{args:?}");
        // One origin for both makes codex-cli 0.162.1 send model calls to the origin its account check answers.
        assert!(args.contains(&second_origin()), "{args:?}");
        // A copied config saying keyring makes codex look only there, and a machine has no secret service.
        assert!(args.contains(&"cli_auth_credentials_store=\"file\"".to_string()), "{args:?}");
        assert!(args.contains(&"analytics.enabled=false".to_string()), "{args:?}");
        let env = env_of(&dir);
        assert_eq!(env.get("CODEX_REFRESH_TOKEN_URL_OVERRIDE"), Some(&format!("{d}/oauth/token")));
        assert_eq!(env.get("CODEX_REVOKE_TOKEN_URL_OVERRIDE"), Some(&format!("{d}/oauth/revoke")));
        assert_eq!(env.get("CODEX_API_KEY"), None);
        assert_eq!(env.get("OPENAI_API_KEY"), None);
    }

    #[test]
    fn starts_no_codex_and_writes_no_file_when_the_launch_carries_no_turn_token_on_eith() {
        for broker in [chatgpt(), apikey()] {
            let Launched { dir, status, .. } = launch(&broker, &[]);
            assert_ne!(status, 0);
            assert!(!exists(&dir, "home/auth.json"));
            assert!(!exists(&dir, "args"));
        }
    }
}

mod a_brokered_codex_launch_on_an_api_key {
    use super::*;

    #[test]
    fn writes_the_key_file_codex_login_with_api_key_writes_and_no_key_in_codex_s_enviro() {
        let Launched { dir, command, status } = launch(&apikey(), DEFAULT_ENV);
        assert_eq!(status, 0);
        assert_eq!(login_of(&dir), json!({ "auth_mode": "apikey", "OPENAI_API_KEY": key() }));
        assert!(command.contains(&format!("\"${KEY_FILE_ENV}\"")), "{command}");
        let env = env_of(&dir);
        // app-server reads neither variable; the variable the key file was written from is gone before codex starts.
        assert_eq!(env.get("CODEX_API_KEY"), None);
        assert_eq!(env.get("OPENAI_API_KEY"), None);
        assert_eq!(env.get(KEY_FILE_ENV), None);
        assert!(!env.values().any(|v| *v == key()), "{env:?}");
        assert!(args_of(&dir).contains(&format!("openai_base_url=\"{}/v1\"", door())));
    }

    #[test]
    fn refuses_a_key_beside_the_host_s_chatgpt_login_and_a_key_turn_handed_no_placehold() {
        let beside = build_env(&CodexEnvOptions { home: "/root/.codex".into(), api_key: Some(key()), broker: Some(chatgpt()), ..Default::default() }).unwrap_err();
        assert!(beside.contains("carries no key"), "{beside}");
        // A launch with no turn token carries no key and runs on the key file the last turn left; a turn refuses to start.
        let keyless = build_env(&CodexEnvOptions { home: "/root/.codex".into(), broker: Some(apikey()), ..Default::default() }).unwrap();
        assert!(!keyless.contains_key(KEY_FILE_ENV), "{keyless:?}");
        let (dir, program) = stand_in();
        let handed = build_env(&CodexEnvOptions {
            base: base(DEFAULT_ENV),
            home: dir.join("home").to_string_lossy().into_owned(),
            broker: Some(apikey()),
            ..Default::default()
        })
        .unwrap();
        let mut shell_env: Vec<(String, String)> = vec![("PATH".into(), path_env())];
        shell_env.extend(handed);
        let line = build_command(
            &BuildCommandOptions { cwd: Some(dir.s()), broker: Some(apikey()), launch: Some(program_launch(&program)), ..Default::default() },
            now_ms(),
        )
        .unwrap();
        let ran = run_shell("bash", &line, &shell_env, "");
        assert_ne!(ran.status, 0);
        assert!(ran.stderr.contains("without its placeholder key"), "{}", ran.stderr);
        assert!(!exists(&dir, "home/auth.json"));
        assert!(!exists(&dir, "args"));
        let quoted = build_env(&CodexEnvOptions {
            home: "/root/.codex".into(),
            api_key: Some(format!("{PLACEHOLDER_KEY_PREFIX}\"x")),
            broker: Some(apikey()),
            ..Default::default()
        })
        .unwrap_err();
        assert!(quoted.contains("one plain word"), "{quoted}");
    }
}

mod every_other_codex_launch_on_a_brokered_machine {
    use super::*;

    #[test]
    fn writes_the_placeholder_login_and_runs_codex_with_the_broker_s_overrides_ahead_of() {
        let (dir, program) = stand_in();
        let broker = chatgpt();
        assert_eq!(run(&dir, &format!("{} exec --json -", ahead(&program, &broker)), &broker, DEFAULT_ENV), 0);
        assert_eq!(payload_of(&token_of(&login_of(&dir), "access_token"))[TURN_CLAIM], json!(TOKEN));
        let args = args_of(&dir);
        assert_eq!(&args[args.len() - 3..], &strs(&["exec", "--json", "-"])[..]);
        let d = door();
        let flags = [
            format!("openai_base_url=\"{d}/backend-api/codex\""),
            second_origin(),
            "cli_auth_credentials_store=\"file\"".to_string(),
            "analytics.enabled=false".to_string(),
        ];
        let exec_at = args.iter().position(|a| a == "exec").unwrap();
        for flag in flags {
            let at = args.iter().position(|a| *a == flag);
            assert!(at.is_some(), "{flag} in {args:?}");
            assert!(at.unwrap() < exec_at, "{flag} in {args:?}");
        }
        assert_eq!(env_of(&dir).get("CODEX_REFRESH_TOKEN_URL_OVERRIDE"), Some(&format!("{d}/oauth/token")));
    }

    #[test]
    fn with_no_turn_token_writes_nothing_and_still_runs_at_the_door_on_the_login_the_la() {
        let (dir, program) = stand_in();
        let broker = chatgpt();
        assert_eq!(run(&dir, &format!("{} --version", ahead(&program, &broker)), &broker, &[]), 0);
        assert!(!exists(&dir, "home/auth.json"));
        assert!(args_of(&dir).contains(&format!("openai_base_url=\"{}/backend-api/codex\"", door())));
    }

    #[test]
    fn on_an_api_key_writes_the_key_file_and_leaves_the_key_out_of_codex_s_environment() {
        let (dir, program) = stand_in();
        let broker = apikey();
        assert_eq!(run(&dir, &format!("{} --version", ahead(&program, &broker)), &broker, DEFAULT_ENV), 0);
        assert_eq!(login_of(&dir), json!({ "auth_mode": "apikey", "OPENAI_API_KEY": key() }));
        assert_eq!(env_of(&dir).get(KEY_FILE_ENV), None);
    }

    #[test]
    fn is_the_plain_program_word_on_today_s_road() {
        assert_eq!(codex_word(None, None).unwrap(), program_word("codex", None));
    }

    // Outside a turn the last turn's token is dead: on codex-cli 0.163.0 a probe on that login made 11 calls the door
    // refused 403 at every wake, and one with neither the login nor plugins made none.
    #[test]
    fn a_probe_a_rename_or_a_conversation_read_reads_no_login_and_runs_no_plugin_sync() {
        let cases: Vec<(CodexBroker, Vec<(&str, &str)>)> = vec![(chatgpt(), vec![]), (chatgpt(), DEFAULT_ENV.to_vec()), (apikey(), vec![])];
        for (broker, env) in cases {
            let (dir, program) = stand_in();
            std::fs::create_dir(dir.join("home")).unwrap();
            std::fs::write(dir.join("home").join("auth.json"), "last turn's").unwrap();
            let word = codex_word(Some(&program_launch(&program)), Some(&broker)).unwrap();
            assert_eq!(run(&dir, &format!("{word} app-server"), &broker, &env), 0);
            let args = args_of(&dir);
            assert!(args.contains(&"cli_auth_credentials_store=\"ephemeral\"".to_string()), "{args:?}");
            assert!(!args.contains(&"cli_auth_credentials_store=\"file\"".to_string()), "{args:?}");
            assert!(args.contains(&"features.plugins=false".to_string()), "{args:?}");
            assert_eq!(args.last().unwrap(), "app-server");
            let base_url = if is_apikey(&broker) { "openai_base_url=\"http://127.0.0.1:0/v1\"".to_string() } else { second_origin() };
            assert!(args.contains(&base_url), "{base_url} in {args:?}");
            assert_eq!(std::fs::read_to_string(dir.join("home").join("auth.json")).unwrap(), "last turn's");
            assert_eq!(env_of(&dir).get(KEY_FILE_ENV), None);
        }
    }

    #[test]
    fn a_brokered_probe_asks_nothing_of_the_sign_in_which_the_host_holds() {
        let brokered = catalog_probe_command(None, Some(&chatgpt())).unwrap();
        assert!(!brokered.contains("account/read"), "{brokered}");
        let plain = catalog_probe_command(None, None).unwrap();
        assert!(plain.contains("account/read"), "{plain}");
    }

    // codex-cli 0.162.1 drops every root -c once `exec` names one of its own: a title with the overrides ahead of exec
    // went to chatgpt.com (measured).
    #[test]
    fn a_title_hands_codex_the_broker_s_overrides_after_exec_beside_its_own() {
        let (dir, program) = stand_in();
        let broker = chatgpt();
        let line = title_for_command("name this", None, Some(&program_launch(&program)), Some(&broker), now_ms()).unwrap();
        assert_eq!(run(&dir, &line, &broker, DEFAULT_ENV), 0);
        let args = args_of(&dir);
        assert_eq!(args[0], "exec");
        assert!(args.contains(&format!("openai_base_url=\"{}/backend-api/codex\"", door())), "{args:?}");
        assert!(args.contains(&"cli_auth_credentials_store=\"file\"".to_string()), "{args:?}");
        assert!(args.contains(&"sandbox_mode=\"read-only\"".to_string()), "{args:?}");
        assert_eq!(payload_of(&token_of(&login_of(&dir), "access_token"))[TURN_CLAIM], json!(TOKEN));
    }
}

mod the_person_s_own_providers_on_a_brokered_launch {
    use super::*;

    fn flags_of(id: &str, entry: Value, mode: BrokerKind) -> ProviderFlags {
        provider_flags(id, &obj(entry), mode, &door()).unwrap()
    }

    // codex merges a -c table into the copied entry (measured on 0.162.1): an env_key beside the copy's auth command
    // left both and codex refused the whole config, so each entry keeps its own credential field.
    #[test]
    fn moves_an_entry_to_the_door_and_gives_the_credential_field_it_holds_the_placehold() {
        let by_auth = flags_of("acme", json!({ "base_url": "https://gw.acme.dev/v1", "auth": { "command": "acme-token" } }), BrokerKind::Apikey);
        let auth_args = serde_json::to_string(&json!(["-c", format!("printf %s \"{PLACEHOLDER_KEY_PREFIX}{TOKEN_WORD}\"")])).unwrap();
        assert_eq!(
            by_auth.flags,
            vec![
                format!("-c model_providers.acme.base_url='\"{}/v1\"'", door()),
                "-c model_providers.acme.auth.command='\"sh\"'".to_string(),
                format!("-c model_providers.acme.auth.args='{auth_args}'"),
            ]
        );
        let by_bearer = flags_of("acme", json!({ "base_url": "https://gw.acme.dev/v1", "experimental_bearer_token": "tok-1" }), BrokerKind::Apikey);
        let bearer = format!("-c model_providers.acme.experimental_bearer_token='\"{PLACEHOLDER_KEY_PREFIX}'\"{TOKEN_WORD}\"'\"'");
        assert!(by_bearer.flags.contains(&bearer), "{:?}", by_bearer.flags);
        let by_key = flags_of("acme", json!({ "base_url": "https://gw.acme.dev/v1", "env_key": "ACME_KEY" }), BrokerKind::Apikey);
        let env_key = format!("-c model_providers.acme.env_key='\"{PROVIDER_KEY_ENV}\"'");
        assert!(by_key.flags.contains(&env_key), "{:?}", by_key.flags);
    }

    #[test]
    fn each_credential_field_run_in_a_shell_holds_the_placeholder_key_and_codex_s_words() {
        let broker = CodexBroker {
            mode: BrokerMode::Chatgpt { claims: claims(), models: None },
            providers: providers(&[
                ("by_auth", json!({ "base_url": "https://gw.acme.dev/v1", "auth": { "command": "acme-token" }, "http_headers": { "X-Acme-Key": "s3cret" } })),
                ("by_bearer", json!({ "base_url": "https://gw.acme.dev/v1", "experimental_bearer_token": "tok-1" })),
                ("by_key", json!({ "base_url": "https://gw.acme.dev/v1", "env_key": "ACME_KEY" })),
            ]),
        };
        let Launched { dir, .. } = launch(&broker, DEFAULT_ENV);
        let args = args_of(&dir);
        assert!(args.contains(&format!("model_providers.by_bearer.experimental_bearer_token=\"{}\"", key())), "{args:?}");
        assert!(args.contains(&"model_providers.by_auth.http_headers={\"X-Acme-Key\"=\"\"}".to_string()), "{args:?}");
        assert!(!args.join("\n").contains("s3cret"));
        assert!(!args.join("\n").contains("tok-1"));
        assert_eq!(env_of(&dir).get(PROVIDER_KEY_ENV), Some(&key()));
        // The auth command codex runs prints the key off the environment it runs in.
        let flag = args.iter().find(|a| a.starts_with("model_providers.by_auth.auth.args=")).unwrap();
        let words: Vec<String> = serde_json::from_str(&flag[flag.find('=').unwrap() + 1..]).unwrap();
        assert_eq!(words[0], "-c");
        let ran = run_shell("sh", &words[1], &[(HOST_TOKEN_ENV.to_string(), TOKEN.to_string())], "");
        assert_eq!(ran.stdout, key());
    }

    #[test]
    fn names_an_entry_codex_read_as_azure_by_its_address_azure_and_leaves_a_name_that_a() {
        let work = flags_of(
            "work",
            json!({ "name": "Work", "base_url": "https://acme.openai.azure.com/openai", "env_key": "AZURE_OPENAI_API_KEY" }),
            BrokerKind::Apikey,
        );
        assert!(work.flags.contains(&"-c model_providers.work.name='\"azure\"'".to_string()), "{:?}", work.flags);
        let named = flags_of("work", json!({ "name": "Azure", "base_url": "https://example.com" }), BrokerKind::Apikey);
        assert!(!named.flags.join(" ").contains(".name="), "{:?}", named.flags);
    }

    #[test]
    fn sends_an_entry_that_asks_for_openai_s_own_sign_in_down_the_placeholder_login_nam() {
        let corp = flags_of("corp", json!({ "base_url": "https://gw.acme.dev/backend-api/codex", "requires_openai_auth": true }), BrokerKind::Chatgpt);
        assert_eq!(corp.flags, vec![format!("-c model_providers.corp.base_url='\"{}/backend-api/codex\"'", door())]);
        assert!(!corp.keyed);
    }

    #[test]
    fn leaves_an_aws_signed_entry_on_today_s_road_and_exports_no_placeholder_key_when_n() {
        let broker = CodexBroker {
            mode: BrokerMode::Apikey,
            providers: providers(&[("bedrock", json!({ "name": "Bedrock", "aws": { "region": "us-east-1" } }))]),
        };
        let Launched { dir, command, .. } = launch(&broker, DEFAULT_ENV);
        assert!(!command.contains("model_providers"), "{command}");
        assert_eq!(env_of(&dir).get(PROVIDER_KEY_ENV), None);
    }

    #[test]
    fn refuses_a_provider_id_that_is_not_one_plain_word_of_a_config_key() {
        let broker = CodexBroker {
            mode: BrokerMode::Apikey,
            providers: providers(&[("acme.gw", json!({ "base_url": "https://gw.acme.dev/v1" }))]),
        };
        let err = build_command(&BuildCommandOptions { broker: Some(broker), ..Default::default() }, NOW).unwrap_err();
        assert!(err.contains("one plain word"), "{err}");
    }
}

mod codex_typed_in_a_terminal_on_a_brokered_machine {
    use super::*;

    /// `line` typed in a shell whose PATH finds codex's program for the shell first and the stand-in for codex after it,
    /// under the environment the shell is opened with.
    fn typed(broker: &CodexBroker, line: &str, env: &[(&str, &str)]) -> (TempDir, i32) {
        let (dir, _program) = stand_in();
        let bin = dir.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let bin_s = bin.to_string_lossy().into_owned();
        write_stub(&bin.join("codex"), &codex_for_shell(broker, &bin_s, now_ms()).unwrap());
        let has_token = env.iter().any(|(k, _)| *k == HOST_TOKEN_ENV);
        let api_key = if is_apikey(broker) && has_token { Some(key()) } else { None };
        let mut shell_env: IndexMap<String, String> = IndexMap::new();
        shell_env.insert("CODEX_HOME".into(), dir.join("home").to_string_lossy().into_owned());
        for (k, v) in env {
            shell_env.insert(k.to_string(), v.to_string());
        }
        shell_env.insert("PATH".into(), format!("{bin_s}:{}:{}", dir.s(), path_env()));
        for (k, v) in brokered_codex_env(broker, api_key.as_deref()).unwrap() {
            shell_env.insert(k, v);
        }
        let pairs: Vec<(String, String)> = shell_env.into_iter().collect();
        let status = run_shell("sh", line, &pairs, "").status;
        (dir, status)
    }

    #[test]
    fn runs_the_real_codex_at_the_door_on_the_shell_s_token_with_what_was_typed_after_t() {
        let (dir, status) = typed(&chatgpt(), "codex exec \"say ok\"", DEFAULT_ENV);
        assert_eq!(status, 0);
        let args = args_of(&dir);
        assert_eq!(&args[args.len() - 3..], &strs(&["exec", "--skip-git-repo-check", "say ok"])[..]);
        let d = door();
        assert!(args.contains(&format!("openai_base_url=\"{d}/backend-api/codex\"")), "{args:?}");
        assert!(args.contains(&second_origin()), "{args:?}");
        assert!(args.contains(&"cli_auth_credentials_store=\"file\"".to_string()), "{args:?}");
        assert_eq!(payload_of(&token_of(&login_of(&dir), "access_token"))[TURN_CLAIM], json!(TOKEN));
        // The real codex is found past the program's own folder, which it takes off the PATH codex inherits.
        let env = env_of(&dir);
        let bin = dir.join("bin").to_string_lossy().into_owned();
        assert!(!env.get("PATH").unwrap().split(':').any(|p| p == bin), "{env:?}");
        assert_eq!(env.get("CODEX_REFRESH_TOKEN_URL_OVERRIDE"), Some(&format!("{d}/oauth/token")));
    }

    #[test]
    fn runs_exec_in_a_folder_that_holds_no_repo_which_codex_s_exec_refuses_without_its() {
        for word in ["exec", "e"] {
            let (dir, status) = typed(&chatgpt(), &format!("codex {word} --json \"say ok\""), DEFAULT_ENV);
            assert_eq!(status, 0);
            let args = args_of(&dir);
            assert_eq!(&args[args.len() - 4..], &strs(&[word, "--skip-git-repo-check", "--json", "say ok"])[..]);
        }
        for line in ["codex exec --skip-git-repo-check \"say ok\"", "codex e resume --last --skip-git-repo-check"] {
            let (dir, status) = typed(&chatgpt(), line, DEFAULT_ENV);
            assert_eq!(status, 0);
            assert_eq!(args_of(&dir).iter().filter(|a| *a == "--skip-git-repo-check").count(), 1);
        }
        for line in ["codex", "codex --version", "codex review \"say ok\""] {
            let (dir, status) = typed(&chatgpt(), line, DEFAULT_ENV);
            assert_eq!(status, 0);
            assert!(!args_of(&dir).contains(&"--skip-git-repo-check".to_string()));
        }
    }

    #[test]
    fn writes_the_key_file_from_the_shell_s_placeholder_key_and_leaves_the_key_out_of_c() {
        let (dir, status) = typed(&apikey(), "codex --version", DEFAULT_ENV);
        assert_eq!(status, 0);
        assert_eq!(login_of(&dir), json!({ "auth_mode": "apikey", "OPENAI_API_KEY": key() }));
        assert_eq!(env_of(&dir).get(KEY_FILE_ENV), None);
        assert!(args_of(&dir).contains(&format!("openai_base_url=\"{}/v1\"", door())));
    }

    #[test]
    fn writes_the_login_where_codex_itself_looks_in_a_shell_that_names_no_codex_home() {
        let home = TempDir::new("codex-home-");
        let home_s = home.s();
        let (dir, status) = typed(&chatgpt(), "codex exec \"say ok\"", &[(HOST_TOKEN_ENV, TOKEN), ("HOME", &home_s), ("CODEX_HOME", "")]);
        assert_eq!(status, 0);
        let login: Value = serde_json::from_str(&std::fs::read_to_string(home.join(".codex").join("auth.json")).unwrap()).unwrap();
        assert_eq!(payload_of(&token_of(&login, "access_token"))[TURN_CLAIM], json!(TOKEN));
        assert_eq!(env_of(&dir).get("CODEX_HOME"), Some(&home.join(".codex").to_string_lossy().into_owned()));
    }

    #[test]
    fn runs_codex_on_the_login_file_already_there_writing_none_in_a_shell_that_carries() {
        let (dir, status) = typed(&chatgpt(), "codex", &[]);
        assert_eq!(status, 0);
        assert!(!exists(&dir, "home/auth.json"));
        assert!(args_of(&dir).contains(&"cli_auth_credentials_store=\"file\"".to_string()));
    }
}
