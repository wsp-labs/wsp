// SPDX-License-Identifier: AGPL-3.0-only
//! spec/agents_read_targets.md: where a read runs, the report, and every write there, ported from the reads, skills,
//! servers and projects groups of packages/runtime/test/agents-read.test.ts (25 cases, the sign-ins group is in
//! agents_read_sign_ins.rs).
mod agents_read_support;
mod rest_support;

use indexmap::IndexMap;
use serde_json::{json, Value};

use agents_read_support::*;
use wsp_machines::agents_read::{
    page_reach_of, project_of, AgentsOn, AgentsReadOptions, AgentsReads, ServerToolsAsk, SkillAddAsk, SkillAsk, NO_AGENTS_READER,
};
use wsp_machines::places::types::NO_PLACE_DOOR;
use wsp_wire::types::agents_report::{AgentSignInState, PageReach, PluginAsk, ServerAdd, ServerAsk};
use wsp_wire::types::thread_defaults::{AgentSetupSet, AgentSetupView};
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_wire::wire::capabilities::PauseMode;
use wsp_wire::wire::machine_link::no_such_place_refusal;
use wsp_words::agents_report::{
    napping_agents_refusal, napping_plugins_refusal, napping_servers_refusal, napping_sign_in_refusal, napping_skills_refusal,
    napping_tools_refusal, no_such_agents_project_refusal, provider_agents_refusal, shared_agents_project_refusal,
};
use wsp_words::place_name::table_name;
use wsp_words::place_word::PlaceRef;
use wsp_words::words::computer::THIS_COMPUTER;

fn spoo() -> wsp_wire::types::agents_report::AgentsProject {
    project("pr_spoo", "spoo", "/home/ada/spoo")
}

fn www() -> wsp_wire::types::agents_report::AgentsProject {
    project("pr_www", "www", "/home/ada/www")
}

fn landing_json() -> Value {
    js(&landing())
}

fn full_report(target: Value, reach: &str) -> Value {
    json!({ "target": target, "home": "/root", "user": "root", "readAt": READ_AT, "agents": [], "skills": [], "servers": [], "refused": [], "reach": reach })
}

fn machine_on_json() -> Value {
    json!({ "kind": "machine", "machine": "m_landing", "projects": [landing_json()] })
}

fn said(id: &str, name: &str, kind: &str) -> String {
    table_name(&PlaceRef { id: id.into(), name: name.into(), kind: Some(kind.into()) })
}

fn door_lines(rig: &Rig) -> Vec<String> {
    rig.lines_of(&["door.", "reader."])
}

fn list_line(now: i64) -> String {
    format!("door.list now={now}")
}

fn ids_line(place: &str) -> String {
    format!("door.sign_ins_at {place} {}", CATALOG_IDS.join(","))
}

