// SPDX-License-Identifier: AGPL-3.0-only
//! The thread-defaults helpers the command line and the runtime read: setupPatched, agentEnvRefusal, configDirRefusal,
//! noConfigDirLine, configDirSignInLine, pickRefusal, notRunRefusal and accessWordRefusal. Every expected value is what
//! node 22 printed for the TS at the cut (383708c78, packages/protocol/src/thread-defaults.ts), run as one esbuild
//! bundle of packages/protocol/src.

use regex::Regex;
use serde_json::{json, Value};
use wsp_wire::types::thread_defaults::{AgentSetup, AgentSetupSet, ResolvedFolder};
use wsp_wire::views::harness::HarnessCatalog;
use wsp_words::thread_defaults::*;

fn catalog_json(text: &str) -> HarnessCatalog {
    let value: Value = serde_json::from_str(text).unwrap();
    serde_json::from_value(value).unwrap()
}

fn catalog(source: &str) -> HarnessCatalog {
    catalog_json(
        &json!({
            "harness": "claude", "label": "Claude Code", "source": source, "version": null, "models": [], "efforts": [],
            "contextWindows": [], "permissionModes": [], "steers": true, "renames": true, "images": true
        })
        .to_string(),
    )
}

// setupPatched(undefined, {})
#[test]
fn setup_patched_nothing_over_nothing() {
    let kept: Option<AgentSetup> = None;
    let set: AgentSetupSet = serde_json::from_str("{}").unwrap();
    let got = setup_patched(kept.as_ref(), &set);
    assert_eq!(got.map_or("undefined".to_string(), |s| serde_json::to_string(&s).unwrap()), "undefined");
}

// setupPatched({"program":"x","env":{"A":"1"}}, {"env":{"A":null,"B":"2"},"args":["--a"]})
#[test]
fn setup_patched_a_variable_taken_away_and_one_added_and_args_set() {
    let kept: Option<AgentSetup> = Some(serde_json::from_str("{\"program\":\"x\",\"env\":{\"A\":\"1\"}}").unwrap());
    let set: AgentSetupSet = serde_json::from_str("{\"env\":{\"A\":null,\"B\":\"2\"},\"args\":[\"--a\"]}").unwrap();
    let got = setup_patched(kept.as_ref(), &set);
    assert_eq!(
        got.map_or("undefined".to_string(), |s| serde_json::to_string(&s).unwrap()),
        "{\"program\":\"x\",\"args\":[\"--a\"],\"env\":{\"B\":\"2\"}}"
    );
}

// setupPatched({"on":false,"program":"p","configDir":"/c","args":["a"],"env":{"Z":"1","A":"2"}}, {"on":true,"program":null})
#[test]
fn setup_patched_on_back_to_the_agents_own_and_a_program_taken_away() {
    let kept: Option<AgentSetup> = Some(
        serde_json::from_str("{\"on\":false,\"program\":\"p\",\"configDir\":\"/c\",\"args\":[\"a\"],\"env\":{\"Z\":\"1\",\"A\":\"2\"}}")
            .unwrap(),
    );
    let set: AgentSetupSet = serde_json::from_str("{\"on\":true,\"program\":null}").unwrap();
    let got = setup_patched(kept.as_ref(), &set);
    assert_eq!(
        got.map_or("undefined".to_string(), |s| serde_json::to_string(&s).unwrap()),
        "{\"configDir\":\"/c\",\"args\":[\"a\"],\"env\":{\"Z\":\"1\",\"A\":\"2\"}}"
    );
}

// setupPatched({"on":false}, {})
#[test]
fn setup_patched_off_stays_off_when_the_change_names_nothing() {
    let kept: Option<AgentSetup> = Some(serde_json::from_str("{\"on\":false}").unwrap());
    let set: AgentSetupSet = serde_json::from_str("{}").unwrap();
    let got = setup_patched(kept.as_ref(), &set);
    assert_eq!(got.map_or("undefined".to_string(), |s| serde_json::to_string(&s).unwrap()), "{\"on\":false}");
}

// setupPatched(undefined, {"on":false})
#[test]
fn setup_patched_off_from_nothing() {
    let kept: Option<AgentSetup> = None;
    let set: AgentSetupSet = serde_json::from_str("{\"on\":false}").unwrap();
    let got = setup_patched(kept.as_ref(), &set);
    assert_eq!(got.map_or("undefined".to_string(), |s| serde_json::to_string(&s).unwrap()), "{\"on\":false}");
}

