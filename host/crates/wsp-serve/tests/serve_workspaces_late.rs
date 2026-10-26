// SPDX-License-Identifier: AGPL-3.0-only
//! serve_workspaces: events, status, the workspace verbs, daemon channels, readings and exec. Spec:
//! spec/serve_workspaces.md. Each case is the TS case ported at this crate's seam: the fake runtime is scripted with
//! what the area would have answered, and the test reads what the socket sent and what the runtime was asked.
#![allow(dead_code, unused_imports)]
mod support;

include!("serve_workspaces/world.rs");

// S6. serve.ts:1214-1394 (the answer is spread after ok, in the area's own order)
#[test]
fn spread_verbs_spread_the_answer_after_ok() {
    support::block(async {
        let rt = FakeRuntime::new();
        let made: wsp_wire::views::workspace_more::WorktreeMade =
            de(json!({ "path": "/home/z/lab-feat", "branch": "feat/x", "made": true }));
        rt.script.once::<Result<wsp_wire::views::workspace_more::WorktreeMade, HostError>>("workspaces.worktree", Ok(made));
        rt.script.once::<Result<wsp_wire::types::changes::CheckoutReply, HostError>>("workspaces.checkout", Ok(de(json!({}))));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "worktree.make", json!({ "project": "p", "branch": "feat/x" })).await;
        assert_eq!(
            reply.to_string(),
            json!({ "id": 1, "ok": true, "path": "/home/z/lab-feat", "branch": "feat/x", "made": true }).to_string()
        );
        let checked = request(&socket, &sink, 2, "workspaces.checkout", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(checked.to_string(), json!({ "id": 2, "ok": true }).to_string());
    })
}

// S7. serve.ts:1344-1376
#[test]
fn start_and_review_hand_the_workspace_out() {
    support::block(async {
        let rt = FakeRuntime::new();
        let started = json!({ "workspace": ws_json("ws_1", true), "threadId": "t_1", "sessionId": "s_1" });
        rt.script.once::<Result<wsp_wire::views::harness::StartResult, HostError>>("workspaces.start", Ok(de(started.clone())));
        rt.script.once::<Result<wsp_wire::views::harness::StartResult, HostError>>("workspaces.review", Ok(de(started)));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(
            &socket,
            &sink,
            1,
            "workspaces.start",
            json!({ "url": "https://github.com/o/r/pull/1", "project": "pr_1", "agent": "claude", "access": "full" }),
        )
        .await;
        assert_eq!(
            reply.to_string(),
            json!({ "id": 1, "ok": true, "workspace": ws_json("ws_1", false), "threadId": "t_1", "sessionId": "s_1" }).to_string()
        );
        let want = StartFromLinkAsk {
            url: "https://github.com/o/r/pull/1".into(),
            project: Some("pr_1".into()),
            agent: Some("claude".into()),
            model: None,
            effort: None,
            access: Some(wsp_wire::types::thread_defaults::AccessChoice::Full),
        };
        assert_eq!(rt.script.calls_to("workspaces.start")[0].args, json!({ "ask": dbg_s(&want), "origin": null }));

        let reviewed = request(&socket, &sink, 2, "workspaces.review", json!({ "workspaceId": "ws_1", "model": "opus" })).await;
        assert_eq!(reviewed["workspace"], ws_json("ws_1", false));
        let want = ReviewAsk { url: None, workspace_id: Some("ws_1".into()), agent: None, model: Some("opus".into()), effort: None };
        assert_eq!(rt.script.calls_to("workspaces.review")[0].args, json!({ "ask": dbg_s(&want), "origin": null }));
    })
}