fn map(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn skill(name: &str) -> SkillAsk {
    SkillAsk { name: name.to_string(), project: None }
}

fn skill_in_project(name: &str) -> SkillAsk {
    SkillAsk { name: name.to_string(), project: Some(true) }
}

/// The projects group's host: a joined computer named box, the projects held on it and on this computer, at T1.
fn computer(held_pl: Vec<wsp_wire::types::agents_report::AgentsProject>, held_here: Vec<wsp_wire::types::agents_report::AgentsProject>) -> Rig {
    let rig = Rig::at(T1);
    rig.door_on.set(true);
    rig.rows.borrow_mut().push(place_row("pl_1", "box", "computer"));
    rig.hold("pl_1", held_pl);
    rig.hold("here", held_here);
    rig
}

// A1: agents-read.test.ts:572
#[test]
fn a_write_in_one_project_is_the_computers_change() {
    run(async {
        let rig = computer(vec![spoo(), www()], vec![www()]);
        let api = rig.reads();
        api.changed(None);
        api.changed(Some(&ws("ws_1")));
        api.changed(Some(&target(json!({ "placeId": "pl_1", "project": "pr_www" }))));
        api.changed(Some(&pl("pl_1")));
        assert_eq!(
            rig.changed_json(),
            vec![Value::Null, json!({ "workspaceId": "ws_1" }), json!({ "placeId": "pl_1" }), json!({ "placeId": "pl_1" })]
        );

        rig.changed.borrow_mut().clear();
        let removed = api.skills_remove(&target(json!({ "placeId": "pl_1", "project": "pr_www" })), &skill_in_project("deploy"), None).await;
        assert_eq!(removed.unwrap(), vec!["~/.agents/skills/pdf".to_string()]);
        assert_eq!(rig.changed_json(), vec![json!({ "placeId": "pl_1" })]);
    });
}

macro_rules! refused_without_port {
    ($tweak:expr, $api:ident => $call:expr) => {{
        let rig = Rig::new();
        let mut o = rig.options();
        $tweak(&mut o, &rig);
        let $api = AgentsReads::new(o);
        match $call.await {
            Err(e) => assert_eq!(e, plain_error(NO_AGENTS_READER), "{}", stringify!($call)),
            Ok(_) => panic!("{} answered", stringify!($call)),
        }
        assert_eq!(rig.lines(), Vec::<String>::new(), "{} read something first", stringify!($call));
    }};
}

fn no_reader(o: &mut AgentsReadOptions, _: &Rig) {
    o.reader = None;
}
fn no_server_road(_: &mut AgentsReadOptions, rig: &Rig) {
    rig.has_server.set(false);
}
fn no_acts(o: &mut AgentsReadOptions, _: &Rig) {
    o.acts = None;
}
fn no_skills(o: &mut AgentsReadOptions, _: &Rig) {
    o.skills = None;
}
fn no_servers(o: &mut AgentsReadOptions, _: &Rig) {
    o.servers = None;
}
fn no_plugins(o: &mut AgentsReadOptions, _: &Rig) {
    o.plugins = None;
}
fn no_setup_write(o: &mut AgentsReadOptions, _: &Rig) {
    o.setup_write = None;
}

// A2: agents-read.test.ts:516
#[test]
fn every_road_refuses_without_its_port_before_reading() {
    run(async {
        let t = ws("ws_1");
        let on_here = pl("here");
        refused_without_port!(no_reader, api => api.read(&t, None));
        refused_without_port!(no_reader, api => api.tools(&t, "claude", "db", None, None));
        refused_without_port!(no_reader, api => api.server(&t, "claude", "db", None));
        refused_without_port!(no_server_road, api => api.server(&t, "claude", "db", None));

        refused_without_port!(no_acts, api => api.add_tools(&t, "codex", None));
        refused_without_port!(no_acts, api => api.key("claude", "sk-x", None));
        refused_without_port!(no_acts, api => api.sign_in(t.clone(), ask("codex"), no_follower(), None));
        refused_without_port!(no_acts, api => api.sign_in_line(&t, &ask("codex"), None));

        refused_without_port!(no_skills, api => api.skills_search("pdf", None));
        refused_without_port!(no_skills, api => api.skills_get("a/b/pdf"));
        refused_without_port!(no_skills, api => api.skills_preview(&t, &skill("pdf"), None));
        refused_without_port!(no_skills, api => api.skills_add(&t, &SkillAddAsk { skill: "a/b/pdf".into(), agents: None, project: None }, None));
        refused_without_port!(no_skills, api => api.skills_remove(&t, &skill("pdf"), None));
        refused_without_port!(no_skills, api => api.skills_toggle(&t, &skill("pdf"), false, None));

        let add: ServerAdd = serde_json::from_value(json!({ "agent": "claude", "name": "acme", "command": "npx" })).unwrap();
        let ask_server: ServerAsk = serde_json::from_value(json!({ "agent": "claude", "name": "acme" })).unwrap();
        refused_without_port!(no_servers, api => api.servers_add(&on_here, &add, None));
        refused_without_port!(no_servers, api => api.servers_remove(&t, &ask_server, None));
        refused_without_port!(no_servers, api => api.servers_toggle(&t, &ask_server, true, None));

        let plugin = PluginAsk { agent: "claude".into(), plugin: "pdf-tools@acme".into() };
        refused_without_port!(no_plugins, api => api.plugins_toggle(&t, &plugin, true, None));

        let change: AgentSetupSet = serde_json::from_value(json!({ "on": true })).unwrap();
        refused_without_port!(no_setup_write, api => api.setup("here", "claude", change.clone(), None));
    });
}

// A3: agents-read.test.ts:31, 79, 170
#[test]
fn a_workspace_reads_off_its_machine_with_its_project() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let origin = Caller::Road(WorkspaceOrigin::Paired);
        let first = api.read(&ws("ws_1"), Some(&origin)).await.unwrap();
        assert_eq!(js(&first), full_report(json!({ "workspaceId": "ws_1" }), "none"));
        assert_eq!(rig.asked_on(), vec![machine_on_json()]);
        assert_eq!(*rig.workspace_origins.borrow(), vec![Some(origin)]);

        let rig = Rig::new();
        rig.relayed.set(Some(true));
        rig.brokered.set(Some(true));
        *rig.ws_stores.borrow_mut() = Some(map(&[("claude", "/kept/claude")]));
        *rig.ws_seeds.borrow_mut() = Some(map(&[("claude", "/seed/claude")]));
        rig.reads().read(&ws("ws_1"), None).await.unwrap();
        assert_eq!(
            rig.asked_on(),
            vec![json!({
                "kind": "machine", "machine": "m_landing", "projects": [landing_json()], "relayed": true, "brokered": true,
                "stores": { "claude": "/kept/claude" }, "seeds": { "claude": "/seed/claude" },
            })]
        );

        let rig = Rig::new();
        rig.relayed.set(Some(false));
        rig.brokered.set(Some(false));
        rig.reads().read(&ws("ws_1"), None).await.unwrap();
        assert_eq!(rig.asked_on(), vec![machine_on_json()]);
    });
}

// A3: agents-read.test.ts:42
#[test]
fn a_workspace_on_this_computer_reads_as_this_computer() {
    run(async {
        let rig = Rig::new();
        rig.local.set(true);
        let api = rig.reads();
        let report = api.read(&ws("ws_3"), None).await.unwrap();
        assert_eq!(rig.asked_on(), vec![json!({ "kind": "here", "projects": [landing_json()] })]);
        assert_eq!(js(&report)["reach"], json!("here"));

        let rig = Rig::new();
        rig.local.set(true);
        *rig.stores_here.borrow_mut() = Some(Ok(map(&[("claude", "/kept/claude")])));
        rig.reads().read(&ws("ws_3"), None).await.unwrap();
        assert_eq!(rig.asked_on(), vec![json!({ "kind": "here", "projects": [landing_json()], "stores": { "claude": "/kept/claude" } })]);

        let rig = Rig::new();
        rig.local.set(true);
        *rig.stores_here.borrow_mut() = Some(Err(plain_error("a kept folder the setup now refuses")));
        rig.reads().read(&ws("ws_3"), None).await.unwrap();
        assert_eq!(rig.asked_on(), vec![json!({ "kind": "here", "projects": [landing_json()] })]);
    });
}