// setupPatched({"env":{"A":"1"}}, {"env":{"A":null}})
#[test]
fn setup_patched_the_last_variable_taken_away_leaves_nothing() {
    let kept: Option<AgentSetup> = Some(serde_json::from_str("{\"env\":{\"A\":\"1\"}}").unwrap());
    let set: AgentSetupSet = serde_json::from_str("{\"env\":{\"A\":null}}").unwrap();
    let got = setup_patched(kept.as_ref(), &set);
    assert_eq!(got.map_or("undefined".to_string(), |s| serde_json::to_string(&s).unwrap()), "undefined");
}

// setupPatched({"configDir":"/c"}, {"configDir":null,"args":[]})
#[test]
fn setup_patched_a_config_folder_taken_away_and_empty_args_kept() {
    let kept: Option<AgentSetup> = Some(serde_json::from_str("{\"configDir\":\"/c\"}").unwrap());
    let set: AgentSetupSet = serde_json::from_str("{\"configDir\":null,\"args\":[]}").unwrap();
    let got = setup_patched(kept.as_ref(), &set);
    assert_eq!(got.map_or("undefined".to_string(), |s| serde_json::to_string(&s).unwrap()), "{\"args\":[]}");
}

// setupPatched({"env":{"B":"1","A":"2"}}, {"env":{"B":"3","C":"4"}})
#[test]
fn setup_patched_a_variable_changed_keeps_its_place_and_a_new_one_goes_last() {
    let kept: Option<AgentSetup> = Some(serde_json::from_str("{\"env\":{\"B\":\"1\",\"A\":\"2\"}}").unwrap());
    let set: AgentSetupSet = serde_json::from_str("{\"env\":{\"B\":\"3\",\"C\":\"4\"}}").unwrap();
    let got = setup_patched(kept.as_ref(), &set);
    assert_eq!(
        got.map_or("undefined".to_string(), |s| serde_json::to_string(&s).unwrap()),
        "{\"env\":{\"B\":\"3\",\"A\":\"2\",\"C\":\"4\"}}"
    );
}

// setupPatched({"args":["a","b"],"program":"p"}, {"args":null})
#[test]
fn setup_patched_args_are_copied_and_null_args_go() {
    let kept: Option<AgentSetup> = Some(serde_json::from_str("{\"args\":[\"a\",\"b\"],\"program\":\"p\"}").unwrap());
    let set: AgentSetupSet = serde_json::from_str("{\"args\":null}").unwrap();
    let got = setup_patched(kept.as_ref(), &set);
    assert_eq!(got.map_or("undefined".to_string(), |s| serde_json::to_string(&s).unwrap()), "{\"program\":\"p\"}");
}

// agentEnvRefusal("WSP_X", "Claude Code", [])
#[test]
fn agent_env_refusal_0_wsp_x() {
    assert_eq!(
        agent_env_refusal("WSP_X", "Claude Code", &[]).unwrap_or_else(|| "null".to_string()),
        "WSP_X is wsp's own, so an agent's setup does not set it"
    );
}

// agentEnvRefusal("wsp_lower", "Claude Code", [])
#[test]
fn agent_env_refusal_1_wsp_lower() {
    assert_eq!(
        agent_env_refusal("wsp_lower", "Claude Code", &[]).unwrap_or_else(|| "null".to_string()),
        "wsp_lower is wsp's own, so an agent's setup does not set it"
    );
}

// agentEnvRefusal("PATH", "Claude Code", [])
#[test]
fn agent_env_refusal_2_path() {
    assert_eq!(
        agent_env_refusal("PATH", "Claude Code", &[]).unwrap_or_else(|| "null".to_string()),
        "PATH decides how Claude Code starts or what it loads, so an agent's setup does not set it"
    );
}

// agentEnvRefusal("path", "Claude Code", [])
#[test]
fn agent_env_refusal_3_path() {
    assert_eq!(
        agent_env_refusal("path", "Claude Code", &[]).unwrap_or_else(|| "null".to_string()),
        "path decides how Claude Code starts or what it loads, so an agent's setup does not set it"
    );
}

// agentEnvRefusal("LD_PRELOAD", "Codex", [])
#[test]
fn agent_env_refusal_4_ld_preload() {
    assert_eq!(
        agent_env_refusal("LD_PRELOAD", "Codex", &[]).unwrap_or_else(|| "null".to_string()),
        "LD_PRELOAD decides how Codex starts or what it loads, so an agent's setup does not set it"
    );
}

// agentEnvRefusal("dyld_insert_libraries", "Codex", [])
#[test]
fn agent_env_refusal_5_dyld_insert_libraries() {
    assert_eq!(
        agent_env_refusal("dyld_insert_libraries", "Codex", &[]).unwrap_or_else(|| "null".to_string()),
        "dyld_insert_libraries decides how Codex starts or what it loads, so an agent's setup does not set it"
    );
}