// S8. serve.ts:1214-1394
#[test]
fn unit_verbs_answer_ok_after_the_await() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let verbs: Vec<(&str, Value, &str, Value)> = vec![
            (
                "workspaces.restartDaemon",
                json!({ "workspaceId": "ws_1" }),
                "workspaces.restartDaemon",
                json!({ "id": "ws_1", "origin": null }),
            ),
            ("workspaces.delete", json!({ "workspaceId": "ws_1" }), "workspaces.delete", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.forget", json!({ "workspaceId": "ws_1" }), "workspaces.forget", json!({ "id": "ws_1", "origin": null })),
            ("workspaces.touch", json!({ "workspaceId": "ws_1" }), "workspaces.touch", json!({ "id": "ws_1", "origin": null })),
        ];
        for (i, (op, fields, call, args)) in verbs.into_iter().enumerate() {
            let id = 10 + 2 * i as u64;
            rt.script.once::<Result<(), HostError>>(call, Err(err("the machine said no")));
            rt.script.once::<Result<(), HostError>>(call, Ok(()));
            let failed = request(&socket, &sink, id, op, fields.clone()).await;
            assert_eq!(failed.to_string(), refused(id, "the machine said no", None), "{op}: no ok before the await");
            let done = request(&socket, &sink, id + 1, op, fields).await;
            assert_eq!(done.to_string(), json!({ "id": id + 1, "ok": true }).to_string(), "{op}");
            assert_eq!(rt.script.calls_to(call)[0].args, args, "{op}");
        }

        rt.script.once::<Result<(), HostError>>("workspaces.worktreeRemove", Ok(()));
        rt.script.once::<Result<(), HostError>>("workspaces.worktreeRemove", Ok(()));
        let removed =
            request(&socket, &sink, 40, "worktree.remove", json!({ "project": "p", "branch": "feat/x", "force": false, "check": false }))
                .await;
        assert_eq!(removed.to_string(), json!({ "id": 40, "ok": true }).to_string());
        request(&socket, &sink, 41, "worktree.remove", json!({ "project": "p", "branch": "feat/x", "check": true })).await;
        let asked = rt.script.calls_to("workspaces.worktreeRemove");
        let first = WorktreeRemoveAsk { project: "p".into(), branch: "feat/x".into(), force: Some(false), check: None };
        let second = WorktreeRemoveAsk { project: "p".into(), branch: "feat/x".into(), force: None, check: Some(true) };
        assert_eq!(asked[0].args, json!({ "ask": dbg_s(&first), "origin": null }), "check is passed only when true; force as sent");
        assert_eq!(asked[1].args, json!({ "ask": dbg_s(&second), "origin": null }));
    })
}

// S9. serve.test.ts:676, 693
#[test]
fn port_reach_and_probe_name_their_field() {
    support::block(async {
        let rt = FakeRuntime::new();
        let reach: PortReachView = de(json!({ "url": "https://m1-3000.preview.example/?pt_token=e", "expiresAt": 1_800_000_000_000u64 }));
        let probe: PortProbeView = de(
            json!({ "status": 403, "body": "Blocked request. This host is not allowed. To allow this host, add it to server.allowedHosts" }),
        );
        rt.script.once::<Result<PortReachView, HostError>>("workspaces.portReach", Ok(reach));
        rt.script.once::<Result<PortReachView, HostError>>("workspaces.portReach", Err(err("no such workspace: ws_nobody")));
        rt.script.once::<Result<PortProbeView, HostError>>("workspaces.portProbe", Ok(probe));
        rt.script.once::<Result<PortProbeView, HostError>>("workspaces.portProbe", Err(err("no such workspace: ws_nobody")));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let got = request(&socket, &sink, 1, "workspaces.portReach", json!({ "workspaceId": "ws_1", "port": 3000 })).await;
        assert_eq!(got.to_string(), json!({ "id": 1, "ok": true, "reach": { "url": "https://m1-3000.preview.example/?pt_token=e", "expiresAt": 1_800_000_000_000u64 } }).to_string());
        assert_eq!(rt.script.calls_to("workspaces.portReach")[0].args, json!({ "id": "ws_1", "port": 3000, "origin": null }));
        let missing = request(&socket, &sink, 2, "workspaces.portReach", json!({ "workspaceId": "ws_nobody", "port": 3000 })).await;
        assert_eq!(missing.to_string(), refused(2, "no such workspace: ws_nobody", None));

        let probed = request(&socket, &sink, 3, "workspaces.portProbe", json!({ "workspaceId": "ws_1", "port": 5173 })).await;
        assert_eq!(probed["probe"]["status"], json!(403));
        assert!(probed["probe"]["body"].as_str().unwrap().contains("server.allowedHosts"));
        assert_eq!(rt.script.calls_to("workspaces.portProbe")[0].args, json!({ "id": "ws_1", "port": 5173, "origin": null }));
        let missing = request(&socket, &sink, 4, "workspaces.portProbe", json!({ "workspaceId": "ws_nobody", "port": 5173 })).await;
        assert_eq!(missing["ok"], json!(false));
    })
}

