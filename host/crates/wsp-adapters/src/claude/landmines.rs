// SPDX-License-Identifier: AGPL-3.0-only
//! Claude Code's launch: its environment, its sign-in order and its command line. TS:
//! packages/adapter-claude/src/landmines.ts at a3b870a6e. Environments are [`Env`]s whose key order is the TS
//! object's: a key set again keeps its first place, a key deleted and set again goes last.

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::claude::permissions::{PERMISSION_PROMPT_TOOL, SKIP_PROMPTS_MODE};
use crate::pending::{in_folder, launch_has_slate, shell_line, shell_quote, McpServerSpec};
use crate::port::{agent_word, program_word, AdapterError, AgentLaunch, Env, ExecSecret, ServerValues, TurnImage};
use wsp_wire::text::json_stringify;
use wsp_wire::types::daemon_contract::BROKER_PORT;
use wsp_wire::types::env::PLUGIN_SEED_ENV;
use wsp_wire::types::wsp_tools::{MCP_SERVER_NAME, SLATE_BRIEF};

/// A login environment as the runtime hands it: a `None` value is a variable named and unset (TS `undefined`).
pub type BaseEnv = IndexMap<String, Option<String>>;

const TOKEN_ENV: &str = "CLAUDE_CODE_OAUTH_TOKEN";
const HEADLESS_OVERRIDES: [(&str, &str); 5] = [
    ("CLAUDE_CODE_AUTO_CONNECT_IDE", "0"),
    ("CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL", "1"),
    ("CLAUDE_CODE_ENABLE_TODO_TOOLS", "1"),
    ("CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS", "0"),
    ("CLAUDE_CODE_MCP_STARTUP_WAIT_MS", "0"),
];
const BROKERED_DROP: [&str; 5] =
    ["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "ANTHROPIC_CUSTOM_HEADERS", "ANTHROPIC_FOUNDRY_API_KEY", "AWS_BEARER_TOKEN_BEDROCK"];

/// landmines.ts:12: `^CLAUDE_CODE_`, `^CLAUDECODE$`, `^FORCE_CODE_TERMINAL$`. Whether a variable is a nesting mark.
pub fn is_landmine(key: &str) -> bool {
    key.starts_with("CLAUDE_CODE_") || key == "CLAUDECODE" || key == "FORCE_CODE_TERMINAL"
}

/// landmines.ts:41.
pub const FIRST_BYTE_ENV: &str = "CLAUDE_STREAM_FIRST_BYTE_TIMEOUT_MS";
/// landmines.ts:47.
pub const PROJECT_DIR_ENV: &str = "CLAUDE_CODE_PROJECT_DIR_NAME";
/// landmines.ts:159.
pub const KEY_ENV: &str = "ANTHROPIC_API_KEY";

/// landmines.ts:53.
pub fn terminal_resume_command(base: Option<&BaseEnv>, launch: Option<&AgentLaunch>) -> String {
    let dir = base.and_then(|b| b.get("CLAUDE_CONFIG_DIR")).and_then(|v| v.as_deref());
    let prefix = dir.map_or(String::new(), |dir| format!("CLAUDE_CONFIG_DIR={} ", shell_line(&[dir.to_string()])));
    format!("{prefix}{} --resume", program_word("claude", launch))
}

/// The kind of credential the host adds at the door. TS `BrokerStatus["claude"]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrokerKind {
    Login,
    Token,
    Key,
    Gateway,
}

/// landmines.ts:78.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokeredSignIn {
    pub token: String,
    pub kind: BrokerKind,
}

/// landmines.ts:58.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClaudeEnvOptions {
    pub base: Option<BaseEnv>,
    pub api_key: Option<String>,
    pub oauth_token: Option<String>,
    pub project_dir_name: Option<String>,
    pub brokered: Option<BrokeredSignIn>,
}

/// landmines.ts:84: `http://127.0.0.1:<BROKER_PORT>`.
pub fn broker_door_url() -> String {
    format!("http://127.0.0.1:{BROKER_PORT}")
}

/// landmines.ts:104, keys in that order.
pub fn brokered_sign_in_env(b: &BrokeredSignIn) -> Env {
    let mut env = Env::new();
    env.insert("ANTHROPIC_BASE_URL".into(), broker_door_url());
    match b.kind {
        BrokerKind::Login | BrokerKind::Token => {
            env.insert(TOKEN_ENV.into(), b.token.clone());
        }
        BrokerKind::Key => {
            env.insert(KEY_ENV.into(), b.token.clone());
        }
        BrokerKind::Gateway => {
            env.insert("ANTHROPIC_AUTH_TOKEN".into(), b.token.clone());
            env.insert(KEY_ENV.into(), String::new());
        }
    }
    for (name, value) in [
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
        ("CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST", "1"),
        ("DISABLE_UPDATES", "1"),
        ("API_TIMEOUT_MS", "3600000"),
    ] {
        env.insert(name.into(), value.into());
    }
    if b.kind != BrokerKind::Gateway {
        env.insert("ENABLE_TOOL_SEARCH".into(), "true".into());
    }
    env
}

