// SPDX-License-Identifier: AGPL-3.0-only
//! spec/kinds_table.md: the three kind modules, as the runtime tests read their answers (local-workspace,
//! places-fork, box-folder, terminal-folder and runtime-reach). Each test asks `module_of(kind)`.
//!
//! A module answers through wsp-core's `KindModule`: its methods take `&LiveWorkspace` (so an entry is borrowed for
//! the one call) and its futures own what they read. The helpers of `kinds_ctx` make each awaited call in one
//! statement, which ends the borrow of the entry before anything is awaited. The refusals the TS throws from `home`,
//! `homeDir` and `env` are `KindsArea::cloud_home`, `place_of` and `place_env`, asserted there.
mod machines_b_support;

use std::rc::Rc;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Value};

use machines_b_support::kinds_ctx::*;
use machines_b_support::*;
use wsp_core::types::internal::{FolderAdmit, ImportLanded, KindModule, ServerValuesRoad};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::MachineBackend;
use wsp_machines::machines::kinds::KindsArea;
use wsp_machines::machines::machines::Entry;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::projects::REGISTERING_LINE;
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::project_bundle::ProjectImportResult;
use wsp_wire::views::workspace::WorkspaceProject;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_wire::wire::machine_link::{place_forks_nowhere_line, place_serves_daemon_line};
use wsp_words::exit::Kind;
use wsp_words::project_path::place_daemon_paths;

fn module(area: &KindsArea, kind: WorkspaceKind) -> Rc<dyn KindModule> {
    area.module_of(kind).expect("the kind is wired")
}

fn record(id: &str, extra: Value, without: &[&str]) -> WorkspaceRecord {
    serde_json::from_value(record_of(id, extra, without)).unwrap()
}

/// The wsp server a launch on a fork or a joined computer hands its agent.
fn guest_wsp() -> McpServerSpec {
    McpServerSpec {
        command: "wsp".to_string(),
        args: vec!["mcp".to_string()],
        no_slate: None,
    }
}

fn cloud_entry(host: &FakeHost, machine: Arc<FakeMachine>) -> Entry {
    hold(&project("pr_1", "/root/spoo-landing", json!({})));
    put_live_preview(host, sub_record("ws_c", "running"), machine)
}

fn place_entry(host: &FakeHost, place: &str) -> Entry {
    hold(&project(
        "pr_p",
        "/home/srv/lab",
        json!({ "computer": place }),
    ));
    host.put_live(
        record_of(
            "ws_p",
            json!({ "kind": "place", "place": place, "project": "pr_p", "name": "lab" }),
            &["sub"],
        ),
        FakeMachine::new("srv-machine"),
    )
}

fn local_entry(host: &FakeHost, extra: Value) -> Entry {
    let mut e = json!({ "kind": "local", "project": "pr_l" });
    for (k, v) in extra.as_object().unwrap() {
        e[k] = v.clone();
    }
    host.put_live(
        record_of("ws_l", e, &["place", "sub"]),
        FakeMachine::new("local"),
    )
}

fn dests(of: &[&str]) -> Vec<String> {
    of.iter().map(|d| d.to_string()).collect()
}

#[test]
fn cloud_backend_is_the_hosts_own_or_the_places_and_refuses_one_that_forks_nowhere() {
    run(async {
        let boat = FakeBackend::new();
        let (host, area) = kinds_with(|w| {
            let mut door = joined_door("Boat", "/root", false);
            door.backend = Some(boat.clone());
            w.doors.insert("box".to_string(), door);
            w.doors.insert(
                "gone".to_string(),
                Door {
                    name: "Orchard".to_string(),
                    ..Door::default()
                },
            );
            w.place_door = true;
        });
        let cloud = module(&area, WorkspaceKind::Cloud);
        let own: Arc<dyn MachineBackend> = host.backend.clone();
        let boat: Arc<dyn MachineBackend> = boat;
        let backend =
            |extra: Value, without: &[&str]| cloud.backend(&record("ws_k", extra, without));
        assert!(same_backend(
            &backend(json!({}), &["place", "sub"]).ok().unwrap(),
            &own
        ));
        assert!(same_backend(
            &backend(json!({ "place": "box" }), &["sub"]).ok().unwrap(),
            &boat
        ));
        // A place the door no longer holds a backend for refuses here, and says which.
        let e = backend(json!({ "place": "gone" }), &["sub"]).err().unwrap();
        assert_eq!(
            (e.message, e.kind),
            (place_forks_nowhere_line("Orchard"), None)
        );

        // With no door wired a record naming a place has nowhere to ask.
        let (_host, area) = kinds();
        let e = module(&area, WorkspaceKind::Cloud)
            .backend(&record("ws_k", json!({ "place": "box" }), &["sub"]))
            .err()
            .unwrap();
        assert_eq!(
            e.message,
            "this host has no place backend wired, so it serves nothing of that kind"
        );
    });
}

