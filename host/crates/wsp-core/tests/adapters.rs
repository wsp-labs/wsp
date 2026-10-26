// SPDX-License-Identifier: AGPL-3.0-only
//! adapters: the cases of packages/runtime/test/adapters.test.ts, then the TS's own answers in
//! tests/fixtures/core_rest/adapters_setups.json (written by oracle/core_rest.oracle.ts).
mod rest;

use std::sync::Arc;

use indexmap::IndexMap;
use rest::*;
use serde_json::{json as jv, Value};
use wsp_adapters::claude::permissions::SKIP_PROMPTS_MODE;
use wsp_adapters::port::{AgentLaunch, AsideQuestion, Env, ExecStreamFactory, HarnessAdapter};
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::git_hosts::THREAD_AGENTS;
use wsp_core::adapters::{harness_adapters, log_limits, plan_resets, secret_env_of, secrets_of, Secrets, BROKERED_AGENTS};
use wsp_core::harness_catalog::harness_catalogs;
use wsp_core::machine_exec::{machine_exec_stream, MachineExecOptions};
use wsp_core::types::harness::HarnessAdapterContext;
use wsp_engine::Machine;
use wsp_wire::types::env::PLUGIN_SEED_ENV;
use wsp_wire::views::harness::{takes_mcp_servers, HarnessCatalog, HarnessOption, ScreenCommand};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_words::workspace_state::sign_in_refusal_line;

