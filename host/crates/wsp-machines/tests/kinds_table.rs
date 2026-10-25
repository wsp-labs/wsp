// SPDX-License-Identifier: AGPL-3.0-only
//! spec/kinds_table.md: the three kind modules, as the runtime tests read their answers (local-workspace,
//! places-fork, box-folder, terminal-folder and runtime-reach). Each test asks `module_of(kind)`.
mod machines_b_support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Value};

use machines_b_support::kinds_ctx::*;
use machines_b_support::*;
use wsp_engine::machine::MachineBackend;
use wsp_machines::machines::core_types::{KindModule, Live, WorkspaceRecord};
use wsp_machines::machines::kinds::KindsArea;
use wsp_wire::types::projects::REGISTERING_LINE;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::machine_link::{place_forks_nowhere_line, place_serves_daemon_line};
use wsp_words::exit::Kind;
use wsp_words::project_path::place_daemon_paths;

fn module(area: &KindsArea, kind: WorkspaceKind) -> Rc<dyn KindModule> {
    area.module_of(kind).ok().expect("the kind is wired")
}

fn record(id: &str, extra: Value, without: &[&str]) -> WorkspaceRecord {
    serde_json::from_value(record_of(id, extra, without)).unwrap()
}

/// The wsp server a launch on a fork or a joined computer hands its agent.
fn guest_wsp() -> Value {
    json!({ "command": "wsp", "args": ["mcp"] })
}

fn cloud_entry(host: &FakeHost, machine: Arc<FakeMachine>) -> Live {
    hold(&project("pr_1", "/root/spoo-landing", json!({})));
    host.put_live(sub_record("ws_c", "running"), machine)
}

fn place_entry(host: &FakeHost, place: &str) -> Live {
    hold(&project("pr_p", "/home/srv/lab", json!({ "computer": place })));
    host.put_live(record_of("ws_p", json!({ "kind": "place", "place": place, "project": "pr_p", "name": "lab" }), &["sub"]), FakeMachine::new("srv-machine"))
}

fn local_entry(host: &FakeHost, extra: Value) -> Live {
    let mut e = json!({ "kind": "local", "project": "pr_l" });
    for (k, v) in extra.as_object().unwrap() {
        e[k] = v.clone();
    }
    host.put_live(record_of("ws_l", e, &["place", "sub"]), FakeMachine::new("local"))
}

fn dests(of: &[&str]) -> Vec<String> {
    of.iter().map(|d| d.to_string()).collect()
}

#[test]
fn cloud_backend_is_the_hosts_own_or_the_places_and_refuses_one_that_forks_nowhere() {
    run(async {
        let boat = FakeBackend::new();
        let (host, area) = kinds_with(|w| {
            let (mut door, _m) = joined_door("Boat", "/root", false);
            door.backend = Some(boat.clone());
            w.doors.insert("box".to_string(), door);
            w.doors.insert("gone".to_string(), Door { name: "Orchard".to_string(), ..Door::default() });
            w.place_door = true;
        });
        let cloud = module(&area, WorkspaceKind::Cloud);
        let own: Arc<dyn MachineBackend> = host.backend.clone();
        let boat: Arc<dyn MachineBackend> = boat;
        let backend = |extra: Value, without: &[&str]| cloud.backend(&record("ws_k", extra, without));
        assert!(same_backend(&backend(json!({}), &["place", "sub"]).ok().unwrap(), &own));
        assert!(same_backend(&backend(json!({ "place": "box" }), &["sub"]).ok().unwrap(), &boat));
        // A place the door no longer holds a backend for refuses here, and says which.
        let e = backend(json!({ "place": "gone" }), &["sub"]).err().unwrap();
        assert_eq!((e.message, e.kind), (place_forks_nowhere_line("Orchard"), None));

        // With no door wired a record naming a place has nowhere to ask.
        let (_host, area) = kinds();
        let e = module(&area, WorkspaceKind::Cloud).backend(&record("ws_k", json!({ "place": "box" }), &["sub"])).err().unwrap();
        assert_eq!(e.message, "this host has no place backend wired, so it serves nothing of that kind");
    });
}

