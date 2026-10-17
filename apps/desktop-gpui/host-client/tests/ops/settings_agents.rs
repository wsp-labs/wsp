// SPDX-License-Identifier: AGPL-3.0-only
use serde_json::json;
use wsp_host_client::protocol::*;

use crate::common::{answered, ask, refused, shared};

/// The test host stages no wsp-daemon, and every sign-in runs in a terminal the daemon opens.
const NO_DAEMON: &str = "the daemon for this computer did not start";
const NO_SIGN_IN: &str = "That sign-in is not running any more.";

#[tokio::test]
async fn report_and_setup() {
    let client = shared().host.client().await;
    let report = answered("agents.read", client.call(ask::<AgentsReadRequest>(json!({ "target": { "placeId": "here" } }))).await);
    assert!(report.report.agents.iter().any(|a| a.id == "claude" && a.installed), "{:?}", report.report.agents);

    let set = answered(
        "agents.setup",
        client.call(ask::<AgentsSetupRequest>(json!({ "placeId": "here", "agent": "opencode", "args": ["--sa"] }))).await,
    );
    assert_eq!(set.agent.setup.map(|s| s.args), Some(vec!["--sa".to_owned()]));
    let mut clear = ask::<AgentsSetupRequest>(json!({ "placeId": "here", "agent": "opencode" }));
    clear.args = Some(AgentsSetupRequestArgsOrNull(None));
    let cleared = answered("agents.setup", client.call(clear).await);
    assert!(cleared.agent.setup.is_some_and(|s| s.args.is_empty()));
}

#[tokio::test]
async fn keys_tools_and_broker() {
    let client = shared().host.client().await;
    answered("agents.key", client.call(ask::<AgentsKeyRequest>(json!({ "agent": "gemini", "key": "settings-agents-key" }))).await);
    refused(
        "agents.key",
        client.call(ask::<AgentsKeyRequest>(json!({ "agent": "claude", "key": "not-a-token" }))).await,
        "That is not a Claude Code token.",
    );

    let tools = answered(
        "agents.addTools",
        client.call(ask::<AgentsAddToolsRequest>(json!({ "target": { "placeId": "here" }, "agent": "opencode" }))).await,
    );
    assert_eq!(tools.file, "~/.config/opencode/opencode.json");

    answered("broker.status", client.call(ask::<BrokerStatusRequest>(json!({}))).await);
    let works =
        answered("providers.checkKey", client.call(ask::<ProvidersCheckKeyRequest>(json!({ "provider": "fake", "key": "k" }))).await);
    assert!(works.works);
}

#[tokio::test]
async fn servers() {
    let host = &shared().host;
    let folder = host.repo("settings_agents-servers");
    host.wsp(&["add", &folder.to_string_lossy()]);
    let client = host.client().await;
    let project =
        client.projects().await.unwrap().into_iter().find(|p| p.name == "settings_agents-servers").expect("the project just added").id;
    let target = json!({ "placeId": "here", "project": project });
    let server = json!({ "target": target, "agent": "opencode", "name": "sa-echo", "scope": "project" });
    let with = |extra: serde_json::Value| {
        let mut ask = server.clone();
        ask.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        ask
    };

    let add = json!({ "target": target, "agent": "opencode", "name": "sa-echo", "project": true, "command": "echo", "args": ["hi"] });
    let added = answered("servers.add", client.call(ask::<ServersAddRequest>(add)).await);
    assert_eq!(added.file, "~/settings_agents-servers/opencode.json");
    assert!(std::fs::read_to_string(folder.join("opencode.json")).unwrap().contains("sa-echo"));

    let tools = answered("servers.tools", client.call(ask::<ServersToolsRequest>(with(json!({})))).await);
    assert!(tools.answer.refused.is_some(), "echo answers no tools/list: {:?}", tools.answer);

    answered("servers.toggle", client.call(ask::<ServersToggleRequest>(with(json!({ "on": false })))).await);
    answered("servers.toggle", client.call(ask::<ServersToggleRequest>(with(json!({ "on": true })))).await);
    refused("servers.signIn", client.call(ask::<ServersSignInRequest>(with(json!({})))).await, NO_DAEMON);

    answered("servers.remove", client.call(ask::<ServersRemoveRequest>(with(json!({})))).await);
    assert!(!std::fs::read_to_string(folder.join("opencode.json")).unwrap().contains("sa-echo"));

    // A name only this network knows never leaves the host, so the answer is no icon and nothing is asked of Google.
    let icon = answered("servers.icon", client.call(ask::<ServersIconRequest>(json!({ "host": "mcp.localhost" }))).await);
    assert_eq!(icon.icon, None);
}