// ---- S10-S16: daemon channels --------------------------------------------------------------------------------

// S10. serve-daemon.test.ts:198-348
#[test]
fn daemon_open_names_one_of_place_and_workspace() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let both = request(&socket, &sink, 1, "daemon.open", json!({ "placeId": "p_1", "workspaceId": "ws_1" })).await;
        assert_eq!(both.to_string(), refused(1, DAEMON_OPEN_ONE_OF, None));
        let neither = request(&socket, &sink, 2, "daemon.open", json!({})).await;
        assert_eq!(neither.to_string(), refused(2, DAEMON_OPEN_ONE_OF, None));
        assert!(rt.daemon_listeners.borrow().is_empty());
        assert!(rt.script.names().iter().all(|n| !n.contains("hereChannel") && !n.contains("daemonChannel") && n != "places.channel"));
    })
}

// S10. serve-daemon.test.ts:~300 (a socket let in on a ticket is refused)
#[test]
fn daemon_open_here_is_the_own_road() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let ch = FakeChannel::new(&rt.script, "here-channel");
        rt.script.once::<Result<Rc<dyn DaemonChannel>, HostError>>("runtime.hereChannel", Ok(ch.clone() as Rc<dyn DaemonChannel>));
        let served = served_with(&rt, base(&rt));
        let (host, host_sink) = host_socket(&served).await;
        let opened = request(&host, &host_sink, 1, "daemon.open", json!({ "placeId": "here" })).await;
        assert_eq!(opened["ok"], json!(true));
        assert!(is_hex12(opened["channel"].as_str().unwrap()), "6 random bytes in lower hex: {opened}");
        assert_eq!(rt.script.calls_to("runtime.hereChannel").len(), 1);
        assert_eq!(rt.daemon_listeners.borrow().len(), 1, "the serve hands the runtime its on_event");

        let (ticketed_socket, ticketed_sink) = ticketed(&served, "connect").await;
        let refused_reply = request(&ticketed_socket, &ticketed_sink, 2, "daemon.open", json!({ "placeId": "here" })).await;
        assert_eq!(refused_reply.to_string(), refused(2, PLACES_TICKET_REFUSAL, Some("ticket")));
        assert_eq!(rt.script.calls_to("runtime.hereChannel").len(), 1, "a ticketed socket opens no channel");
    })
}

// S10. serve.ts:1407-1467 (another place: the place door's channel, wrapped by place_kills)
#[test]
fn daemon_open_a_joined_place_is_wrapped_by_place_kills() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        let ch = FakeChannel::new(&rt.script, "p1-channel");
        rt.script.once::<Option<Rc<dyn DaemonChannel>>>("places.channel", Some(ch.clone() as Rc<dyn DaemonChannel>));
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let opened = request(&socket, &sink, 1, "daemon.open", json!({ "placeId": "p_1" })).await;
        assert_eq!(opened["ok"], json!(true));
        assert_eq!(rt.script.calls_to("places.channel")[0].args, json!({ "placeId": "p_1", "onEvent": "_" }));
        assert_eq!(rt.script.calls_to("runtime.placeKills").len(), 1, "the channel is wrapped by place_kills");
        let place = rt.places.borrow().clone().unwrap();
        assert_eq!(place.channel_listeners.borrow().len(), 1);
        let on_event = place.channel_listeners.borrow()[0].clone();
        let channel = opened["channel"].as_str().unwrap().to_string();
        on_event(map(json!({ "type": "daemon.hello" })));
        assert_eq!(sink.pushes(), vec![json!({ "type": "daemon.event", "channel": channel, "event": { "type": "daemon.hello" } })]);
    })
}