#[test]
fn cloud_answers_its_fixed_facts() {
    run(async {
        let (host, area) = kinds();
        let machine = FakeMachine::new("bx_c");
        let entry = cloud_entry(&host, machine.clone());
        let cloud = module(&area, WorkspaceKind::Cloud);
        assert_eq!((cloud.relayed(), cloud.turn_road(), cloud.keeps_agents(), cloud.server_values(), cloud.shared_daemon()), (true, "relayed", false, "environment", false));
        assert_eq!(cloud.turn_reach(), Some(IndexMap::new()));
        assert_eq!(cloud.wsp_mcp(&entry), Some(guest_wsp()));
        assert_eq!(cloud.scratch(&entry).unwrap(), "/tmp");
        assert_eq!(cloud.home_dir(&entry.record.borrow()).unwrap(), "/root");
        assert_eq!(cloud.home(&entry, "claude").unwrap(), "/root/.claude-cfg");
        assert_eq!(cloud.home(&entry, "codex").unwrap(), "/root/.codex");
        assert_eq!(cloud.home(&entry, "nope").unwrap_err().message, "the catalog has no home for nope");
        assert_eq!(cloud.folder(&entry.record.borrow()), Some("/root/web".to_string()));
        let plain = record("ws_k", json!({ "project": "pr_1" }), &["sub"]);
        assert_eq!(cloud.folder(&plain), None);
        cloud.dropped(&entry).await.unwrap();
        assert!(!cloud.deploys_daemon() && !cloud.restarts_daemon());

        // The guest's login first, then the variable pointing the agent at its store there.
        let login = cloud.env(&entry, None, &|id| format!("/h/{id}")).unwrap();
        let claude = cloud.env(&entry, Some("claude"), &|id| format!("/h/{id}")).unwrap();
        let login_keys: Vec<&String> = login.keys().collect();
        assert!(!login_keys.is_empty());
        assert_eq!(claude.keys().take(login_keys.len()).collect::<Vec<_>>(), login_keys);
        assert_eq!(claude["CLAUDE_CONFIG_DIR"], "/root/.claude-cfg");
        assert!(!login.contains_key("CLAUDE_CONFIG_DIR"));
        assert_eq!(text(&cloud.env(&entry, Some("nope"), &|id| format!("/h/{id}")).unwrap()), text(&login));
        assert!(!cloud.login_stands(&entry, "claude"));

        // The key the project was recorded with, never one off the fork's own path.
        assert_eq!(cloud.memory_key(&entry, "claude"), Some("-recorded-pr_1".to_string()));

        // The copy is the host's; the roots file is written through the machine.
        let (seen, report) = {
            let seen = Rc::new(RefCell::new(Vec::new()));
            let keep = seen.clone();
            let report: Rc<dyn Fn(&str, &str)> = Rc::new(move |stage, line| keep.borrow_mut().push((stage.to_string(), line.to_string())));
            (seen, report)
        };
        let o = json!({ "dest": "/root/x" });
        assert_eq!(cloud.import(&entry, o.clone(), report).await.unwrap(), json!({ "copied": "ws_c" }));
        assert_eq!(host.calls_of("copy_import"), vec![vec!["ws_c".to_string(), serde_json::to_string(&o).unwrap()]]);
        assert!(seen.borrow().is_empty());
        cloud.roots(&entry, &dests(&["/root/a", "/root/b"])).await.unwrap();
        let script = "next='/root/.wsp/roots.next.'$$\nmkdir -p '/root/.wsp'\nprintf '%s\\n' '/root/a' '/root/b' > \"$next\"\nmv -f \"$next\" '/root/.wsp/roots'";
        assert_eq!(machine.log(), vec![script.to_string()]);
        let said = format!("{}{}", "x".repeat(60), "y".repeat(200));
        machine.answer("mv -f", exit(1, &said));
        let e = cloud.roots(&entry, &dests(&["/root/a", "/root/b"])).await.unwrap_err();
        assert_eq!(e.message, format!("could not make /root/b browsable on the machine: {}", "y".repeat(200)));
        world(|w| w.hello = Some(5.0));
        assert_eq!(cloud.daemon_version(&entry).await.unwrap(), Some(5.0));
    });
}

