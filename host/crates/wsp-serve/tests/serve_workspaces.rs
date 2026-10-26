// SPDX-License-Identifier: AGPL-3.0-only
//! serve_workspaces: events, status, the workspace verbs, daemon channels, readings and exec. Spec:
//! spec/serve_workspaces.md. Each case is the TS case ported at this crate's seam: the fake runtime is scripted with
//! what the area would have answered, and the test reads what the socket sent and what the runtime was asked.
#![allow(dead_code, unused_imports)]
mod support;

include!("serve_workspaces/world.rs");

// ---- S1: events ----------------------------------------------------------------------------------------------

// S1. serve.test.ts:345
#[test]
fn events_reach_only_subscribed_sockets() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (sub, sub_sink) = host_socket(&served).await;
        let (_quiet, quiet_sink) = host_socket(&served).await;
        let reply = request(&sub, &sub_sink, 1, "events.subscribe", json!({})).await;
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": true, "seq": 0, "stream": "stream-a" }).to_string());
        let reads: Vec<String> = rt.script.names().into_iter().filter(|n| n.starts_with("events.")).collect();
        assert_eq!(reads, vec!["events.since", "events.on"], "the replay is read and the listener taken in one step");
        assert_eq!(rt.script.calls_to("events.since")[0].args, json!({ "after": null, "stream": null }));
        assert_eq!(rt.script.calls_to("events.on")[0].args, json!({ "type": "*" }));

        rt.bus.emit(&ev("ws_1", 1));
        rt.bus.emit(&ev("ws_2", 2));
        assert_eq!(sub_sink.pushes(), vec![ev_json("ws_1", 1), ev_json("ws_2", 2)]);
        assert!(quiet_sink.pushes().is_empty(), "a socket that did not subscribe hears nothing");

        sub.closed();
        assert_eq!(rt.bus.listening(), 0, "the subscription dies with the socket");
        rt.bus.emit(&ev("ws_3", 3));
        assert_eq!(sub_sink.pushes().len(), 2);
    })
}

// S1. serve-replay.test.ts:83-186
#[test]
fn subscribe_answers_head_and_stream_then_replays_after_the_cursor() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.once::<Since>("events.since", since(14, vec![ev("ws_1", 13), ev("ws_1", 14)], false));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 2, "events.subscribe", json!({ "after": 12, "stream": "stream-a" })).await;
        assert_eq!(
            reply.to_string(),
            json!({ "id": 2, "ok": true, "seq": 14, "stream": "stream-a" }).to_string(),
            "no gap key unless there is a gap"
        );
        assert_eq!(rt.script.calls_to("events.since")[0].args, json!({ "after": 12, "stream": "stream-a" }));
        let after_reply: Vec<Value> = frames_of(&sink, 2);
        assert_eq!(after_reply[1..].to_vec(), vec![ev_json("ws_1", 13), ev_json("ws_1", 14)], "the reply goes before the replayed events");
        rt.bus.emit(&ev("ws_1", 15));
        assert_eq!(
            sink.pushes(),
            vec![ev_json("ws_1", 13), ev_json("ws_1", 14), ev_json("ws_1", 15)],
            "once, in order, then live continues"
        );
    })
}

// S1. serve-replay.test.ts:136 (a cursor older than the ring)
#[test]
fn a_cursor_older_than_the_ring_is_a_gap_with_no_replay() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.once::<Since>("events.since", since(5011, vec![], true));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 2, "events.subscribe", json!({ "after": 1 })).await;
        assert_eq!(reply.to_string(), json!({ "id": 2, "ok": true, "seq": 5011, "stream": "stream-a", "gap": true }).to_string());
        assert!(sink.pushes().is_empty(), "a gap replays nothing");
        rt.bus.emit(&ev("ws_1", 5012));
        assert_eq!(sink.pushes(), vec![ev_json("ws_1", 5012)]);
    })
}

// S1. serve-replay.test.ts:136 (a cursor from a runtime that restarted is past head)
#[test]
fn a_cursor_past_head_is_a_gap() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.once::<Since>("events.since", since(11, vec![], true));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 2, "events.subscribe", json!({ "after": 90 })).await;
        assert_eq!(reply.to_string(), json!({ "id": 2, "ok": true, "seq": 11, "stream": "stream-a", "gap": true }).to_string());
        rt.bus.emit(&ev("ws_1", 12));
        assert_eq!(sink.pushes(), vec![ev_json("ws_1", 12)]);
    })
}

