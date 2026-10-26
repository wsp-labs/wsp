// SPDX-License-Identifier: AGPL-3.0-only
//! The bash line that runs `codex app-server` for one turn, the environment it runs under, and the launch words every
//! other codex command starts with; on a brokered machine, the placeholder login and the door. TS
//! adapter-codex/src/command.ts. Spec: spec/codex_command.md. `now_ms` stands for the TS `Date.now()`.

use std::sync::Arc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::pending::{launch_has_slate, CodexClaims, McpServerSpec};
use crate::port::{agent_word, launch_words, program_word, AgentLaunch, Env};
use wsp_wire::text::json_stringify;
use wsp_wire::types::env::{HOST_TOKEN_ENV, LAUNCH_ENV};
use wsp_wire::types::placeholders::{PLACEHOLDER_KEY_PREFIX, PLACEHOLDER_REFRESH_PREFIX, PLACEHOLDER_SIGNATURE, TURN_CLAIM};
use wsp_wire::types::shell_quote::{in_folder, shell_quote};
use wsp_wire::types::wsp_tools::{MCP_SERVER_NAME, SLATE_SERVER_NAME, SLATE_TOOLS, WSP_TOOL_TIMEOUT_SEC};
use wsp_words::js::{js_key_order, js_trim};
use wsp_words::js_date::iso_string;

/// The variable each custom provider's env_key names on a brokered turn. TS command.ts:22.
pub const PROVIDER_KEY_ENV: &str = "WSP_CODEX_PROVIDER_KEY";

/// Where a brokered launch finds its placeholder key until it writes the key file. TS command.ts:46.
pub const KEY_FILE_ENV: &str = "WSP_CODEX_KEY";

/// The door on the machine's loopback. TS command.ts:16.
pub const DOOR: &str = "http://127.0.0.1:7071";

/// The same door under the second origin, for chatgpt_base_url alone. TS command.ts:20.
pub const BACKEND_DOOR: &str = "http://localhost:7071";

/// The model address of a launch that asks no model. TS command.ts:221.
pub const NO_MODELS: &str = "http://127.0.0.1:0";

/// ChatGPT's list of the models the host's sign-in may run on the codex version named, read by the host: `Ok(None)`
/// or `Err` where it could not read one. TS command.ts:27.
pub type AccountModels = Arc<dyn Fn(String) -> LocalBoxFuture<'static, Result<Option<Value>, String>>>;

/// Which placeholder login a brokered machine runs on. TS command.ts:31.
#[derive(Clone)]
pub enum BrokerMode {
    Chatgpt { claims: CodexClaims, models: Option<AccountModels> },
    Apikey,
}

/// A turn on a machine whose credentials the host keeps. TS command.ts:31.
#[derive(Clone)]
pub struct CodexBroker {
    pub mode: BrokerMode,
    /// The person's own `model_providers` entries, each a JSON object in its own key order.
    pub providers: Option<IndexMap<String, Map<String, Value>>>,
}

/// The two broker modes by name, as `provider_flags` takes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrokerKind {
    Chatgpt,
    Apikey,
}

/// TS command.ts:35.
#[derive(Clone, Default)]
pub struct CodexEnvOptions {
    /// The base environment; an entry whose value is `None` is left out.
    pub base: Option<IndexMap<String, Option<String>>>,
    /// Absolute path for CODEX_HOME.
    pub home: String,
    pub api_key: Option<String>,
    pub broker: Option<CodexBroker>,
}

/// CODEX_HOME and the key variables over the base environment. TS command.ts:66.
pub fn build_env(options: &CodexEnvOptions) -> Result<Env, String> {
    let home = js_trim(&options.home);
    if !home.starts_with('/') {
        return Err(format!("home must be an absolute path, got \"{}\"", options.home));
    }
    let mut env: Env = IndexMap::new();
    for (key, value) in options.base.iter().flatten() {
        if let Some(value) = value {
            env.insert(key.clone(), value.clone());
        }
    }
    env.insert("CODEX_HOME".into(), home.to_string());
    let keys = match &options.broker {
        None => key_env(options.api_key.as_deref(), None)?,
        Some(broker) => brokered_codex_env(broker, options.api_key.as_deref())?,
    };
    env.extend(keys);
    Ok(env)
}