// S10. serve-daemon.test.ts:312
#[test]
fn daemon_open_an_absent_computer_says_so() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<String>("places.nameOf", "spoo".to_string());
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let reply = request(&socket, &sink, 1, "daemon.open", json!({ "placeId": "p_9" })).await;
        assert_eq!(reply.to_string(), refused(1, &absent_computer("spoo", None).sentence, None));
        assert_eq!(rt.script.calls_to("places.channel").len(), 1);
        assert!(rt.script.calls_to("runtime.placeKills").is_empty());
    })
}

// S10. serve-daemon.test.ts:198-290
#[test]
fn daemon_open_carries_the_workspace_refusal() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let cases = vec![
            (err("m1 is not answering yet"), None),
            (HostError::with_kind("the machine refused this host's daemon token (4401 unauthorized)", Kind::Reauth), Some("reauth")),
            (HostError::with_kind("the connection was refused with 403: cross-origin websocket denied", Kind::Refused), Some("refused")),
            (err("no such workspace: ws_nobody"), None),
        ];
        for (i, (e, kind)) in cases.into_iter().enumerate() {
            let id = 1 + i as u64;
            let said = e.message.clone();
            rt.script.once::<Result<Rc<dyn DaemonChannel>, HostError>>("workspaces.daemonChannel", Err(e));
            let reply = request(&socket, &sink, id, "daemon.open", json!({ "workspaceId": "ws_1" })).await;
            assert_eq!(reply.to_string(), refused(id, &said, kind));
        }
        assert_eq!(rt.script.calls_to("workspaces.daemonChannel")[0].args, json!({ "id": "ws_1", "onEvent": "_", "origin": null }));

        // a socket on a relay ticket asks as the machine's road, and the runtime's refusal is the reply
        rt.script.once::<Result<Rc<dyn DaemonChannel>, HostError>>(
            "workspaces.daemonChannel",
            Err(err("a request relayed from a machine cannot drive ws_1")),
        );
        let (relayed, relayed_sink) = ticketed(&served, "relay").await;
        let reply = request(&relayed, &relayed_sink, 9, "daemon.open", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(reply.to_string(), refused(9, "a request relayed from a machine cannot drive ws_1", None));
        assert_eq!(rt.script.calls_to("workspaces.daemonChannel").last().unwrap().args["origin"], json!("relayed"));
    })
}

// S11. serve-daemon.test.ts:86
#[test]
fn daemon_open_replies_before_any_event_and_queues_the_hello() {
    support::block(async {
        let rt = FakeRuntime::new();
        let ch = FakeChannel::new(&rt.script, "c1");
        let in_flight = rt.clone();
        let answered = ch.clone();
        rt.script.on("workspaces.daemonChannel", move |_| -> Result<Rc<dyn DaemonChannel>, HostError> {
            // the daemon pushes its hello while the dial is still in flight
            let listener = in_flight.daemon_listeners.borrow()[0].clone();
            listener(map(json!({ "type": "daemon.hello", "version": 54 })));
            listener(map(json!({ "type": "ports.changed", "ports": [] })));
            Ok(answered.clone() as Rc<dyn DaemonChannel>)
        });
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let opened = request(&socket, &sink, 1, "daemon.open", json!({ "workspaceId": "ws_1" })).await;
        let channel = opened["channel"].as_str().unwrap().to_string();
        assert!(is_hex12(&channel));
        let got = frames_of(&sink, 1);
        assert_eq!(got[0].to_string(), json!({ "id": 1, "ok": true, "channel": channel }).to_string());
        assert_eq!(
            got[1..].iter().map(|f| f.to_string()).collect::<Vec<_>>(),
            vec![
                json!({ "type": "daemon.event", "channel": channel, "event": { "type": "daemon.hello", "version": 54 } }).to_string(),
                json!({ "type": "daemon.event", "channel": channel, "event": { "type": "ports.changed", "ports": [] } }).to_string(),
            ],
            "a page hears a channel's id before anything arrives on it, and the queued events keep their order"
        );
        // a later event goes out at once
        let listener = rt.daemon_listeners.borrow()[0].clone();
        listener(map(json!({ "type": "pty.data", "data": "x" })));
        assert_eq!(
            sink.pushes().last().unwrap(),
            &json!({ "type": "daemon.event", "channel": channel, "event": { "type": "pty.data", "data": "x" } })
        );
    })
}