// S1. serve-replay.test.ts:136 (the old stream's cursor, the new runtime's stream)
#[test]
fn a_cursor_from_another_stream_is_a_gap() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.once::<Since>("events.since", Since { stream: "stream-b".into(), head: 19, events: vec![], gap: true });
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 2, "events.subscribe", json!({ "after": 12, "stream": "stream-a" })).await;
        assert_eq!(
            reply.to_string(),
            json!({ "id": 2, "ok": true, "seq": 19, "stream": "stream-b", "gap": true }).to_string(),
            "the bus's own stream is the one named"
        );
        assert_eq!(
            rt.script.calls_to("events.since")[0].args,
            json!({ "after": 12, "stream": "stream-a" }),
            "the client's stream is what the bus compares"
        );
        assert!(sink.pushes().is_empty());
    })
}

// S1. serve.test.ts:1326 and the runtime's one rule: seen_by decides, shown_to words the frame
#[test]
fn an_event_the_caller_may_not_see_is_never_sent() {
    support::block(async {
        let rt = FakeRuntime::new();
        rt.script.on("workspaces.seenBy", |call| -> bool { call.args["event"]["workspaceId"] != json!("ws_hidden") });
        rt.script.on("workspaces.shownTo", |call| -> Value {
            let mut shown = call.args["event"].clone();
            shown["shownTo"] = call.args["origin"].clone();
            shown
        });
        rt.script.once::<Since>("events.since", since(2, vec![ev("ws_hidden", 1), ev("ws_ok", 2)], false));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        request(&socket, &sink, 2, "events.subscribe", json!({})).await;
        rt.bus.emit(&ev("ws_hidden", 3));
        rt.bus.emit(&ev("ws_ok", 4));
        let mut first = ev_json("ws_ok", 2);
        first["shownTo"] = Value::Null;
        let mut second = ev_json("ws_ok", 4);
        second["shownTo"] = Value::Null;
        assert_eq!(
            sink.pushes(),
            vec![first, second],
            "a hidden event is never sent, replayed or live; the rest go out as shown_to words them"
        );

        // the frame's own word is the origin an unstamped socket is read by
        let (relayed, relayed_sink) = host_socket(&served).await;
        request(&relayed, &relayed_sink, 3, "events.subscribe", json!({ "origin": "relayed" })).await;
        rt.bus.emit(&ev("ws_ok", 5));
        let last = relayed_sink.pushes().pop().expect("an event");
        assert_eq!(last["shownTo"], json!("relayed"));
        let seen = rt.script.calls_to("workspaces.seenBy");
        assert_eq!(seen.last().unwrap().args["origin"], json!("relayed"));
    })
}

// S1. serve.ts:1170-1188: one cost_moved per subscribe
#[test]
fn a_cost_tick_that_moved_nothing_is_dropped() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (first, first_sink) = host_socket(&served).await;
        request(&first, &first_sink, 1, "events.subscribe", json!({})).await;
        rt.bus.emit(&cost("ws_a", 0.5));
        let (second, second_sink) = host_socket(&served).await;
        request(&second, &second_sink, 1, "events.subscribe", json!({})).await;
        rt.bus.emit(&cost("ws_a", 0.5));
        assert_eq!(first_sink.pushes().len(), 1, "the same figures are dropped on the socket that already heard them");
        assert_eq!(second_sink.pushes().len(), 1, "each socket has its own memory of what it was sent");
        rt.bus.emit(&cost("ws_a", 0.75));
        assert_eq!((first_sink.pushes().len(), second_sink.pushes().len()), (2, 2), "a figure that moved goes out");
    })
}