/// The key a turn carries: under both of codex's names with no broker, and on a brokered one only where the key file reads it. TS command.ts:52, :62.
fn key_env(api_key: Option<&str>, broker: Option<&CodexBroker>) -> Result<Env, String> {
    let mut env: Env = IndexMap::new();
    match broker.map(|b| &b.mode) {
        Some(BrokerMode::Chatgpt { .. }) => {
            if api_key.is_some() {
                return Err("a turn on the host's ChatGPT login carries no key; its login is a placeholder".into());
            }
        }
        Some(BrokerMode::Apikey) => {
            if let Some(key) = api_key {
                if !is_plain_word(key, "._~+/=-") {
                    return Err("a turn on the host's API key needs the placeholder key the host shaped for it, one plain word".into());
                }
                env.insert(KEY_FILE_ENV.into(), key.to_string());
            }
        }
        None => {
            if let Some(key) = api_key {
                env.insert("CODEX_API_KEY".into(), key.to_string());
                env.insert("OPENAI_API_KEY".into(), key.to_string());
            }
        }
    }
    Ok(env)
}

/// A non-empty run of ASCII letters and digits, and the punctuation given.
fn is_plain_word(value: &str, punctuation: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || punctuation.contains(c))
}

/// The placeholder key where the launch reads one, and the refresh and revoke addresses at the door. TS command.ts:76.
pub fn brokered_codex_env(broker: &CodexBroker, api_key: Option<&str>) -> Result<Env, String> {
    let mut env = key_env(api_key, Some(broker))?;
    env.insert("CODEX_REFRESH_TOKEN_URL_OVERRIDE".into(), format!("{DOOR}/oauth/token"));
    env.insert("CODEX_REVOKE_TOKEN_URL_OVERRIDE".into(), format!("{DOOR}/oauth/revoke"));
    Ok(env)
}

/// One of the person's providers as overrides of the copied config's entry. TS command.ts:140.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderFlags {
    pub flags: Vec<String>,
    pub keyed: bool,
}

const AZURE_MARKERS: [&str; 6] =
    ["openai.azure.", "cognitiveservices.azure.", "aoai.azure.", "azure-api.", "azurefd.", "windows.net/openai"];

/// TS command.ts:140; `models` defaults to DOOR there.
pub fn provider_flags(id: &str, entry: &Map<String, Value>, mode: BrokerKind, models: &str) -> Result<ProviderFlags, String> {
    if !is_plain_word(id, "_-") {
        return Err(format!("a model provider's id must be one plain word of a config key, got \"{id}\""));
    }
    if entry.contains_key("aws") {
        return Ok(ProviderFlags { flags: Vec::new(), keyed: false });
    }
    let at = format!("model_providers.{id}");
    let name = entry.get("name").and_then(Value::as_str).unwrap_or("");
    let url = entry.get("base_url").and_then(Value::as_str).unwrap_or("").to_lowercase();
    let azure = name.to_lowercase() != "azure" && AZURE_MARKERS.iter().any(|m| url.contains(m));
    let headers: Vec<String> = match entry.get("http_headers") {
        Some(Value::Object(h)) => js_key_order(h.iter()).into_iter().map(|(k, _)| k.clone()).collect(),
        Some(Value::Array(h)) => (0..h.len()).map(|i| i.to_string()).collect(),
        _ => Vec::new(),
    };
    let openai_auth = entry.get("requires_openai_auth") == Some(&Value::Bool(true));
    let keyed = !openai_auth && !entry.contains_key("auth") && !entry.contains_key("experimental_bearer_token");
    let credential = if entry.contains_key("auth") {
        let print_key = json!(["-c", format!("printf %s \"{PLACEHOLDER_KEY_PREFIX}${HOST_TOKEN_ENV}\"")]);
        vec![config(&format!("{at}.auth.command"), "sh"), config_raw(&format!("{at}.auth.args"), &json_stringify(&print_key))]
    } else if entry.contains_key("experimental_bearer_token") {
        vec![format!("-c {at}.experimental_bearer_token='\"{PLACEHOLDER_KEY_PREFIX}'\"${HOST_TOKEN_ENV}\"'\"'")]
    } else if keyed {
        vec![config(&format!("{at}.env_key"), PROVIDER_KEY_ENV)]
    } else {
        Vec::new()
    };
    let base = if openai_auth && mode == BrokerKind::Chatgpt { format!("{models}/backend-api/codex") } else { format!("{models}/v1") };
    let mut flags = vec![config(&format!("{at}.base_url"), &base)];
    if azure {
        flags.push(config(&format!("{at}.name"), "azure"));
    }
    flags.extend(credential);
    if !headers.is_empty() {
        let blanked: Vec<String> = headers.iter().map(|h| format!("{}=\"\"", json_stringify(&Value::String(h.clone())))).collect();
        flags.push(config_raw(&format!("{at}.http_headers"), &format!("{{{}}}", blanked.join(","))));
    }
    Ok(ProviderFlags { flags, keyed })
}