// S12. serve.ts:1436
#[test]
fn a_socket_gone_mid_dial_closes_the_channel() {
    support::block(async {
        let rt = FakeRuntime::new();
        let ch = FakeChannel::new(&rt.script, "c1");
        let (socket, sink) = {
            let served = served_with(&rt, base(&rt));
            let (socket, sink) = host_socket(&served).await;
            (socket, sink)
        };
        let page = sink.clone();
        let answered = ch.clone();
        rt.script.on("workspaces.daemonChannel", move |_| -> Result<Rc<dyn DaemonChannel>, HostError> {
            page.open.set(false);
            Ok(answered.clone() as Rc<dyn DaemonChannel>)
        });
        send(&socket, frame(1, "daemon.open", json!({ "workspaceId": "ws_1" }))).await;
        assert!(sink.reply(1).is_none(), "nothing is sent to a socket that left");
        assert_eq!(rt.script.calls_to("channel.close").len(), 1, "the machine keeps no socket for a tab that is gone");
        assert!(sink.pushes().is_empty());
    })
}

// S13. serve-daemon.test.ts:149
#[test]
fn a_channel_is_never_reachable_from_another_socket() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (owner, owner_sink) = host_socket(&served).await;
        let (other, other_sink) = host_socket(&served).await;
        let ch = FakeChannel::new(&rt.script, "c1");
        let channel = open_channel(&rt, &owner, &owner_sink, 1, &ch).await;

        let stolen = request(&other, &other_sink, 2, "daemon.send", json!({ "channel": channel, "frame": { "op": "ping" } })).await;
        assert_eq!(stolen.to_string(), refused(2, NO_SUCH_CHANNEL, None));
        let closed = request(&other, &other_sink, 3, "daemon.close", json!({ "channel": channel })).await;
        assert_eq!(closed.to_string(), refused(3, NO_SUCH_CHANNEL, None));
        assert!(rt.script.calls_to("channel.send").is_empty());
        assert!(rt.script.calls_to("channel.close").is_empty(), "another socket cannot close it");

        let mine = request(&owner, &owner_sink, 4, "daemon.send", json!({ "channel": channel, "frame": { "op": "ping" } })).await;
        assert_eq!(mine["ok"], json!(true));
        assert_eq!(rt.script.calls_to("channel.send")[0].args, json!({ "channel": "c1", "frame": { "op": "ping" } }));
    })
}

// S14. serve-daemon.test.ts:120
#[test]
fn daemon_send_carries_the_daemons_refusal_under_ok() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let ch = FakeChannel::new(&rt.script, "c1");
        let channel = open_channel(&rt, &socket, &sink, 1, &ch).await;
        let refusal = map(json!({ "ok": false, "code": "bad-request", "error": "path must be a string" }));
        rt.script.once::<Result<Frame, ChannelError>>("channel.send", Ok(refusal));
        let reply = request(&socket, &sink, 2, "daemon.send", json!({ "channel": channel, "frame": { "op": "ping" } })).await;
        assert_eq!(
            reply.to_string(),
            json!({ "id": 2, "ok": true, "reply": { "ok": false, "code": "bad-request", "error": "path must be a string" } }).to_string(),
            "the daemon's refusal rides inside an ok reply, as it wrote it"
        );
        rt.script.once::<Result<Frame, ChannelError>>(
            "channel.send",
            Err(ChannelError::Unanswered { message: "daemon unreachable".into(), cause: None }),
        );
        let broken = request(&socket, &sink, 3, "daemon.send", json!({ "channel": channel, "frame": { "op": "ping" } })).await;
        assert_eq!(broken.to_string(), refused(3, "daemon unreachable", None), "a channel whose daemon socket ended fails the send");
    })
}