// S1. serve.test.ts:1257, 1274
#[test]
fn doctor_lines_reach_subscribers_before_the_reply_and_are_never_replayed() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let rows = vec![de::<PlaceView>(json!({ "id": "p_1", "kind": "computer", "name": "spoo", "default": false }))];
        rt.script.on("places.list", move |_| -> Result<Vec<PlaceView>, HostError> { Ok(rows.clone()) });
        rt.script.on("places.pending", |_| Ok::<Vec<wsp_wire::views::place::PendingComputer>, HostError>(vec![]));
        let doctor = Rc::new(FakeDoctor {
            script: rt.script.clone(),
            lines: RefCell::new(vec![("spoo answers".into(), "out".into()), ("DOCTOR FAIL: spoo".into(), "err".into())]),
            listeners: Rc::new(RefCell::new(Vec::new())),
            next: Cell::new(0),
        });
        let served = served_with(&rt, ServeOptions { doctor: Some(doctor.clone()), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        request(&socket, &sink, 1, "events.subscribe", json!({})).await;
        let reply = request(&socket, &sink, 2, "places.doctor", json!({ "placeId": "p_1", "doctorId": "d_1" })).await;
        assert_eq!(reply.to_string(), json!({ "id": 2, "ok": true, "code": 1 }).to_string());
        let frames = sink.frames();
        let at = frames.iter().position(|f| f.get("id") == Some(&json!(2))).unwrap();
        assert_eq!(
            frames[at - 2..at].to_vec(),
            vec![
                json!({ "type": "doctor.line", "doctorId": "d_1", "line": "spoo answers", "stream": "out" }),
                json!({ "type": "doctor.line", "doctorId": "d_1", "line": "DOCTOR FAIL: spoo", "stream": "err" }),
            ],
            "every line landed before the reply, in the order the road said them, with no sequence"
        );

        // a socket that subscribes after the road spoke is replayed none of them
        let (late, late_sink) = host_socket(&served).await;
        let sub = request(&late, &late_sink, 3, "events.subscribe", json!({})).await;
        assert_eq!(sub["ok"], json!(true));
        let listed = request(&late, &late_sink, 4, "places.list", json!({})).await;
        assert_eq!(listed["ok"], json!(true), "a round trip after the subscription");
        assert!(late_sink.pushes().is_empty());

        socket.closed();
        late.closed();
        assert!(doctor.listeners.borrow().is_empty(), "the doctor listener is let go with the socket");
    })
}

// S1. serve.ts:1170-1188: the release door's changes ride the same pass
#[test]
fn release_changes_reach_subscribers() {
    support::block(async {
        let rt = FakeRuntime::new();
        let release = Rc::new(FakeRelease { listeners: Rc::new(RefCell::new(Vec::new())) });
        let served = served_with(&rt, ServeOptions { release: Some(release.clone()), ..base(&rt) });
        let (socket, sink) = host_socket(&served).await;
        let (_quiet, quiet_sink) = host_socket(&served).await;
        request(&socket, &sink, 1, "events.subscribe", json!({})).await;
        let changed: ReleaseChangedEvent = de(json!({ "release": { "state": "checking" } }));
        let heard: Vec<Rc<dyn Fn(&ReleaseChangedEvent)>> = release.listeners.borrow().clone();
        assert_eq!(heard.len(), 1);
        for f in heard {
            f(&changed);
        }
        assert_eq!(sink.pushes(), vec![json!({ "type": "release.changed", "release": { "state": "checking" } })]);
        assert!(quiet_sink.pushes().is_empty());
        socket.closed();
        assert!(release.listeners.borrow().is_empty());
    })
}