#[test]
fn cloud_answers_its_fixed_facts() {
    run(async {
        let (host, area) = kinds();
        let machine = FakeMachine::new("bx_c");
        let entry = cloud_entry(&host, machine.clone());
        let cloud = module(&area, WorkspaceKind::Cloud);
        assert_eq!(
            (
                cloud.relayed(Some("bx_c")),
                cloud.turn_road(),
                cloud.keeps_agents(),
                cloud.server_values(),
                cloud.shared_daemon()
            ),
            (
                true,
                WorkspaceOrigin::Relayed,
                false,
                ServerValuesRoad::Environment,
                false
            )
        );
        assert_eq!(
            cloud.turn_reach(&entry.borrow()),
            Some(None),
            "a reach with no url"
        );
        assert_eq!(cloud.wsp_mcp(&entry.borrow()), Some(guest_wsp()));
        assert_eq!(cloud.scratch(&entry.borrow()), "/tmp");
        assert_eq!(
            cloud.home_dir(&held_record(&entry)),
            Some("/root".to_string())
        );
        assert_eq!(cloud.home(&entry.borrow(), "claude"), "/root/.claude-cfg");
        assert_eq!(cloud.home(&entry.borrow(), "codex"), "/root/.codex");
        assert_eq!(
            cloud.folder(&held_record(&entry)),
            Some("/root/web".to_string())
        );
        let plain = record("ws_k", json!({ "project": "pr_1" }), &["sub"]);
        assert_eq!(cloud.folder(&plain), None);
        dropped_of(&*cloud, &entry).await.unwrap();
        assert!(
            deploy_of(&*cloud, &entry).is_none() && cloud.restart_daemon(&entry.borrow()).is_none()
        );
        assert!(cloud
            .admit_folder(wsp_core::types::internal::FolderAdmit {
                kind: WorkspaceKind::Cloud,
                place: None,
                name: "x".to_string()
            })
            .is_none());
        assert!(
            cloud
                .end_thread(&entry.borrow(), "t_1", Default::default())
                .is_none()
                && cloud.end_owed(&entry.borrow(), "t_1").is_none()
        );

        // What the TS throws from `home` for an agent the catalog has no home for is the area's, not the module's.
        assert_eq!(area.cloud_home("claude").unwrap(), "/root/.claude-cfg");
        assert_eq!(
            area.cloud_home("nope").unwrap_err().message,
            "the catalog has no home for nope"
        );

        // The guest's login first, then the variable pointing the agent at its store there.
        let login = cloud.env(&entry.borrow(), None, &|id| format!("/h/{id}"));
        let claude = cloud.env(&entry.borrow(), Some("claude"), &|id| format!("/h/{id}"));
        let login_keys: Vec<&String> = login.keys().collect();
        assert!(!login_keys.is_empty());
        assert_eq!(
            claude.keys().take(login_keys.len()).collect::<Vec<_>>(),
            login_keys
        );
        assert_eq!(claude["CLAUDE_CONFIG_DIR"], "/root/.claude-cfg");
        assert!(!login.contains_key("CLAUDE_CONFIG_DIR"));
        assert_eq!(
            text(&cloud.env(&entry.borrow(), Some("nope"), &|id| format!("/h/{id}"))),
            text(&login)
        );
        assert!(!cloud.login_stands(&entry.borrow(), "claude"));

        // The key the project was recorded with, never one off the fork's own path.
        assert_eq!(
            cloud.memory_key(&entry.borrow(), "claude"),
            Some("-recorded-pr_1".to_string())
        );

        // The copy is the host's; the roots file is written through the machine.
        let (o, _bundler) = import_options("ws_c", "/root/x");
        let words = import_words(&o);
        let (seen, report) = import_report();
        let landed = import_of(&*cloud, &entry, o, report).await.unwrap();
        assert_eq!(landed.done, "copied ws_c");
        assert_eq!(
            host.calls_of("copy_import"),
            vec![vec!["ws_c".to_string(), words]]
        );
        assert!(seen.borrow().is_empty());
        roots_of(&*cloud, &entry, &dests(&["/root/a", "/root/b"]))
            .await
            .unwrap();
        let script = "next='/root/.wsp/roots.next.'$$\nmkdir -p '/root/.wsp'\nprintf '%s\\n' '/root/a' '/root/b' > \"$next\"\nmv -f \"$next\" '/root/.wsp/roots'";
        assert_eq!(machine.log(), vec![script.to_string()]);
        let said = format!("{}{}", "x".repeat(60), "y".repeat(200));
        machine.answer("mv -f", exit(1, &said));
        let e = roots_of(&*cloud, &entry, &dests(&["/root/a", "/root/b"]))
            .await
            .unwrap_err();
        assert_eq!(
            e.message,
            format!(
                "could not make /root/b browsable on the machine: {}",
                "y".repeat(200)
            )
        );
        world(|w| w.hello = Some(5));
        assert_eq!(daemon_version_of(&*cloud, &entry).await.unwrap(), Some(5));
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
        assert!(!cloud.has_daemon(&entry.borrow()));
        machine.offers.lock().unwrap().preview_url = true;
        assert!(cloud.has_daemon(&entry.borrow()));
        let road = daemon_road_of(&*cloud, &entry).await.unwrap();
        assert_eq!(
            (
                road.url.as_str(),
                road.expires_at.0,
                road.daemon_token.clone()
            ),
            ("http://127.0.0.1:1/", 9_999_999_999_999.0, None)
        );
        host.script("daemon_token_of", Ok(json!("tok")));
        let road = daemon_road_of(&*cloud, &entry).await.unwrap();
        assert_eq!(road.daemon_token, Some("tok".to_string()));
        assert_eq!(
            host.calls_of("daemon_token_of").last().unwrap(),
            &vec!["bx_c".to_string()]
        );

        world(|w| {
            w.served.insert("ws_c".to_string());
        });
        assert!(!cloud.has_daemon(&entry.borrow()));
        let e = daemon_road_of(&*cloud, &entry).await.unwrap_err();
        assert_eq!(
            e.message,
            place_serves_daemon_line("Fix the flaky upload test", "Boat")
        );

        // A deploy puts the daemon on with the token held, then forgets that token and mints it again.
        let (host, area) = kinds_with(|w| w.deploy = true);
        let entry = cloud_entry(&host, FakeMachine::new("bx_c"));
        let cloud = module(&area, WorkspaceKind::Cloud);
        host.script("daemon_token_of", Ok(json!("old")));
        let deploy = deploy_of(&*cloud, &entry).expect("a recipe that wires a deploy has one");
        deploy.await.unwrap();
        let names: Vec<String> = host
            .calls
            .borrow()
            .iter()
            .map(|(n, _)| n.clone())
            .filter(|n| {
                ["daemon_token_of", "deploy_daemon", "forget_daemon_token"].contains(&n.as_str())
            })
            .collect();
        assert_eq!(
            names,
            vec![
                "daemon_token_of",
                "deploy_daemon",
                "forget_daemon_token",
                "daemon_token_of"
            ]
        );
        assert_eq!(
            host.calls_of("deploy_daemon"),
            vec![vec!["bx_c".to_string(), "old".to_string()]]
        );
        assert_eq!(
            host.calls_of("forget_daemon_token"),
            vec![vec!["bx_c".to_string()]]
        );
    });
}