fn env_of(pairs: &[(&str, &str)]) -> Env {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn context(kind: WorkspaceKind, env: Env, vault: Env, stands: bool, launch: Option<AgentLaunch>) -> HarnessAdapterContext {
    let machine: Arc<dyn Machine> = Arc::new(RecMachine::plain(vec![]));
    let exec_stream: Arc<dyn ExecStreamFactory> = machine_exec_stream(machine.clone(), MachineExecOptions::default(), None);
    HarnessAdapterContext {
        machine,
        workspace_id: "ws_1".into(),
        exec_stream,
        home: Arc::new(|_| "/root/.state".to_string()),
        env,
        project_key: None,
        sign_in_refusal: sign_in_refusal_line(Some(&kind)),
        vault,
        brokered: None,
        codex_broker: None,
        login_stands: Arc::new(move |_| stands),
        launch,
    }
}

fn cloud() -> HarnessAdapterContext {
    context(WorkspaceKind::Cloud, Env::new(), Env::new(), false, None)
}

fn adapter(id: &str, ctx: HarnessAdapterContext) -> Arc<dyn HarnessAdapter> {
    let factories = harness_adapters();
    let factory = factories.get(id).unwrap_or_else(|| panic!("no adapter factory for {id}"));
    factory(ctx).unwrap_or_else(|e| panic!("{id} refused its context: {e}"))
}

fn row(id: &str) -> HarnessCatalog {
    harness_catalogs().into_iter().find(|c| c.harness == id).unwrap_or_else(|| panic!("no catalog row for {id}"))
}

fn mode_values(options: &[HarnessOption]) -> Vec<String> {
    options.iter().map(|o| o.value.clone()).collect()
}

fn screen_commands_of(row: &HarnessCatalog) -> Vec<ScreenCommand> {
    row.screen_commands.clone().unwrap_or_default()
}

fn has_aside(adapter: &dyn HarnessAdapter) -> bool {
    adapter.aside(AsideQuestion::default()).is_some()
}

fn secrets_json(s: &Secrets) -> Value {
    let mut o = serde_json::Map::new();
    if let Some(t) = &s.oauth_token {
        o.insert("oauthToken".into(), Value::String(t.clone()));
    }
    if let Some(k) = &s.api_key {
        o.insert("apiKey".into(), Value::String(k.clone()));
    }
    if let Some(e) = &s.key_env {
        o.insert("keyEnv".into(), Value::String(e.clone()));
    }
    Value::Object(o)
}

fn names(keys: Vec<&str>) -> Vec<String> {
    keys.into_iter().map(String::from).collect()
}

// TS: adapters.test.ts:548
#[test]
fn the_agents_wsp_can_open_a_thread_on_is_one_list_the_adapter_registry_is_keyed_by_it_and_every_id_is_a_catalog_agent() {
    let mut keys = names(harness_adapters().keys().copied().collect());
    keys.sort();
    let mut listed = names(THREAD_AGENTS.to_vec());
    listed.sort();
    assert_eq!(keys, listed);
    assert_eq!(THREAD_AGENTS, ["claude", "codex", "opencode", "cursor"]);
    let catalog_ids: Vec<String> = catalog_agents().iter().map(|a| a.id.clone()).collect();
    for id in THREAD_AGENTS {
        assert!(catalog_ids.contains(&id.to_string()), "{id} is not a catalog agent");
    }
}

// TS: adapters.test.ts:554
#[test]
fn each_catalog_row_says_what_its_own_adapter_can_hand_a_launch_so_a_client_reads_the_row_before_any_workspace_exists() {
    run_local(async {
        for id in THREAD_AGENTS {
            let adapter = adapter(id, cloud());
            let row = row(id);
            assert_eq!(takes_mcp_servers(Some(&row)), adapter.mcp_servers(), "{id} mcpServers");
            assert_eq!(screen_commands_of(&row), adapter.screen_commands(), "{id} screenCommands");
            assert_eq!(row.moves_access == Some(true), adapter.moves_access(), "{id} movesAccess");
            assert_eq!(row.steers_images == Some(true), adapter.steers_images(), "{id} steersImages");
            assert_eq!(row.asides == Some(true), has_aside(adapter.as_ref()), "{id} asides");
        }
        let claude_commands: Vec<String> = screen_commands_of(&row("claude")).into_iter().map(|c| c.name).collect();
        assert!(claude_commands.contains(&"login".to_string()), "{claude_commands:?}");
        assert_eq!(screen_commands_of(&row("codex")), vec![]);
        assert_eq!(row("claude").bypass_mode.as_deref(), Some(SKIP_PROMPTS_MODE));
        for id in ["claude", "codex"] {
            let adapter = adapter(id, cloud());
            assert!(adapter.steers(), "{id} steers");
            assert!(has_aside(adapter.as_ref()), "{id} aside");
            assert_eq!(adapter.compacts().as_deref(), Some("/compact"), "{id} compacts");
        }
        assert!(takes_mcp_servers(Some(&row("claude"))));
        assert!(takes_mcp_servers(Some(&row("codex"))));
    });
}

// TS: adapters.test.ts:591
#[test]
fn claude_codes_adapter_names_the_command_that_opens_one_of_its_sessions_in_the_persons_terminal_and_no_other_adapter_names_one() {
    let local = || context(WorkspaceKind::Local, Env::new(), Env::new(), false, None);
    assert_eq!(adapter("claude", local()).terminal_resume().as_deref(), Some("claude --resume"));
    let set = context(
        WorkspaceKind::Local,
        env_of(&[("CLAUDE_CONFIG_DIR", "/Users/dev/claude work")]),
        Env::new(),
        false,
        Some(AgentLaunch { program: Some("/opt/claude/bin/claude".into()), args: None }),
    );
    assert_eq!(
        adapter("claude", set).terminal_resume().as_deref(),
        Some("CLAUDE_CONFIG_DIR='/Users/dev/claude work' '/opt/claude/bin/claude' --resume")
    );
    for id in ["codex", "opencode", "cursor"] {
        assert_eq!(adapter(id, local()).terminal_resume(), None, "{id}");
    }
}

// TS: adapters.test.ts:601
#[test]
fn the_catalog_points_a_guests_claude_code_at_its_images_plugins_by_the_variable_the_adapter_keeps_through_its_strip() {
    assert_eq!(PLUGIN_SEED_ENV, "CLAUDE_CODE_PLUGIN_SEED_DIR");
    let claude = catalog_agents().into_iter().find(|a| a.id == "claude").expect("claude is a catalog agent");
    assert_eq!(claude.plugins.and_then(|p| p.seed()).map(|seed| seed.env()), Some(PLUGIN_SEED_ENV));
}

// TS: adapters.test.ts:606
#[test]
fn opencodes_row_offers_auto_alone_cursors_the_two_modes_its_cli_has_and_neither_takes_a_message_mid_turn() {
    let opencode = row("opencode");
    assert_eq!(mode_values(&opencode.permission_modes), ["auto"]);
    assert_eq!(json(&opencode.access), jv!({ "full": "auto" }));
    assert_eq!(opencode.bypass_mode.as_deref(), Some("auto"));
    let cursor = row("cursor");
    assert_eq!(cursor.label, "Cursor");
    assert_eq!(cursor.models, vec![]);
    assert_eq!(json(&cursor.access), jv!({ "full": "force", "plan": "default" }));
    assert_eq!(cursor.bypass_mode.as_deref(), Some("force"));
    assert_eq!(mode_values(&cursor.permission_modes), ["default", "force"]);
    for id in ["opencode", "cursor"] {
        assert!(!adapter(id, cloud()).steers(), "{id}");
    }
    let keyed = |stands: bool| context(WorkspaceKind::Cloud, Env::new(), env_of(&[("CURSOR_API_KEY", "key_x")]), stands, None);
    assert_eq!(adapter("cursor", keyed(false)).env(), Some(env_of(&[("CURSOR_API_KEY", "key_x")])));
    assert_eq!(adapter("cursor", keyed(true)).env(), Some(Env::new()));
}

// TS: adapters.test.ts:621
#[test]
fn every_adapter_exports_the_login_env_the_runtime_hands_it_so_a_turn_carries_the_goldens_path_onto_the_machine() {
    let path = "/root/.local/bin:/usr/bin";
    for id in THREAD_AGENTS {
        let ctx = context(WorkspaceKind::Cloud, env_of(&[("PATH", path)]), Env::new(), false, None);
        let env = adapter(id, ctx).env().unwrap_or_else(|| panic!("{id} exports no env"));
        assert_eq!(env.get("PATH").map(String::as_str), Some(path), "{id}");
    }
}

// fixture: adapters_setups.json secrets
#[test]
fn every_vault_reads_into_the_secrets_and_the_env_the_ts_made_of_it() {
    let fx = fixture("adapters_setups");
    let rows = fx["secrets"].as_array().unwrap();
    assert!(!rows.is_empty());
    for r in rows {
        let name = r["name"].as_str().unwrap();
        let vault: IndexMap<String, String> =
            r["vault"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        let (id, stands) = (r["id"].as_str().unwrap(), r["stands"].as_bool().unwrap());
        assert_eq!(secrets_json(&secrets_of(&vault, id, stands)), r["secrets"], "{name} secrets");
        assert_eq!(json(&secret_env_of(&vault, id, stands)), r["env"], "{name} env");
    }
}

// fixture: adapters_setups.json adapters
#[test]
fn the_adapter_registry_lists_the_agents_in_the_ts_order() {
    let fx = fixture("adapters_setups");
    assert_eq!(json(&harness_adapters().keys().collect::<Vec<_>>()), fx["adapters"]);
}

// fixture: adapters_setups.json brokered
#[test]
fn the_brokered_agents_are_the_ts_list() {
    let fx = fixture("adapters_setups");
    assert_eq!(json(&BROKERED_AGENTS), fx["brokered"]);
}

// fixture: adapters_setups.json planResets
#[test]
fn the_plan_reset_agents_are_the_ts_list() {
    let fx = fixture("adapters_setups");
    assert_eq!(json(&plan_resets().keys().collect::<Vec<_>>()), fx["planResets"]);
}

// fixture: adapters_setups.json logLimits
#[test]
fn the_log_limit_agents_are_the_ts_list() {
    let fx = fixture("adapters_setups");
    assert_eq!(json(&log_limits().keys().collect::<Vec<_>>()), fx["logLimits"]);
}