// A3: agents-read.test.ts:179, 227
#[test]
fn a_joined_computer_reads_through_the_door_with_its_login_and_sign_ins() {
    run(async {
        let rig = Rig::new();
        rig.door_on.set(true);
        let setup_row = json!({ "id": "claude", "label": "Claude Code", "outcome": "installed" });
        rig.rows.borrow_mut().push(place_row_with(
            "pl_1",
            "spoo",
            json!({ "applied": { "hash": "h1", "at": "2026-09-24T11:00:00.000Z", "rows": [setup_row.clone()] }, "logins": "claude, codex" }),
        ));
        *rig.report.borrow_mut() = Some(report(json!({ "agentVersions": { "claude": "2.1.0" } })));
        let mut signed = IndexMap::new();
        signed.insert("claude".to_string(), AgentSignInState::SignedIn);
        signed.insert("codex".to_string(), AgentSignInState::None);
        *rig.sign_ins.borrow_mut() = Some(signed);
        *rig.place_stores.borrow_mut() = Some(map(&[("claude", "/home/ada/.claude-x")]));
        rig.hold("pl_1", vec![spoo()]);
        let api = rig.reads();
        let relay = FakeRelay::new(&rig, "pl_1");
        let _off = api.forwards(relay.clone());

        api.read(&pl("pl_1"), None).await.unwrap();
        assert_eq!(
            rig.asked_on(),
            vec![json!({
                "kind": "box", "name": "spoo", "login": { "HOME": "/home/ada", "PATH": "/usr/bin" },
                "signIns": { "claude": "signed-in", "codex": "none" }, "versions": { "claude": "2.1.0" },
                "setupRows": [setup_row], "logins": "claude, codex", "stores": { "claude": "/home/ada/.claude-x" },
                "relayed": true, "projects": [js(&spoo())],
            })]
        );
        assert_eq!(
            door_lines(&rig),
            vec![list_line(T0), "door.report_of pl_1".to_string(), "door.logins_again pl_1".to_string(), ids_line("pl_1"), "reader.read box latest=true".to_string()]
        );
        assert_eq!(rig.lines_of(&["place_stores"]), vec!["place_stores pl_1 /home/ada".to_string()]);

        let exec = match &rig.read_calls.borrow()[0].0 {
            AgentsOn::Box { exec, .. } => exec.clone(),
            _ => panic!("a box"),
        };
        let done = exec("uname -a".to_string(), Some(5000), Some(vec![1, 2])).await.unwrap();
        assert_eq!(js(&done), json!({ "exitCode": 0, "stdout": "", "stderr": "" }));
        assert_eq!(rig.lines_of(&["door.exec"]), vec!["door.exec pl_1 uname -a timeout=Some(5000) stdin=Some([1, 2])".to_string()]);

        rig.clear_log();
        api.tools(&pl("pl_1"), "claude", "db", None, None).await.unwrap();
        assert_eq!(door_lines(&rig), vec![list_line(T0), "door.report_of pl_1".to_string(), ids_line("pl_1"), "reader.tools box {\"placeId\":\"pl_1\"}".to_string()]);
    });
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        rig.reads().read(&pl("pl_1"), None).await.unwrap();
        assert_eq!(rig.asked_on(), vec![json!({ "kind": "box", "name": "spoo", "login": {}, "projects": [] })]);
    });
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        *rig.report.borrow_mut() = Some(report(json!({ "login": { "HOME": "/root" } })));
        *rig.place_stores.borrow_mut() = Some(map(&[("codex", "/root/.codex-x")]));
        rig.reads().read(&pl("pl_1"), None).await.unwrap();
        assert_eq!(
            rig.asked_on(),
            vec![json!({ "kind": "box", "name": "spoo", "login": { "HOME": "/root" }, "stores": { "codex": "/root/.codex-x" }, "projects": [] })]
        );
        assert_eq!(rig.lines_of(&["place_stores"]), vec!["place_stores pl_1 /root".to_string()]);

        let rig = Rig::new();
        rig.with_spoo();
        *rig.report.borrow_mut() = Some(report(json!({ "login": { "USER": "ada" } })));
        *rig.place_stores.borrow_mut() = Some(map(&[("codex", "/root/.codex-x")]));
        rig.reads().read(&pl("pl_1"), None).await.unwrap();
        assert_eq!(rig.asked_on(), vec![json!({ "kind": "box", "name": "spoo", "login": {}, "projects": [] })]);
        assert_eq!(rig.lines_of(&["place_stores"]), Vec::<String>::new());
    });
}

// A3: agents-read.test.ts:580
#[test]
fn a_computer_the_door_does_not_list_is_refused_usage() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        rig.rows.borrow_mut().push(place_row("pl_b", "boat", "provider"));
        let api = rig.reads();
        let want = usage(&no_such_place_refusal("pl_9", &[said("pl_1", "spoo", "computer"), said("pl_b", "boat", "provider")]));
        assert_eq!(api.read(&pl("pl_9"), None).await.err(), Some(want.clone()));
        assert_eq!(api.tools(&pl("pl_9"), "claude", "db", None, None).await.err(), Some(want));
        assert_eq!(door_lines(&rig), vec![list_line(T0), list_line(T0)]);
    });
}

// A3: agents-read.test.ts:179
#[test]
fn a_provider_place_is_refused_usage() {
    run(async {
        let rig = Rig::new();
        rig.door_on.set(true);
        rig.rows.borrow_mut().push(place_row("pl_b", "boat", "provider"));
        let api = rig.reads();
        let want = usage(&provider_agents_refusal(&said("pl_b", "boat", "provider")));
        assert_eq!(api.read(&pl("pl_b"), None).await.err(), Some(want.clone()));
        assert_eq!(api.tools(&pl("pl_b"), "claude", "db", None, None).await.err(), Some(want));
        assert_eq!(door_lines(&rig), vec![list_line(T0), list_line(T0)]);
    });
}

// A3: agents-read.test.ts:516
#[test]
fn no_door_fails_no_place_door() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        assert_eq!(api.read(&pl("pl_1"), None).await.err(), Some(plain_error(NO_PLACE_DOOR)));
        assert_eq!(rig.lines_of(&["reader."]), Vec::<String>::new());

        api.read(&pl("here"), None).await.unwrap();
        assert_eq!(rig.asked_on(), vec![json!({ "kind": "here", "projects": [] })]);
    });
}

// A3: agents-read.test.ts:49
#[test]
fn this_computers_stores_are_left_out_when_empty_or_failing() {
    run(async {
        let cases: Vec<(Option<Result<IndexMap<String, String>, wsp_words::exit::HostError>>, Value)> = vec![
            (Some(Ok(map(&[("claude", "/kept/claude"), ("codex", "/kept/codex")]))), json!({ "kind": "here", "stores": { "claude": "/kept/claude", "codex": "/kept/codex" }, "projects": [] })),
            (Some(Ok(IndexMap::new())), json!({ "kind": "here", "projects": [] })),
            (Some(Err(plain_error("a kept folder the setup now refuses"))), json!({ "kind": "here", "projects": [] })),
            (None, json!({ "kind": "here", "projects": [] })),
        ];
        for (stores, want) in cases {
            let rig = Rig::new();
            *rig.stores_here.borrow_mut() = stores;
            rig.reads().read(&pl("here"), None).await.unwrap();
            assert_eq!(rig.asked_on(), vec![want]);
        }
    });
}