// agentEnvRefusal("NODE_OPTIONS", "Codex", [])
#[test]
fn agent_env_refusal_6_node_options() {
    assert_eq!(
        agent_env_refusal("NODE_OPTIONS", "Codex", &[]).unwrap_or_else(|| "null".to_string()),
        "NODE_OPTIONS decides how Codex starts or what it loads, so an agent's setup does not set it"
    );
}

// agentEnvRefusal("GIT_SSH_COMMAND", "Codex", [])
#[test]
fn agent_env_refusal_7_git_ssh_command() {
    assert_eq!(
        agent_env_refusal("GIT_SSH_COMMAND", "Codex", &[]).unwrap_or_else(|| "null".to_string()),
        "GIT_SSH_COMMAND decides how Codex starts or what it loads, so an agent's setup does not set it"
    );
}

// agentEnvRefusal("ANTHROPIC_API_KEY", "Claude Code", [/^CLAUDE_CODE_/, /^ANTHROPIC_API_KEY$/])
#[test]
fn agent_env_refusal_8_anthropic_api_key() {
    assert_eq!(
        agent_env_refusal(
            "ANTHROPIC_API_KEY",
            "Claude Code",
            &[Regex::new("^CLAUDE_CODE_").unwrap(), Regex::new("^ANTHROPIC_API_KEY$").unwrap()]
        )
        .unwrap_or_else(|| "null".to_string()),
        "ANTHROPIC_API_KEY never reaches Claude Code: its launch drops every variable of that kind"
    );
}

// agentEnvRefusal("CLAUDE_CODE_USE_BEDROCK", "Claude Code", [/^CLAUDE_CODE_/])
#[test]
fn agent_env_refusal_9_claude_code_use_bedrock() {
    assert_eq!(
        agent_env_refusal("CLAUDE_CODE_USE_BEDROCK", "Claude Code", &[Regex::new("^CLAUDE_CODE_").unwrap()])
            .unwrap_or_else(|| "null".to_string()),
        "CLAUDE_CODE_USE_BEDROCK never reaches Claude Code: its launch drops every variable of that kind"
    );
}

// agentEnvRefusal("MY_VAR", "Codex", [])
#[test]
fn agent_env_refusal_10_my_var() {
    assert_eq!(agent_env_refusal("MY_VAR", "Codex", &[]).unwrap_or_else(|| "null".to_string()), "null");
}

// agentEnvRefusal("MY_VAR", "Codex", [/^OTHER/])
#[test]
fn agent_env_refusal_11_my_var() {
    assert_eq!(agent_env_refusal("MY_VAR", "Codex", &[Regex::new("^OTHER").unwrap()]).unwrap_or_else(|| "null".to_string()), "null");
}

// agentEnvRefusal("HOMEWORK", "Codex", [])
#[test]
fn agent_env_refusal_12_homework() {
    assert_eq!(agent_env_refusal("HOMEWORK", "Codex", &[]).unwrap_or_else(|| "null".to_string()), "null");
}

// configDirRefusal("Claude Code", "~", {"folder":"/home/me","home":"/home/me","kept":[]})
#[test]
fn config_dir_refusal_0() {
    let real = ResolvedFolder { folder: "/home/me".to_string(), home: "/home/me".to_string(), kept: vec![] };
    assert_eq!(
        config_dir_refusal("Claude Code", "~", &real).unwrap_or_else(|| "null".to_string()),
        "Claude Code's config folder cannot be the home folder itself; name a folder under it"
    );
}

// configDirRefusal("Claude Code", "~/", {"folder":"/home/me/","home":"/home/me/","kept":[]})
#[test]
fn config_dir_refusal_1() {
    let real = ResolvedFolder { folder: "/home/me/".to_string(), home: "/home/me/".to_string(), kept: vec![] };
    assert_eq!(
        config_dir_refusal("Claude Code", "~/", &real).unwrap_or_else(|| "null".to_string()),
        "Claude Code's config folder cannot be the home folder itself; name a folder under it"
    );
}

// configDirRefusal("Claude Code", "/etc/claude", {"folder":"/etc/claude","home":"/home/me","kept":[]})
#[test]
fn config_dir_refusal_2() {
    let real = ResolvedFolder { folder: "/etc/claude".to_string(), home: "/home/me".to_string(), kept: vec![] };
    assert_eq!(
        config_dir_refusal("Claude Code", "/etc/claude", &real).unwrap_or_else(|| "null".to_string()),
        "Claude Code's config folder has to be under the home folder /home/me, and /etc/claude is not"
    );
}