/// landmines.ts:116. Drops unset values and nesting marks, keeping PLUGIN_SEED_ENV.
pub fn strip_landmine_env(base: &BaseEnv) -> Env {
    base.iter()
        .filter_map(|(key, value)| {
            let value = value.as_ref()?;
            (key == PLUGIN_SEED_ENV || !is_landmine(key)).then(|| (key.clone(), value.clone()))
        })
        .collect()
}

fn vaulted(options: &ClaudeEnvOptions) -> bool {
    options.brokered.is_some() || options.api_key.is_some() || options.oauth_token.is_some()
}

/// A token or key the launch inherited: the token where both are, since inside the CLI the key outranks it.
fn inherited_sign_in(base: &BaseEnv) -> Option<(&'static str, String)> {
    let present = |key: &str| base.get(key).and_then(|v| v.as_deref()).filter(|v| !v.is_empty()).map(String::from);
    present(TOKEN_ENV).map(|token| (TOKEN_ENV, token)).or_else(|| present(KEY_ENV).map(|key| (KEY_ENV, key)))
}

/// landmines.ts:137.
pub fn build_env(options: &ClaudeEnvOptions) -> Env {
    let raw = options.base.clone().unwrap_or_default();
    let mut base = strip_landmine_env(&raw);
    base.shift_remove(KEY_ENV);
    let mut env = Env::new();
    if let Some(brokered) = &options.brokered {
        for name in BROKERED_DROP {
            base.shift_remove(name);
        }
        env.extend(base);
        env.extend(HEADLESS_OVERRIDES.map(|(k, v)| (k.to_string(), v.to_string())));
        env.extend(brokered_sign_in_env(brokered));
    } else {
        env.insert(FIRST_BYTE_ENV.into(), "30000".into());
        env.extend(base);
        env.extend(HEADLESS_OVERRIDES.map(|(k, v)| (k.to_string(), v.to_string())));
        if !vaulted(options) {
            if let Some((name, value)) = inherited_sign_in(&raw) {
                env.insert(name.into(), value);
            }
        }
        if let Some(key) = &options.api_key {
            env.insert(KEY_ENV.into(), key.clone());
        }
        if let Some(token) = &options.oauth_token {
            env.insert(TOKEN_ENV.into(), token.clone());
        }
    }
    if let Some(name) = &options.project_dir_name {
        env.insert(PROJECT_DIR_ENV.into(), name.clone());
    }
    env
}

/// landmines.ts:180. `""` where the vault signs in or nothing was inherited.
pub fn sign_in_guard(options: &ClaudeEnvOptions, launch: Option<&AgentLaunch>) -> String {
    let raw = options.base.clone().unwrap_or_default();
    if vaulted(options) || inherited_sign_in(&raw).is_none() {
        return String::new();
    }
    let status = format!("env -u {KEY_ENV} -u {TOKEN_ENV} {} auth status 2>/dev/null", program_word("claude", launch));
    format!(
        "wsp_auth=$({status}); case \"$wsp_auth\" in *'\"authMethod\": \"api_key_helper\"'*) ;; *'\"loggedIn\": true'*) unset {KEY_ENV} {TOKEN_ENV} ;; esac; unset wsp_auth; "
    )
}

/// landmines.ts:191. A random v4 UUID, lower case.
pub fn new_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// landmines.ts:195, case-insensitive.
pub fn is_uuid(id: &str) -> bool {
    id.len() == 36 && id.bytes().enumerate().all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { c == b'-' } else { c.is_ascii_hexdigit() })
}

/// landmines.ts:197.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildCommandOptions {
    pub session_id: Option<String>,
    pub resume: Option<String>,
    pub copy_of: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub context_window: Option<String>,
    pub name: Option<String>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub server_values: bool,
    pub off_servers: Option<Vec<String>>,
    pub fast: Option<bool>,
    pub memory_dir: Option<String>,
    pub launch: Option<AgentLaunch>,
    pub subagent_text: bool,
    pub aside: bool,
}

/// landmines.ts:243.
pub fn memory_settings(dir: Option<&str>) -> Map<String, Value> {
    let mut settings = Map::new();
    if let Some(dir) = dir {
        settings.insert("autoMemoryDirectory".into(), Value::String(dir.into()));
    }
    settings
}