#[test]
fn cloud_daemon_road_refuses_a_workspace_its_computer_serves() {
    run(async {
        let (host, area) = kinds();
        let machine = FakeMachine::new("bx_c");
        let entry = cloud_entry(&host, machine.clone());
        let cloud = module(&area, WorkspaceKind::Cloud);
        // A machine with no preview route has no daemon to dial; one with it has, until its computer serves it.
        assert!(!cloud.has_daemon(&entry));
        machine.offers.lock().unwrap().preview_url = true;
        assert!(cloud.has_daemon(&entry));
        let road = cloud.daemon_road(&entry).await.unwrap();
        assert_eq!((road.url.as_str(), road.expires_at.0, road.daemon_token.clone()), ("http://127.0.0.1:1/", 9_999_999_999_999.0, None));
        host.script("daemon_token_of", Ok(json!("tok")));
        let road = cloud.daemon_road(&entry).await.unwrap();
        assert_eq!(road.daemon_token, Some("tok".to_string()));
        assert_eq!(host.calls_of("daemon_token_of").last().unwrap(), &vec!["bx_c".to_string()]);

        world(|w| {
            w.served.insert("ws_c".to_string());
        });
        assert!(!cloud.has_daemon(&entry));
        let e = cloud.daemon_road(&entry).await.unwrap_err();
        assert_eq!(e.message, place_serves_daemon_line("Fix the flaky upload test", "Boat"));

        // A deploy puts the daemon on with the token held, then forgets that token and mints it again.
        let (host, area) = kinds_with(|w| w.deploy = true);
        let entry = cloud_entry(&host, FakeMachine::new("bx_c"));
        let cloud = module(&area, WorkspaceKind::Cloud);
        assert!(cloud.deploys_daemon());
        host.script("daemon_token_of", Ok(json!("old")));
        cloud.deploy_daemon(&entry).await.unwrap();
        let names: Vec<String> = host.calls.borrow().iter().map(|(n, _)| n.clone()).filter(|n| ["daemon_token_of", "deploy_daemon", "forget_daemon_token"].contains(&n.as_str())).collect();
        assert_eq!(names, vec!["daemon_token_of", "deploy_daemon", "forget_daemon_token", "daemon_token_of"]);
        assert_eq!(host.calls_of("deploy_daemon"), vec![vec!["bx_c".to_string(), "old".to_string()]]);
        assert_eq!(host.calls_of("forget_daemon_token"), vec![vec!["bx_c".to_string()]]);
    });
}

#[test]
fn local_wsp_mcp_carries_the_scoped_arg_and_turn_reach_the_here_url() {
    run(async {
        let (host, area) = kinds();
        let local = module(&area, WorkspaceKind::Local);
        let entry = local_entry(&host, json!({}));
        assert_eq!(local.wsp_mcp(&entry), None);
        assert_eq!(local.turn_reach(), None);
        world(|w| w.agents_here_url = Some(String::new()));
        assert_eq!(local.turn_reach(), None, "an empty url is no url");
        world(|w| {
            w.agents_wsp_mcp = Some(json!({ "command": "wsp", "args": ["mcp"] }));
            w.agents_here_url = Some("ws://127.0.0.1:4400".to_string());
        });
        assert_eq!(local.wsp_mcp(&entry), Some(json!({ "command": "wsp", "args": ["mcp", "--scoped"] })));
        assert_eq!(local.turn_reach(), Some(IndexMap::from([("url".to_string(), "ws://127.0.0.1:4400".to_string())])));

        // What else a folder here answers.
        assert_eq!((local.relayed(), local.turn_road(), local.keeps_agents(), local.server_values(), local.shared_daemon()), (false, "here", true, "file", true));
        let e = local.daemon_road(&entry).await.unwrap_err();
        assert_eq!(e.message, "this host wired no daemon for this computer, so nothing on it can be dialled");
        world(|w| w.local_road = serde_json::from_value(json!({ "url": "ws://127.0.0.1:7777", "expiresAt": 9_999_999_999_999u64 })).ok());
        let road = local.daemon_road(&entry).await.unwrap();
        assert_eq!((road.url.as_str(), road.daemon_token), ("ws://127.0.0.1:7777", None));
    });
}