#[test]
fn local_wsp_mcp_carries_the_scoped_arg_and_turn_reach_the_here_url() {
    run(async {
        let (host, area) = kinds();
        let local = module(&area, WorkspaceKind::Local);
        let entry = local_entry(&host, json!({}));
        assert_eq!(local.wsp_mcp(&entry.borrow()), None);
        assert_eq!(local.turn_reach(&entry.borrow()), None);
        world(|w| w.agents_here_url = Some(String::new()));
        assert_eq!(
            local.turn_reach(&entry.borrow()),
            None,
            "an empty url is no url"
        );
        world(|w| {
            w.agents_wsp_mcp = Some(guest_wsp());
            w.agents_here_url = Some("ws://127.0.0.1:4400".to_string());
        });
        assert_eq!(
            local.wsp_mcp(&entry.borrow()),
            Some(McpServerSpec {
                args: vec!["mcp".to_string(), "--scoped".to_string()],
                ..guest_wsp()
            })
        );
        assert_eq!(
            local.turn_reach(&entry.borrow()),
            Some(Some("ws://127.0.0.1:4400".to_string()))
        );

        // What else a folder here answers.
        assert_eq!(
            (
                local.relayed(None),
                local.turn_road(),
                local.keeps_agents(),
                local.server_values(),
                local.shared_daemon()
            ),
            (
                false,
                WorkspaceOrigin::Here,
                true,
                ServerValuesRoad::File,
                true
            )
        );
        assert!(local.has_daemon(&entry.borrow()));
        let e = daemon_road_of(&*local, &entry).await.unwrap_err();
        assert_eq!(
            e.message,
            "this host wired no daemon for this computer, so nothing on it can be dialled"
        );
        world(|w| {
            w.local_road = serde_json::from_value(
                json!({ "url": "ws://127.0.0.1:7777", "expiresAt": 9_999_999_999_999u64 }),
            )
            .ok()
        });
        let road = daemon_road_of(&*local, &entry).await.unwrap();
        assert_eq!(
            (road.url.as_str(), road.daemon_token),
            ("ws://127.0.0.1:7777", None)
        );

        // The local wiring's own homes, environment and scratch folder; a restart only where it wired one.
        assert_eq!(local.home(&entry.borrow(), "claude"), "/home/me/.claude");
        assert_eq!(
            local.home_dir(&held_record(&entry)),
            Some("/home/me".to_string())
        );
        assert_eq!(local.scratch(&entry.borrow()), "/tmp/wsp-local");
        assert!(local.restart_daemon(&entry.borrow()).is_none());
        hold(&project(
            "pr_l",
            "/home/z/lab",
            json!({ "computer": "here" }),
        ));
        assert_eq!(
            local.folder(&held_record(&entry)),
            Some("/home/z/lab".to_string())
        );
        assert_eq!(daemon_version_of(&*local, &entry).await.unwrap(), None);
        world(|w| w.local_wiring.as_mut().unwrap().here_daemon = Some(3));
        assert_eq!(daemon_version_of(&*local, &entry).await.unwrap(), Some(3));
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
        assert_eq!(
            local.memory_key(&entry.borrow(), "claude"),
            Some("-recorded-key".to_string())
        );
        held(json!({ "git": { "top": nested } }));
        assert_eq!(
            local.memory_key(&entry.borrow(), "claude"),
            Some("-recorded-key".to_string())
        );
        // A subfolder project takes the key Claude Code gives it in the person's own terminal: its repo's top.
        held(json!({ "git": { "top": "/repos/lab" } }));
        assert_eq!(
            local.memory_key(&entry.borrow(), "claude"),
            Some("-repos-lab".to_string())
        );
        // A worktree wsp made keys off its own path, so a builder there never loads the person's notes.
        set_worktree(&entry, json!({ "path": "/wt/lab-feature", "made": true }));
        assert_eq!(
            local.memory_key(&entry.borrow(), "claude"),
            Some("-wt-lab-feature".to_string())
        );
        // Not once it is gone, and never one the person made.
        set_worktree(
            &entry,
            json!({ "path": "/wt/lab-feature", "made": true, "gone": true }),
        );
        assert_eq!(
            local.memory_key(&entry.borrow(), "claude"),
            Some("-repos-lab".to_string())
        );
        set_worktree(&entry, json!({ "path": "/wt/lab-feature", "made": false }));
        assert_eq!(
            local.memory_key(&entry.borrow(), "claude"),
            Some("-repos-lab".to_string())
        );
    });
}

