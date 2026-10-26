// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/adapters.ts at 65f6b89c7: one adapter factory per agent wsp can drive, and what the
//! vault hands each. Spec: spec/agent_setups.md.
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{Map, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::claude::landmines::{BrokerKind, BrokeredSignIn};
use wsp_adapters::codex::adapter::{create_codex_adapter, CodexAdapterDeps};
use wsp_adapters::codex::limits::{limit_of, LimitAccount};
use wsp_adapters::codex::reset::CodexPlanResets;
use wsp_adapters::cursor::adapter::{create_cursor_adapter, CursorAdapterDeps};
use wsp_adapters::opencode::adapter::{create_opencode_adapter, OpenCodeAdapterDeps};
use wsp_adapters::port::{Env, HarnessAdapter, PlanResets};
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::signin::{has_login, key_env_of, mints_token, SignIn};
use wsp_wire::types::usage::HarnessLimit;
use wsp_words::exit::HostError;

use crate::types::harness::{BrokeredKind, HarnessAdapterContext, HarnessAdapterFactory};

/// What the vault holds for one agent: its token, else its key with the variable it travels under, never both.
/// adapters.ts:37.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Secrets {
    pub oauth_token: Option<String>,
    pub api_key: Option<String>,
    pub key_env: Option<String>,
}

/// adapters.ts:33. Nothing where a login of the agent's own stands or the catalog has no such agent.
pub fn secrets_of(vault: &IndexMap<String, String>, id: &str, login_stands: bool) -> Secrets {
    let Some(sign_in) = sign_in_of(id) else {
        return Secrets::default();
    };
    if login_stands {
        return Secrets::default();
    }
    if let Some(token) = mints_token(sign_in).and_then(|t| vault.get(&t.token_env)) {
        return Secrets { oauth_token: Some(token.clone()), ..Secrets::default() };
    }
    let Some(key_env) = key_env_of(sign_in) else {
        return Secrets::default();
    };
    match vault.get(key_env) {
        Some(key) => Secrets { api_key: Some(key.clone()), key_env: Some(key_env.to_string()), ..Secrets::default() },
        None => Secrets::default(),
    }
}

/// adapters.ts:50. The same under the variable the agent reads it from: one entry or none.
pub fn secret_env_of(vault: &IndexMap<String, String>, id: &str, login_stands: bool) -> IndexMap<String, String> {
    let secrets = secrets_of(vault, id, login_stands);
    if let (Some(token), Some(sign_in)) = (&secrets.oauth_token, sign_in_of(id)) {
        if let Some(mint) = mints_token(sign_in) {
            return IndexMap::from([(mint.token_env.clone(), token.clone())]);
        }
    }
    match (secrets.api_key, secrets.key_env) {
        (Some(key), Some(variable)) => IndexMap::from([(variable, key)]),
        _ => IndexMap::new(),
    }
}

/// The sign-in a person runs on a machine: the row's headless fallback, else its login; refused where the catalog
/// row has no login. adapters.ts:18.
pub fn machine_login(id: &str) -> Result<String, HostError> {
    match sign_in_of(id).and_then(has_login) {
        Some(login) => Ok(login.fallback.clone().unwrap_or_else(|| login.login.clone())),
        None => Err(HostError::new(format!("the catalog has no sign-in for {id}"))),
    }
}