// S1. serve.test.ts:1335
#[test]
fn creates_follow_the_replay_once_and_a_plain_subscribe_hears_none() {
    support::block(async {
        let rt = FakeRuntime::new();
        let refused_create: WorkspaceCreatingEvent =
            de(json!({ "workspaceId": "ws_r", "name": "refused", "stage": "failed", "message": "Snapshot not found", "elapsedMs": 3 }));
        let held_create: WorkspaceCreatingEvent =
            de(json!({ "workspaceId": "ws_h", "name": "held", "stage": "fork-requested", "message": "forking", "elapsedMs": 0 }));
        let creating = vec![refused_create, held_create];
        rt.script.on("workspaces.creating", move |_| creating.clone());
        rt.script.once::<Since>("events.since", since(7, vec![ev("ws_1", 7)], false));
        let served = served_with(&rt, base(&rt));
        let (window, window_sink) = host_socket(&served).await;
        let reply = request(&window, &window_sink, 2, "events.subscribe", json!({ "creates": true })).await;
        assert_eq!(reply["ok"], json!(true));
        let heard = frames_of(&window_sink, 2);
        assert_eq!(heard.len(), 4, "the reply, the replay, then the creates: {heard:?}");
        assert_eq!(heard[1], ev_json("ws_1", 7));
        assert_eq!(heard[2]["type"], json!("workspace.creating"));
        assert_eq!(
            (heard[2]["name"].clone(), heard[2]["stage"].clone(), heard[2]["message"].clone()),
            (json!("refused"), json!("failed"), json!("Snapshot not found"))
        );
        assert_eq!((heard[3]["name"].clone(), heard[3]["stage"].clone()), (json!("held"), json!("fork-requested")));
        for e in &heard[2..] {
            assert!(e.get("seq").is_none(), "a snapshot frame is no position in the stream");
        }

        let (plain, plain_sink) = host_socket(&served).await;
        request(&plain, &plain_sink, 3, "events.subscribe", json!({})).await;
        assert!(plain_sink.pushes().is_empty(), "a plain subscribe hears neither");
        assert_eq!(rt.script.calls_to("workspaces.creating").len(), 1, "only creates: true reads the creates in flight");
    })
}

// S1. serve.test.ts:911
#[test]
fn forward_events_reach_subscribers_and_die_with_the_socket() {
    support::block(async {
        let rt = FakeRuntime::new();
        let source = FakeForwards::new();
        let served = served_with(&rt, ServeOptions { forwards: Some(source.clone() as Rc<dyn ForwardsSource>), ..base(&rt) });
        let (sub, sub_sink) = host_socket(&served).await;
        let (_quiet, quiet_sink) = host_socket(&served).await;
        request(&sub, &sub_sink, 1, "events.subscribe", json!({})).await;
        let f = json!({ "workspaceId": "ws_1", "port": 8123, "startedAt": "2026-09-04T10:00:00.000Z", "name": "api", "kind": "url" });
        let open = json!({ "type": "forward.open", "forward": f });
        let close = json!({ "type": "forward.close", "workspaceId": "ws_1", "port": 8123 });
        source.emit(&ForwardEvent::Open(ForwardOpenEvent { forward: serde_json::from_value(f).expect("a forward") }));
        source.emit(&ForwardEvent::Close(ForwardCloseEvent {
            workspace_id: "ws_1".into(),
            port: serde_json::from_value(json!(8123)).expect("a port"),
        }));
        assert_eq!(sub_sink.pushes(), vec![open, close]);
        assert!(quiet_sink.pushes().is_empty());
        assert_eq!(source.listeners.borrow().len(), 1);
        sub.closed();
        assert_eq!(source.listeners.borrow().len(), 0, "the subscription dies with the socket");
    })
}

// ---- S2, S3: status ------------------------------------------------------------------------------------------

// S2. status.test.ts
#[test]
fn status_subscribe_watches_before_the_snapshot() {
    support::block(async {
        let rt = FakeRuntime::new();
        let detached = Rc::new(Cell::new(0));
        let counter = detached.clone();
        rt.script.once::<Rc<dyn Fn()>>("status.watch", Rc::new(move || counter.set(counter.get() + 1)));
        let status: WorkspaceStatus = de(status_json("ws_1", Some("https://m1-3000.preview.example/?pt_token=e")));
        rt.script.once::<Result<Vec<WorkspaceStatus>, HostError>>("status.list", Ok(vec![status]));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "status.subscribe", json!({})).await;
        assert_eq!(reply["ok"], json!(true));
        assert_eq!(
            reply["statuses"],
            json!([status_json("ws_1", Some("https://m1-3000.preview.example/?pt_token=e"))]),
            "the app reads the stream off the status its own socket subscribes to"
        );
        let order: Vec<String> = rt.script.names().into_iter().filter(|n| n.starts_with("status.")).collect();
        assert_eq!(order, vec!["status.watch", "status.list"], "watch before the snapshot");
        assert_eq!(rt.script.calls_to("status.list")[0].args, json!({ "opts": null, "origin": null }));
        assert_eq!(detached.get(), 0);
        socket.closed();
        assert_eq!(detached.get(), 1, "the watcher is released with the socket");
    })
}