#[test]
fn local_memory_key_follows_a_made_worktree_then_the_git_top() {
    run(async {
        let (host, area) = kinds();
        let local = module(&area, WorkspaceKind::Local);
        let nested = "/repos/lab/packages/web";
        let held = |extra: Value| {
            let mut v = json!({ "computer": "here", "memoryKey": "-recorded-key" });
            for (k, val) in extra.as_object().unwrap() {
                v[k] = val.clone();
            }
            hold(&project("pr_l", nested, v));
        };
        let entry = local_entry(&host, json!({}));
        // A project with no git, or whose path is its repo's top, takes the key recorded with it.
        held(json!({}));
        assert_eq!(local.memory_key(&entry, "claude"), Some("-recorded-key".to_string()));
        held(json!({ "git": { "top": nested } }));
        assert_eq!(local.memory_key(&entry, "claude"), Some("-recorded-key".to_string()));
        // A subfolder project takes the key Claude Code gives it in the person's own terminal: its repo's top.
        held(json!({ "git": { "top": "/repos/lab" } }));
        assert_eq!(local.memory_key(&entry, "claude"), Some("-repos-lab".to_string()));
        // A worktree wsp made keys off its own path, so a builder there never loads the person's notes.
        entry.record.borrow_mut().worktree = Some(json!({ "path": "/wt/lab-feature", "made": true }));
        assert_eq!(local.memory_key(&entry, "claude"), Some("-wt-lab-feature".to_string()));
        // Not once it is gone, and never one the person made.
        entry.record.borrow_mut().worktree = Some(json!({ "path": "/wt/lab-feature", "made": true, "gone": true }));
        assert_eq!(local.memory_key(&entry, "claude"), Some("-repos-lab".to_string()));
        entry.record.borrow_mut().worktree = Some(json!({ "path": "/wt/lab-feature" }));
        assert_eq!(local.memory_key(&entry, "claude"), Some("-repos-lab".to_string()));
    });
}

