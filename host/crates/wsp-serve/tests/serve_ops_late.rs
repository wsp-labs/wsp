// SPDX-License-Identifier: AGPL-3.0-only
//! The op switch's own ops: the prefix ops, sessions, images, usage, forwards, folders, agents, editors, the host's
//! doors and the catch. Each TS case is ported at this crate's seam: the area is scripted, and the test reads what the
//! socket sent and what the runtime was asked. Spec: spec/serve_ops.md.
#![allow(dead_code, unused_imports)]
mod support;

include!("serve_ops/world.rs");

// O14. serve.test.ts:893
#[test]
fn forwards_list_shows_only_drivable_and_stop_refuses_an_unknown() {
    block(async {
        let source = Forwards::with(vec![forward("ws_1", 8123), forward("ws_2", 8200)]);
        let r = rig(|o| o.forwards = Some(source.clone() as Rc<dyn ForwardsSource>));
        r.rt.script.on::<Result<Option<String>, HostError>>("workspaces.originRefusal", |call| {
            Ok(if call.args["id"] == json!("ws_2") { Some("no workspace ws_2 on this road".to_string()) } else { None })
        });
        let (host, sink) = r.host().await;
        let listed = request(&host, &sink, 1, "forwards.list", json!({})).await;
        assert_eq!(listed, json!({ "id": 1, "ok": true, "forwards": [forward("ws_1", 8123)] }));
        let asked: Vec<Value> = r.rt.script.calls_to("workspaces.originRefusal").iter().map(|c| c.args["id"].clone()).collect();
        assert_eq!(asked, vec![json!("ws_1"), json!("ws_2")], "each asked in the source's order");

        let refused = request(&host, &sink, 2, "forwards.stop", json!({ "workspaceId": "ws_2", "port": 8200 })).await;
        assert_eq!(refused, json!({ "id": 2, "ok": false, "error": "no workspace ws_2 on this road" }));
        assert!(source.stops.borrow().is_empty(), "a forward the caller may not drive is not stopped");

        let stopped = request(&host, &sink, 3, "forwards.stop", json!({ "workspaceId": "ws_1", "port": 8123 })).await;
        assert_eq!(stopped, json!({ "id": 3, "ok": true }));
        let again = request(&host, &sink, 4, "forwards.stop", json!({ "workspaceId": "ws_1", "port": 8123 })).await;
        assert_eq!(again, json!({ "id": 4, "ok": false, "error": "nothing is forwarding localhost:8123 there" }));
        assert_eq!(*source.stops.borrow(), vec![("ws_1".to_string(), 8123), ("ws_1".to_string(), 8123)]);
        let after = request(&host, &sink, 5, "forwards.list", json!({})).await;
        assert_eq!(after["forwards"], json!([]));

        let low = request_unchecked(&host, &sink, 6, "forwards.stop", json!({ "workspaceId": "ws_1", "port": 80 })).await;
        assert_eq!(low["ok"], json!(false));
        assert_eq!(source.stops.borrow().len(), 2, "below 1024 never reaches the source");
        assert_eq!(args_of(&r.rt, "workspaces.originRefusal", 0)["origin"], Value::Null);
    })
}