/// `-c key=<JSON string of value>`, shell-quoted. TS command.ts:263.
fn config(key: &str, value: &str) -> String {
    format!("-c {key}={}", shell_quote(&json_stringify(&Value::String(value.to_string()))))
}

/// `-c key=<toml, shell-quoted>`. TS command.ts:265.
fn config_raw(key: &str, toml: &str) -> String {
    format!("-c {key}={}", shell_quote(toml))
}

/// What a launch that is not a turn starts codex with: the word, and the broker's overrides for after the subcommand.
/// TS command.ts:198.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexLaunch {
    pub word: String,
    pub flags: Vec<String>,
}

/// TS command.ts:198.
pub fn codex_launch(launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>, now_ms: i64) -> Result<CodexLaunch, String> {
    let Some(broker) = broker else {
        return Ok(CodexLaunch { word: program_word("codex", launch), flags: Vec::new() });
    };
    let (steps, flags) = brokered_line(broker, now_ms, false)?;
    Ok(CodexLaunch { word: format!("sh -c {} {}", shell_quote(&with_exec(steps)), program_word("codex", launch)), flags })
}

/// The steps and then the exec of the program the shell was given. TS command.ts:199, :214.
fn with_exec(mut steps: Vec<String>) -> String {
    steps.push("exec \"$0\" \"$@\"".into());
    steps.join(" && ")
}

/// How a launch that asks no model starts codex, the overrides ahead of the subcommand. TS command.ts:210.
pub fn codex_word(launch: Option<&AgentLaunch>, broker: Option<&CodexBroker>) -> Result<String, String> {
    let program = program_word("codex", launch);
    let Some(broker) = broker else {
        return Ok(program);
    };
    let (steps, flags) = no_login_line(broker)?;
    let word = if steps.is_empty() { program } else { format!("sh -c {} {program}", shell_quote(&with_exec(steps))) };
    let mut words = vec![word];
    words.extend(flags);
    Ok(words.join(" "))
}

/// The program a terminal on a brokered machine finds as codex in `folder`. TS command.ts:231.
pub fn codex_for_shell(broker: &CodexBroker, folder: &str, now_ms: i64) -> Result<String, String> {
    let CodexLaunch { word, flags } = codex_launch(None, Some(broker), now_ms)?;
    let unpath = format!("IFS=:; p=; for d in $PATH; do [ \"$d\" = {} ] || p=${{p:+$p:}}$d; done; PATH=$p; unset IFS", shell_quote(folder));
    let any_folder = r#"case "$1" in exec|e) k=; for a; do [ "$a" = --skip-git-repo-check ] && k=1; done; [ -n "$k" ] || { s=$1; shift; set -- "$s" --skip-git-repo-check "$@"; };; esac"#;
    let mut command = vec![word];
    command.extend(flags);
    let lines = [
        "#!/bin/sh".to_string(),
        unpath,
        "export CODEX_HOME=\"${CODEX_HOME:-$HOME/.codex}\"".to_string(),
        any_folder.to_string(),
        format!("exec {} \"$@\"", command.join(" ")),
        String::new(),
    ];
    Ok(lines.join("\n"))
}

/// TS command.ts:242.
#[derive(Clone, Default)]
pub struct BuildCommandOptions {
    pub cwd: Option<String>,
    pub mcp_servers: Option<IndexMap<String, McpServerSpec>>,
    pub launch: Option<AgentLaunch>,
    pub broker: Option<CodexBroker>,
    pub off_servers: Option<Vec<String>>,
    /// `true` is the TS `turn: false`: a server run that starts no turn, on no login. The default is a turn's launch.
    pub side_run: bool,
}

/// A value that may ride a codex command line or a SQL literal unquoted. TS command.ts:257.
pub fn slug(name: &str, value: &str) -> Result<String, String> {
    let mut chars = value.chars();
    let plain = chars.next().is_some_and(|c| c.is_ascii_alphanumeric()) && chars.all(|c| c.is_ascii_alphanumeric() || "._:-".contains(c));
    if plain {
        Ok(value.to_string())
    } else {
        Err(format!("{name} must be a plain slug, got \"{value}\""))
    }
}