// configDirRefusal("Claude Code", "/home/mexx/c", {"folder":"/home/mexx/c","home":"/home/me","kept":[]})
#[test]
fn config_dir_refusal_3() {
    let real = ResolvedFolder { folder: "/home/mexx/c".to_string(), home: "/home/me".to_string(), kept: vec![] };
    assert_eq!(
        config_dir_refusal("Claude Code", "/home/mexx/c", &real).unwrap_or_else(|| "null".to_string()),
        "Claude Code's config folder has to be under the home folder /home/me, and /home/mexx/c is not"
    );
}

// configDirRefusal("Claude Code", "~/.wsp/x", {"folder":"/home/me/.wsp/x","home":"/home/me","kept":["/home/me/.wsp"]})
#[test]
fn config_dir_refusal_4() {
    let real =
        ResolvedFolder { folder: "/home/me/.wsp/x".to_string(), home: "/home/me".to_string(), kept: vec!["/home/me/.wsp".to_string()] };
    assert_eq!(
        config_dir_refusal("Claude Code", "~/.wsp/x", &real).unwrap_or_else(|| "null".to_string()),
        "~/.wsp/x is inside wsp's own folder, so it is no agent's config folder"
    );
}

// configDirRefusal("Claude Code", "~/.wsp", {"folder":"/home/me/.wsp","home":"/home/me","kept":["/home/me/.wsp/"]})
#[test]
fn config_dir_refusal_5() {
    let real =
        ResolvedFolder { folder: "/home/me/.wsp".to_string(), home: "/home/me".to_string(), kept: vec!["/home/me/.wsp/".to_string()] };
    assert_eq!(
        config_dir_refusal("Claude Code", "~/.wsp", &real).unwrap_or_else(|| "null".to_string()),
        "~/.wsp is inside wsp's own folder, so it is no agent's config folder"
    );
}

// configDirRefusal("Claude Code", "~/c", {"folder":"/home/me/c","home":"/home/me//","kept":["/home/me/.wsp"]})
#[test]
fn config_dir_refusal_6() {
    let real = ResolvedFolder { folder: "/home/me/c".to_string(), home: "/home/me//".to_string(), kept: vec!["/home/me/.wsp".to_string()] };
    assert_eq!(config_dir_refusal("Claude Code", "~/c", &real).unwrap_or_else(|| "null".to_string()), "null");
}

// configDirRefusal("Codex", "~/.codex-work", {"folder":"/home/me/.codex-work","home":"/home/me","kept":[]})
#[test]
fn config_dir_refusal_7() {
    let real = ResolvedFolder { folder: "/home/me/.codex-work".to_string(), home: "/home/me".to_string(), kept: vec![] };
    assert_eq!(config_dir_refusal("Codex", "~/.codex-work", &real).unwrap_or_else(|| "null".to_string()), "null");
}

#[test]
fn two_lines_of_one_agent() {
    // noConfigDirLine("Cursor"), configDirSignInLine("Codex")
    assert_eq!(no_config_dir_line("Cursor"), "Cursor takes no config folder of its own, so there is nothing to point at one");
    assert_eq!(
        config_dir_sign_in_line("Codex"),
        "Codex keeps its sign-in under its config folder, so sign it in again there before its next thread"
    );
}

// pickRefusal({"message":"sonnet is not a model; the models are opus","kind":"usage","offered":1}, { source: "table" })
#[test]
fn pick_refusal_a_table_list_quoted() {
    let e =
        PickError { message: "sonnet is not a model; the models are opus".to_string(), kind: Some("usage".to_string()), offered: Some(1) };
    let got = pick_refusal(&e, Some(&catalog("table")));
    assert_eq!(format!("{} | {} | {}", got.message, got.fix.as_deref().unwrap_or("-"), got.kind.as_ref().map_or("-", |k| k.as_str())), "sonnet is not a model; the models are opus; that list is wsp's built-in one, since no agent there described itself, and the agent on that computer may take more. Drop the flag, or give it a value the agent offers. | Drop the flag, or give it a value the agent offers. | usage");
}

// pickRefusal({"message":"high is not an effort; opus takes none","kind":"usage","offered":0}, { source: "table" })
#[test]
fn pick_refusal_a_table_with_no_list() {
    let e = PickError { message: "high is not an effort; opus takes none".to_string(), kind: Some("usage".to_string()), offered: Some(0) };
    let got = pick_refusal(&e, Some(&catalog("table")));
    assert_eq!(format!("{} | {} | {}", got.message, got.fix.as_deref().unwrap_or("-"), got.kind.as_ref().map_or("-", |k| k.as_str())), "high is not an effort; opus takes none; wsp's built-in table says so, since no agent there described itself. Drop the flag, or give it a value the agent offers. | Drop the flag, or give it a value the agent offers. | usage");
}