// A4: agents-read.test.ts:562
#[test]
fn a_read_of_a_computer_covers_every_project_held_there() {
    run(async {
        let rig = computer(vec![spoo(), www()], vec![www()]);
        let api = rig.reads();
        api.read(&pl("pl_1"), None).await.unwrap();
        api.read(&pl("here"), None).await.unwrap();
        let seen: Vec<(&'static str, Option<Vec<_>>)> = rig.read_calls.borrow().iter().map(|(on, _)| (on_kind(on), on_projects(on))).collect();
        assert_eq!(seen, vec![("box", Some(vec![spoo(), www()])), ("here", Some(vec![www()]))]);
        assert_eq!(rig.lines_of(&["projects.list"]), vec!["projects.list pl_1".to_string(), "projects.list here".to_string()]);

        rig.read_calls.borrow_mut().clear();
        api.read(&target(json!({ "placeId": "pl_1", "project": "pr_www" })), None).await.unwrap();
        assert_eq!(on_projects(&rig.read_calls.borrow()[0].0), Some(vec![www()]));
    });
}

// A4: agents-read.test.ts:572, 584
#[test]
fn an_act_names_one_project_by_id_or_by_a_unique_name() {
    run(async {
        let rig = computer(vec![spoo(), www()], vec![www()]);
        let api = rig.reads();
        api.skills_remove(&target(json!({ "placeId": "pl_1", "project": "pr_www" })), &skill_in_project("deploy"), None).await.unwrap();
        api.tools(&target(json!({ "placeId": "pl_1", "project": "pr_spoo" })), "claude", "db", None, None).await.unwrap();
        rig.clear_log();
        api.tools(&pl("pl_1"), "claude", "notion", None, None).await.unwrap();
        assert_eq!(rig.lines_of(&["projects.list"]), Vec::<String>::new());
        let acted = rig.acted.borrow().clone();
        assert_eq!(acted.iter().map(on_projects).collect::<Vec<_>>(), vec![Some(vec![www()]), Some(vec![spoo()]), None]);
        assert_eq!(acted.iter().map(project_of).collect::<Vec<_>>(), vec![Some("/home/ada/www".to_string()), Some("/home/ada/spoo".to_string()), None]);
    });
    run(async {
        let www2 = project("pr_www2", "www", "/home/ada/www2");
        let rig = computer(vec![spoo(), www(), www2], vec![spoo()]);
        let api = rig.reads();
        api.skills_remove(&target(json!({ "placeId": "here", "project": "spoo" })), &skill_in_project("deploy"), None).await.unwrap();
        api.skills_remove(&target(json!({ "placeId": "pl_1", "project": "pr_www2" })), &skill_in_project("deploy"), None).await.unwrap();
        api.skills_remove(&target(json!({ "placeId": "pl_1", "project": "spoo" })), &skill_in_project("deploy"), None).await.unwrap();
        let acted = rig.acted.borrow().clone();
        assert_eq!(
            acted.iter().map(project_of).collect::<Vec<_>>(),
            vec![Some("/home/ada/spoo".to_string()), Some("/home/ada/www2".to_string()), Some("/home/ada/spoo".to_string())]
        );
    });
}

// A4: agents-read.test.ts:572, 584
#[test]
fn a_shared_name_and_an_unknown_project_are_refused_with_control_characters_out() {
    run(async {
        let rig = computer(vec![spoo(), www()], vec![www()]);
        let api = rig.reads();
        assert_eq!(
            api.read(&target(json!({ "placeId": "pl_1", "project": "pr_gone" })), None).await.err(),
            Some(usage(&no_such_agents_project_refusal("pr_gone", "box")))
        );
        assert_eq!(
            api.skills_remove(&target(json!({ "placeId": "here", "project": "pr_spoo" })), &skill_in_project("deploy"), None).await.err(),
            Some(usage(&no_such_agents_project_refusal("pr_spoo", THIS_COMPUTER)))
        );
        assert_eq!(
            api.skills_remove(&target(json!({ "placeId": "pl_1", "project": "pr_\u{1b}[2Kgone\n" })), &skill_in_project("deploy"), None).await.err(),
            Some(usage(&no_such_agents_project_refusal("pr_[2Kgone", "box")))
        );
        assert_eq!(rig.skill_calls.borrow().len(), 0);
    });
    run(async {
        let www2 = project("pr_www2", "www", "/home/ada/www2");
        let rig = computer(vec![spoo(), www(), www2.clone()], vec![spoo()]);
        let api = rig.reads();
        assert_eq!(
            api.skills_remove(&target(json!({ "placeId": "pl_1", "project": "www" })), &skill_in_project("deploy"), None).await.err(),
            Some(usage(&shared_agents_project_refusal("www", "box")))
        );
        assert_eq!(rig.skill_calls.borrow().len(), 0);

        let rig = computer(vec![project("a", "w\tww", "/a"), project("b", "w\tww", "/b")], vec![]);
        assert_eq!(
            rig.reads().skills_remove(&target(json!({ "placeId": "pl_1", "project": "w\tww" })), &skill_in_project("deploy"), None).await.err(),
            Some(usage(&shared_agents_project_refusal("www", "box")))
        );
    });
}

// A5: agents-read.test.ts:31
#[test]
fn a_napping_workspace_answers_its_last_report_stale() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let first = js(&api.read(&ws("ws_1"), None).await.unwrap());
        assert_eq!(first["readAt"], json!(READ_AT));
        rig.phase.set(WorkspacePhase::Napping);
        let mut stale = first.clone();
        stale["stale"] = json!("napping");
        assert_eq!(js(&api.read(&ws("ws_1"), None).await.unwrap()), stale);
        assert_eq!(rig.read_calls.borrow().len(), 1);

        rig.pause_mode.set(Some(PauseMode::Disk));
        stale["pauseMode"] = json!("disk");
        assert_eq!(js(&api.read(&ws("ws_1"), None).await.unwrap()), stale);
        assert_eq!(rig.read_calls.borrow().len(), 1);

        rig.phase.set(WorkspacePhase::Running);
        rig.pause_mode.set(None);
        rig.reader_answer.borrow_mut().user = "ada".into();
        let latest = js(&api.read(&ws("ws_1"), None).await.unwrap());
        assert_eq!(latest["user"], json!("ada"));
        rig.phase.set(WorkspacePhase::Napping);
        let mut stale = latest.clone();
        stale["stale"] = json!("napping");
        assert_eq!(js(&api.read(&ws("ws_1"), None).await.unwrap()), stale);
        assert_eq!(rig.read_calls.borrow().len(), 2);
    });
}