// S3. status.test.ts
#[test]
fn status_list_reads_the_table_and_drops_the_route() {
    support::block(async {
        let rt = FakeRuntime::new();
        let status: WorkspaceStatus = de(status_json("ws_1", Some("https://m1-3000.preview.example/?pt_token=e")));
        rt.script.once::<Result<Vec<WorkspaceStatus>, HostError>>("status.list", Ok(vec![status]));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "status.list", json!({})).await;
        assert_eq!(reply["ok"], json!(true));
        let rows = reply["statuses"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["reach"], json!({ "state": "reachable" }), "the minted route is dropped");
        assert!(!reply.to_string().contains("pt_token"), "{reply}");
        assert!(rows[0].get("screen").is_none());
        let want = StatusListOptions {
            probe_timeout_ms: None,
            reader: Some(StatusReader::Table),
            prompt_ms: None,
            reconcile: Some(Reconcile::OnFailure),
            zombie_probe: Some(false),
        };
        assert_eq!(rt.script.calls_to("status.list")[0].args, json!({ "opts": dbg(&want), "origin": null }));
    })
}

// ---- S4, S5: the workspace verbs -----------------------------------------------------------------------------

// S4, S5. runtime.test.ts:288
#[test]
fn create_strips_origin_and_splits_notice_and_sub() {
    support::block(async {
        let rt = FakeRuntime::new();
        let created: CreatedWorkspace = de({
            let mut v = ws_json("ws_1", true);
            v["notice"] = json!("the clone is cold");
            v
        });
        rt.script.once::<Result<CreatedWorkspace, HostError>>("workspaces.create", Ok(created));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(
            &socket,
            &sink,
            1,
            "workspaces.create",
            json!({ "project": "pr_1", "name": "x", "golden": "snap_g", "origin": "relayed" }),
        )
        .await;
        let keys: Vec<&String> = reply.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["id", "ok", "workspace", "notice"], "{reply}");
        assert_eq!(reply["workspace"], ws_json("ws_1", false), "the display stream is not in the reply, nor the notice in the workspace");
        assert_eq!(reply["notice"], json!("the clone is cold"));
        let asked = &rt.script.calls_to("workspaces.create")[0].args;
        assert_eq!(asked["origin"], json!("relayed"), "the frame's word is the origin of an unstamped socket");
        let opts = asked["opts"].as_str().unwrap();
        assert!(opts.contains("name: \"x\"") && opts.contains("snap_g"), "{opts}");
        assert!(!opts.contains("relayed"), "the client's origin never reaches the create as an option: {opts}");
        assert!(!opts.contains("Relayed"), "{opts}");
    })
}