#[test]
fn local_import_registers_in_place() {
    run(async {
        let (host, area) = kinds();
        let local = module(&area, WorkspaceKind::Local);
        let entry = local_entry(&host, json!({}));
        hold(&project(
            "pr_l",
            "/home/z/lab",
            json!({ "computer": "here" }),
        ));
        let (seen, report) = import_report();
        let (o, bundler) = import_options("ws_l", "/home/z/lab");
        let landed = import_of(&*local, &entry, o, report).await.unwrap();
        assert_eq!(
            *seen.borrow(),
            vec![("landing".to_string(), REGISTERING_LINE.to_string())]
        );
        assert_eq!(bundler.plans(), 1);
        let registered = WorkspaceProject {
            name: "lab".to_string(),
            dest: "/home/z/lab".to_string(),
            imported_at: "2025-10-09T08:53:20.000Z".to_string(),
            size: Some(js_int(3456.0)),
        };
        let want = ImportLanded {
            result: ProjectImportResult {
                dest: "/home/z/lab".to_string(),
                files: js(12.0),
                bytes: js(3456.0),
                parts: js(0.0),
                cut: Vec::new(),
                rewritten: Vec::new(),
                agents: Vec::new(),
                project: registered.clone(),
            },
            done: "lab registered at /home/z/lab; nothing was copied.".to_string(),
        };
        assert_eq!(landed, want);
        assert_eq!(area.project_of("/home/z/lab", 3456.0), registered);
        // Nothing is landed on the way: a folder here is the person's own.
        let held = host.calls.borrow().len();
        land_project_of(
            &*local,
            &entry,
            project("pr_l", "/home/z/lab", json!({ "computer": "here" })),
            reports().1,
        )
        .await
        .unwrap();
        assert_eq!(host.calls.borrow().len(), held);
    });
}