// A5: agents-read.test.ts:42
#[test]
fn a_napping_workspace_never_read_is_refused() {
    run(async {
        let rig = Rig::new();
        rig.phase.set(WorkspacePhase::Napping);
        let api = rig.reads();
        assert_eq!(api.read(&ws("ws_2"), None).await.err(), Some(usage(&napping_agents_refusal("landing", None))));
        rig.pause_mode.set(Some(PauseMode::Memory));
        assert_eq!(api.read(&ws("ws_2"), None).await.err(), Some(usage(&napping_agents_refusal("landing", Some(&PauseMode::Memory)))));
        assert_eq!(rig.lines_of(&["reader."]), Vec::<String>::new());
    });
}

// A5: agents-read.ts:449-456 (no TS case)
#[test]
fn the_report_carries_each_agents_setup_on_a_computer() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        rig.reader_answer.borrow_mut().agents = vec![agent_row("claude"), agent_row("codex")];
        let view: AgentSetupView = serde_json::from_value(json!({ "on": true, "program": "claude-x", "envNames": [] })).unwrap();
        let mut setups = IndexMap::new();
        setups.insert(("pl_1".to_string(), "claude".to_string()), view);
        *rig.setups.borrow_mut() = Some(setups);
        let report = js(&rig.reads().read(&pl("pl_1"), None).await.unwrap());
        assert_eq!(report["agents"][0]["setup"], json!({ "on": true, "program": "claude-x", "envNames": [] }));
        assert_eq!(report["agents"][1], js(&agent_row("codex")));
        assert_eq!(rig.lines_of(&["setup_of"]), vec!["setup_of pl_1 claude".to_string(), "setup_of pl_1 codex".to_string()]);

        rig.clear_log();
        let report = js(&rig.reads().read(&ws("ws_1"), None).await.unwrap());
        assert_eq!(report["agents"], json!([js(&agent_row("claude")), js(&agent_row("codex"))]));
        assert_eq!(rig.lines_of(&["setup_of"]), Vec::<String>::new());
    });
}

// A5: agents-read.ts:449 (no TS case)
#[test]
fn the_report_asks_no_vendor_where_the_person_said_no() {
    run(async {
        for (said, latest) in [(Some(false), false), (None, true), (Some(true), true)] {
            let rig = Rig::new();
            rig.latest.set(said);
            rig.reads().read(&ws("ws_1"), None).await.unwrap();
            assert_eq!(rig.read_calls.borrow()[0].1, latest, "latest_on {said:?}");
            assert_eq!(rig.lines_of(&["reader.read"]), vec![format!("reader.read machine latest={latest}")]);
        }
    });
}

// A6: agents-read.test.ts:49
#[test]
fn the_report_says_where_a_sign_in_page_reaches() {
    run(async {
        let reach = |local: bool, relayed: bool, target: wsp_wire::types::agents_report::AgentsTarget| async move {
            let rig = Rig::new();
            rig.local.set(local);
            rig.relayed.set(Some(relayed));
            js(&rig.reads().read(&target, None).await.unwrap())["reach"].clone()
        };
        assert_eq!(reach(true, false, ws("ws_1")).await, json!("here"));
        assert_eq!(reach(false, true, ws("ws_1")).await, json!("relay"));
        assert_eq!(reach(false, false, ws("ws_1")).await, json!("none"));
        assert_eq!(reach(false, false, pl("here")).await, json!("here"));

        let rig = Rig::new();
        let here = AgentsOn::Here { projects: None, stores: None };
        assert_eq!(
            vec![page_reach_of(&here, None), page_reach_of(&machine_on(&rig, Some(true)), None), page_reach_of(&machine_on(&rig, None), None), page_reach_of(&box_on(None), None), page_reach_of(&box_on(Some(true)), None)],
            vec![PageReach::Here, PageReach::Relay, PageReach::None, PageReach::None, PageReach::Relay]
        );
    });
}

// A6: agents-read.test.ts:227
#[test]
fn a_joined_computer_whose_lines_go_to_another_login_reports_none() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        {
            let mut read = rig.reader_answer.borrow_mut();
            read.home = "/home/ada".into();
            read.user = "ada".into();
            read.run_as = Some("ada".into());
        }
        let api = rig.reads();
        let _off = api.forwards(FakeRelay::new(&rig, "pl_1"));
        let report = js(&api.read(&pl("pl_1"), None).await.unwrap());
        assert_eq!(report["reach"], json!("none"));
        assert_eq!(report.get("runAs"), None);
        assert_eq!(page_reach_of(&box_on(Some(true)), Some("ada")), PageReach::None);

        rig.reader_answer.borrow_mut().run_as = None;
        assert_eq!(js(&api.read(&pl("pl_1"), None).await.unwrap())["reach"], json!("relay"));
    });
}

fn codex_row_with_setup() -> Value {
    let mut v = js(&agent_row("codex"));
    v["setup"] = json!({ "on": true, "program": "codex-x", "envNames": [] });
    v
}