// pickRefusal({"message":"sonnet is not a model; the models are opus","kind":"usage","offered":1}, { source: "harness" })
#[test]
fn pick_refusal_a_binary_that_answered() {
    let e =
        PickError { message: "sonnet is not a model; the models are opus".to_string(), kind: Some("usage".to_string()), offered: Some(1) };
    let got = pick_refusal(&e, Some(&catalog("harness")));
    assert_eq!(format!("{} | {} | {}", got.message, got.fix.as_deref().unwrap_or("-"), got.kind.as_ref().map_or("-", |k| k.as_str())), "sonnet is not a model; the models are opus. Drop the flag, or give it a value the agent offers. | Drop the flag, or give it a value the agent offers. | usage");
}

// pickRefusal({"message":"sonnet is not a model","kind":"usage","offered":2}, undefined)
#[test]
fn pick_refusal_no_catalog() {
    let e = PickError { message: "sonnet is not a model".to_string(), kind: Some("usage".to_string()), offered: Some(2) };
    let got = pick_refusal(&e, None);
    assert_eq!(format!("{} | {} | {}", got.message, got.fix.as_deref().unwrap_or("-"), got.kind.as_ref().map_or("-", |k| k.as_str())), "sonnet is not a model. Drop the flag, or give it a value the agent offers. | Drop the flag, or give it a value the agent offers. | usage");
}

// pickRefusal({"message":"--fast is not on this agent"}, { source: "table" })
#[test]
fn pick_refusal_no_kind_and_no_count() {
    let e = PickError { message: "--fast is not on this agent".to_string(), kind: None, offered: None };
    let got = pick_refusal(&e, Some(&catalog("table")));
    assert_eq!(format!("{} | {} | {}", got.message, got.fix.as_deref().unwrap_or("-"), got.kind.as_ref().map_or("-", |k| k.as_str())), "--fast is not on this agent; that list is wsp's built-in one, since no agent there described itself, and the agent on that computer may take more. Drop the flag, or give it a value the agent offers. | Drop the flag, or give it a value the agent offers. | -");
}

// pickRefusal({"message":"the agent has no fast mode","kind":"invalid"}, { source: "harness" })
#[test]
fn pick_refusal_an_invalid_kind() {
    let e = PickError { message: "the agent has no fast mode".to_string(), kind: Some("invalid".to_string()), offered: None };
    let got = pick_refusal(&e, Some(&catalog("harness")));
    assert_eq!(format!("{} | {} | {}", got.message, got.fix.as_deref().unwrap_or("-"), got.kind.as_ref().map_or("-", |k| k.as_str())), "the agent has no fast mode. Drop the flag, or give it a value the agent offers. | Drop the flag, or give it a value the agent offers. | invalid");
}

// notRunRefusal(lists v2.0.1/harness, table claude 2.1.0, "sonnet", "claude update")
#[test]
fn not_run_refusal_an_older_binary_told_to_update() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"2.0.1\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"haiku\",\"label\":\"Haiku\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let table = Some(catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"claude 2.1.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"sonnet\",\"label\":\"Sonnet\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\"}"));
    let got = not_run_refusal(Some(&lists), table.as_ref(), Some("sonnet"), Some("claude update"));
    assert_eq!(got.map_or("null".to_string(), |e| format!("{} | {} | {}", e.message, e.fix.as_deref().unwrap_or("-"), e.kind.as_ref().map_or("-", |k| k.as_str()))), "Claude Code 2.0.1 does not run Sonnet; a newer Claude Code does. Update it with claude update, or pick a model it lists. | Update it with claude update, or pick a model it lists. | usage");
}

// notRunRefusal(lists v2.0.1/harness, table claude 2.1.0, "sonnet", undefined)
#[test]
fn not_run_refusal_an_older_binary_with_no_update_road() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"2.0.1\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"haiku\",\"label\":\"Haiku\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let table = Some(catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"claude 2.1.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"sonnet\",\"label\":\"Sonnet\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\"}"));
    let got = not_run_refusal(Some(&lists), table.as_ref(), Some("sonnet"), None);
    assert_eq!(got.map_or("null".to_string(), |e| format!("{} | {} | {}", e.message, e.fix.as_deref().unwrap_or("-"), e.kind.as_ref().map_or("-", |k| k.as_str()))), "Claude Code 2.0.1 does not run Sonnet; it lists Opus (opus), Haiku (haiku). Pick a model it lists. | Pick a model it lists. | usage");
}