/// TS command.ts:310.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sandbox {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

/// TS command.ts:312.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalPolicy {
    OnRequest,
    Never,
}

/// The sandbox and approval pair a thread's start carries, written `sandbox` then `approvalPolicy`. TS command.ts:310.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessParams {
    pub sandbox: Sandbox,
    pub approval_policy: ApprovalPolicy,
}

impl Sandbox {
    /// `read-only`, `workspace-write`, `danger-full-access`.
    pub fn word(&self) -> &'static str {
        match self {
            Sandbox::ReadOnly => "read-only",
            Sandbox::WorkspaceWrite => "workspace-write",
            Sandbox::DangerFullAccess => "danger-full-access",
        }
    }
}

impl ApprovalPolicy {
    /// `on-request`, `never`.
    pub fn word(&self) -> &'static str {
        match self {
            ApprovalPolicy::OnRequest => "on-request",
            ApprovalPolicy::Never => "never",
        }
    }
}

/// TS command.ts:317.
pub fn access_params(mode: Option<&str>) -> Result<AccessParams, String> {
    let (sandbox, approval_policy) = match mode {
        None | Some("danger-full-access") => (Sandbox::DangerFullAccess, ApprovalPolicy::Never),
        Some("read-only") => (Sandbox::ReadOnly, ApprovalPolicy::OnRequest),
        Some("workspace-write") => (Sandbox::WorkspaceWrite, ApprovalPolicy::OnRequest),
        Some(other) => {
            return Err(format!("permissionMode must be one of read-only, workspace-write, danger-full-access, got \"{other}\""))
        }
    };
    Ok(AccessParams { sandbox, approval_policy })
}

/// The server as one bash line in the folder the turn runs in. TS command.ts:326.
pub fn build_command(options: &BuildCommandOptions, now_ms: i64) -> Result<String, String> {
    let (steps, broker_flags) = match &options.broker {
        None => (Vec::new(), Vec::new()),
        Some(broker) if options.side_run => no_login_line(broker)?,
        Some(broker) => brokered_line(broker, now_ms, true)?,
    };
    let launch = options.launch.as_ref();
    let mut server = vec![format!("{} app-server", agent_word("codex", launch))];
    server.extend(launch_words(launch));
    server.push(config_raw("tools.update_plan.enabled", "true"));
    server.extend(broker_flags);
    server.extend(server_flags(options.mcp_servers.as_ref())?);
    server.extend(off_flags(options.off_servers.as_deref().unwrap_or_default()));
    let mut lines = steps;
    lines.push(server.join(" "));
    Ok(in_folder(options.cwd.as_deref(), &lines.join(" && ")))
}

/// TS command.ts:334.
pub fn image_path(path: &str) -> Result<String, String> {
    if !path.starts_with('/') || path.contains('\n') {
        return Err(format!("an image path must be one absolute path on the machine, got \"{path}\""));
    }
    Ok(path.to_string())
}

const TURN_MARK: &str = "@@wsp-turn@@";
const JWT_MARK: &str = "@@wsp-jwt@@";
const KEY_MARK: &str = "@@wsp-key@@";
/// base64url of `{"alg":"none","typ":"JWT"}`.
const JWT_HEADER: &str = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0";
const YEAR_S: i64 = 365 * 24 * 60 * 60;

/// JSON whose marks become the shell words standing for them, every other run of it quoted as it is. TS command.ts:88.
fn spliced(text: &str, words: &[(&str, String)]) -> String {
    let mut out = String::new();
    let mut rest = text;
    loop {
        let next = words.iter().filter_map(|(mark, word)| rest.find(*mark).map(|at| (at, mark.len(), word))).min_by_key(|&(at, _, _)| at);
        let Some((at, len, word)) = next else {
            break;
        };
        if at > 0 {
            out.push_str(&shell_quote(&rest[..at]));
        }
        out.push_str(word);
        rest = &rest[at + len..];
    }
    if !rest.is_empty() {
        out.push_str(&shell_quote(rest));
    }
    out
}