fn setup_rig() -> Rig {
    let rig = Rig::new();
    rig.with_spoo();
    rig.reader_answer.borrow_mut().agents = vec![agent_row("claude"), agent_row("codex")];
    let view: AgentSetupView = serde_json::from_value(json!({ "on": true, "program": "codex-x", "envNames": [] })).unwrap();
    let mut setups = IndexMap::new();
    setups.insert(("pl_1".to_string(), "codex".to_string()), view);
    *rig.setups.borrow_mut() = Some(setups);
    rig
}

fn one_read(now: i64) -> Vec<String> {
    vec![list_line(now), "door.report_of pl_1".into(), "door.logins_again pl_1".into(), ids_line("pl_1"), "reader.read box latest=true".into()]
}

// A7: agents-read.ts:461-470 (no TS case)
#[test]
fn setup_writes_reads_again_and_answers_the_row() {
    run(async {
        let rig = setup_rig();
        let change: AgentSetupSet = serde_json::from_value(json!({ "on": true })).unwrap();
        let row = rig.reads().setup("pl_1", "codex", change.clone(), None).await.unwrap();
        assert_eq!(js(&row), codex_row_with_setup());
        let mut want = vec![list_line(T0), "door.report_of pl_1".to_string(), ids_line("pl_1")];
        want.push("setup_write pl_1 codex {\"on\":true}".to_string());
        want.push("changed {\"placeId\":\"pl_1\"}".to_string());
        want.extend(one_read(T0));
        assert_eq!(rig.lines_of(&["door.", "setup_write", "changed", "reader.read"]), want);

        let rig = setup_rig();
        rig.fail("setup_write", "config refused");
        let failed = rig.reads().setup("pl_1", "codex", change, None).await;
        assert_eq!(failed.err(), Some(plain_error("config refused")));
        assert_eq!(rig.changed_json(), vec![json!({ "placeId": "pl_1" })]);
        assert_eq!(rig.lines_of(&["reader.read"]), Vec::<String>::new());
    });
}

// A7: agents-read.ts:468 (no TS case)
#[test]
fn setup_refuses_an_agent_not_in_the_report() {
    run(async {
        let rig = setup_rig();
        let change: AgentSetupSet = serde_json::from_value(json!({ "on": false })).unwrap();
        let refused = rig.reads().setup("pl_1", "ghost", change, None).await;
        assert_eq!(refused.err(), Some(usage("there is no agent ghost in the catalog")));
        assert_eq!(rig.lines_of(&["setup_write", "changed", "reader.read"]), vec!["setup_write pl_1 ghost {\"on\":false}", "changed {\"placeId\":\"pl_1\"}", "reader.read box latest=true"]);
    });
}

// A8: agents-read.test.ts:79
#[test]
fn tools_are_asked_on_the_workspaces_machine_keyed_by_the_target() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let answer = api.tools(&ws("ws_5"), "claude", "airtable", None, None).await.unwrap();
        assert_eq!(js(&answer), json!({ "auth": "open", "tools": [], "readAt": READ_AT }));
        let calls = rig.tools_calls.borrow().clone();
        assert_eq!(on_json(&calls[0].0), machine_on_json());
        assert_eq!(calls[0].1, ServerToolsAsk { key: "{\"workspaceId\":\"ws_5\"}".into(), agent: "claude".into(), name: "airtable".into(), refresh: None });

        let rig = computer(vec![spoo(), www()], vec![]);
        rig.reads().tools(&target(json!({ "placeId": "pl_1", "project": "pr_www" })), "claude", "db", Some(true), None).await.unwrap();
        let calls = rig.tools_calls.borrow().clone();
        assert_eq!(calls[0].1, ServerToolsAsk { key: "{\"placeId\":\"pl_1\",\"project\":\"pr_www\"}".into(), agent: "claude".into(), name: "db".into(), refresh: Some(true) });
        assert_eq!(on_projects(&calls[0].0), Some(vec![www()]));
    });
}

// A8: agents-read.test.ts:79
#[test]
fn tools_and_server_refuse_a_napping_workspace() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let server = api.server(&ws("ws_5"), "claude", "airtable", None).await.unwrap();
        assert_eq!(js(&server.spec), json!({ "command": "npx", "args": ["-y", "airtable"] }));
        let calls = rig.server_calls.borrow().clone();
        assert_eq!((on_json(&calls[0].0), calls[0].1.clone(), calls[0].2.clone()), (machine_on_json(), "claude".to_string(), "airtable".to_string()));

        rig.phase.set(WorkspacePhase::Napping);
        let want = usage(&napping_tools_refusal("landing", None));
        assert_eq!(api.tools(&ws("ws_5"), "claude", "airtable", None, None).await.err(), Some(want.clone()));
        assert_eq!(api.server(&ws("ws_5"), "claude", "airtable", None).await.err(), Some(want));
        assert_eq!(rig.tools_calls.borrow().len(), 0);
        assert_eq!(rig.server_calls.borrow().len(), 1);
    });
}

// A9: agents-read.test.ts:70
#[test]
fn forget_drops_the_last_report() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        api.read(&ws("ws_4"), None).await.unwrap();
        api.read(&ws("ws_6"), None).await.unwrap();
        api.forget("ws_4");
        rig.phase.set(WorkspacePhase::Napping);
        assert_eq!(api.read(&ws("ws_4"), None).await.err(), Some(usage(&napping_agents_refusal("landing", None))));
        assert_eq!(js(&api.read(&ws("ws_6"), None).await.unwrap())["stale"], json!("napping"));
    });
}

fn on_machine_line(t: &str, rest: &str) -> String {
    format!("{t} machine {rest}")
}