// notRunRefusal(lists v2.5.0/harness, table claude 2.1.0, "sonnet", "claude update")
#[test]
fn not_run_refusal_a_binary_newer_than_the_table() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"2.5.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"haiku\",\"label\":\"Haiku\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let table = Some(catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"claude 2.1.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"sonnet\",\"label\":\"Sonnet\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\"}"));
    let got = not_run_refusal(Some(&lists), table.as_ref(), Some("sonnet"), Some("claude update"));
    assert_eq!(got.map_or("null".to_string(), |e| format!("{} | {} | {}", e.message, e.fix.as_deref().unwrap_or("-"), e.kind.as_ref().map_or("-", |k| k.as_str()))), "Claude Code 2.5.0 does not run Sonnet; it lists Opus (opus), Haiku (haiku). Pick a model it lists. | Pick a model it lists. | usage");
}

// notRunRefusal(lists v2.0.1/harness, table claude 2.1.0, "opus", "claude update")
#[test]
fn not_run_refusal_a_binary_that_lists_the_model() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"2.0.1\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"haiku\",\"label\":\"Haiku\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let table = Some(catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"claude 2.1.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"sonnet\",\"label\":\"Sonnet\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\"}"));
    let got = not_run_refusal(Some(&lists), table.as_ref(), Some("opus"), Some("claude update"));
    assert_eq!(
        got.map_or("null".to_string(), |e| format!(
            "{} | {} | {}",
            e.message,
            e.fix.as_deref().unwrap_or("-"),
            e.kind.as_ref().map_or("-", |k| k.as_str())
        )),
        "null"
    );
}

// notRunRefusal(lists v2.0.1/harness, table claude 2.1.0, "gpt", "claude update")
#[test]
fn not_run_refusal_a_model_the_table_does_not_know_either() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"2.0.1\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"haiku\",\"label\":\"Haiku\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let table = Some(catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"claude 2.1.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"sonnet\",\"label\":\"Sonnet\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\"}"));
    let got = not_run_refusal(Some(&lists), table.as_ref(), Some("gpt"), Some("claude update"));
    assert_eq!(
        got.map_or("null".to_string(), |e| format!(
            "{} | {} | {}",
            e.message,
            e.fix.as_deref().unwrap_or("-"),
            e.kind.as_ref().map_or("-", |k| k.as_str())
        )),
        "null"
    );
}

// notRunRefusal(lists vclaude 2.1.0/table, table claude 2.1.0, "sonnet", "claude update")
#[test]
fn not_run_refusal_lists_that_are_the_tables_own() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"claude 2.1.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"sonnet\",\"label\":\"Sonnet\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\"}");
    let table = Some(catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"claude 2.1.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"sonnet\",\"label\":\"Sonnet\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\"}"));
    let got = not_run_refusal(Some(&lists), table.as_ref(), Some("sonnet"), Some("claude update"));
    assert_eq!(
        got.map_or("null".to_string(), |e| format!(
            "{} | {} | {}",
            e.message,
            e.fix.as_deref().unwrap_or("-"),
            e.kind.as_ref().map_or("-", |k| k.as_str())
        )),
        "null"
    );
}

// notRunRefusal(lists v2.0.1/harness, table claude 2.1.0, undefined, "claude update")
#[test]
fn not_run_refusal_no_model_named() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"2.0.1\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"haiku\",\"label\":\"Haiku\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let table = Some(catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"claude 2.1.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"sonnet\",\"label\":\"Sonnet\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\"}"));
    let got = not_run_refusal(Some(&lists), table.as_ref(), None, Some("claude update"));
    assert_eq!(
        got.map_or("null".to_string(), |e| format!(
            "{} | {} | {}",
            e.message,
            e.fix.as_deref().unwrap_or("-"),
            e.kind.as_ref().map_or("-", |k| k.as_str())
        )),
        "null"
    );
}

// notRunRefusal(lists v2.0.1/harness, table undefined, "sonnet", "claude update")
#[test]
fn not_run_refusal_no_table() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"2.0.1\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"haiku\",\"label\":\"Haiku\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let table = None;
    let got = not_run_refusal(Some(&lists), table.as_ref(), Some("sonnet"), Some("claude update"));
    assert_eq!(
        got.map_or("null".to_string(), |e| format!(
            "{} | {} | {}",
            e.message,
            e.fix.as_deref().unwrap_or("-"),
            e.kind.as_ref().map_or("-", |k| k.as_str())
        )),
        "null"
    );
}