// S15. serve-daemon.test.ts:177
#[test]
fn a_daemon_end_is_said_once_and_a_page_close_says_nothing() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let cut = FakeChannel::new(&rt.script, "cut");
        let asked = FakeChannel::new(&rt.script, "asked");
        let cut_id = open_channel(&rt, &socket, &sink, 1, &cut).await;
        let asked_id = open_channel(&rt, &socket, &sink, 2, &asked).await;
        let closed = request(&socket, &sink, 3, "daemon.close", json!({ "channel": asked_id })).await;
        assert_eq!(closed.to_string(), json!({ "id": 3, "ok": true }).to_string());
        assert_eq!(rt.script.calls_to("channel.close")[0].args, json!({ "channel": "asked" }));

        cut.end(1006, "gone");
        cut.end(1000, "ignored: it ended once already");
        settle().await;
        let said: Vec<Value> = sink.pushes().into_iter().filter(|f| f["type"] == json!("daemon.closed")).collect();
        assert_eq!(
            said,
            vec![json!({ "type": "daemon.closed", "channel": cut_id, "code": 1006, "reason": "gone" })],
            "once, with the daemon's own code and reason"
        );
        assert!(said.iter().all(|f| f["channel"] != json!(asked_id)), "a close the page asked for says nothing then or later");

        let after = request(&socket, &sink, 4, "daemon.send", json!({ "channel": cut_id, "frame": { "op": "ping" } })).await;
        assert_eq!(after.to_string(), refused(4, NO_SUCH_CHANNEL, None), "the channel was forgotten");
    })
}

// S16. serve-daemon.test.ts:165
#[test]
fn a_socket_that_goes_takes_its_channels() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let first = FakeChannel::new(&rt.script, "first");
        let second = FakeChannel::new(&rt.script, "second");
        open_channel(&rt, &socket, &sink, 1, &first).await;
        open_channel(&rt, &socket, &sink, 2, &second).await;
        assert!(rt.script.calls_to("channel.close").is_empty());
        socket.closed();
        let mut closed: Vec<String> =
            rt.script.calls_to("channel.close").iter().map(|c| c.args["channel"].as_str().unwrap().to_string()).collect();
        closed.sort();
        assert_eq!(closed, vec!["first", "second"], "the socket's close closes every channel it holds");
        settle().await;
        assert!(sink.pushes().is_empty(), "a channel the socket's close ended says nothing");
    })
}

// ---- S17: readings -------------------------------------------------------------------------------------------