/// landmines.ts:252.
pub const OFF_SERVERS_FILE: &str = "/etc/claude-code/managed-settings.d/wsp-servers-off.json";

/// landmines.ts:260.
pub fn settings_flag(settings: &Map<String, Value>) -> Vec<String> {
    if settings.is_empty() {
        return vec![];
    }
    vec![format!("--settings {}", shell_quote(&json_stringify(&Value::Object(settings.clone()))))]
}

/// The person's launch words less a JSON --settings, and that object. landmines.ts:266.
#[derive(Clone, Debug, PartialEq)]
pub struct PersonSettings {
    pub words: Vec<String>,
    pub settings: Map<String, Value>,
    pub file: bool,
}

/// landmines.ts:266.
pub fn person_settings(args: &[String]) -> PersonSettings {
    let mut words = Vec::new();
    let mut settings = Map::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let joined = arg.starts_with("--settings=");
        if arg != "--settings" && !joined {
            words.push(arg.to_string());
            i += 1;
            continue;
        }
        let value = if joined {
            Some(&arg["--settings=".len()..])
        } else {
            i += 1;
            args.get(i).map(String::as_str)
        };
        match value.and_then(wsp_words::v8_json::parse_json_lax) {
            Some(Value::Object(object)) => settings = object,
            _ => return PersonSettings { words: args.to_vec(), settings: Map::new(), file: true },
        }
        i += 1;
    }
    PersonSettings { words, settings, file: false }
}

/// The person's launch words less every --settings and the value it names.
fn without_settings(args: &[String]) -> Vec<String> {
    let mut words = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--settings" {
            i += 1;
        } else if !args[i].starts_with("--settings=") {
            words.push(args[i].clone());
        }
        i += 1;
    }
    words
}

/// landmines.ts:303. True from 2.1.270 on; false where no version reads.
pub fn forwards_subagent_text(version: Option<&str>) -> bool {
    let Some(version) = version else { return false };
    let mut rest = version;
    let mut parts = [0f64; 3];
    for (i, part) in parts.iter_mut().enumerate() {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return false;
        }
        *part = rest[..digits].parse().unwrap_or(f64::INFINITY);
        rest = &rest[digits..];
        if i < 2 {
            match rest.strip_prefix('.') {
                Some(after) => rest = after,
                None => return false,
            }
        }
    }
    for (part, want) in parts.iter().zip([2.0, 1.0, 270.0]) {
        if *part != want {
            return *part > want;
        }
    }
    true
}

/// landmines.ts:313. `Err("<name> must be a plain slug, got \"<value>\"")`.
pub fn slug_flag(flag: &str, name: &str, value: Option<&str>) -> Result<Vec<String>, AdapterError> {
    let Some(value) = value else { return Ok(vec![]) };
    let mut chars = value.chars();
    let plain = chars.next().is_some_and(|c| c.is_ascii_alphanumeric()) && chars.all(|c| c.is_ascii_alphanumeric() || "._:[]-".contains(c));
    if !plain {
        return Err(AdapterError(format!("{name} must be a plain slug, got \"{value}\"")));
    }
    Ok(vec![format!("{flag} {}", shell_quote(value))])
}

fn model_with_context(model: Option<&str>, window: Option<&str>) -> Result<Option<String>, AdapterError> {
    match (model, window) {
        (model, None) => Ok(model.map(String::from)),
        (None, Some(_)) => Err(AdapterError("contextWindow needs a model to ride on".into())),
        (Some(model), Some("200k")) => Ok(Some(model.to_string())),
        (Some(model), Some("1m")) => Ok(Some(format!("{model}[1m]"))),
        (Some(_), Some(window)) => Err(AdapterError(format!("contextWindow must be \"200k\" or \"1m\", got \"{window}\""))),
    }
}

/// landmines.ts:327. Bypass, and no mode at all, skip permissions outright; every other mode names itself.
fn permission_flags(mode: Option<&str>) -> Result<Vec<String>, AdapterError> {
    let prompts = format!("--permission-prompt-tool {PERMISSION_PROMPT_TOOL}");
    match mode {
        None => Ok(vec!["--dangerously-skip-permissions".into(), prompts]),
        Some(mode) if mode == SKIP_PROMPTS_MODE => Ok(vec!["--dangerously-skip-permissions".into(), prompts]),
        Some(_) => {
            let mut flags = slug_flag("--permission-mode", "permissionMode", mode)?;
            flags.push(prompts);
            Ok(flags)
        }
    }
}