// O14. serve.test.ts:928
#[test]
fn without_a_source_the_list_is_empty_and_a_stop_fails() {
    block(async {
        let r = rig(|_| {});
        r.rt.script.on::<Result<Option<String>, HostError>>("workspaces.originRefusal", |_| Ok(None));
        let (host, sink) = r.host().await;
        let listed = request(&host, &sink, 1, "forwards.list", json!({})).await;
        assert_eq!(listed, json!({ "id": 1, "ok": true, "forwards": [] }));
        let stop = request(&host, &sink, 2, "forwards.stop", json!({ "workspaceId": "ws_1", "port": 8123 })).await;
        assert_eq!(stop, json!({ "id": 2, "ok": false, "error": "nothing is forwarding localhost:8123 there" }));
        assert_eq!(r.failed_ops(), vec!["forwards.stop"]);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O15. Folders.

// O15. places-agents.test.ts
#[test]
fn host_folders_wide_only_for_this_computers_own_window() {
    block(async {
        let folders = Rc::new(Folders { asked: RefCell::new(Vec::new()) });
        let r = rig(|o| o.folders = Some(folders.clone() as Rc<dyn HostFolders>));
        let (host, sink) = r.host().await;
        let asked = || folders.asked.borrow().clone();

        let reply =
            request(&host, &sink, 1, "host.folders", json!({ "origin": "here", "dir": "/x", "hidden": false, "repos": true })).await;
        assert_eq!(reply, json!({ "id": 1, "ok": true, "listing": listing() }));
        assert_eq!(asked()[0], json!({ "dir": "/x", "hidden": false, "repos": true, "wide": true }));

        request(&host, &sink, 2, "host.folders", json!({ "origin": "here", "on": "here", "repos": false })).await;
        assert_eq!(asked()[1], json!({ "wide": true }), "on: here is this computer; repos false is not sent");
        request(&host, &sink, 3, "host.folders", json!({})).await;
        assert_eq!(asked()[2], json!({ "wide": false }), "a frame that names no road is not the own window");
        request(&host, &sink, 4, "host.folders", json!({ "origin": "relayed" })).await;
        assert_eq!(asked()[3], json!({ "wide": false }));

        // host.folders is in neither DEVICE_OPS nor THREAD_OPS (protocol runtime.ts), so a paired computer and a thread
        // are refused at the door by name and the folders door is never asked.
        let (paired, paired_sink) = r.paired().await;
        let held = request(&paired, &paired_sink, 1, "host.folders", json!({ "origin": "here" })).await;
        assert_eq!(held["error"], json!(device_held_refusal("host.folders")));
        let (thread, thread_sink) = r.thread("ws_1").await;
        let reply = request(&thread, &thread_sink, 1, "host.folders", json!({ "origin": "here" })).await;
        assert_eq!(reply["ok"], json!(false));
        assert_eq!(asked().len(), 4);
        assert!(r.rt.script.calls_to("places.list").is_empty(), "this computer is read through the folders door, never the place door");
    })
}

// O15. places-agents.test.ts
#[test]
fn place_folders_refuse_in_order_and_close_the_link() {
    block(async {
        let rt = FakeRuntime::with_places();
        let folders = Rc::new(Folders { asked: RefCell::new(Vec::new()) });
        let r = rig_over(rt.clone(), |o| {
            o.folders = Some(folders.clone() as Rc<dyn HostFolders>);
            o.now = Some(Rc::new(|| 1234));
        });
        let rows = || vec![place("pl_1", "laptop", "computer"), place("box", "box", "provider")];
        let report = |version: i64| -> PlaceReport {
            from(json!({
                "name": "laptop", "platform": "linux", "arch": "x64", "os": "linux", "shape": { "cpu": 2, "memMb": 4096 }, "login": {},
                "runsWorkspaces": true, "engine": "none", "daemonVersion": version, "wsp": [], "agents": [], "dialed": "10.0.0.2",
            }))
        };
        let (host, sink) = r.host().await;
        let on = |place: &str| json!({ "on": place, "dir": "/d" });

        // 1. A word that names no row.
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        let held = vec![said("laptop", "computer"), said("box", "provider")];
        let nobody = request(&host, &sink, 1, "host.folders", on("pl_9")).await;
        assert_eq!(
            nobody.to_string(),
            json!({ "id": 1, "ok": false, "error": no_such_place_refusal("pl_9", &held), "kind": "usage" }).to_string()
        );
        assert_eq!(args_of(&rt, "places.list", 0)["now"], json!(1234));

        // 2. A provider keeps no computer to browse; nothing more is asked of it.
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        let provider = request(&host, &sink, 2, "host.folders", on("box")).await;
        assert_eq!(
            provider.to_string(),
            json!({ "id": 2, "ok": false, "error": provider_folders_refusal(&said("box", "provider")), "kind": "usage" }).to_string()
        );
        assert!(rt.script.calls_to("places.reportOf").is_empty());

        // 3. A daemon older than the folders op.
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        rt.script.once::<Result<Option<PlaceReport>, HostError>>("places.reportOf", Ok(Some(report(60))));
        let behind = request(&host, &sink, 3, "host.folders", on("pl_1")).await;
        let word = place_daemon_behind(Some(60)).expect("60 is behind");
        assert_eq!(behind.to_string(), json!({ "id": 3, "ok": false, "error": place_behind_line("laptop", &word) }).to_string());
        assert_eq!(args_of(&rt, "places.reportOf", 0)["placeId"], json!("pl_1"));
        assert!(rt.script.calls_to("places.channel").is_empty(), "no link is opened for a daemon that cannot answer");

        // 4. No link held.
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        rt.script.once::<Result<Option<PlaceReport>, HostError>>("places.reportOf", Ok(Some(report(80))));
        rt.script.once::<Option<Rc<dyn DaemonChannel>>>("places.channel", None);
        let absent = request(&host, &sink, 4, "host.folders", on("pl_1")).await;
        assert_eq!(absent.to_string(), json!({ "id": 4, "ok": false, "error": absent_computer("laptop", None).sentence }).to_string());
        assert!(rt.script.calls_to("projects.list").is_empty());

        // 5. The link answers: the folders of that computer's projects ride the frame, and the link is closed.
        let l = link(Ok(json!({ "ok": true, "dir": "/d", "roots": ["/home/u"], "folders": [], "hidden": 0 })));
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        rt.script.once::<Result<Option<PlaceReport>, HostError>>("places.reportOf", Ok(Some(report(80))));
        rt.script.once::<Option<Rc<dyn DaemonChannel>>>("places.channel", Some(l.clone() as Rc<dyn DaemonChannel>));
        rt.script.once::<Result<Vec<ProjectView>, HostError>>(
            "projects.list",
            Ok(vec![project("pr_1", "pl_1", Some("/co")), project("pr_2", "pl_1", None), project("pr_3", "pl_2", Some("/else"))]),
        );
        let ok = request(&host, &sink, 5, "host.folders", json!({ "on": "pl_1", "dir": "/d", "repos": true })).await;
        assert_eq!(ok, json!({ "id": 5, "ok": true, "listing": { "dir": "/d", "roots": ["/home/u"], "folders": [], "hidden": 0 } }));
        let sent = l.sent.borrow().clone();
        assert_eq!(sent.len(), 1);
        assert_eq!(Value::Object(sent[0].clone()), json!({ "op": "fs.folders", "dir": "/d", "repos": true, "projects": ["/co", "/app"] }));
        assert_eq!(l.closes.get(), 1);
        assert!(folders.asked.borrow().is_empty(), "another computer is not read through this computer's folders door");

        // 6. The daemon says no: its words fail the op, and the link is closed all the same.
        let l = link(Ok(json!({ "ok": false, "error": "not a folder" })));
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        rt.script.once::<Result<Option<PlaceReport>, HostError>>("places.reportOf", Ok(Some(report(80))));
        rt.script.once::<Option<Rc<dyn DaemonChannel>>>("places.channel", Some(l.clone() as Rc<dyn DaemonChannel>));
        rt.script.once::<Result<Vec<ProjectView>, HostError>>("projects.list", Ok(vec![]));
        let no = request(&host, &sink, 6, "host.folders", on("pl_1")).await;
        assert_eq!(no, json!({ "id": 6, "ok": false, "error": "not a folder" }));
        assert_eq!(l.closes.get(), 1);

        // 7. The link itself fails.
        let l = link(Err(ChannelError::Refused { message: "connection lost".into() }));
        rt.script.once::<Result<Vec<PlaceView>, HostError>>("places.list", Ok(rows()));
        rt.script.once::<Result<Option<PlaceReport>, HostError>>("places.reportOf", Ok(None));
        rt.script.once::<Option<Rc<dyn DaemonChannel>>>("places.channel", Some(l.clone() as Rc<dyn DaemonChannel>));
        rt.script.once::<Result<Vec<ProjectView>, HostError>>("projects.list", Ok(vec![]));
        let lost = request(&host, &sink, 7, "host.folders", on("pl_1")).await;
        assert_eq!(lost, json!({ "id": 7, "ok": false, "error": "connection lost" }));
        assert_eq!(l.closes.get(), 1, "closed whatever happens");

        // A socket off the own road is refused before any row is read.
        let (ticketed, ticket_sink) = r.ticketed().await;
        let before = rt.script.calls_to("places.list").len();
        let refused = request(&ticketed, &ticket_sink, 1, "host.folders", on("pl_1")).await;
        assert_eq!(refused, ticket_reply(1));
        assert_eq!(rt.script.calls_to("places.list").len(), before);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O16-O19. Agents.

// O16, O17. places-agents.test.ts
#[test]
fn agents_ops_gates_and_sentences() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let (ticketed, ticket_sink) = r.ticketed().await;
        let t = target();

        for (id, op, fields) in
            [(1, "agents.read", json!({ "target": t })), (2, "servers.tools", json!({ "target": t, "agent": "claude", "name": "s" }))]
        {
            let reply = request(&ticketed, &ticket_sink, id, op, fields).await;
            assert_eq!(
                reply.to_string(),
                json!({ "id": id, "ok": false, "error": PLACES_TICKET_REFUSAL }).to_string(),
                "{op}: no kind, as the TS sends it"
            );
        }
        let gated: Vec<(&str, Value)> = vec![
            ("servers.icon", json!({ "host": "x.com" })),
            ("agents.addTools", json!({ "target": t, "agent": "claude" })),
            ("agents.setup", json!({ "placeId": "pl_1", "agent": "claude" })),
            ("skills.search", json!({ "q": "a" })),
            ("skills.get", json!({ "skill": "x" })),
            ("skills.preview", json!({ "target": t, "name": "n" })),
            ("skills.add", json!({ "target": t, "skill": "x" })),
            ("skills.remove", json!({ "target": t, "name": "n" })),
            ("skills.toggle", json!({ "target": t, "name": "n", "on": true })),
            ("servers.add", json!({ "target": t, "agent": "claude", "name": "n" })),
            ("servers.remove", json!({ "target": t, "agent": "claude", "name": "n" })),
            ("servers.toggle", json!({ "target": t, "agent": "claude", "name": "n", "on": true })),
            ("plugins.toggle", json!({ "target": t, "agent": "claude", "plugin": "p", "on": true })),
        ];
        for (i, (op, fields)) in gated.iter().enumerate() {
            let id = 10 + i as u64;
            assert_eq!(request(&ticketed, &ticket_sink, id, op, fields.clone()).await, ticket_reply(id), "{op}");
        }
        assert!(
            rt.script.names().iter().all(|n| !n.starts_with("agents.")),
            "a socket off the own road reaches no agents area: {:?}",
            rt.script.names()
        );
        assert!(r.failed.borrow().is_empty());

        // agents.signInLine: host only. A paired computer is held at the door (places-agents.test.ts:513-520); the
        // browser wsp init let in is on the own road without the host's token.
        let (paired, paired_sink) = r.paired().await;
        let (browser, browser_sink) = r.browser().await;
        let line = json!({ "target": t, "agent": "claude" });
        let held = request(&paired, &paired_sink, 1, "agents.signInLine", line.clone()).await;
        assert_eq!(held.to_string(), json!({ "id": 1, "ok": false, "error": device_held_refusal("agents.signInLine") }).to_string());
        let by_device = request(&browser, &browser_sink, 1, "agents.signInLine", line.clone()).await;
        assert_eq!(by_device.to_string(), json!({ "id": 1, "ok": false, "error": SIGN_IN_LINE_REFUSAL }).to_string());
        let off_road = request(&ticketed, &ticket_sink, 40, "agents.signInLine", line.clone()).await;
        assert_eq!(off_road, ticket_reply(40));
        assert!(rt.script.calls_to("agents.signInLine").is_empty());
        let (host, sink) = r.host().await;
        rt.script.once::<Result<wsp_wire::types::agents_report::SignInLine, HostError>>(
            "agents.signInLine",
            Ok(from(json!({ "command": "wsp login claude" }))),
        );
        let ok = request(&host, &sink, 1, "agents.signInLine", json!({ "target": t, "agent": "claude", "name": "srv" })).await;
        assert_eq!(ok["line"]["command"], json!("wsp login claude"));
        let ask = wsp_core::types::pending::SignInAsk {
            agent: "claude".into(),
            server: Some("srv".into()),
            terminal: None,
            scope: None,
            project: None,
        };
        assert_eq!(args_of(rt, "agents.signInLine", 0)["ask"], json!(format!("{ask:?}")), "name rides as server");

        // agents.key: host only; the key never comes back.
        let held = request(&paired, &paired_sink, 2, "agents.key", json!({ "agent": "claude", "key": "sk-secret-1" })).await;
        assert_eq!(held.to_string(), json!({ "id": 2, "ok": false, "error": device_held_refusal("agents.key") }).to_string());
        let by_device = request(&browser, &browser_sink, 2, "agents.key", json!({ "agent": "claude", "key": "sk-secret-1" })).await;
        assert_eq!(by_device.to_string(), json!({ "id": 2, "ok": false, "error": AGENTS_KEY_REFUSAL }).to_string());
        assert!(rt.script.calls_to("agents.key").is_empty());
        rt.script.once::<Result<(), HostError>>("agents.key", Ok(()));
        let keyed = request(&host, &sink, 2, "agents.key", json!({ "agent": "claude", "key": "sk-secret-1", "way": "token" })).await;
        assert_eq!(keyed.to_string(), r#"{"id":2,"ok":true}"#);
        let a = args_of(rt, "agents.key", 0);
        assert_eq!((a["agent"].clone(), a["key"].clone(), a["way"].clone()), (json!("claude"), json!("sk-secret-1"), json!("Token")));

        // agents.read is no paired computer's either: the door refuses it by name.
        let held = request(&paired, &paired_sink, 3, "agents.read", json!({ "target": t })).await;
        assert_eq!(held["error"], json!(device_held_refusal("agents.read")));
        assert!(rt.script.calls_to("agents.read").is_empty());
    })
}

// O18. vault.test.ts
#[test]
fn agents_setup_env_values_only_from_the_host() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let stop = || Err::<wsp_wire::types::agents_report::AgentRow, _>(HostError::new("stop"));
        let (paired, paired_sink) = r.browser().await;
        let with_value = json!({ "placeId": "pl_1", "agent": "claude", "env": { "TOKEN": "v-1" } });
        let refused = request(&paired, &paired_sink, 1, "agents.setup", with_value.clone()).await;
        assert_eq!(refused.to_string(), json!({ "id": 1, "ok": false, "error": ENV_VALUE_REFUSAL }).to_string());
        assert!(rt.script.calls_to("agents.setup").is_empty());

        rt.script.once::<Result<wsp_wire::types::agents_report::AgentRow, HostError>>("agents.setup", stop());
        let cleared = request(
            &paired,
            &paired_sink,
            2,
            "agents.setup",
            json!({ "placeId": "pl_1", "agent": "claude", "on": true, "env": { "TOKEN": null } }),
        )
        .await;
        assert_eq!(cleared["error"], json!("stop"), "a socket that is no host's may clear a variable");
        let change = args_of(rt, "agents.setup", 0);
        assert_eq!((change["placeId"].clone(), change["agent"].clone()), (json!("pl_1"), json!("claude")));
        assert_eq!(change["change"]["on"], json!(true));
        assert_eq!(change["change"]["env"], json!({ "TOKEN": null }));
        assert_eq!(change["origin"], Value::Null, "the browser wsp init let in is no paired computer and sends no word of its own");

        rt.script.once::<Result<wsp_wire::types::agents_report::AgentRow, HostError>>("agents.setup", stop());
        let (host, sink) = r.host().await;
        request(&host, &sink, 1, "agents.setup", with_value).await;
        assert_eq!(args_of(rt, "agents.setup", 1)["change"]["env"], json!({ "TOKEN": "v-1" }), "the host's own token may put a value in");
        let change = args_of(rt, "agents.setup", 1)["change"].clone();
        for key in ["id", "op", "placeId", "agent"] {
            assert!(change.get(key).is_none(), "{key} is not part of the change");
        }

        let (ticketed, ticket_sink) = r.ticketed().await;
        let off =
            request(&ticketed, &ticket_sink, 1, "agents.setup", json!({ "placeId": "pl_1", "agent": "claude", "env": { "TOKEN": "v-1" } }))
                .await;
        assert_eq!(off, ticket_reply(1), "the road is refused before the env is read");
    })
}

// O19. places-agents.test.ts, vault.test.ts
#[test]
fn skills_and_servers_pass_their_fields() {
    block(async {
        let r = rig(|_| {});
        let rt = &r.rt;
        let (host, sink) = r.host().await;
        let t = target();
        let stop = || HostError::new("stop");

        rt.script.once::<Result<Vec<wsp_wire::types::agents_report::SkillHit>, HostError>>("agents.skillsSearch", Ok(vec![]));
        rt.script.once::<Result<Vec<wsp_wire::types::agents_report::SkillHit>, HostError>>("agents.skillsSearch", Ok(vec![]));
        let found = request(&host, &sink, 1, "skills.search", json!({ "q": "pdf" })).await;
        assert_eq!(found.to_string(), r#"{"id":1,"ok":true,"skills":[]}"#);
        request(&host, &sink, 2, "skills.search", json!({ "q": "pdf", "limit": 5 })).await;
        assert_eq!(
            (args_of(rt, "agents.skillsSearch", 0)["q"].clone(), args_of(rt, "agents.skillsSearch", 0)["limit"].clone()),
            (json!("pdf"), Value::Null)
        );
        assert_eq!(args_of(rt, "agents.skillsSearch", 1)["limit"], json!(5));

        rt.script.once::<Result<wsp_wire::types::agents_report::SkillPreview, HostError>>("agents.skillsGet", Err(stop()));
        request(&host, &sink, 3, "skills.get", json!({ "skill": "owner/pdf" })).await;
        assert_eq!(args_of(rt, "agents.skillsGet", 0)["skill"], json!("owner/pdf"));

        rt.script.once::<Result<wsp_wire::types::agents_report::SkillPreview, HostError>>("agents.skillsPreview", Err(stop()));
        request(&host, &sink, 4, "skills.preview", json!({ "target": t, "name": "pdf", "project": false })).await;
        let preview = wsp_core::types::pending::SkillAsk { name: "pdf".into(), project: Some(false) };
        assert_eq!(args_of(rt, "agents.skillsPreview", 0)["ask"], json!(format!("{preview:?}")), "a false project is sent");
        assert_eq!(args_of(rt, "agents.skillsPreview", 0)["target"], t);

        rt.script.once::<Result<wsp_wire::types::agents_report::SkillAdded, HostError>>("agents.skillsAdd", Err(stop()));
        rt.script.once::<Result<wsp_wire::types::agents_report::SkillAdded, HostError>>("agents.skillsAdd", Err(stop()));
        request(&host, &sink, 5, "skills.add", json!({ "target": t, "skill": "pdf" })).await;
        request(&host, &sink, 6, "skills.add", json!({ "target": t, "skill": "pdf", "agents": ["claude"], "project": true })).await;
        let add = |agents: Option<Vec<String>>, project: Option<bool>| {
            json!(format!("{:?}", SkillsAddAsk { skill: "pdf".into(), agents, project }))
        };
        assert_eq!(args_of(rt, "agents.skillsAdd", 0)["ask"], add(None, None));
        assert_eq!(args_of(rt, "agents.skillsAdd", 1)["ask"], add(Some(vec!["claude".into()]), Some(true)));

        rt.script.once::<Result<Vec<String>, HostError>>("agents.skillsRemove", Err(stop()));
        rt.script.once::<Result<Vec<String>, HostError>>("agents.skillsToggle", Err(stop()));
        request(&host, &sink, 7, "skills.remove", json!({ "target": t, "name": "pdf" })).await;
        request(&host, &sink, 8, "skills.toggle", json!({ "target": t, "name": "pdf", "project": true, "on": false })).await;
        let remove = wsp_core::types::pending::SkillAsk { name: "pdf".into(), project: None };
        assert_eq!(args_of(rt, "agents.skillsRemove", 0)["ask"], json!(format!("{remove:?}")));
        let toggle = wsp_core::types::pending::SkillAsk { name: "pdf".into(), project: Some(true) };
        assert_eq!(args_of(rt, "agents.skillsToggle", 0)["ask"], json!(format!("{toggle:?}")));
        assert_eq!(args_of(rt, "agents.skillsToggle", 0)["on"], json!(false));

        rt.script.once::<Result<String, HostError>>("agents.serversAdd", Err(stop()));
        request(
            &host,
            &sink,
            9,
            "servers.add",
            json!({ "target": t, "agent": "claude", "name": "s", "command": "npx", "args": ["x"], "env": { "K": "v" } }),
        )
        .await;
        let add = args_of(rt, "agents.serversAdd", 0);
        assert_eq!(add["target"], t);
        let ask = &add["ask"];
        assert_eq!((ask["agent"].clone(), ask["name"].clone(), ask["command"].clone()), (json!("claude"), json!("s"), json!("npx")));
        assert_eq!((ask["args"].clone(), ask["env"].clone()), (json!(["x"]), json!({ "K": "v" })));
        for key in ["target", "id", "op"] {
            assert!(ask.get(key).is_none(), "{key} is not part of the server");
        }
        assert!(ask.get("url").map_or(true, Value::is_null), "what was not sent is not sent");

        rt.script.once::<Result<String, HostError>>("agents.serversRemove", Err(stop()));
        rt.script.once::<Result<String, HostError>>("agents.serversToggle", Err(stop()));
        request(&host, &sink, 10, "servers.remove", json!({ "target": t, "agent": "claude", "name": "s" })).await;
        request(&host, &sink, 11, "servers.toggle", json!({ "target": t, "agent": "claude", "name": "s", "scope": "user", "on": true }))
            .await;
        let remove = &args_of(rt, "agents.serversRemove", 0)["ask"];
        assert_eq!((remove["agent"].clone(), remove["name"].clone()), (json!("claude"), json!("s")));
        assert!(remove.get("scope").map_or(true, Value::is_null));
        let toggle = args_of(rt, "agents.serversToggle", 0);
        assert_eq!(toggle["ask"]["scope"], json!("user"));
        assert_eq!(toggle["on"], json!(true));

        rt.script.once::<Result<wsp_wire::types::agents_report::PluginRow, HostError>>("agents.pluginsToggle", Err(stop()));
        request(&host, &sink, 12, "plugins.toggle", json!({ "target": t, "agent": "claude", "plugin": "p", "on": false })).await;
        let plugin = args_of(rt, "agents.pluginsToggle", 0);
        assert_eq!((plugin["ask"].clone(), plugin["on"].clone()), (json!({ "agent": "claude", "plugin": "p" }), json!(false)));

        rt.script.once::<Result<wsp_wire::types::agents_report::ServerToolsAnswer, HostError>>("agents.tools", Err(stop()));
        rt.script.once::<Result<wsp_wire::types::agents_report::ServerToolsAnswer, HostError>>("agents.tools", Err(stop()));
        request(&host, &sink, 13, "servers.tools", json!({ "target": t, "agent": "claude", "name": "s" })).await;
        request(&host, &sink, 14, "servers.tools", json!({ "target": t, "agent": "claude", "name": "s", "refresh": true })).await;
        let tools = |refresh: Option<bool>| json!(format!("{:?}", ToolsAsk { agent: "claude".into(), name: "s".into(), refresh }));
        assert_eq!(args_of(rt, "agents.tools", 0)["ask"], tools(None));
        assert_eq!(args_of(rt, "agents.tools", 1)["ask"], tools(Some(true)));

        rt.script.once::<Result<Option<String>, HostError>>("agents.serversIcon", Ok(None));
        let icon = request(&host, &sink, 15, "servers.icon", json!({ "host": "x.com", "refresh": true })).await;
        assert_eq!(icon.to_string(), r#"{"id":15,"ok":true,"icon":null}"#);
        let a = args_of(rt, "agents.serversIcon", 0);
        assert_eq!((a["host"].clone(), a["refresh"].clone()), (json!("x.com"), json!(true)));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O20, O21. Editors and ssh.

fn editor_rig(include: bool, remote_refusal: Option<&str>) -> (Rig, Rc<Editor>, Rc<Ssh>, Rc<RefCell<Vec<String>>>) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let editor = Rc::new(Editor { log: log.clone(), opens: RefCell::new(Vec::new()), remote_refusal: remote_refusal.map(String::from) });
    let ssh = Rc::new(Ssh { log: log.clone(), include: Cell::new(include) });
    let (e, s) = (editor.clone(), ssh.clone());
    let r = rig(move |o| {
        o.editor = Some(e as Rc<dyn HostEditor>);
        o.ssh = Some(s as Rc<dyn HostSsh>);
    });
    (r, editor, ssh, log)
}

// O20. editor-door.test.ts
#[test]
fn editor_open_here_and_over_ssh() {
    block(async {
        let (r, editor, _ssh, log) = editor_rig(true, None);
        let rt = &r.rt;
        let (host, sink) = r.host().await;

        // A workspace on this computer opens inside its worktree, then its project.
        rt.script.once::<Result<WorkspaceView, HostError>>(
            "workspaces.get",
            Ok(workspace("ws_1", "app", "local", "running", json!({ "worktree": { "path": "/wt/app", "made": true } }))),
        );
        let here =
            request(&host, &sink, 1, "editor.open", json!({ "workspaceId": "ws_1", "path": "src/a.ts", "line": 7, "editor": "zed" })).await;
        assert_eq!(here, json!({ "id": 1, "ok": true, "editor": "zed" }));
        assert_eq!(
            editor.opens.borrow()[0],
            EditorOpen {
                path: "src/a.ts".into(),
                line: Some(7),
                inside: vec!["/wt/app".into(), "/app".into()],
                editor: Some(EditorId::Zed),
                remote: None
            }
        );
        assert_eq!(args_of(rt, "workspaces.get", 0)["id"], json!("ws_1"));
        assert!(rt.script.calls_to("preferences.get").is_empty(), "a named editor reads no preferences");
        assert_eq!(*log.borrow(), vec!["open".to_string()]);

        // A napping workspace elsewhere is woken once the editor says it can open a remote.
        log.borrow_mut().clear();
        rt.script.once::<Result<WorkspaceView, HostError>>(
            "workspaces.get",
            Ok(workspace("ws_2", "box1", "cloud", "napping", json!({ "folder": "/home/u/app" }))),
        );
        rt.script.once::<Result<WorkspaceView, HostError>>(
            "workspaces.wake",
            Ok(workspace("ws_2", "box1-woken", "cloud", "running", json!({}))),
        );
        let over = request(&host, &sink, 2, "editor.open", json!({ "workspaceId": "ws_2", "path": "a.rs", "editor": "vscode" })).await;
        assert_eq!(over, json!({ "id": 2, "ok": true, "editor": "vscode" }));
        assert_eq!(
            *log.borrow(),
            vec![
                "remoteRefusal Some(Vscode) box1".to_string(),
                "include".to_string(),
                "port ws_2 box1-woken".to_string(),
                "open".to_string()
            ],
            "the editor is asked first, then the include, then the port on the woken workspace, then the editor opens"
        );
        assert_eq!(args_of(rt, "workspaces.wake", 0)["id"], json!("ws_2"));
        let remote = EditorRemote { alias: ssh_alias("box1"), folder: "/home/u/app".into(), name: "box1".into() };
        assert_eq!(
            editor.opens.borrow()[1],
            EditorOpen { path: "a.rs".into(), line: None, inside: vec![], editor: Some(EditorId::Vscode), remote: Some(remote) }
        );

        // A running one is not woken, and without a folder the project's path is the folder.
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_3", "box2", "cloud", "running", json!({}))));
        request(&host, &sink, 3, "editor.open", json!({ "workspaceId": "ws_3", "path": "b.rs", "editor": "vscode" })).await;
        assert_eq!(rt.script.calls_to("workspaces.wake").len(), 1);
        let last = editor.opens.borrow()[2].clone();
        assert_eq!(last.remote, Some(EditorRemote { alias: ssh_alias("box2"), folder: "/app".into(), name: "box2".into() }));
        assert!(log.borrow().contains(&"port ws_3 box2".to_string()));
    })
}

// O20. editor-door.test.ts
#[test]
fn editor_refuses_without_ssh_include() {
    block(async {
        let (r, editor, _ssh, log) = editor_rig(false, None);
        let rt = &r.rt;
        let (host, sink) = r.host().await;
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_2", "box1", "cloud", "napping", json!({}))));
        let reply = request(&host, &sink, 1, "editor.open", json!({ "workspaceId": "ws_2", "path": "a.rs", "editor": "zed" })).await;
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": false, "error": ssh_include_line("box1"), "kind": "sshInclude" }).to_string());
        assert!(rt.script.calls_to("workspaces.wake").is_empty(), "no wake for a line the person has not added");
        assert!(editor.opens.borrow().is_empty());
        assert_eq!(log.borrow().iter().filter(|l| l.starts_with("port")).count(), 0);
        assert!(r.failed.borrow().is_empty(), "the switch sent it");

        // The editor that cannot open a remote costs no wake and no include read.
        let (r2, editor2, _s, log2) = editor_rig(true, Some("zed cannot open a remote folder"));
        r2.rt
            .script
            .once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_2", "box1", "cloud", "napping", json!({}))));
        let (host2, sink2) = r2.host().await;
        let refused = request(&host2, &sink2, 1, "editor.open", json!({ "workspaceId": "ws_2", "path": "a.rs", "editor": "zed" })).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": "zed cannot open a remote folder" }));
        assert_eq!(*log2.borrow(), vec!["remoteRefusal Some(Zed) box1".to_string()]);
        assert!(r2.rt.script.calls_to("workspaces.wake").is_empty() && editor2.opens.borrow().is_empty());
        assert_eq!(r2.failed_ops(), vec!["editor.open"]);

        // No ssh door at all: the editor is not asked.
        let log3 = Rc::new(RefCell::new(Vec::new()));
        let editor3 = Rc::new(Editor { log: log3.clone(), opens: RefCell::new(Vec::new()), remote_refusal: None });
        let r3 = rig(|o| o.editor = Some(editor3 as Rc<dyn HostEditor>));
        r3.rt
            .script
            .once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_2", "box1", "cloud", "running", json!({}))));
        let (host3, sink3) = r3.host().await;
        let none = request(&host3, &sink3, 1, "editor.open", json!({ "workspaceId": "ws_2", "path": "a.rs", "editor": "zed" })).await;
        assert_eq!(none, json!({ "id": 1, "ok": false, "error": editor_opens_here_line("box1") }));
        assert!(log3.borrow().is_empty());

        // A socket off the own road reads neither the list nor the open.
        let (ticketed, ticket_sink) = r.ticketed().await;
        for (id, op, fields) in [(1, "editor.list", json!({})), (2, "editor.open", json!({ "workspaceId": "ws_2", "path": "a.rs" }))] {
            let reply = request(&ticketed, &ticket_sink, id, op, fields).await;
            assert_eq!(reply, json!({ "id": id, "ok": false, "error": EDITOR_TICKET_REFUSAL, "kind": "ticket" }), "{op}");
        }
        let listed = request(&host, &sink, 2, "editor.list", json!({})).await;
        assert_eq!(listed, json!({ "id": 2, "ok": true, "editors": [{ "id": "vscode", "name": "VS Code" }] }));
    })
}

// O21. ssh-door.test.ts
#[test]
fn ssh_port_refuses_a_workspace_here() {
    block(async {
        let (r, _editor, ssh, log) = editor_rig(true, None);
        let rt = &r.rt;
        let (host, sink) = r.host().await;
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_1", "app", "local", "running", json!({}))));
        let here = request(&host, &sink, 1, "ssh.port", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(here, json!({ "id": 1, "ok": false, "error": ssh_copy_here_line("app") }));
        assert_eq!(r.failed_ops(), vec!["ssh.port"]);
        assert!(log.borrow().iter().all(|l| !l.starts_with("port")), "no port is asked for a copy on this computer");

        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_2", "box1", "cloud", "napping", json!({}))));
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.wake", Ok(workspace("ws_2", "box1", "cloud", "running", json!({}))));
        let there = request(&host, &sink, 2, "ssh.port", json!({ "workspaceId": "ws_2" })).await;
        assert_eq!(there, json!({ "id": 2, "ok": true, "port": 2222 }));
        assert!(log.borrow().contains(&"port ws_2 box1".to_string()));
        assert_eq!(args_of(rt, "workspaces.wake", 0)["id"], json!("ws_2"));

        log.borrow_mut().clear();
        let read = request(&host, &sink, 3, "ssh.include", json!({})).await;
        assert_eq!(read, json!({ "id": 3, "ok": true, "sshInclude": true }));
        let set = request(&host, &sink, 4, "ssh.include", json!({ "on": false })).await;
        assert_eq!(set, json!({ "id": 4, "ok": true, "sshInclude": false }));
        assert_eq!(*log.borrow(), vec!["include".to_string(), "setInclude false".to_string()]);
        assert!(!ssh.include.get());

        let (ticketed, ticket_sink) = r.ticketed().await;
        let off = request(&ticketed, &ticket_sink, 1, "ssh.port", json!({ "workspaceId": "ws_2" })).await;
        assert_eq!(off, json!({ "id": 1, "ok": false, "error": SSH_TICKET_REFUSAL, "kind": "ticket" }));
        let off = request(&ticketed, &ticket_sink, 2, "ssh.include", json!({})).await;
        assert_eq!(off["kind"], json!("ticket"));
        assert_eq!(log.borrow().len(), 2, "the door is not read for a socket off the own road");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O22. The host's doors.

// O22. serve.test.ts:1174
#[test]
fn init_keys_reaches_the_door_and_the_key_never_comes_back() {
    block(async {
        let door = Rc::new(Init { calls: RefCell::new(Vec::new()) });
        let r = rig(|o| o.init = Some(door.clone() as Rc<dyn InitDoor>));
        let (host, sink) = r.host().await;
        let got = request(&host, &sink, 1, "init.get", json!({})).await;
        assert_eq!(got["setup"], setup());
        request(&host, &sink, 2, "init.get", json!({ "on": "pl_1" })).await;
        let saved = request(&host, &sink, 3, "init.keys", json!({ "provider": "box", "key": "ascii_live_fake" })).await;
        assert_eq!(keys(&saved), vec!["id", "ok", "setup"]);
        let mut expected = setup();
        expected["keys"] = json!({ "box": true, "solari": true });
        assert_eq!(saved["setup"], expected);
        assert!(!saved.to_string().contains("ascii_live_fake"), "a key never comes back");
        request(&host, &sink, 4, "init.keys", json!({ "provider": "solari" })).await;
        assert_eq!(
            *door.calls.borrow(),
            vec![json!(["get", null]), json!(["get", "pl_1"]), json!(["keys", "box", "ascii_live_fake"]), json!(["keys", "solari", null])],
            "the door is handed each field the frame named and no other"
        );
    })
}

// O22. serve.test.ts, release.test.ts
#[test]
fn host_doors_pass_their_fields() {
    block(async {
        let terminal = Rc::new(Terminal { schemes: RefCell::new(Vec::new()) });
        let release = Rc::new(Release { checks: RefCell::new(Vec::new()) });
        let r = rig(|o| {
            o.terminal_config = Some(terminal.clone() as Rc<dyn HostTerminalConfig>);
            o.release = Some(release.clone() as Rc<dyn ReleaseDoor>);
        });
        let (host, sink) = r.host().await;

        let config = request(&host, &sink, 1, "host.terminalConfig", json!({ "scheme": "dark" })).await;
        assert_eq!(keys(&config), vec!["id", "ok", "config"]);
        assert_eq!(config["config"]["fontFamily"], json!(["Menlo"]));
        request(&host, &sink, 2, "host.terminalConfig", json!({})).await;
        assert_eq!(*terminal.schemes.borrow(), vec![Some(wsp_wire::views::project_bundle::TerminalScheme::Dark), None]);

        let got = request(&host, &sink, 3, "release.get", json!({})).await;
        assert_eq!(got["release"]["installed"], json!("0.3.7"));
        assert!(release.checks.borrow().is_empty(), "get reads nothing from the network");
        request(&host, &sink, 4, "release.check", json!({})).await;
        request(&host, &sink, 5, "release.check", json!({ "force": false })).await;
        let forced = request(&host, &sink, 6, "release.check", json!({ "force": true })).await;
        assert_eq!(*release.checks.borrow(), vec![false, false, true], "force is read as === true");
        assert_eq!(forced["release"]["installed"], json!("0.3.8"));

        r.rt.script.once::<Result<wsp_core::types::api::PreferencesSet, HostError>>("preferences.set", Err(HostError::new("stop")));
        request(&host, &sink, 7, "preferences.set", json!({ "patch": {} })).await;
        assert_eq!(args_of(&r.rt, "preferences.set", 0)["patch"], json!({}));
        r.rt.script.once::<Result<wsp_wire::views::preferences::Preferences, HostError>>("preferences.get", Err(HostError::new("stop")));
        request(&host, &sink, 8, "preferences.get", json!({})).await;
        assert_eq!(r.rt.script.calls_to("preferences.get").len(), 1);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O23. Restart.

struct Restart {
    refusal: Option<String>,
    started: Cell<u32>,
    fail: bool,
    gate: Rc<tokio::sync::Notify>,
    reply_seen: RefCell<Vec<bool>>,
    sink: RefCell<Option<Rc<RecordingSink>>>,
}

fn restart_door(fail: bool, refusal: Option<&str>) -> Rc<Restart> {
    Rc::new(Restart {
        refusal: refusal.map(String::from),
        started: Cell::new(0),
        fail,
        gate: Rc::new(tokio::sync::Notify::new()),
        reply_seen: RefCell::new(Vec::new()),
        sink: RefCell::new(None),
    })
}

#[async_trait(?Send)]
impl RestartDoor for Restart {
    fn refusal(&self) -> Option<String> {
        self.refusal.clone()
    }
    async fn restart(&self) -> Result<(), HostError> {
        self.started.set(self.started.get() + 1);
        let seen = self.sink.borrow().as_ref().map_or(false, |s| s.frames().iter().any(|f| f["ok"] == json!(true)));
        self.reply_seen.borrow_mut().push(seen);
        if self.fail {
            return Err(HostError::new("boom"));
        }
        self.gate.notified().await;
        Ok(())
    }
}

async fn settle() {
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
}

// O23. restart-door.test.ts
#[test]
fn host_restart_replies_then_restarts_once() {
    block(async {
        let door = restart_door(false, None);
        let r = rig(|o| o.restart = Some(door.clone() as Rc<dyn RestartDoor>));
        let (host, sink) = r.host().await;
        *door.sink.borrow_mut() = Some(sink.clone());
        let first = request(&host, &sink, 1, "host.restart", json!({})).await;
        assert_eq!(first, json!({ "id": 1, "ok": true }));
        settle().await;
        assert_eq!(door.started.get(), 1);
        assert_eq!(*door.reply_seen.borrow(), vec![true], "the restart starts after the reply is sent");

        let second = request(&host, &sink, 2, "host.restart", json!({})).await;
        assert_eq!(second, json!({ "id": 2, "ok": true }));
        settle().await;
        assert_eq!(door.started.get(), 1, "an ask that overlaps joins the restart under way");

        door.gate.notify_one();
        settle().await;
        request(&host, &sink, 3, "host.restart", json!({})).await;
        settle().await;
        assert_eq!(door.started.get(), 1, "a restart that went through is never started twice");
        assert!(r.failed.borrow().is_empty());
    })
}

// O23. restart-door.test.ts
#[test]
fn host_restart_is_the_own_road_and_names_what_would_stop_it() {
    block(async {
        let door = restart_door(false, None);
        let r = rig(|o| o.restart = Some(door.clone() as Rc<dyn RestartDoor>));
        let (ticketed, ticket_sink) = r.ticketed().await;
        let off = request(&ticketed, &ticket_sink, 1, "host.restart", json!({})).await;
        assert_eq!(off, json!({ "id": 1, "ok": false, "error": HOST_RESTART_TICKET_REFUSAL, "kind": "ticket" }));
        settle().await;
        assert_eq!(door.started.get(), 0);

        let none = rig(|_| {});
        let (host, sink) = none.host().await;
        let unwired = request(&host, &sink, 1, "host.restart", json!({})).await;
        assert_eq!(unwired, json!({ "id": 1, "ok": false, "error": HOST_NO_RESTART_LINE }));
        assert_eq!(none.failed_ops(), vec!["host.restart"]);

        let refusing = restart_door(false, Some("this host was started by hand; start it again there"));
        let r = rig(|o| o.restart = Some(refusing.clone() as Rc<dyn RestartDoor>));
        let (host, sink) = r.host().await;
        let refused = request(&host, &sink, 1, "host.restart", json!({})).await;
        assert_eq!(refused, json!({ "id": 1, "ok": false, "error": "this host was started by hand; start it again there" }));
        settle().await;
        assert_eq!(refusing.started.get(), 0);
        assert_eq!(sink.replies(1).len(), 1, "no ok is sent before a refusal");
    })
}

// O23. restart-door.test.ts
#[test]
fn a_failed_restart_is_logged_and_tried_again() {
    block(async {
        let door = restart_door(true, None);
        let r = rig(|o| o.restart = Some(door.clone() as Rc<dyn RestartDoor>));
        let (host, sink) = r.host().await;
        let first = request(&host, &sink, 1, "host.restart", json!({})).await;
        assert_eq!(first, json!({ "id": 1, "ok": true }));
        settle().await;
        assert_eq!(door.started.get(), 1);
        assert_eq!(*r.logs.borrow(), vec!["host.restart failed: boom".to_string()]);

        let again = request(&host, &sink, 2, "host.restart", json!({})).await;
        assert_eq!(again, json!({ "id": 2, "ok": true }));
        settle().await;
        assert_eq!(door.started.get(), 2, "a failed restart forgets itself, so the next ask tries again");
        assert_eq!(*r.logs.borrow(), vec!["host.restart failed: boom".to_string(), "host.restart failed: boom".to_string()]);
        assert!(r.failed.borrow().is_empty(), "the failure is logged, not told to failed: the reply was already sent");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// O24. Project ops.

// O24. project-import.test.ts, project-export.test.ts
#[test]
fn project_ops_hand_the_bundler_and_lander() {
    block(async {
        let sources = Rc::new(RefCell::new(Vec::new()));
        let kept = sources.clone();
        let r = rig(move |o| {
            o.projects = Some(Rc::new(move |source: &str| {
                kept.borrow_mut().push(source.to_string());
                Rc::new(Bundler { source: source.to_string() }) as Rc<dyn ProjectBundler>
            }));
            o.landing = Some(Rc::new(Lander) as Rc<dyn ProjectLander>);
        });
        let rt = &r.rt;
        let (host, sink) = r.host().await;

        let plan = json!({ "source": "/s", "remote": null, "branch": "main", "defaultBranch": null, "unpushed": null, "uncommitted": 0, "memory": null, "files": [], "remembered": false });
        rt.script.once::<Result<wsp_wire::views::workspace::SeedPlan, HostError>>("projects.seedPlan", Ok(from(plan)));
        let seeded = request(&host, &sink, 1, "project.seed.plan", json!({ "source": "/s", "on": "pl_1" })).await;
        assert_eq!(keys(&seeded), vec!["id", "ok", "plan"]);
        assert_eq!(seeded["plan"]["source"], json!("/s"));
        let a = args_of(rt, "projects.seedPlan", 0);
        assert_eq!((a["source"].clone(), a["on"].clone()), (json!("/s"), json!("pl_1")));
        assert!(sources.borrow().is_empty(), "the seed plan reads no folder through the bundler");

        let planned = request(&host, &sink, 2, "project.plan", json!({ "source": "/src" })).await;
        assert_eq!(keys(&planned), vec!["id", "ok", "plan"]);
        assert_eq!(planned["plan"]["source"], json!("/src"));
        assert_eq!(planned["plan"]["files"], json!(3));
        assert_eq!(*sources.borrow(), vec!["/src".to_string()], "the bundler is made over the frame's source");
        assert!(rt.script.calls_to("projects.seedPlan").len() == 1, "project.plan reads the folder, not the area");

        rt.script.once::<Result<wsp_wire::views::project_bundle::ProjectImportResult, HostError>>(
            "projects.import",
            Ok(from(json!({
                "dest": "/d", "files": 3, "bytes": 10, "parts": 1, "cut": [], "rewritten": [], "agents": [],
                "project": { "name": "app", "dest": "/d", "importedAt": WHEN },
            }))),
        );
        let imported = request(
            &host, &sink, 3, "project.import",
            json!({ "workspaceId": "ws_1", "source": "/src2", "dest": "/d", "replace": true, "carry": ["a"], "rewrite": [], "agents": ["claude"], "origin": "here" }),
        )
        .await;
        assert_eq!(keys(&imported), vec!["id", "ok", "imported"]);
        assert_eq!(imported["imported"]["dest"], json!("/d"));
        assert_eq!(*sources.borrow(), vec!["/src".to_string(), "/src2".to_string()], "the import's bundler is made over its own source");
        assert_eq!(args_of(rt, "projects.import", 0)["origin"], json!("here"));

        rt.script.once::<Result<wsp_wire::views::project_bundle::ProjectExportResult, HostError>>(
            "projects.export",
            Ok(from(json!({ "dest": "/e", "files": 2, "bytes": 5, "excluded": [], "agents": [] }))),
        );
        let exported = request(&host, &sink, 4, "project.export", json!({ "workspaceId": "ws_1", "source": "/work", "dest": "/e" })).await;
        assert_eq!(keys(&exported), vec!["id", "ok", "exported"]);
        assert_eq!(exported["exported"]["dest"], json!("/e"));
        assert_eq!(rt.script.calls_to("projects.export").len(), 1);
        assert_eq!(sources.borrow().len(), 2, "an export makes no bundler: it hands the lander");
        assert!(r.failed.borrow().is_empty());
    })
}