#[test]
fn place_port_reach_forwards_and_holds_for_the_pane_hold() {
    run(async {
        let (host, area) = kinds_with(|w| {
            let door = joined_door("Srv", "/home/srv", true);
            w.doors.insert("srv".to_string(), door);
            w.place_door = true;
        });
        let entry = place_entry(&host, "srv");
        let place = module(&area, WorkspaceKind::Place);
        let reach = port_reach_of(&*place, &entry, 3000, false).await.unwrap();
        assert_eq!(
            (reach.url.as_str(), reach.expires_at.0),
            ("http://localhost:41000/", 1_760_000_000_000.0 + 120_000.0)
        );
        port_reach_of(&*place, &entry, 3000, true).await.unwrap();
        let pane = r#"{"workspaceId":"ws_p","name":"lab"}"#;
        assert_eq!(
            host.calls_of("door_forward"),
            vec![
                ["srv", "3000", pane, "false"].map(String::from).to_vec(),
                ["srv", "3000", pane, "true"].map(String::from).to_vec()
            ]
        );
        host.clock.advance(60_000.0).await;
        assert_eq!(
            port_reach_of(&*place, &entry, 3000, false)
                .await
                .unwrap()
                .expires_at
                .0,
            1_760_000_060_000.0 + 120_000.0
        );
        host.script("door_forward", Err(failed("link is down")));
        assert!(port_reach_of(&*place, &entry, 3001, false).await.is_err());
        // The cloud and the local kind forward no port of their own.
        assert!(module(&area, WorkspaceKind::Cloud)
            .port_reach(&entry.borrow(), 3000, false)
            .is_none());

        // The roots file goes beside the home the computer was joined with, written by the door and never as the login.
        roots_of(&*place, &entry, &dests(&["/home/srv/lab", "/home/srv/web"]))
            .await
            .unwrap();
        let script = "next='/home/srv/.wsp/roots.next.'$$\nmkdir -p '/home/srv/.wsp'\nprintf '%s\\n' '/home/srv/lab' '/home/srv/web' > \"$next\"\nmv -f \"$next\" '/home/srv/.wsp/roots'";
        assert_eq!(
            host.calls_of("door_exec"),
            vec![vec![
                "srv".to_string(),
                script.to_string(),
                "20000".to_string()
            ]]
        );
        let said = format!("{}{}", "x".repeat(60), "y".repeat(200));
        host.script(
            "door_exec",
            Ok(json!({ "exitCode": 1, "stdout": "", "stderr": said })),
        );
        let e = roots_of(&*place, &entry, &dests(&["/home/srv/lab", "/home/srv/web"]))
            .await
            .unwrap_err();
        assert_eq!(
            e.message,
            format!(
                "could not make /home/srv/web browsable on Srv: {}",
                "y".repeat(200)
            )
        );
    });
}

#[test]
fn place_import_refuses_with_the_add_line() {
    run(async {
        let (host, area) = kinds_with(|w| {
            let door = joined_door("Srv", "/home/srv", true);
            w.doors.insert("srv".to_string(), door);
            w.place_names.insert("srv".to_string(), "Srv".to_string());
            w.place_door = true;
        });
        let entry = place_entry(&host, "srv");
        let (o, bundler) = import_options("ws_p", "/home/srv/x");
        let e = import_of(
            &*module(&area, WorkspaceKind::Place),
            &entry,
            o,
            import_report().1,
        )
        .await
        .unwrap_err();
        assert_eq!(e.message, "lab is a project folder on Srv; a folder from this computer goes there as a project of its own, with wsp add <folder> --on Srv");
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(host.calls_of("copy_import").is_empty() && bundler.plans() == 0);
    });
}