/// landmines.ts:342.
fn brief_flag(servers: Option<&IndexMap<String, McpServerSpec>>) -> Vec<String> {
    if launch_has_slate(servers) {
        vec![format!("--append-system-prompt {}", shell_quote(SLATE_BRIEF))]
    } else {
        vec![]
    }
}

/// landmines.ts:349.
pub const SERVER_VALUES_ENV: &str = "WSP_MCP_VALUES";

/// landmines.ts:353: one file under SERVER_VALUES_ENV holding `{"mcpServers": entries}`; `None` with no entries.
pub fn server_values_file(values: Option<&ServerValues>) -> Option<ExecSecret> {
    let entries = values?.entries.as_ref().filter(|entries| !entries.is_empty())?;
    let servers: Map<String, Value> = entries
        .iter()
        .map(|(name, entry)| (name.clone(), Value::Object(entry.iter().map(|(k, v)| (k.clone(), v.clone())).collect())))
        .collect();
    let mut files = Env::new();
    files.insert(SERVER_VALUES_ENV.into(), json_stringify(&json!({"mcpServers": servers})));
    Some(ExecSecret { files: Some(files), input: false })
}

/// landmines.ts:362.
pub fn mcp_config_flag(servers: Option<&IndexMap<String, McpServerSpec>>) -> Vec<String> {
    let Some(servers) = servers.filter(|servers| !servers.is_empty()) else { return vec![] };
    let up_front = launch_has_slate(Some(servers));
    let mut mcp_servers = Map::new();
    for (name, spec) in servers {
        let mut entry = Map::new();
        entry.insert("command".into(), Value::String(spec.command.clone()));
        entry.insert("args".into(), json!(spec.args));
        if up_front && name == MCP_SERVER_NAME {
            entry.insert("alwaysLoad".into(), Value::Bool(true));
        }
        mcp_servers.insert(name.clone(), Value::Object(entry));
    }
    vec![format!("--mcp-config {}", shell_quote(&json_stringify(&json!({"mcpServers": mcp_servers}))))]
}

/// The settings that leave the named servers out of a launch. TS landmines.ts:246.
fn denied_servers(names: Option<&[String]>) -> Map<String, Value> {
    let mut settings = Map::new();
    if let Some(names) = names.filter(|names| !names.is_empty()) {
        settings.insert("deniedMcpServers".into(), Value::Array(names.iter().map(|name| json!({"serverName": name})).collect()));
    }
    settings
}

/// The step ahead of a launch that leaves `names` out, run before its cd. landmines.ts:256.
fn off_servers_step(names: Option<&[String]>) -> String {
    const DIR: &str = "/etc/claude-code/managed-settings.d";
    match names {
        None => String::new(),
        Some([]) => format!("rm -f {OFF_SERVERS_FILE} 2>/dev/null; "),
        Some(_) => {
            let body = json_stringify(&Value::Object(denied_servers(names)));
            format!("{{ mkdir -p {DIR} && printf '%s' {} > {OFF_SERVERS_FILE}; }} 2>/dev/null; ", shell_quote(&body))
        }
    }
}