// A10: agents-read.test.ts:445
#[test]
fn skills_search_asks_twenty_unless_told() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let hits = api.skills_search("pdf", None).await.unwrap();
        assert_eq!(js(&hits), json!([{ "id": "a/b/pdf", "source": "a/b", "skillId": "pdf", "name": "pdf", "installs": 3 }]));
        api.skills_search("pdf", Some(5)).await.unwrap();
        let got = api.skills_get("a/b/pdf").await.unwrap();
        assert_eq!(js(&got), json!({ "text": "# pdf", "size": 5 }));
        assert_eq!(
            *rig.skill_calls.borrow(),
            vec![SkillCall::Search("pdf".into(), 20), SkillCall::Search("pdf".into(), 5), SkillCall::Get("a/b/pdf".into())]
        );
    });
}

// A10: agents-read.test.ts:453
#[test]
fn a_skill_write_says_the_target_changed_held_or_not() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let t = ws("ws_1");
        let add = SkillAddAsk { skill: "a/b/pdf".into(), agents: None, project: Some(true) };
        let added = api.skills_add(&t, &add, None).await.unwrap();
        assert_eq!(js(&added), json!({ "path": "~/.agents/skills/pdf", "agents": [] }));
        let toggled = api.skills_toggle(&t, &skill("pdf"), false, None).await.unwrap();
        assert_eq!(toggled, vec!["~/.agents/skills/pdf".to_string()]);
        rig.fail("skills.remove", "rm failed");
        assert_eq!(api.skills_remove(&t, &skill("pdf"), None).await.err(), Some(plain_error("rm failed")));
        let previewed = api.skills_preview(&t, &skill("pdf"), None).await.unwrap();
        assert_eq!(js(&previewed), json!({ "text": "# pdf", "size": 5 }));

        assert_eq!(
            *rig.skill_calls.borrow(),
            vec![
                SkillCall::Add(machine_on_json(), add),
                SkillCall::Toggle(machine_on_json(), skill("pdf"), false),
                SkillCall::Remove(machine_on_json(), skill("pdf")),
                SkillCall::Preview(machine_on_json(), skill("pdf")),
            ]
        );
        assert_eq!(rig.changed_json(), vec![js(&t), js(&t), js(&t)]);
        let changed = "changed {\"workspaceId\":\"ws_1\"}";
        assert_eq!(
            rig.lines_of(&["skills.", "changed"]),
            vec![
                on_machine_line("skills.add", "a/b/pdf"),
                changed.to_string(),
                on_machine_line("skills.toggle", "{\"name\":\"pdf\"} to=false"),
                changed.to_string(),
                on_machine_line("skills.remove", "{\"name\":\"pdf\"}"),
                changed.to_string(),
                on_machine_line("skills.preview", "{\"name\":\"pdf\"}"),
            ]
        );
    });
}

// A10: agents-read.test.ts:464
#[test]
fn skills_never_touch_a_napping_workspace() {
    run(async {
        let rig = Rig::new();
        rig.phase.set(WorkspacePhase::Napping);
        let api = rig.reads();
        let t = ws("ws_1");
        let want = usage(&napping_skills_refusal("landing", None));
        let add = SkillAddAsk { skill: "a/b/pdf".into(), agents: None, project: None };
        assert_eq!(api.skills_add(&t, &add, None).await.err(), Some(want.clone()));
        assert_eq!(api.skills_toggle(&t, &skill("pdf"), false, None).await.err(), Some(want.clone()));
        assert_eq!(api.skills_remove(&t, &skill("pdf"), None).await.err(), Some(want.clone()));
        assert_eq!(api.skills_preview(&t, &skill("pdf"), None).await.err(), Some(want));
        rig.pause_mode.set(Some(PauseMode::Memory));
        assert_eq!(api.skills_remove(&t, &skill("pdf"), None).await.err(), Some(usage(&napping_skills_refusal("landing", Some(&PauseMode::Memory)))));
        assert_eq!(*rig.skill_calls.borrow(), vec![]);
        assert_eq!(rig.changed_json(), Vec::<Value>::new());
    });
}

// A11: agents-read.test.ts:502
#[test]
fn a_server_write_says_the_target_changed_held_or_not() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let t = ws("ws_1");
        let add: ServerAdd = serde_json::from_value(json!({ "agent": "claude", "name": "acme", "command": "npx", "env": { "A": "sk-x" } })).unwrap();
        let toggle: ServerAsk = serde_json::from_value(json!({ "agent": "codex", "name": "acme" })).unwrap();
        let remove: ServerAsk = serde_json::from_value(json!({ "agent": "claude", "name": "acme", "scope": "project" })).unwrap();
        assert_eq!(api.servers_add(&t, &add, None).await.unwrap(), "~/.claude.json");
        rig.fail("servers.remove", "the file changed");
        assert_eq!(api.servers_toggle(&t, &toggle, false, None).await.unwrap(), "~/.codex/config.toml");
        assert_eq!(api.servers_remove(&t, &remove, None).await.err(), Some(plain_error("the file changed")));
        assert_eq!(
            *rig.server_writes.borrow(),
            vec![
                ServerCall::Add(machine_on_json(), js(&add)),
                ServerCall::Toggle(machine_on_json(), json!({ "agent": "codex", "name": "acme" }), false),
                ServerCall::Remove(machine_on_json(), json!({ "agent": "claude", "name": "acme", "scope": "project" })),
            ]
        );
        assert_eq!(rig.changed_json(), vec![js(&t), js(&t), js(&t)]);
        let changed = "changed {\"workspaceId\":\"ws_1\"}".to_string();
        assert_eq!(
            rig.lines_of(&["servers.", "changed"]),
            vec!["servers.add machine acme".to_string(), changed.clone(), "servers.toggle machine acme to=false".to_string(), changed.clone(), "servers.remove machine acme".to_string(), changed]
        );
    });
}