#[test]
fn place_wsp_mcp_only_where_the_report_says_tools() {
    run(async {
        let (host, area) = kinds_with(|w| {
            let mut door = joined_door("Srv", "/home/srv", true);
            door.daemon_version = Some(7);
            w.doors.insert("srv".to_string(), door);
            let plain = joined_door("Plain", "/home/plain", false);
            w.doors.insert("plain".to_string(), plain);
            w.doors.insert(
                "bare".to_string(),
                Door {
                    name: "Bare".to_string(),
                    joined: true,
                    ..Door::default()
                },
            );
            w.place_names.insert("srv".to_string(), "Srv".to_string());
            w.place_door = true;
        });
        let place = module(&area, WorkspaceKind::Place);
        let at = |id: &str, name: &str| {
            host.put_live(
                record_of(
                    id,
                    json!({ "kind": "place", "place": name, "project": "pr_p", "name": "lab" }),
                    &["sub"],
                ),
                FakeMachine::new(&format!("{name}-machine")),
            )
        };
        hold(&project("pr_p", "/home/srv/lab", json!({})));
        assert_eq!(
            place.wsp_mcp(&at("ws_1", "srv").borrow()),
            Some(guest_wsp())
        );
        assert_eq!(
            place.wsp_mcp(&at("ws_2", "plain").borrow()),
            None,
            "a daemon that bound no door has no wsp to launch"
        );
        assert_eq!(place.wsp_mcp(&at("ws_3", "bare").borrow()), None);
        let none = host.put_live(
            record_of(
                "ws_4",
                json!({ "kind": "place", "project": "pr_p" }),
                &["place", "sub"],
            ),
            FakeMachine::new("m4"),
        );
        assert_eq!(place.wsp_mcp(&none.borrow()), None);

        // The daemon's version is what the computer's report says, none without a place or a report.
        let srv = at("ws_1", "srv");
        assert_eq!(daemon_version_of(&*place, &srv).await.unwrap(), Some(7));
        assert_eq!(
            daemon_version_of(&*place, &at("ws_2", "plain"))
                .await
                .unwrap(),
            None
        );
        assert_eq!(daemon_version_of(&*place, &none).await.unwrap(), None);

        // The rest of what a folder on a joined computer answers.
        assert_eq!(
            (
                place.relayed(Some("srv-machine")),
                place.turn_road(),
                place.keeps_agents(),
                place.server_values(),
                place.shared_daemon()
            ),
            (
                true,
                WorkspaceOrigin::Relayed,
                true,
                ServerValuesRoad::Launch,
                true
            )
        );
        assert_eq!(place.turn_reach(&srv.borrow()), Some(None));
        assert!(
            !place.has_daemon(&srv.borrow())
                && deploy_of(&*place, &srv).is_none()
                && place.restart_daemon(&srv.borrow()).is_none()
        );
        assert_eq!(
            place.scratch(&srv.borrow()),
            place_daemon_paths("/home/srv").put_dir
        );
        assert_eq!(
            place.home_dir(&held_record(&srv)),
            Some("/home/srv".to_string())
        );
        assert_eq!(
            place.folder(&held_record(&srv)),
            Some("/home/srv/lab".to_string())
        );
        assert_eq!(
            place.memory_key(&srv.borrow(), "claude"),
            Some("-recorded-pr_p".to_string())
        );
        let e = daemon_road_of(&*place, &srv).await.unwrap_err();
        assert_eq!(e.message, place_serves_daemon_line("lab", "Srv"));
        let before = host.calls.borrow().len();
        land_project_of(
            &*place,
            &srv,
            project("pr_p", "/home/srv/lab", json!({})),
            reports().1,
        )
        .await
        .unwrap();
        assert_eq!(host.calls.borrow().len(), before);

        // What the TS throws from `homeDir` and `env` for a record its door no longer holds is `place_of`'s.
        let held = area
            .place_of(&held_record(&srv))
            .expect("srv is a computer the door holds");
        assert_eq!((held.home.as_str(), held.tools), ("/home/srv", true));
        let at_none = record(
            "ws_n",
            json!({ "kind": "place", "name": "lab" }),
            &["place", "sub"],
        );
        assert_eq!(
            area.place_of(&at_none).err().unwrap().message,
            "lab stands on no computer, which this host no longer holds"
        );
        let away = record(
            "ws_a",
            json!({ "kind": "place", "place": "away", "name": "lab" }),
            &["sub"],
        );
        assert_eq!(
            area.place_of(&away).err().unwrap().message,
            "lab stands on away, which this host no longer holds"
        );
        let (_host, bare) = kinds();
        let e = bare.place_of(&away).err().unwrap();
        assert_eq!(
            e.message,
            "this host has no place backend wired, so it serves nothing of that kind"
        );
    });
}