// S17. serve-sys.test.ts:177
#[test]
fn sys_subscribe_on_a_place_is_the_own_road() {
    support::block(async {
        let rt = FakeRuntime::with_places();
        rt.script.once::<Result<Detach, HostError>>("workspaces.watchSys", Ok(Box::new(|| {})));
        rt.script.once::<Result<Detach, HostError>>("runtime.watchPlaceSys", Ok(Box::new(|| {})));
        let served = served_with(&rt, base(&rt));
        let (ticket, ticket_sink) = ticketed(&served, "relay").await;
        let gated = request(&ticket, &ticket_sink, 5, "sys.subscribe", json!({ "placeId": "p_1" })).await;
        assert_eq!(gated.to_string(), refused(5, PLACES_TICKET_REFUSAL, Some("ticket")));
        assert!(rt.script.calls_to("runtime.watchPlaceSys").is_empty(), "the road is never reached");

        // a workspace's readings, and any unsubscribe, are not the places gate's
        let of_workspace = request(&ticket, &ticket_sink, 6, "sys.subscribe", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(of_workspace.to_string(), json!({ "id": 6, "ok": true }).to_string());
        let unsub = request(&ticket, &ticket_sink, 7, "sys.unsubscribe", json!({ "placeId": "p_1" })).await;
        assert_eq!(unsub["ok"], json!(true));
        let watched = rt.script.calls_to("workspaces.watchSys");
        assert_eq!(watched.len(), 1, "a workspace's readings are not the places gate's");
        assert_eq!(watched[0].args["id"], json!("ws_1"));
        assert!(rt.script.calls_to("runtime.watchPlaceSys").is_empty(), "an unsubscribe reads nothing of the place");

        let (host, host_sink) = host_socket(&served).await;
        let own = request(&host, &host_sink, 8, "sys.subscribe", json!({ "placeId": "p_1" })).await;
        assert_eq!(own.to_string(), json!({ "id": 8, "ok": true }).to_string());
        assert_eq!(rt.script.calls_to("runtime.watchPlaceSys").last().unwrap().args["placeId"], json!("p_1"));
    })
}

// ---- S18: exec -----------------------------------------------------------------------------------------------

struct FakeExec {
    lines: Mutex<Option<mpsc::UnboundedReceiver<Result<String, ExecFailure>>>>,
    exited: SharedFuture<Option<i32>>,
    torn: AtomicUsize,
}

struct ExecHandle {
    lines: mpsc::UnboundedSender<Result<String, ExecFailure>>,
    exit: Option<oneshot::Sender<Option<i32>>>,
}

impl ExecHandle {
    fn say(&self, line: &str) {
        self.lines.unbounded_send(Ok(line.to_string())).unwrap();
    }
    fn finish(mut self, code: Option<i32>) {
        let exit = self.exit.take().unwrap();
        drop(self.lines);
        let _ = exit.send(code);
    }
}

fn fake_exec() -> (Arc<FakeExec>, ExecHandle) {
    let (lines_tx, lines_rx) = mpsc::unbounded();
    let (exit_tx, exit_rx) = oneshot::channel::<Option<i32>>();
    let exited = async move { exit_rx.await.unwrap_or(None) }.boxed_local().shared();
    (
        Arc::new(FakeExec { lines: Mutex::new(Some(lines_rx)), exited, torn: AtomicUsize::new(0) }),
        ExecHandle { lines: lines_tx, exit: Some(exit_tx) },
    )
}

impl ExecStream for FakeExec {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        match self.lines.lock().unwrap().take() {
            Some(rx) => rx.boxed_local(),
            None => futures::stream::empty().boxed_local(),
        }
    }
    fn teardown(&self) {
        self.torn.fetch_add(1, Ordering::SeqCst);
    }
    fn kill(&self) {}
    fn write(&self, _line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        async { Ok(WriteOutcome::Written) }.boxed_local()
    }
    fn close_input(&self) {}
    fn exited(&self) -> SharedFuture<Option<i32>> {
        self.exited.clone()
    }
}

fn run_of(exec: &Arc<FakeExec>, ran_in: Option<&str>) -> RunningExec {
    RunningExec { stream: exec.clone() as Arc<dyn ExecStream>, ran_in: ran_in.map(str::to_string) }
}

fn exec_frames(sink: &RecordingSink) -> Vec<Value> {
    sink.pushes().into_iter().filter(|f| f["type"].as_str().is_some_and(|t| t.starts_with("exec."))).collect()
}

// S18. serve.test.ts:992
#[test]
fn exec_replies_then_streams_lines_and_exit_to_the_asker_only() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (asker, asker_sink) = host_socket(&served).await;
        let (other, other_sink) = host_socket(&served).await;
        request(&other, &other_sink, 1, "events.subscribe", json!({})).await;
        let (exec, handle) = fake_exec();
        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Ok(run_of(&exec, None)));
        let argv = json!(["sh", "-c", "printf 'one\\ntwo\\n'; exit 3"]);
        let started = request(&asker, &asker_sink, 2, "workspaces.exec", json!({ "workspaceId": "ws_1", "argv": argv })).await;
        assert_eq!(started["ok"], json!(true));
        let exec_id = started["execId"].as_str().unwrap().to_string();
        assert!(is_hex12(&exec_id));
        assert_eq!(
            started.to_string(),
            json!({ "id": 2, "ok": true, "execId": exec_id }).to_string(),
            "no cwd where the stream names none"
        );
        assert_eq!(rt.script.calls_to("workspaces.execStream")[0].args, json!({ "id": "ws_1", "argv": argv, "cwd": null, "origin": null }));

        handle.say("one");
        handle.say("two");
        handle.finish(Some(3));
        settle().await;
        assert_eq!(
            exec_frames(&asker_sink),
            vec![
                json!({ "type": "exec.output", "execId": exec_id, "text": "one" }),
                json!({ "type": "exec.output", "execId": exec_id, "text": "two" }),
                json!({ "type": "exec.exit", "execId": exec_id, "exitCode": 3 }),
            ]
        );
        assert!(exec_frames(&other_sink).is_empty(), "nothing reaches another socket");
        let all = asker_sink.frames();
        let reply_at = all.iter().position(|f| f.get("id") == Some(&json!(2))).unwrap();
        let first_line_at = all.iter().position(|f| f["type"] == json!("exec.output")).unwrap();
        assert!(reply_at < first_line_at, "the reply goes first");

        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Err(err("no such workspace: ws_nope")));
        let missing = request(&asker, &asker_sink, 3, "workspaces.exec", json!({ "workspaceId": "ws_nope", "argv": ["true"] })).await;
        assert_eq!(missing.to_string(), refused(3, "no such workspace: ws_nope", None));
    })
}