/// The step before the server starts on a brokered turn: CODEX_HOME's auth.json as a placeholder login. TS command.ts:103.
fn placeholder_login(broker: &CodexBroker, now: i64, turn: bool) -> Result<String, String> {
    let words =
        [(TURN_MARK, format!("\"${HOST_TOKEN_ENV}\"")), (JWT_MARK, "\"$jwt\"".to_string()), (KEY_MARK, format!("\"${KEY_FILE_ENV}\""))];
    let apikey = matches!(broker.mode, BrokerMode::Apikey);
    let needs = if apikey { KEY_FILE_ENV } else { HOST_TOKEN_ENV };
    let write = |steps: Vec<String>| -> String {
        let body = std::iter::once("umask 077 && mkdir -p \"$CODEX_HOME\"".to_string()).chain(steps).collect::<Vec<_>>().join(" && ");
        if turn {
            let key_guard = if apikey {
                format!(" && : \"${{{KEY_FILE_ENV}:?a turn on the key the host keeps was launched without its placeholder key}}\"")
            } else {
                String::new()
            };
            format!("( : \"${{{HOST_TOKEN_ENV}:?a turn whose credentials the host keeps was launched without its token}}\"{key_guard} && {body} )")
        } else {
            format!("{{ [ -z \"${{{needs}:-}}\" ] || ( {body} ); }}")
        }
    };
    let claims = match &broker.mode {
        BrokerMode::Chatgpt { claims, .. } => claims,
        BrokerMode::Apikey => {
            let file = json_stringify(&json!({"auth_mode": "apikey", "OPENAI_API_KEY": KEY_MARK}));
            return Ok(write(vec![format!("printf %s {} > \"$CODEX_HOME/auth.json\"", spliced(&file, &words))]));
        }
    };
    let mut auth = Map::new();
    auth.insert("chatgpt_account_id".into(), json!(claims.account_id));
    if let Some(plan) = &claims.plan_type {
        auth.insert("chatgpt_plan_type".into(), json!(plan));
    }
    if let Some(user) = &claims.user_id {
        auth.insert("chatgpt_user_id".into(), json!(user));
    }
    auth.insert("chatgpt_account_is_fedramp".into(), json!(claims.fedramp == Some(true)));
    let mut payload = Map::new();
    payload.insert("exp".into(), json!(now.div_euclid(1000) + YEAR_S));
    payload.insert("https://api.openai.com/auth".into(), Value::Object(auth));
    payload.insert(TURN_CLAIM.into(), json!(TURN_MARK));
    let payload = json_stringify(&Value::Object(payload));
    let jwt = format!("{JWT_HEADER}.{JWT_MARK}.{PLACEHOLDER_SIGNATURE}");
    let last_refresh = iso_string(now as f64).ok_or_else(|| "Invalid time value".to_string())?;
    let file = json_stringify(&json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {"id_token": jwt, "access_token": jwt, "refresh_token": format!("{PLACEHOLDER_REFRESH_PREFIX}{TURN_MARK}"), "account_id": claims.account_id},
        "last_refresh": last_refresh,
    }));
    let jwt_line = format!("jwt=$(printf %s {} | base64 | tr -d '\\n=' | tr '+/' '-_')", spliced(&payload, &words));
    Ok(write(vec![jwt_line, format!("printf %s {} > \"$CODEX_HOME/auth.json\"", spliced(&file, &words))]))
}

/// The providers of a broker, each as its flags against the given address, in JS key order. TS command.ts:138.
fn providers_of(broker: &CodexBroker, models: &str) -> Result<Vec<ProviderFlags>, String> {
    let Some(providers) = &broker.providers else {
        return Ok(Vec::new());
    };
    let kind = match broker.mode {
        BrokerMode::Chatgpt { .. } => BrokerKind::Chatgpt,
        BrokerMode::Apikey => BrokerKind::Apikey,
    };
    js_key_order(providers.iter()).into_iter().map(|(id, entry)| provider_flags(id, entry, kind, models)).collect()
}

/// The steps and the overrides a brokered launch adds, as TS command.ts:176 writes them. `turn` is false for a launch
/// that is not a turn, which takes the login only where the token it holds is set.
fn brokered_line(broker: &CodexBroker, now: i64, turn: bool) -> Result<(Vec<String>, Vec<String>), String> {
    let providers = providers_of(broker, DOOR)?;
    let mut steps = vec![placeholder_login(broker, now, turn)?];
    if matches!(broker.mode, BrokerMode::Apikey) {
        steps.push(format!("unset {KEY_FILE_ENV}"));
    }
    if providers.iter().any(|p| p.keyed) {
        steps.push(format!("export {PROVIDER_KEY_ENV}=\"{PLACEHOLDER_KEY_PREFIX}${HOST_TOKEN_ENV}\""));
    }
    Ok((steps, door_flags(broker, &providers, "file", DOOR)))
}