#[test]
fn module_of_refuses_a_kind_the_host_did_not_wire() {
    run(async {
        // With no place door wired the place kind has no module, and the refusal names the kind.
        let joined = |w: &mut World| {
            let door = joined_door("Srv", "/home/srv", true);
            w.doors.insert("srv".to_string(), door);
        };
        let (_host, area) = kinds_with(joined);
        let e = area
            .module_of(WorkspaceKind::Place)
            .err()
            .expect("no place door, no place module");
        assert_eq!(
            e.message,
            "this host has no place backend wired, so it serves nothing of that kind"
        );
        assert_eq!(e.kind, None);
        assert!(area.module_of(WorkspaceKind::Cloud).is_ok());
        assert!(area.module_of(WorkspaceKind::Local).is_ok());
        // A computer is a place only where a door is wired to say it is joined; here is local and any other a cloud.
        assert_eq!(
            (
                area.kind_of("here"),
                area.kind_of("srv"),
                area.kind_of("default")
            ),
            (
                WorkspaceKind::Local,
                WorkspaceKind::Cloud,
                WorkspaceKind::Cloud
            )
        );

        // With the door wired the place module answers and a joined computer lands as a place.
        let (_host, area) = kinds_with(|w| {
            joined(w);
            w.place_door = true;
        });
        assert!(area.module_of(WorkspaceKind::Place).is_ok());
        assert_eq!(
            (
                area.kind_of("here"),
                area.kind_of("srv"),
                area.kind_of("default")
            ),
            (
                WorkspaceKind::Local,
                WorkspaceKind::Place,
                WorkspaceKind::Cloud
            )
        );
    });
}

#[test]
fn place_admits_only_a_root_login() {
    run(async {
        let (host, area) = kinds_with(|w| {
            let door = joined_door("Srv", "/home/srv", true);
            w.doors.insert("srv".to_string(), door);
            w.place_names.insert("srv".to_string(), "Srv".to_string());
            w.place_door = true;
        });
        let place = module(&area, WorkspaceKind::Place);
        let admit = FolderAdmit {
            kind: WorkspaceKind::Place,
            place: Some("srv".to_string()),
            name: "lab".to_string(),
        };
        place
            .admit_folder(admit.clone())
            .expect("a place admits folders")
            .await
            .unwrap();
        world(|w| w.login_user = "dev".to_string());
        let e = place
            .admit_folder(admit)
            .expect("a place admits folders")
            .await
            .unwrap_err();
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(
            e.message,
            wsp_words::projects::place_login_not_root_line("Srv", "dev")
        );
        assert_eq!(
            host.calls_of("door_login_user"),
            vec![vec!["srv".to_string()], vec!["srv".to_string()]]
        );
    });
}

#[test]
fn module_of_refuses_the_local_kind_on_a_host_with_no_local_road() {
    run(async {
        let (_host, area) = kinds_with(|w| w.local_wiring = None);
        let e = area
            .module_of(WorkspaceKind::Local)
            .err()
            .expect("no local module");
        assert!(e.message.contains("local"), "{}", e.message);
    });
}

#[test]
fn a_door_that_signs_in_is_read_as_the_agents_word() {
    run(async {
        let (host, area) = kinds_with(|w| {
            let mut door = joined_door("Srv", "/home/srv", true);
            door.sign_ins = Some(IndexMap::from([(
                "codex".to_string(),
                AgentSignInState::SignedIn,
            )]));
            w.doors.insert("srv".to_string(), door);
            w.place_door = true;
        });
        let entry = place_entry(&host, "srv");
        let place = module(&area, WorkspaceKind::Place);
        assert!(
            place.login_stands(&entry.borrow(), "codex")
                && !place.login_stands(&entry.borrow(), "claude")
        );
    });
}