/// landmines.ts:379.
pub fn build_command(options: &BuildCommandOptions) -> Result<String, AdapterError> {
    let launch_args = options.launch.as_ref().and_then(|launch| launch.args.clone()).unwrap_or_default();
    let named = person_settings(&launch_args);
    let aside = options.aside;
    let person = if aside && named.file {
        PersonSettings { words: without_settings(&launch_args), settings: Map::new(), file: false }
    } else {
        named
    };
    let (session_id, resume) = (options.session_id.as_deref(), options.resume.as_deref());
    if session_id.is_none() == resume.is_none() {
        return Err(AdapterError("buildCommand needs exactly one of sessionId or resume".into()));
    }
    let id = session_id.or(resume).unwrap_or("");
    if !is_uuid(id) {
        return Err(AdapterError(format!("session identifier must be a UUID, got \"{id}\"")));
    }
    if let Some(copy_of) = options.copy_of.as_deref() {
        if session_id.is_none() || !is_uuid(copy_of) {
            return Err(AdapterError(format!("a copy opens a new session of a session named by its UUID, got \"{copy_of}\"")));
        }
    }
    if aside && resume.is_none() {
        return Err(AdapterError("a side question resumes the copy it is asked on".into()));
    }
    let id_flag = if aside {
        format!("--max-turns 2 --resume {id}")
    } else if session_id.is_none() {
        format!("--resume {id}")
    } else {
        match options.copy_of.as_deref() {
            None => format!("--session-id {id}"),
            Some(copy_of) => format!("--session-id {id} --resume {copy_of} --fork-session"),
        }
    };

    let mut words = vec![format!("{} -p", agent_word("claude", options.launch.as_ref()))];
    words.extend(person.words.iter().map(|word| shell_quote(word)));
    words.extend(["--input-format stream-json", "--output-format stream-json", "--verbose"].map(String::from));
    if options.subagent_text {
        words.push("--forward-subagent-text".into());
    }
    if aside {
        words.push("--include-partial-messages".into());
    }
    words.extend(permission_flags(if aside { Some("default") } else { options.permission_mode.as_deref() })?);
    let model = model_with_context(options.model.as_deref(), options.context_window.as_deref())?;
    words.extend(slug_flag("--model", "model", model.as_deref())?);
    words.extend(slug_flag("--effort", "effort", options.effort.as_deref())?);
    if let Some(name) = &options.name {
        words.push(format!("--name {}", shell_quote(name)));
    }
    words.extend(mcp_config_flag(options.mcp_servers.as_ref()));
    if options.server_values {
        words.push(format!("--mcp-config \"${SERVER_VALUES_ENV}\""));
    }
    words.extend(brief_flag(options.mcp_servers.as_ref()));
    if !person.file {
        let mut settings = Map::new();
        if options.fast == Some(true) {
            settings.insert("fastMode".into(), Value::Bool(true));
        }
        settings.extend(memory_settings(options.memory_dir.as_deref()));
        settings.extend(person.settings);
        settings.extend(denied_servers(options.off_servers.as_deref()));
        if aside {
            settings.insert("disableAllHooks".into(), Value::Bool(true));
        }
        words.extend(settings_flag(&settings));
    }
    words.push(id_flag);
    Ok(format!("{}{}", off_servers_step(options.off_servers.as_deref()), in_folder(options.cwd.as_deref(), &words.join(" "))))
}

/// What launch_command takes: BuildCommandOptions less the session id, the name and the two flags it sets itself.
/// landmines.ts:424.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchOptions {
    pub resume: Option<String>,
    pub copy_of: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub context_window: Option<String>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub off_servers: Option<Vec<String>>,
    pub fast: Option<bool>,
    pub memory_dir: Option<String>,
    pub launch: Option<AgentLaunch>,
    pub title: Option<String>,
    pub version: Option<String>,
}

/// landmines.ts:424.
pub fn launch_command(o: &LaunchOptions, local_id: &str, valued: bool) -> Result<String, AdapterError> {
    build_command(&BuildCommandOptions {
        session_id: o.resume.is_none().then(|| local_id.to_string()),
        resume: o.resume.clone(),
        copy_of: o.copy_of.clone(),
        cwd: o.cwd.clone(),
        model: o.model.clone(),
        effort: o.effort.clone(),
        permission_mode: o.permission_mode.clone(),
        context_window: o.context_window.clone(),
        name: o.title.clone(),
        mcp_servers: o.mcp_servers.clone(),
        server_values: valued,
        off_servers: o.off_servers.clone(),
        fast: o.fast,
        memory_dir: o.memory_dir.clone(),
        launch: o.launch.clone(),
        subagent_text: forwards_subagent_text(o.version.as_deref()),
        aside: false,
    })
}

/// landmines.ts:437.
pub const SAVED_SPEND_TAIL_BYTES: u64 = 8 * 1024 * 1024;

/// landmines.ts:441.
pub fn saved_spend_command(config_dir: &str, session_id: &str) -> String {
    let file = format!("{}/*/{}", shell_quote(&format!("{config_dir}/projects")), shell_quote(&format!("{session_id}.jsonl")));
    format!(
        "wsp_saved=$(ls -1td {file} 2>/dev/null | head -n 1); [ -z \"$wsp_saved\" ] || tail -c {SAVED_SPEND_TAIL_BYTES} \"$wsp_saved\" | grep -F {} | tail -n 1; ",
        shell_quote("\"type\":\"cost-state\"")
    )
}

/// landmines.ts:451.
pub fn user_message_line(text: &str, session_id: &str, images: &[TurnImage], uuid: Option<&str>) -> String {
    let mut content: Vec<Value> = images
        .iter()
        .map(|image| json!({"type": "image", "source": {"type": "base64", "media_type": image.media_type, "data": image.bytes}}))
        .collect();
    content.push(json!({"type": "text", "text": text}));
    let mut line =
        json!({"type": "user", "message": {"role": "user", "content": content}, "parent_tool_use_id": null, "session_id": session_id});
    if let Some(uuid) = uuid {
        line["uuid"] = json!(uuid);
    }
    json_stringify(&line)
}