/// The overrides every brokered launch carries, the person's providers after them. TS command.ts:184.
fn door_flags(broker: &CodexBroker, providers: &[ProviderFlags], store: &str, models: &str) -> Vec<String> {
    let chatgpt = matches!(broker.mode, BrokerMode::Chatgpt { .. });
    let openai_base = if chatgpt { format!("{models}/backend-api/codex") } else { format!("{models}/v1") };
    let mut flags = vec![
        config("openai_base_url", &openai_base),
        config("chatgpt_base_url", &format!("{BACKEND_DOOR}/backend-api/")),
        config("cli_auth_credentials_store", store),
        config_raw("analytics.enabled", "false"),
    ];
    flags.extend(providers.iter().flat_map(|p| p.flags.iter().cloned()));
    flags
}

/// The steps and overrides of a launch that asks no model, run on no login. TS command.ts:223.
fn no_login_line(broker: &CodexBroker) -> Result<(Vec<String>, Vec<String>), String> {
    let steps = if matches!(broker.mode, BrokerMode::Apikey) { vec![format!("unset {KEY_FILE_ENV}")] } else { Vec::new() };
    let providers = providers_of(broker, NO_MODELS)?;
    let mut flags = door_flags(broker, &providers, "ephemeral", NO_MODELS);
    flags.push(config_raw("features.plugins", "false"));
    Ok((steps, flags))
}

/// The `enabled=false` override of each off server the config names, silently dropping a bad name. TS command.ts:306.
fn off_flags(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| is_plain_word(name, "_-"))
        .map(|name| {
            let pattern = format!(r"^[[:space:]]*(\[mcp_servers\.{name}[].]|mcp_servers\.{name}\.)");
            format!(
                "$(grep -Eqs {} \"$CODEX_HOME/config.toml\" && printf %s {})",
                shell_quote(&pattern),
                shell_quote(&format!("-c mcp_servers.{name}.enabled=false"))
            )
        })
        .collect()
}

/// Each MCP server as a whole config entry, with the slate's own server beside the wsp one where the launch has a slate. TS command.ts:272.
fn server_flags(servers: Option<&IndexMap<String, McpServerSpec>>) -> Result<Vec<String>, String> {
    let Some(servers) = servers else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (name, spec) in js_key_order(servers.iter()) {
        if !is_plain_word(name, "_-") {
            return Err(format!("an MCP server name must be one plain word of a config key, got \"{name}\""));
        }
        out.extend(server_entry(name, spec));
        if name == MCP_SERVER_NAME && launch_has_slate(Some(servers)) {
            let slate = json_stringify(&json!(SLATE_TOOLS));
            out.push(config_raw(&format!("mcp_servers.{name}.disabled_tools"), &slate));
            out.extend(server_entry(SLATE_SERVER_NAME, spec));
            out.push(config_raw(&format!("mcp_servers.{SLATE_SERVER_NAME}.enabled_tools"), &slate));
            out.push(config_raw(&format!("mcp_servers.{SLATE_SERVER_NAME}.omit_tools_from"), &json_stringify(&json!(["deferred"]))));
            for tool in SLATE_TOOLS {
                out.push(config(&format!("mcp_servers.{SLATE_SERVER_NAME}.tools.{tool}.approval_mode"), "approve"));
            }
        }
    }
    Ok(out)
}

/// One server's command, args and, for the wsp servers, the launch's variable names and the tool timeout. TS command.ts:272.
fn server_entry(name: &str, spec: &McpServerSpec) -> Vec<String> {
    let at = format!("mcp_servers.{name}");
    let mut out =
        vec![config(&format!("{at}.command"), &spec.command), config_raw(&format!("{at}.args"), &json_stringify(&json!(spec.args)))];
    if name == MCP_SERVER_NAME || name == SLATE_SERVER_NAME {
        out.push(config_raw(&format!("{at}.env_vars"), &json_stringify(&json!(LAUNCH_ENV))));
        out.push(config_raw(&format!("{at}.tool_timeout_sec"), &WSP_TOOL_TIMEOUT_SEC.to_string()));
    }
    out
}