// A11: agents-read.test.ts:516
#[test]
fn servers_never_touch_a_napping_workspace() {
    run(async {
        let rig = Rig::new();
        rig.phase.set(WorkspacePhase::Napping);
        let api = rig.reads();
        let t = ws("ws_1");
        let add: ServerAdd = serde_json::from_value(json!({ "agent": "claude", "name": "acme", "command": "npx" })).unwrap();
        let ask_one: ServerAsk = serde_json::from_value(json!({ "agent": "codex", "name": "acme" })).unwrap();
        let want = usage(&napping_servers_refusal("landing", None));
        assert_eq!(api.servers_add(&t, &add, None).await.err(), Some(want.clone()));
        assert_eq!(api.servers_toggle(&t, &ask_one, true, None).await.err(), Some(want.clone()));
        assert_eq!(api.servers_remove(&t, &ask_one, None).await.err(), Some(want));
        assert_eq!(*rig.server_writes.borrow(), vec![]);
        assert_eq!(rig.changed_json(), Vec::<Value>::new());
    });
}

// A11: agents-read.ts:628 (no TS case)
#[test]
fn servers_icon_is_null_without_icons() {
    run(async {
        let rig = Rig::new();
        let mut o = rig.options();
        o.icons = None;
        let api = AgentsReads::new(o);
        assert_eq!(api.servers_icon("mcp.notion.com", Some(true)).await.unwrap(), None);
        assert_eq!(rig.lines_of(&["icons."]), Vec::<String>::new());

        let api = rig.reads();
        assert_eq!(api.servers_icon("mcp.notion.com", Some(true)).await.unwrap(), Some(ICON.to_string()));
        assert_eq!(rig.lines_of(&["icons."]), vec!["icons.icon mcp.notion.com refresh=Some(true)".to_string()]);
    });
}

// A12: agents-read.ts:630-634 (no TS case)
#[test]
fn plugins_toggle_answers_the_row_and_says_changed() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let t = ws("ws_1");
        let ask = PluginAsk { agent: "claude".into(), plugin: "pdf-tools@acme".into() };
        let row = api.plugins_toggle(&t, &ask, true, None).await.unwrap();
        assert_eq!(js(&row), js(&plugin_row()));
        assert_eq!(*rig.plugin_calls.borrow(), vec![(machine_on_json(), ask.clone(), true)]);
        assert_eq!(rig.changed_json(), vec![js(&t)]);

        rig.fail("plugins.toggle", "the plugin road failed");
        assert_eq!(api.plugins_toggle(&t, &ask, false, None).await.err(), Some(plain_error("the plugin road failed")));
        assert_eq!(rig.changed_json(), vec![js(&t), js(&t)]);

        rig.phase.set(WorkspacePhase::Napping);
        assert_eq!(api.plugins_toggle(&t, &ask, false, None).await.err(), Some(usage(&napping_plugins_refusal("landing", None))));
        assert_eq!(rig.plugin_calls.borrow().len(), 2);
        assert_eq!(rig.changed_json().len(), 2);
    });
}

// A13: agents-read.test.ts:404
#[test]
fn add_tools_says_changed_only_after_it_held() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        assert_eq!(api.add_tools(&pl("here"), "codex", None).await.unwrap(), "~/.codex/config.toml");
        assert_eq!(rig.changed_json(), vec![json!({ "placeId": "here" })]);
        assert_eq!(rig.lines_of(&["acts.", "changed"]), vec!["acts.add_tools here codex".to_string(), "changed {\"placeId\":\"here\"}".to_string()]);

        rig.fail("acts.add_tools", "config is read only");
        assert_eq!(api.add_tools(&pl("here"), "codex", None).await.err(), Some(plain_error("config is read only")));
        assert_eq!(rig.changed_json().len(), 1);

        rig.phase.set(WorkspacePhase::Napping);
        assert_eq!(api.add_tools(&ws("ws_1"), "codex", None).await.err(), Some(usage(&napping_sign_in_refusal("landing", None))));
        assert_eq!(rig.lines_of(&["acts.add_tools"]).len(), 2);
    });
}

// A14: agents-read.test.ts:404
#[test]
fn a_key_changes_every_report_and_lands_on_the_door() {
    run(async {
        let rig = Rig::new();
        rig.door_on.set(true);
        let api = rig.reads();
        api.key("claude", "sk-ant-oat01-x", None).await.unwrap();
        assert_eq!(rig.changed_json(), vec![Value::Null]);
        assert_eq!(rig.lines_of(&["acts.key", "changed", "door."]), vec!["acts.key claude way=None", "changed all", "door.key_landed claude"]);

        rig.clear_log();
        api.key("codex", "sk-x", Some("key")).await.unwrap();
        assert_eq!(rig.lines_of(&["acts.key"]), vec!["acts.key codex way=Some(\"key\")".to_string()]);

        rig.clear_log();
        rig.fail("acts.key", "vault is locked");
        assert_eq!(api.key("claude", "sk-x", None).await.err(), Some(plain_error("vault is locked")));
        assert_eq!(rig.lines_of(&["changed", "door."]), Vec::<String>::new());

        let rig = Rig::new();
        rig.reads().key("claude", "sk-x", None).await.unwrap();
        assert_eq!(rig.lines_of(&["acts.key", "changed", "door."]), vec!["acts.key claude way=None", "changed all"]);
    });
}

// A15: agents-read.test.ts:179
#[test]
fn a_relay_registers_and_unregisters() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let a = FakeRelay::new(&rig, "pl_1");
        let b = FakeRelay::new(&rig, "pl_2");
        assert!(!api.reached(&pl("pl_1")));
        let off_a = api.forwards(a.clone());
        assert_eq!((api.reached(&pl("pl_1")), api.reached(&pl("pl_2")), api.reached(&ws("ws_1"))), (true, false, false));
        let off_b = api.forwards(b.clone());
        assert_eq!((api.reached(&pl("pl_1")), api.reached(&pl("pl_2"))), (true, true));
        assert_eq!(api.relays.borrow().len(), 2);
        off_a();
        assert_eq!((api.reached(&pl("pl_1")), api.reached(&pl("pl_2"))), (false, true));
        assert_eq!(api.relays.borrow().len(), 1);
        off_b();
        assert_eq!((api.reached(&pl("pl_1")), api.reached(&pl("pl_2"))), (false, false));
        assert_eq!(api.relays.borrow().len(), 0);
        assert!(a.asked.borrow().contains(&json!({ "placeId": "pl_1" })));
    });
}