// S18. serve.test.ts:1033
#[test]
fn exec_names_the_folder_it_ran_in() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let (exec, handle) = fake_exec();
        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Ok(run_of(&exec, Some("/root/work/my proj"))));
        let reply = request(
            &socket,
            &sink,
            1,
            "workspaces.exec",
            json!({ "workspaceId": "ws_1", "argv": ["git", "status"], "cwd": "/root/work/my proj" }),
        )
        .await;
        let exec_id = reply["execId"].as_str().unwrap();
        assert_eq!(reply.to_string(), json!({ "id": 1, "ok": true, "execId": exec_id, "cwd": "/root/work/my proj" }).to_string());
        assert_eq!(rt.script.calls_to("workspaces.execStream")[0].args["cwd"], json!("/root/work/my proj"));
        handle.finish(Some(0));
        settle().await;
        assert_eq!(exec_frames(&sink).last().unwrap()["type"], json!("exec.exit"));
    })
}

// S18. serve.test.ts:1072
#[test]
fn the_asking_socket_closing_ends_the_command() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let (exec, _handle) = fake_exec();
        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Ok(run_of(&exec, None)));
        let started = request(&socket, &sink, 1, "workspaces.exec", json!({ "workspaceId": "ws_1", "argv": ["sleep", "600"] })).await;
        assert_eq!(started["ok"], json!(true));
        assert_eq!(exec.torn.load(Ordering::SeqCst), 0);
        socket.closed();
        assert_eq!(exec.torn.load(Ordering::SeqCst), 1, "the socket's close tears the stream down");
    })
}

// S18. serve.test.ts:1088
#[test]
fn an_ended_exec_is_not_torn_down_again() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let (exec, handle) = fake_exec();
        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Ok(run_of(&exec, None)));
        request(&socket, &sink, 1, "workspaces.exec", json!({ "workspaceId": "ws_1", "argv": ["true"] })).await;
        handle.finish(Some(0));
        settle().await;
        assert_eq!(exec_frames(&sink).len(), 1, "the exit was said");
        socket.closed();
        assert_eq!(exec.torn.load(Ordering::SeqCst), 0, "a command that has exited is not signalled again when its socket goes");
    })
}

// S18. serve.ts:1656-1678
#[test]
fn a_failed_read_rides_the_exit() {
    support::block(async {
        let rt = FakeRuntime::new();
        let served = served_with(&rt, base(&rt));
        let (socket, sink) = host_socket(&served).await;
        let (exec, handle) = fake_exec();
        rt.script.once::<Result<RunningExec, HostError>>("workspaces.execStream", Ok(run_of(&exec, None)));
        let started = request(&socket, &sink, 1, "workspaces.exec", json!({ "workspaceId": "ws_1", "argv": ["sleep", "600"] })).await;
        let exec_id = started["execId"].as_str().unwrap().to_string();
        handle.say("partial");
        handle.lines.unbounded_send(Err(ExecFailure { kind: ExecFailureKind::Other, message: "connection lost".into() })).unwrap();
        handle.finish(None);
        settle().await;
        assert_eq!(
            exec_frames(&sink),
            vec![
                json!({ "type": "exec.output", "execId": exec_id, "text": "partial" }),
                json!({ "type": "exec.exit", "execId": exec_id, "exitCode": null, "error": "connection lost" }),
            ],
            "the exit carries the failure of reading the lines, once"
        );
    })
}