// S4. runtime.test.ts:288
#[test]
fn every_workspace_answer_goes_out_without_the_display_stream() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let size = SizeAsk { cpu: de(json!(4)), mem_mb: de(json!(8192)) };
        // (op, frame, the call it makes, the args it passes)
        let cases: Vec<(&str, Value, &str, Value)> = vec![
            ("workspaces.resolve", json!({ "ref": "x" }), "workspaces.resolve", json!({ "reference": "x", "origin": null })),
            (
                "workspaces.get",
                json!({ "workspaceId": "ws_1", "threadId": "t_1" }),
                "workspaces.get",
                json!({ "id": "ws_1", "origin": null, "threadId": "t_1" }),
            ),
            ("workspaces.nap", json!({ "workspaceId": "ws_1" }), "workspaces.nap", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.wake", json!({ "workspaceId": "ws_1" }), "workspaces.wake", json!({ "id": "ws_1", "origin": null })),
            (
                "workspaces.resize",
                json!({ "workspaceId": "ws_1", "size": { "cpu": 4, "memMb": 8192 } }),
                "workspaces.resize",
                json!({ "id": "ws_1", "size": dbg(&size), "origin": null }),
            ),
            ("workspaces.stopWake", json!({ "workspaceId": "ws_1" }), "workspaces.stopWake", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.upgrade", json!({ "workspaceId": "ws_1" }), "workspaces.upgrade", json!({ "id": "ws_1", "origin": null })),
            (
                "workspaces.rename",
                json!({ "workspaceId": "ws_1", "name": "y" }),
                "workspaces.rename",
                json!({ "id": "ws_1", "name": "y", "origin": null }),
            ),
            (
                "workspaces.look",
                json!({ "workspaceId": "ws_1", "theme": null }),
                "workspaces.look",
                json!({ "id": "ws_1", "look": { "theme": null }, "origin": null }),
            ),
            ("folder.make", json!({ "project": "pr_1" }), "workspaces.folder", json!({ "project": "pr_1", "origin": null })),
            ("workspaces.rebuild", json!({ "workspaceId": "ws_1" }), "workspaces.rebuild", json!({ "id": "ws_1", "origin": null })),
        ];
        for (i, (op, fields, call, args)) in cases.into_iter().enumerate() {
            let id = 10 + i as u64;
            rt.script.once::<Result<WorkspaceView, HostError>>(call, Ok(ws("ws_1")));
            let reply = request(&socket, &sink, id, op, fields).await;
            assert_eq!(reply.to_string(), json!({ "id": id, "ok": true, "workspace": ws_json("ws_1", false) }).to_string(), "{op}");
            assert_eq!(rt.script.calls_to(call)[0].args, args, "{op}");
        }
        rt.script.once::<Result<Vec<WorkspaceView>, HostError>>("workspaces.list", Ok(vec![ws("ws_1"), ws("ws_2")]));
        let listed = request(&socket, &sink, 40, "workspaces.list", json!({})).await;
        assert_eq!(
            listed.to_string(),
            json!({ "id": 40, "ok": true, "workspaces": [ws_json("ws_1", false), ws_json("ws_2", false)] }).to_string()
        );
        assert!(!sink.texts.borrow().iter().any(|t| t.contains("streamUrl") || t.contains("minted")), "no reply carries the minted stream");
    })
}

// ---- S6-S9 ---------------------------------------------------------------------------------------------------