// notRunRefusal(lists vnull/harness, table claude 2.1.0, "sonnet", "claude update")
#[test]
fn not_run_refusal_a_binary_with_no_version() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":null,\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"haiku\",\"label\":\"Haiku\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let table = Some(catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"claude 2.1.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"sonnet\",\"label\":\"Sonnet\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\"}"));
    let got = not_run_refusal(Some(&lists), table.as_ref(), Some("sonnet"), Some("claude update"));
    assert_eq!(
        got.map_or("null".to_string(), |e| format!(
            "{} | {} | {}",
            e.message,
            e.fix.as_deref().unwrap_or("-"),
            e.kind.as_ref().map_or("-", |k| k.as_str())
        )),
        "null"
    );
}

// notRunRefusal(lists v2.0.1/harness, table latest, "sonnet", "claude update")
#[test]
fn not_run_refusal_a_table_with_no_version_pin() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"2.0.1\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"haiku\",\"label\":\"Haiku\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let table = Some(catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"latest\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"sonnet\",\"label\":\"Sonnet\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\"}"));
    let got = not_run_refusal(Some(&lists), table.as_ref(), Some("sonnet"), Some("claude update"));
    assert_eq!(got.map_or("null".to_string(), |e| format!("{} | {} | {}", e.message, e.fix.as_deref().unwrap_or("-"), e.kind.as_ref().map_or("-", |k| k.as_str()))), "Claude Code 2.0.1 does not run Sonnet; it lists Opus (opus), Haiku (haiku). Pick a model it lists. | Pick a model it lists. | usage");
}

// notRunRefusal(lists v2.0.1/harness, table claude 2.1.0, "old", "claude update")
#[test]
fn not_run_refusal_a_legacy_model_of_the_table() {
    let lists = catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"2.0.1\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"},{\"value\":\"haiku\",\"label\":\"Haiku\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let table = Some(catalog_json("{\"harness\":\"claude\",\"label\":\"Claude Code\",\"version\":\"claude 2.1.0\",\"models\":[{\"value\":\"opus\",\"label\":\"Opus\"}],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"table\",\"legacyModels\":[{\"value\":\"old\",\"label\":\"Old one\"}]}"));
    let got = not_run_refusal(Some(&lists), table.as_ref(), Some("old"), Some("claude update"));
    assert_eq!(got.map_or("null".to_string(), |e| format!("{} | {} | {}", e.message, e.fix.as_deref().unwrap_or("-"), e.kind.as_ref().map_or("-", |k| k.as_str()))), "Claude Code 2.0.1 does not run Old one; a newer Claude Code does. Update it with claude update, or pick a model it lists. | Update it with claude update, or pick a model it lists. | usage");
}

// accessWordRefusal({"label":"Codex","access":{"ask":"on-request","full":"never","plan":"not-listed"},"modes":["on-request","never","stray"]}, "ask")
#[test]
fn access_word_refusal_a_word_the_agent_maps() {
    let catalog = catalog_json("{\"harness\":\"claude\",\"label\":\"Codex\",\"version\":null,\"models\":[],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[{\"value\":\"on-request\",\"label\":\"On request\"},{\"value\":\"never\",\"label\":\"Never\"},{\"value\":\"stray\",\"label\":\"Stray\"}],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\",\"access\":{\"ask\":\"on-request\",\"full\":\"never\",\"plan\":\"not-listed\"}}");
    let got = access_word_refusal(&catalog, serde_json::from_value(json!("ask")).unwrap());
    assert_eq!(
        got.map_or("null".to_string(), |e| format!(
            "{} | {} | {}",
            e.message,
            e.fix.as_deref().unwrap_or("-"),
            e.kind.as_ref().map_or("-", |k| k.as_str())
        )),
        "null"
    );
}

// accessWordRefusal({"label":"Codex","access":{"ask":"on-request","full":"never","plan":"not-listed"},"modes":["on-request","never","stray"]}, "plan")
#[test]
fn access_word_refusal_a_word_mapped_to_a_mode_it_does_not_list() {
    let catalog = catalog_json("{\"harness\":\"claude\",\"label\":\"Codex\",\"version\":null,\"models\":[],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[{\"value\":\"on-request\",\"label\":\"On request\"},{\"value\":\"never\",\"label\":\"Never\"},{\"value\":\"stray\",\"label\":\"Stray\"}],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\",\"access\":{\"ask\":\"on-request\",\"full\":\"never\",\"plan\":\"not-listed\"}}");
    let got = access_word_refusal(&catalog, serde_json::from_value(json!("plan")).unwrap());
    assert_eq!(got.map_or("null".to_string(), |e| format!("{} | {} | {}", e.message, e.fix.as_deref().unwrap_or("-"), e.kind.as_ref().map_or("-", |k| k.as_str()))), "Codex takes no plan access; it takes ask, full. Name one it takes, or drop --access. | Name one it takes, or drop --access. | usage");
}