/// One factory per THREAD_AGENTS id, in that order: claude, codex, opencode, cursor. adapters.ts:57.
pub fn harness_adapters() -> IndexMap<&'static str, HarnessAdapterFactory> {
    let claude: HarnessAdapterFactory = Arc::new(|ctx: HarnessAdapterContext| {
        let (brokered, secrets) = match &ctx.brokered {
            Some(brokered) => {
                let kind = match brokered.kind {
                    BrokeredKind::Token => BrokerKind::Token,
                    BrokeredKind::Key => BrokerKind::Key,
                    BrokeredKind::Gateway => BrokerKind::Gateway,
                };
                (Some(BrokeredSignIn { token: brokered.token.clone(), kind }), Secrets::default())
            }
            None => (None, secrets_of(&ctx.vault, "claude", (ctx.login_stands)("claude"))),
        };
        let deps = AdapterDeps {
            exec: ctx.exec_stream.clone(),
            config_dir: (ctx.home)("claude"),
            base_env: Some(base_env_of(&ctx.env)),
            project_dir_name: ctx.project_key.clone(),
            api_key: secrets.api_key,
            oauth_token: secrets.oauth_token,
            brokered,
            interrupt_grace_ms: None,
            result_exit_ms: None,
            steer_join_ms: None,
            sign_in_refusal: Some(ctx.sign_in_refusal.clone()),
            aside_wall_ms: None,
            launch: ctx.launch.clone(),
            stop_task_wait_ms: None,
            now: None,
        };
        let adapter = create_claude_adapter(deps).map_err(|e| HostError::new(e.0))?;
        Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
    });
    let codex: HarnessAdapterFactory = Arc::new(|ctx: HarnessAdapterContext| {
        let (broker, secrets) = match &ctx.codex_broker {
            Some(broker) => (Some(broker.clone()), secrets_of(&ctx.vault, "codex", false)),
            None => (None, secrets_of(&ctx.vault, "codex", (ctx.login_stands)("codex"))),
        };
        let deps = CodexAdapterDeps {
            exec: ctx.exec_stream.clone(),
            home: (ctx.home)("codex"),
            login: machine_login("codex")?,
            api_key: secrets.api_key,
            key_env: secrets.key_env,
            base_env: Some(base_env_of(&ctx.env)),
            interrupt_grace_ms: None,
            result_exit_ms: None,
            steer_join_ms: None,
            reconnect_stall_ms: None,
            aside_wall_ms: None,
            launch: ctx.launch.clone(),
            broker,
            now: None,
        };
        let adapter = create_codex_adapter(deps).map_err(|e| HostError::new(e.0))?;
        Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
    });
    let opencode: HarnessAdapterFactory = Arc::new(|ctx: HarnessAdapterContext| {
        let secrets = secrets_of(&ctx.vault, "opencode", (ctx.login_stands)("opencode"));
        let deps = OpenCodeAdapterDeps {
            exec: ctx.exec_stream.clone(),
            base_env: Some(base_env_of(&ctx.env)),
            sign_in_refusal: Some(ctx.sign_in_refusal.clone()),
            api_key: secrets.api_key,
            key_env: secrets.key_env,
            interrupt_grace_ms: None,
            result_exit_ms: None,
            launch: ctx.launch.clone(),
            now: None,
        };
        Ok(Arc::new(create_opencode_adapter(deps)) as Arc<dyn HarnessAdapter>)
    });
    let cursor: HarnessAdapterFactory = Arc::new(|ctx: HarnessAdapterContext| {
        let secrets = secrets_of(&ctx.vault, "cursor", (ctx.login_stands)("cursor"));
        let deps = CursorAdapterDeps {
            exec: ctx.exec_stream.clone(),
            base_env: Some(base_env_of(&ctx.env)),
            sign_in_refusal: Some(ctx.sign_in_refusal.clone()),
            api_key: secrets.api_key,
            key_env: secrets.key_env,
            interrupt_grace_ms: None,
            result_exit_ms: None,
            launch: ctx.launch.clone(),
        };
        Ok(Arc::new(create_cursor_adapter(deps)) as Arc<dyn HarnessAdapter>)
    });
    IndexMap::from([("claude", claude), ("codex", codex), ("opencode", opencode), ("cursor", cursor)])
}

fn base_env_of(env: &Env) -> IndexMap<String, Option<String>> {
    env.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect()
}

/// The agents a brokered machine launches: claude and codex. adapters.ts:82.
pub const BROKERED_AGENTS: [&str; 2] = ["claude", "codex"];

/// The agents whose plans bank resets a person can spend: codex alone. adapters.ts:85.
pub fn plan_resets() -> IndexMap<&'static str, Arc<dyn PlanResets>> {
    IndexMap::from([("codex", Arc::new(CodexPlanResets) as Arc<dyn PlanResets>)])
}

/// A reader of one rate-limits reading off an agent's store.
pub type LogLimit = fn(&Map<String, Value>) -> Option<HarnessLimit>;

/// The agents whose own store keeps the plan's windows: codex alone, read with no account and no credits.
/// adapters.ts:89.
pub fn log_limits() -> IndexMap<&'static str, LogLimit> {
    IndexMap::from([("codex", (|snapshot: &Map<String, Value>| limit_of(snapshot, &LimitAccount::default(), None)) as LogLimit)])
}

fn sign_in_of(id: &str) -> Option<&'static SignIn> {
    catalog_agents().into_iter().find(|a| a.id == id).map(|a| &a.sign_in)
}