#[tokio::test]
async fn skills() {
    let client = shared().host.client().await;
    // An empty query and an id that is not <owner>/<repo>/<skill> are refused before skills.sh is asked.
    refused("skills.search", client.call(ask::<SkillsSearchRequest>(json!({ "q": " " }))).await, "Type something to search skills.sh for.");
    refused(
        "skills.get",
        client.call(ask::<SkillsGetRequest>(json!({ "skill": "settings-agents" }))).await,
        "is not a skill skills.sh names",
    );
    let add = json!({ "target": { "placeId": "here" }, "skill": "settings-agents", "agents": ["claude"] });
    refused("skills.add", client.call(ask::<SkillsAddRequest>(add)).await, "is not a skill skills.sh names");

    let name = "settings-agents-skill";
    let text = format!("---\nname: {name}\ndescription: a skill the settings_agents test made\n---\nbody\n");
    let dir = shared().host.dir.path().join(".claude/skills").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), &text).unwrap();
    let skill = json!({ "target": { "placeId": "here" }, "name": name });
    let with = |on: bool| {
        let mut ask = skill.clone();
        ask["on"] = json!(on);
        ask
    };

    let preview = answered("skills.preview", client.call(ask::<SkillsPreviewRequest>(skill.clone())).await);
    assert_eq!(preview.preview.text, text);
    let off = answered("skills.toggle", client.call(ask::<SkillsToggleRequest>(with(false))).await);
    assert!(!off.paths.is_empty());
    answered("skills.toggle", client.call(ask::<SkillsToggleRequest>(with(true))).await);
    let removed = answered("skills.remove", client.call(ask::<SkillsRemoveRequest>(skill)).await);
    assert!(!removed.removed.is_empty());
    assert!(!dir.exists());
}

#[tokio::test]
async fn plugins() {
    let home = shared().host.dir.path();
    let id = "settings-agents@settings-agents-market";
    let folder = home.join(".claude/plugins/cache/settings-agents-market/settings-agents");
    std::fs::create_dir_all(folder.join(".claude-plugin")).unwrap();
    std::fs::write(folder.join(".claude-plugin/plugin.json"), r#"{"name":"settings-agents"}"#).unwrap();
    let index = home.join(".claude/plugins/installed_plugins.json");
    let mut installed: serde_json::Value =
        std::fs::read_to_string(&index).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({ "version": 2, "plugins": {} }));
    installed["plugins"][id] = json!([{ "scope": "user", "installPath": folder, "version": "1.0.0" }]);
    std::fs::write(&index, installed.to_string()).unwrap();

    let client = shared().host.client().await;
    let toggle =
        |on: bool| ask::<PluginsToggleRequest>(json!({ "target": { "placeId": "here" }, "agent": "claude", "plugin": id, "on": on }));
    let off = answered("plugins.toggle", client.call(toggle(false)).await);
    assert!(off.plugin.id == id && !off.plugin.on, "{:?}", off.plugin);
    let on = answered("plugins.toggle", client.call(toggle(true)).await);
    assert!(on.plugin.on);
}

#[tokio::test]
async fn sign_ins() {
    let client = shared().host.client().await;
    let target = json!({ "placeId": "here" });
    refused("agents.signIn", client.call(ask::<AgentsSignInRequest>(json!({ "target": target, "agent": "claude" }))).await, NO_DAEMON);
    refused("agents.cloudSignIn", client.call(ask::<AgentsCloudSignInRequest>(json!({ "agent": "codex" }))).await, NO_DAEMON);
    let runs = answered("agents.signIns", client.call(ask::<AgentsSignInsRequest>(json!({}))).await);
    assert!(runs.runs.iter().all(|r| r.ended == Some(true) || r.agent != "claude"), "{:?}", runs.runs);
    // A socket types into or stops only a sign-in it started, and none started here.
    refused(
        "agents.signInCode",
        client.call(ask::<AgentsSignInCodeRequest>(json!({ "signInId": "si_none", "code": "1234" }))).await,
        NO_SIGN_IN,
    );
    refused("agents.signInStop", client.call(ask::<AgentsSignInStopRequest>(json!({ "signInId": "si_none" }))).await, NO_SIGN_IN);
}