// S6. serve.ts:1214-1394 (the table: every field only when sent)
#[test]
fn spread_verbs_pass_only_the_fields_sent() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let ask = |call: &str, ask: String| (call.to_string(), json!({ "ask": ask, "origin": null }));
        // (op, frame, the runtime call it makes, the args it passes). The areas answer nothing here: the args are what
        // is read, and the refusal `unscripted call` is the area's.
        let mut cases: Vec<(&str, Value, String, Value)> = Vec::new();
        let mut add = |op: &'static str, fields: Value, expect: (String, Value)| cases.push((op, fields, expect.0, expect.1));
        add(
            "workspaces.landing",
            json!({ "project": "p", "sized": true }),
            ("workspaces.landing".into(), json!({ "project": "p", "sized": true, "origin": null })),
        );
        add(
            "workspaces.landing",
            json!({ "project": "p" }),
            ("workspaces.landing".into(), json!({ "project": "p", "sized": false, "origin": null })),
        );
        add(
            "workspaces.bringBack",
            json!({ "workspaceId": "ws_1", "title": "t" }),
            ("workspaces.bringBack".into(), json!({ "workspaceId": "ws_1", "title": "t", "body": null, "origin": null })),
        );
        add(
            "worktree.make",
            json!({ "project": "p", "branch": "feat/x" }),
            ("workspaces.worktree".into(), json!({ "project": "p", "branch": "feat/x", "origin": null })),
        );
        add(
            "workspaces.checkout",
            json!({ "workspaceId": "ws_1", "fresh": true }),
            ("workspaces.checkout".into(), json!({ "id": "ws_1", "origin": null, "fresh": true })),
        );
        add(
            "workspaces.discard",
            json!({ "workspaceId": "ws_1", "path": "a.txt", "check": true, "threadId": "t_1" }),
            ask(
                "workspaces.discard",
                dbg_s(&DiscardAsk { workspace_id: "ws_1".into(), path: "a.txt".into(), check: Some(true), thread_id: Some("t_1".into()) }),
            ),
        );
        add(
            "workspaces.discard",
            json!({ "workspaceId": "ws_1", "path": "a.txt", "check": false }),
            ask(
                "workspaces.discard",
                dbg_s(&DiscardAsk { workspace_id: "ws_1".into(), path: "a.txt".into(), check: None, thread_id: None }),
            ),
        );
        add(
            "workspaces.commit",
            json!({ "workspaceId": "ws_1", "message": "m", "paths": ["a", "b"], "threadId": "t_1" }),
            ask(
                "workspaces.commit",
                dbg_s(&CommitAsk {
                    workspace_id: "ws_1".into(),
                    message: "m".into(),
                    paths: Some(vec!["a".into(), "b".into()]),
                    thread_id: Some("t_1".into()),
                }),
            ),
        );
        add(
            "workspaces.commit",
            json!({ "workspaceId": "ws_1", "message": "m" }),
            ask("workspaces.commit", dbg_s(&CommitAsk { workspace_id: "ws_1".into(), message: "m".into(), paths: None, thread_id: None })),
        );
        add(
            "workspaces.commitDraft",
            json!({ "workspaceId": "ws_1", "paths": ["a"] }),
            ask(
                "workspaces.commitDraft",
                dbg_s(&CommitDraftAsk { workspace_id: "ws_1".into(), paths: Some(vec!["a".into()]), thread_id: None }),
            ),
        );
        add(
            "workspaces.viewed",
            json!({ "workspaceId": "ws_1", "path": "a", "blob": "b1" }),
            (
                "workspaces.viewed".into(),
                json!({ "workspaceId": "ws_1", "mark": dbg_s(&ViewedMark { path: "a".into(), blob: Some("b1".into()) }), "origin": null }),
            ),
        );
        add(
            "workspaces.viewed",
            json!({ "workspaceId": "ws_1", "path": "a", "blob": null }),
            (
                "workspaces.viewed".into(),
                json!({ "workspaceId": "ws_1", "mark": dbg_s(&ViewedMark { path: "a".into(), blob: None }), "origin": null }),
            ),
        );
        add(
            "workspaces.viewed",
            json!({ "workspaceId": "ws_1" }),
            ("workspaces.viewed".into(), json!({ "workspaceId": "ws_1", "mark": null, "origin": null })),
        );
        add(
            "workspaces.pullRequestView",
            json!({ "workspaceId": "ws_1", "fresh": true }),
            ("workspaces.pullRequestView".into(), json!({ "workspaceId": "ws_1", "fresh": true, "origin": null })),
        );
        add(
            "workspaces.pullRequestView",
            json!({ "workspaceId": "ws_1", "fresh": false }),
            ("workspaces.pullRequestView".into(), json!({ "workspaceId": "ws_1", "fresh": null, "origin": null })),
        );
        add(
            "workspaces.pullRequestDiff",
            json!({ "workspaceId": "ws_1" }),
            ("workspaces.pullRequestDiff".into(), json!({ "workspaceId": "ws_1", "origin": null })),
        );
        add(
            "workspaces.pullRequestReply",
            json!({ "workspaceId": "ws_1", "body": "ok", "replyTo": 3, "threadId": "t_1" }),
            ask(
                "workspaces.pullRequestReply",
                dbg_s(&PullRequestReplyAsk {
                    workspace_id: "ws_1".into(),
                    reply_to: Some(3),
                    thread_id: Some("t_1".into()),
                    body: "ok".into(),
                }),
            ),
        );
        add(
            "workspaces.pullRequestReply",
            json!({ "workspaceId": "ws_1", "body": "ok" }),
            ask(
                "workspaces.pullRequestReply",
                dbg_s(&PullRequestReplyAsk { workspace_id: "ws_1".into(), reply_to: None, thread_id: None, body: "ok".into() }),
            ),
        );
        add(
            "workspaces.pullRequestResolve",
            json!({ "workspaceId": "ws_1", "threadId": "c1", "resolved": true }),
            ("workspaces.pullRequestResolve".into(), json!({ "workspaceId": "ws_1", "threadId": "c1", "resolved": true, "origin": null })),
        );
        add(
            "workspaces.pullRequestReact",
            json!({ "workspaceId": "ws_1", "subject": "s1", "content": "+1", "on": true }),
            (
                "workspaces.pullRequestReact".into(),
                json!({ "workspaceId": "ws_1", "subject": "s1", "content": "+1", "on": true, "origin": null }),
            ),
        );
        add(
            "workspaces.pullRequestSend",
            json!({ "workspaceId": "ws_1", "items": [{ "kind": "comment", "id": 5 }], "threadId": "t_1" }),
            (
                "workspaces.pullRequestSend".into(),
                json!({ "workspaceId": "ws_1", "items": [{ "kind": "comment", "id": 5 }], "threadId": "t_1", "origin": null }),
            ),
        );
        add(
            "workspaces.fix",
            json!({ "workspaceId": "ws_1", "check": "ci", "child": "fix-1", "threadId": "t_1", "childThreadId": "t_2" }),
            ask(
                "workspaces.fix",
                dbg_s(&FixAsk {
                    workspace_id: "ws_1".into(),
                    check: Some("ci".into()),
                    child: Some("fix-1".into()),
                    thread_id: Some("t_1".into()),
                    child_thread_id: Some("t_2".into()),
                }),
            ),
        );
        add(
            "workspaces.fix",
            json!({ "workspaceId": "ws_1" }),
            ask("workspaces.fix", dbg_s(&FixAsk { workspace_id: "ws_1".into(), ..FixAsk::default() })),
        );
        add(
            "workspaces.merge",
            json!({ "workspaceId": "ws_1", "method": "squash", "whenChecksPass": true, "head": "abc", "threadId": "t_1" }),
            ask(
                "workspaces.merge",
                dbg_s(&MergeAsk {
                    workspace_id: "ws_1".into(),
                    method: Some(wsp_frames::MergeMethod::Squash),
                    when_checks_pass: Some(true),
                    head: Some("abc".into()),
                    thread_id: Some("t_1".into()),
                }),
            ),
        );
        add(
            "workspaces.merge",
            json!({ "workspaceId": "ws_1" }),
            ask("workspaces.merge", dbg_s(&MergeAsk { workspace_id: "ws_1".into(), ..MergeAsk::default() })),
        );
        add(
            "workspaces.update",
            json!({ "workspaceId": "ws_1", "threadId": "t_1" }),
            ("workspaces.update".into(), json!({ "workspaceId": "ws_1", "threadId": "t_1", "origin": null })),
        );
        add(
            "workspaces.update",
            json!({ "workspaceId": "ws_1" }),
            ("workspaces.update".into(), json!({ "workspaceId": "ws_1", "threadId": null, "origin": null })),
        );
        add(
            "workspaces.mergeIn",
            json!({ "workspaceId": "ws_1", "child": "ws_2", "threadId": "t_1", "childThreadId": "t_2" }),
            ask(
                "workspaces.mergeIn",
                dbg_s(&MergeInAsk {
                    workspace_id: "ws_1".into(),
                    child: "ws_2".into(),
                    thread_id: Some("t_1".into()),
                    child_thread_id: Some("t_2".into()),
                }),
            ),
        );
        add(
            "workspaces.reviewDraft",
            json!({ "workspaceId": "ws_1", "summary": "looks fine" }),
            ask(
                "workspaces.reviewDraft",
                dbg_s(&ReviewDraftAsk { workspace_id: "ws_1".into(), summary: Some("looks fine".into()), verdict: None, on: None }),
            ),
        );
        add(
            "workspaces.reviewDraft",
            json!({ "workspaceId": "ws_1", "on": [{ "id": "c1", "on": true }] }),
            ask(
                "workspaces.reviewDraft",
                dbg_s(&ReviewDraftAsk { workspace_id: "ws_1".into(), summary: None, verdict: None, on: Some(vec![("c1".into(), true)]) }),
            ),
        );
        add(
            "workspaces.reviewPost",
            json!({ "workspaceId": "ws_1", "threadId": "t_1" }),
            ("workspaces.reviewPost".into(), json!({ "workspaceId": "ws_1", "threadId": "t_1", "origin": null })),
        );

        let mut seen: Vec<(String, usize)> = Vec::new();
        for (i, (op, fields, call, args)) in cases.into_iter().enumerate() {
            let before = rt.script.calls_to(&call).len();
            let reply = request(&socket, &sink, 100 + i as u64, op, fields.clone()).await;
            let made = rt.script.calls_to(&call);
            assert_eq!(made.len(), before + 1, "{op} {fields}: made no {call}: {reply}");
            assert_eq!(made[before].args, args, "{op} {fields}");
            seen.push((call, before));
        }
        assert!(!seen.is_empty());
    })
}