#[test]
fn local_import_registers_in_place() {
    run(async {
        let (host, area) = kinds();
        let local = module(&area, WorkspaceKind::Local);
        let entry = local_entry(&host, json!({}));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let keep = seen.clone();
        let report: Rc<dyn Fn(&str, &str)> = Rc::new(move |stage, line| keep.borrow_mut().push((stage.to_string(), line.to_string())));
        let o = json!({ "dest": "/home/z/lab" });
        let landed = local.import(&entry, o.clone(), report).await.unwrap();
        assert_eq!(*seen.borrow(), vec![("landing".to_string(), REGISTERING_LINE.to_string())]);
        assert_eq!(host.calls_of("bundler_plan"), vec![vec![serde_json::to_string(&o).unwrap()]]);
        let project = r#"{"name":"lab","dest":"/home/z/lab","importedAt":"2025-10-09T08:53:20.000Z","size":3456}"#;
        let want = format!(r#"{{"result":{{"dest":"/home/z/lab","files":12,"bytes":3456,"parts":0,"cut":[],"rewritten":[],"agents":[],"project":{project}}},"done":"lab registered at /home/z/lab; nothing was copied."}}"#);
        assert_eq!(text(&landed), want);
        assert_eq!(text(&area.project_of("/home/z/lab", 3456.0)), project);
        // Nothing is landed on the way: a folder here is the person's own.
        local.land_project(&entry, &json!({}), reports().1).await.unwrap();
    });
}

#[test]
fn place_port_reach_forwards_and_holds_for_the_pane_hold() {
    run(async {
        let (host, area) = kinds_with(|w| {
            let (door, _m) = joined_door("Srv", "/home/srv", true);
            w.doors.insert("srv".to_string(), door);
            w.place_door = true;
        });
        let entry = place_entry(&host, "srv");
        let place = module(&area, WorkspaceKind::Place);
        let reach = place.port_reach(&entry, 3000, false).await.unwrap().expect("a place forwards the port");
        assert_eq!((reach.url.as_str(), reach.expires_at.0), ("http://localhost:41000/", 1_760_000_000_000.0 + 120_000.0));
        place.port_reach(&entry, 3000, true).await.unwrap().unwrap();
        let pane = r#"{"workspaceId":"ws_p","name":"lab"}"#;
        assert_eq!(
            host.calls_of("door_forward"),
            vec![["srv", "3000", pane, "false"].map(String::from).to_vec(), ["srv", "3000", pane, "true"].map(String::from).to_vec()]
        );
        host.clock.advance(60_000.0).await;
        assert_eq!(place.port_reach(&entry, 3000, false).await.unwrap().unwrap().expires_at.0, 1_760_000_060_000.0 + 120_000.0);
        host.script("door_forward", Err(failed("link is down")));
        assert!(place.port_reach(&entry, 3001, false).await.is_err());

        // The roots file goes beside the home the computer was joined with, written by the door and never as the login.
        place.roots(&entry, &dests(&["/home/srv/lab", "/home/srv/web"])).await.unwrap();
        let script = "next='/home/srv/.wsp/roots.next.'$$\nmkdir -p '/home/srv/.wsp'\nprintf '%s\\n' '/home/srv/lab' '/home/srv/web' > \"$next\"\nmv -f \"$next\" '/home/srv/.wsp/roots'";
        assert_eq!(host.calls_of("door_exec"), vec![vec!["srv".to_string(), script.to_string(), "20000".to_string()]]);
        let said = format!("{}{}", "x".repeat(60), "y".repeat(200));
        host.script("door_exec", Ok(json!({ "exitCode": 1, "stdout": "", "stderr": said })));
        let e = place.roots(&entry, &dests(&["/home/srv/lab", "/home/srv/web"])).await.unwrap_err();
        assert_eq!(e.message, format!("could not make /home/srv/web browsable on Srv: {}", "y".repeat(200)));
    });
}

#[test]
fn place_import_refuses_with_the_add_line() {
    run(async {
        let (host, area) = kinds_with(|w| {
            let (door, _m) = joined_door("Srv", "/home/srv", true);
            w.doors.insert("srv".to_string(), door);
            w.place_names.insert("srv".to_string(), "Srv".to_string());
            w.place_door = true;
        });
        let entry = place_entry(&host, "srv");
        let report: Rc<dyn Fn(&str, &str)> = Rc::new(|_, _| {});
        let e = module(&area, WorkspaceKind::Place).import(&entry, json!({ "dest": "/home/srv/x" }), report).await.unwrap_err();
        assert_eq!(e.message, "lab is a project folder on Srv; a folder from this computer goes there as a project of its own, with wsp add <folder> --on Srv");
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(host.calls_of("copy_import").is_empty() && host.calls_of("bundler_plan").is_empty());
    });
}

#[test]
fn place_wsp_mcp_only_where_the_report_says_tools() {
    run(async {
        let (host, area) = kinds_with(|w| {
            let (mut door, _m) = joined_door("Srv", "/home/srv", true);
            door.daemon_version = Some(7.0);
            w.doors.insert("srv".to_string(), door);
            let (plain, _m) = joined_door("Plain", "/home/plain", false);
            w.doors.insert("plain".to_string(), plain);
            w.doors.insert("bare".to_string(), Door { name: "Bare".to_string(), joined: true, ..Door::default() });
            w.place_names.insert("srv".to_string(), "Srv".to_string());
            w.place_door = true;
        });
        let place = module(&area, WorkspaceKind::Place);
        let at = |id: &str, name: &str| host.put_live(record_of(id, json!({ "kind": "place", "place": name, "project": "pr_p", "name": "lab" }), &["sub"]), FakeMachine::new(&format!("{name}-machine")));
        hold(&project("pr_p", "/home/srv/lab", json!({})));
        assert_eq!(place.wsp_mcp(&at("ws_1", "srv")), Some(guest_wsp()));
        assert_eq!(place.wsp_mcp(&at("ws_2", "plain")), None, "a daemon that bound no door has no wsp to launch");
        assert_eq!(place.wsp_mcp(&at("ws_3", "bare")), None);
        let none = host.put_live(record_of("ws_4", json!({ "kind": "place", "project": "pr_p" }), &["place", "sub"]), FakeMachine::new("m4"));
        assert_eq!(place.wsp_mcp(&none), None);

        // The daemon's version is what the computer's report says, none without a place or a report.
        let srv = at("ws_1", "srv");
        assert_eq!(place.daemon_version(&srv).await.unwrap(), Some(7.0));
        assert_eq!(place.daemon_version(&at("ws_2", "plain")).await.unwrap(), None);
        assert_eq!(place.daemon_version(&none).await.unwrap(), None);

        // The rest of what a folder on a joined computer answers.
        assert_eq!((place.relayed(), place.turn_road(), place.keeps_agents(), place.server_values(), place.shared_daemon()), (true, "relayed", true, "launch", true));
        assert_eq!(place.turn_reach(), Some(IndexMap::new()));
        assert!(!place.has_daemon(&srv) && !place.deploys_daemon());
        assert_eq!(place.scratch(&srv).unwrap(), place_daemon_paths("/home/srv").put_dir);
        assert_eq!(place.home_dir(&srv.record.borrow()).unwrap(), "/home/srv");
        assert_eq!(place.folder(&srv.record.borrow()), Some("/home/srv/lab".to_string()));
        assert_eq!(place.memory_key(&srv, "claude"), Some("-recorded-pr_p".to_string()));
        let e = place.daemon_road(&srv).await.unwrap_err();
        assert_eq!(e.message, place_serves_daemon_line("lab", "Srv"));
        place.land_project(&srv, &json!({}), reports().1).await.unwrap();
    });
}

#[test]
fn module_of_refuses_a_kind_the_host_did_not_wire() {
    run(async {
        // With no place door wired the place kind has no module, and the refusal names the kind.
        let joined = |w: &mut World| {
            let (door, _m) = joined_door("Srv", "/home/srv", true);
            w.doors.insert("srv".to_string(), door);
        };
        let (_host, area) = kinds_with(joined);
        let e = area.module_of(WorkspaceKind::Place).err().expect("no place door, no place module");
        assert_eq!(e.message, "this host has no place backend wired, so it serves nothing of that kind");
        assert_eq!(e.kind, None);
        assert!(area.module_of(WorkspaceKind::Cloud).is_ok());
        assert!(area.module_of(WorkspaceKind::Local).is_ok());
        // A computer is a place only where a door is wired to say it is joined; here is local and any other a cloud.
        assert_eq!((area.kind_of("here"), area.kind_of("srv"), area.kind_of("default")), (WorkspaceKind::Local, WorkspaceKind::Cloud, WorkspaceKind::Cloud));

        // With the door wired the place module answers and a joined computer lands as a place.
        let (_host, area) = kinds_with(|w| {
            joined(w);
            w.place_door = true;
        });
        assert!(area.module_of(WorkspaceKind::Place).is_ok());
        assert_eq!((area.kind_of("here"), area.kind_of("srv"), area.kind_of("default")), (WorkspaceKind::Local, WorkspaceKind::Place, WorkspaceKind::Cloud));
    });
}

#[test]
fn place_admits_only_a_root_login() {
    run(async {
        let (host, area) = kinds_with(|w| {
            let (door, _m) = joined_door("Srv", "/home/srv", true);
            w.doors.insert("srv".to_string(), door);
            w.place_names.insert("srv".to_string(), "Srv".to_string());
            w.place_door = true;
        });
        let place = module(&area, WorkspaceKind::Place);
        let record: WorkspaceRecord = serde_json::from_value(json!({ "id": "ws_p", "name": "lab", "kind": "place", "phase": "running", "place": "srv", "size": { "cpu": 2, "memMb": 4096 } })).unwrap();
        place.admit_folder(&record).await.unwrap();
        world(|w| w.login_user = "dev".to_string());
        let e = place.admit_folder(&record).await.unwrap_err();
        assert_eq!(e.kind, Some(wsp_words::exit::Kind::Usage));
        assert_eq!(e.message, wsp_words::projects::place_login_not_root_line("Srv", "dev"));
        assert_eq!(host.calls_of("door_login_user"), vec![vec!["srv".to_string()], vec!["srv".to_string()]]);
    });
}

#[test]
fn module_of_refuses_the_local_kind_on_a_host_with_no_local_road() {
    run(async {
        let (_host, area) = kinds_with(|w| w.local_wiring = None);
        let e = area.module_of(WorkspaceKind::Local).err().expect("no local module");
        assert!(e.message.contains("local"), "{}", e.message);
    });
}