// accessWordRefusal({"label":"Codex","access":{"ask":"on-request","full":"never","plan":"not-listed"},"modes":["on-request","never","stray"]}, "auto-edit")
#[test]
fn access_word_refusal_a_word_with_no_mapping() {
    let catalog = catalog_json("{\"harness\":\"claude\",\"label\":\"Codex\",\"version\":null,\"models\":[],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[{\"value\":\"on-request\",\"label\":\"On request\"},{\"value\":\"never\",\"label\":\"Never\"},{\"value\":\"stray\",\"label\":\"Stray\"}],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\",\"access\":{\"ask\":\"on-request\",\"full\":\"never\",\"plan\":\"not-listed\"}}");
    let got = access_word_refusal(&catalog, serde_json::from_value(json!("auto-edit")).unwrap());
    assert_eq!(got.map_or("null".to_string(), |e| format!("{} | {} | {}", e.message, e.fix.as_deref().unwrap_or("-"), e.kind.as_ref().map_or("-", |k| k.as_str()))), "Codex takes no auto-edit access; it takes ask, full. Name one it takes, or drop --access. | Name one it takes, or drop --access. | usage");
}

// accessWordRefusal({"label":"Codex","modes":["on-request","never","stray"]}, "full")
#[test]
fn access_word_refusal_an_agent_that_maps_none() {
    let catalog = catalog_json("{\"harness\":\"claude\",\"label\":\"Codex\",\"version\":null,\"models\":[],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[{\"value\":\"on-request\",\"label\":\"On request\"},{\"value\":\"never\",\"label\":\"Never\"},{\"value\":\"stray\",\"label\":\"Stray\"}],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\"}");
    let got = access_word_refusal(&catalog, serde_json::from_value(json!("full")).unwrap());
    assert_eq!(
        got.map_or("null".to_string(), |e| format!(
            "{} | {} | {}",
            e.message,
            e.fix.as_deref().unwrap_or("-"),
            e.kind.as_ref().map_or("-", |k| k.as_str())
        )),
        "Codex takes no access of wsp's words. Name one it takes, or drop --access. | Name one it takes, or drop --access. | usage"
    );
}

// accessWordRefusal({"label":"Codex","access":{},"modes":["on-request","never","stray"]}, "ask")
#[test]
fn access_word_refusal_an_agent_with_modes_and_no_map() {
    let catalog = catalog_json("{\"harness\":\"claude\",\"label\":\"Codex\",\"version\":null,\"models\":[],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[{\"value\":\"on-request\",\"label\":\"On request\"},{\"value\":\"never\",\"label\":\"Never\"},{\"value\":\"stray\",\"label\":\"Stray\"}],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\",\"access\":{}}");
    let got = access_word_refusal(&catalog, serde_json::from_value(json!("ask")).unwrap());
    assert_eq!(
        got.map_or("null".to_string(), |e| format!(
            "{} | {} | {}",
            e.message,
            e.fix.as_deref().unwrap_or("-"),
            e.kind.as_ref().map_or("-", |k| k.as_str())
        )),
        "Codex takes no access of wsp's words. Name one it takes, or drop --access. | Name one it takes, or drop --access. | usage"
    );
}

// accessWordRefusal({"label":"Codex","access":{"ask":"on-request","full":"never","plan":"not-listed"},"modes":["on-request","never","stray"]}, "full")
#[test]
fn access_word_refusal_the_full_word() {
    let catalog = catalog_json("{\"harness\":\"claude\",\"label\":\"Codex\",\"version\":null,\"models\":[],\"efforts\":[],\"contextWindows\":[],\"permissionModes\":[{\"value\":\"on-request\",\"label\":\"On request\"},{\"value\":\"never\",\"label\":\"Never\"},{\"value\":\"stray\",\"label\":\"Stray\"}],\"steers\":true,\"renames\":true,\"images\":true,\"source\":\"harness\",\"access\":{\"ask\":\"on-request\",\"full\":\"never\",\"plan\":\"not-listed\"}}");
    let got = access_word_refusal(&catalog, serde_json::from_value(json!("full")).unwrap());
    assert_eq!(
        got.map_or("null".to_string(), |e| format!(
            "{} | {} | {}",
            e.message,
            e.fix.as_deref().unwrap_or("-"),
            e.kind.as_ref().map_or("-", |k| k.as_str())
        )),
        "null"
    );
}
