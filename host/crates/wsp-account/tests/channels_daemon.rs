// SPDX-License-Identifier: AGPL-3.0-only
//! channels_daemon (spec/channels_daemon.md): the facts of a record and the roads to a daemon, channels.ts:1-280.
//! Every expected value is the TS's: oracle/channels_daemon.oracle.ts ran `channelsArea` over the same scenarios and
//! wrote tests/fixtures/oracle/channels_daemon.json, inputs beside answers. The sentences it builds (no such project,
//! spend cap, blocked computer, behind daemon, absent computer, no thread) come from @wsp/protocol through that run.
//!
//! The TS cases that reach this file through a whole host, and the test that pins what each one pins here:
//!   channels.test.ts:162 d16_only_the_git_host_ops_carry_a_token       :180 d16_the_token_is_revoked_after_a_refusal
//!   channels.test.ts:189 d16_an_old_or_silent_daemon_gets_no_token     :202 d16_no_thread_is_a_usage_refusal
//!   channels.test.ts:211 d15_brokered_table (the broker not ready)     :226 d15_brokered_table (a joined computer)
//!   channels.test.ts:559 d16_thread_on_picks_the_named_thread_of_this_workspace_else_the_newest
//!   spawn-guard.test.ts:1038 d6_agents_held_record_then_place_then_default
//!   usage.test.ts:422 d13_reply_of_refusal_keeps_the_code            bring-back.test.ts:88 d14_with_daemon_three_roads
//!   bring-back.test.ts:192, :230 d13_reply_of_refusal_keeps_the_code     serve-daemon.test.ts:198 d11_no_token_is_not_answering_yet
//!   runtime-index.test.ts:81, :134 and local-workspace.test.ts:1715 d1_started_as_and_cut_before
//!   runtime-index.test.ts:585, thread-keeps-picks.test.ts:82, :118 d2_resumed_fact_newest_row_then_index
//!   permission-relay.test.ts:504, :551 d2_access_of_record_then_fact
//!   places-settings.test.ts:354, :402 d5_turn_limit_three_states      places-settings.test.ts:441 d19_place_guard_refuses_at_the_limit
//!   box-folder.test.ts:564 d20_copy_blocked_skips_a_folder
//!   slate-box-server.test.ts:242 and slate-box-http.test.ts:186 d17_stdio_road_table
//!   pull-request.test.ts:215, :231, :610 d18_on_this_computer_*
#![allow(dead_code, unused_imports)]
mod common;
include!("channels_daemon/world.rs");

// D1, TS: runtime-index.test.ts:81, :134, local-workspace.test.ts:1715
#[test]
fn d1_started_as_and_cut_before() {
    run(async {
        let (w, f) = facts_world();
        let area = w.area();
        for q in f["startedAs"].as_array().unwrap() {
            assert_eq!(opt(area.started_as(q["ws"].as_str().unwrap(), q["th"].as_str().unwrap())), q["answer"], "startedAs {q}");
        }
        for q in f["cutBefore"].as_array().unwrap() {
            assert_eq!(json!(area.cut_before(q["ws"].as_str().unwrap(), q["th"].as_str().unwrap())), q["answer"], "cutBefore {q}");
        }
    });
}

// D2, TS: runtime-index.test.ts:585, thread-keeps-picks.test.ts:82, :118
#[test]
fn d2_resumed_fact_newest_row_then_index() {
    run(async {
        let (w, f) = facts_world();
        let area = w.area();
        for q in f["resumedFact"].as_array().unwrap() {
            let got = area.resumed_fact(q["ws"].as_str().unwrap(), q["resume"].as_str().unwrap(), q["fact"].as_str().unwrap());
            assert_eq!(opt(got), q["answer"], "resumedFact {q}");
        }
        for q in f["folderOf"].as_array().unwrap() {
            assert_eq!(opt(area.folder_of(q["ws"].as_str().unwrap(), q["resume"].as_str().unwrap())), q["answer"], "folderOf {q}");
        }
    });
}

// D2, TS: permission-relay.test.ts:504, :551
#[test]
fn d2_access_of_record_then_fact() {
    run(async {
        let (w, f) = facts_world();
        let area = w.area();
        for q in f["accessOf"].as_array().unwrap() {
            let got = area.access_of(q["ws"].as_str().unwrap(), q["th"].as_str().unwrap(), q["resume"].as_str());
            assert_eq!(opt(got), q["answer"], "accessOf {q}");
        }
    });
}

// D3
#[test]
fn d3_home_of_is_the_home_the_kind_answers_else_nothing() {
    run(async {
        let f = fx("home");
        let w = World::new();
        let area = w.area();
        let r = record(&rec(json!({})));
        *w.home.borrow_mut() = Some("/home/dev".into());
        assert_eq!(opt(area.home_of(&r)), f["with"]["home"]);
        *w.home.borrow_mut() = None;
        assert_eq!(f["without"], json!({}));
        assert_eq!(area.home_of(&r), None);
    });
}

// D4
#[test]
fn d4_provider_and_place_of_each_kind() {
    run(async {
        let f = fx("places");
        for (door, rows, settings) in [(true, "withDoor", "settingsWithDoor"), (false, "noDoor", "settingsNoDoor")] {
            let w = places_world(door);
            let area = w.area();
            for (case, expected) in f["records"].as_array().unwrap().iter().zip(f[rows].as_array().unwrap()) {
                let r = record(&case["record"]);
                let name = case["name"].as_str().unwrap();
                assert_eq!(opt(area.provider_of(&r)), expected["provider"], "{rows}: provider of {name}");
                assert_eq!(opt(area.place_id_of(&r)), expected["placeId"], "{rows}: place of {name}");
            }
            for q in f[settings].as_array().unwrap() {
                let got = area.settings_at(q["place"].as_str());
                assert_eq!(serde_json::to_value(got).unwrap().to_string(), q["answer"].to_string(), "{settings} {q}");
            }
        }
    });
}

// D5, TS: places-settings.test.ts:354, :402
#[test]
fn d5_turn_limit_three_states() {
    run(async {
        let f = fx("places");
        for (door, rows) in [(true, "withDoor"), (false, "noDoor")] {
            let w = places_world(door);
            let area = w.area();
            for (case, expected) in f["records"].as_array().unwrap().iter().zip(f[rows].as_array().unwrap()) {
                let got = match area.turn_limit_of(&record(&case["record"])) {
                    None => Value::Null,
                    Some(wall) if wall.deadline_ms.is_infinite() => json!({ "deadlineMs": "Infinity" }),
                    Some(wall) => json!({ "deadlineMs": wall.deadline_ms as i64 }),
                };
                assert_eq!(got, expected["turnLimit"], "{rows}: {}", case["name"]);
            }
        }
    });
}

// D6, TS: spawn-guard.test.ts:1038
#[test]
fn d6_agents_held_record_then_place_then_default() {
    run(async {
        let f = fx("places");
        for (door, rows) in [(true, "withDoor"), (false, "noDoor")] {
            let w = places_world(door);
            let area = w.area();
            for (case, expected) in f["records"].as_array().unwrap().iter().zip(f[rows].as_array().unwrap()) {
                let mut json_record = case["record"].clone();
                // The record's own agents as wsp-wire holds them (the cut dropped `spawn`).
                if json_record["agents"].is_object() {
                    json_record["agents"]["spawn"] = json!(true);
                }
                let got = agents_json(&area.agents_held(&record(&json_record)));
                assert_eq!(got.to_string(), expected["agents"].to_string(), "{rows}: {}", case["name"]);
            }
        }
        let w = places_world(true);
        let area = w.area();
        for q in f["spawnAt"].as_array().unwrap() {
            assert_eq!(agents_json(&area.spawn_at(q["place"].as_str())).to_string(), q["answer"].to_string(), "spawnAt {q}");
        }
        assert_eq!(agents_json(&area.spawn_at(None)).to_string(), fx("constants")["AGENTS_ON"].to_string());
    });
}

// D7
#[test]
fn d7_project_held_refuses_in_its_words() {
    run(async {
        let f = fx("projects");
        for (names, expected) in [(vec![], &f["empty"]), (vec!["alpha", "beta"], &f["two"])] {
            let w = World::new();
            for (i, n) in names.iter().enumerate() {
                w.projects.borrow_mut().insert(format!("p{i}"), project(&project_json(&format!("p{i}"), n)));
            }
            let area = w.area();
            let got = match area.project_held("nope") {
                Ok(_) => json!({ "ok": null }),
                Err(e) => json!({ "error": error_json(&e) }),
            };
            assert_eq!(&got, expected);
        }
        let w = World::new();
        let held = project(&project_json("pA", "alpha"));
        w.projects.borrow_mut().insert("pA".into(), held.clone());
        let area = w.area();
        assert_eq!(area.project_held("pA").unwrap(), held);
        // The four facts a row needs, in this order.
        let r = area.ref_of(&held);
        assert_eq!(
            serde_json::to_value(&r).unwrap().to_string(),
            json!({ "id": "pA", "name": "alpha", "path": "/home/u/alpha", "computer": "here" }).to_string()
        );
        assert_eq!(f["refKeys"], json!(["id", "name", "path", "computer"]));
    });
}

// D8
#[test]
fn d8_checkout_of_table() {
    run(async {
        let f = fx("checkout");
        let w = World::new();
        for p in f["projects"].as_array().unwrap() {
            w.projects.borrow_mut().insert(p["id"].as_str().unwrap().into(), project(p));
        }
        let area = w.area();
        for c in f["cases"].as_array().unwrap() {
            let got = match area.checkout_of(&record(&c["record"])) {
                Ok(path) => json!({ "ok": path }),
                Err(e) => json!({ "error": error_json(&e) }),
            };
            assert_eq!(got, c["answer"], "{}", c["name"]);
        }
    });
}

// D9
#[test]
fn d9_computer_of_names_the_place_or_the_kind() {
    run(async {
        let f = fx("computer");
        for (door, key) in [(true, "withDoor"), (false, "noDoor")] {
            let w = World::new();
            w.has_door.set(door);
            for (p, n) in f["names"].as_object().unwrap() {
                w.names.borrow_mut().insert(p.clone(), n.as_str().unwrap().into());
            }
            let area = w.area();
            for c in f["cases"].as_array().unwrap() {
                assert_eq!(json!(area.computer_of(&entry(&c["record"], &FakeMachine::new("m1")))), c[key], "{key}: {}", c["name"]);
            }
        }
    });
}

// D10
#[test]
fn d10_served_by_its_computer_is_the_machines_daemon_frame() {
    run(async {
        let f = fx("served");
        let w = World::new();
        let area = w.area();
        let heard: Rc<RefCell<Vec<Value>>> = Rc::default();
        let m = serving(heard.clone(), |fr| frame(json!({ "ok": true, "id": fr["id"] })));
        let served = area.served_by_its_computer(&entry(&rec(json!({})), &m)).expect("a machine with a daemon frame is served");
        let reply = served(frame(json!({ "op": "git.status", "id": 4 }))).await.unwrap();
        assert_eq!(Value::Object(reply).to_string(), f["reply"].to_string());
        assert_eq!(json!(heard.borrow().clone()).to_string(), f["heard"].to_string());
        assert_eq!(f["with"], json!(true));
        assert!(area.served_by_its_computer(&entry(&rec(json!({})), &FakeMachine::new("m1"))).is_none());
        assert_eq!(f["without"], json!(false));
    });
}

// ----- The roads -------------------------------------------------------------------------------------------------------

// D11, TS: serve-daemon.test.ts:198
#[test]
fn d11_no_token_is_not_answering_yet() {
    run(async {
        let f = fx("channel_over");
        let w = World::new();
        let area = w.area();
        let none: DaemonReachView = serde_json::from_value(json!({ "url": "wss://x", "expiresAt": 1 })).unwrap();
        let got = area.channel_over(&none, "alpha", quiet()).await;
        assert_eq!(outcome(got, |_| json!(null)), f["none"]);
        assert_eq!(json!(w.dialled.borrow().clone()), f["dialledAfterNone"], "nothing is dialled without a token");
        // With a token: one dial of the reach's url, and the events go to the caller's listener.
        let d = Daemon::ok();
        w.dials.borrow_mut().push_back(Ok(d.clone()));
        let events: Rc<RefCell<Vec<Value>>> = Rc::default();
        let with_token: DaemonReachView =
            serde_json::from_value(json!({ "url": "wss://x/ws", "expiresAt": 1, "daemonToken": "tok" })).unwrap();
        let opened = area.channel_over(&with_token, "alpha", on_event(events.clone())).await.expect("the dial answers");
        assert_eq!(Rc::as_ptr(&opened) as *const (), Rc::as_ptr(&d) as *const ());
        let listener = w.listeners.borrow()[0].clone();
        listener(json!({ "type": "daemon.hello", "version": 1 }));
        assert_eq!(json!(w.dialled.borrow().iter().map(|(u, t)| json!({ "url": u, "token": t })).collect::<Vec<_>>()), f["dialled"]);
        assert_eq!(json!(events.borrow().clone()), f["events"]);
    });
}

// D12
#[test]
fn d12_a_failed_dial_adopts_a_pause() {
    run(async {
        let f = fx("own_channel");
        for c in f.as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let w = World::new();
            let area = w.area();
            let m = FakeMachine::new("m1");
            *m.state.borrow_mut() = match name {
                n if n.contains("failed state read") => Err("no answer".into()),
                n if n.contains("running machine") => Ok(MachineState::Running),
                n if n.contains("something else") => Ok(MachineState::Starting),
                _ => Ok(MachineState::Paused),
            };
            *w.road.borrow_mut() = match c["roadError"].as_str() {
                Some(why) => Err(plain(why).into()),
                None => Ok(reach()),
            };
            if c["dial"] == json!("ok") {
                w.dials.borrow_mut().push_back(Ok(Daemon::ok()));
            } else {
                w.dials.borrow_mut().push_back(Err(plain("dial refused").into()));
            }
            let e = entry(&rec(json!({ "phase": c["phase"] })), &m);
            let got = outcome(area.own_daemon_channel(&e, quiet()).await, |_| json!("opened"));
            assert_eq!(got, c["result"], "{name}: the answer");
            assert_eq!(json!(m.state_calls.get()), c["stateAsked"], "{name}: how often the machine was asked its state");
            assert_eq!(json!(w.adopted.borrow().clone()), c["adopted"], "{name}: the pause adopted");
            assert_eq!(json!(w.roads_asked.borrow().clone()), c["roadsAsked"], "{name}: the road asked");
        }
    });
}

// D13, TS: usage.test.ts:422, bring-back.test.ts:192, :230
#[test]
fn d13_reply_of_refusal_keeps_the_code() {
    run(async {
        let f = fx("reply_of");
        let w = World::new();
        let area = w.area();
        for c in f.as_array().unwrap() {
            let asked = frame(c["frame"].clone());
            let reply = frame(c["reply"].clone());
            assert_eq!(frame_outcome(area.reply_of(&asked, reply)), c["answer"], "{}", c["reply"]);
        }
    });
}

// D13
#[test]
fn d13_over_channel_closes_on_error() {
    run(async {
        let f = fx("over_channel");
        let w = World::new();
        let area = w.area();
        let cases = f.as_array().unwrap();
        // The work's answer comes back and the channel closes.
        let d = Daemon::new(|_| frame(json!({ "ok": true, "v": 1 })));
        let got = area
            .over_channel(d.clone(), work(|ask| async move { Ok(json!({ "first": frame_outcome(ask(frame(json!({ "op": "git.status" }))).await)["ok"], "second": frame_outcome(ask(frame(json!({ "op": "ping" }))).await)["ok"] })) }))
            .await;
        assert_eq!(outcome(got, |v| v), cases[0]["result"]);
        assert_eq!(json!(d.heard()), cases[0]["heard"]);
        assert_eq!(json!(d.closes.get()), cases[0]["closes"]);
        // A refused reply throws its refusal, and the channel closes.
        let d = Daemon::new(|_| frame(json!({ "ok": false, "code": "busy", "error": "x" })));
        let got =
            area.over_channel(d.clone(), work(|ask| async move { ask(frame(json!({ "op": "git.push" }))).await.map(Value::Object) })).await;
        assert_eq!(outcome(got, |v| v), cases[1]["result"]);
        assert_eq!(json!(d.heard()), cases[1]["heard"]);
        assert_eq!(json!(d.closes.get()), cases[1]["closes"]);
        // The work's own error closes it too.
        let d = Daemon::ok();
        let got: AccountResult<Value> = area.over_channel(d.clone(), work(|_| async move { Err(plain("the work failed").into()) })).await;
        assert_eq!(outcome(got, |v| v), cases[2]["result"]);
        assert_eq!(json!(d.heard()), cases[2]["heard"]);
        assert_eq!(json!(d.closes.get()), cases[2]["closes"]);
    });
}

// D14, TS: bring-back.test.ts:88
#[test]
fn d14_with_daemon_three_roads() {
    run(async {
        let f = fx("with_daemon");
        // A served machine's frames go up its computer's link, a refusal thrown with its code, and nothing is dialled.
        let w = World::new();
        let area = w.area();
        let heard: Rc<RefCell<Vec<Value>>> = Rc::default();
        let m = serving(heard.clone(), |fr| {
            if fr["op"] == json!("git.diff") {
                frame(json!({ "ok": false, "code": "no-diff", "error": "nothing to diff" }))
            } else {
                frame(json!({ "ok": true, "id": null }))
            }
        });
        let got = area
            .with_daemon(
                &entry(&rec(json!({})), &m),
                work(|ask| async move {
                    let first = frame_outcome(ask(frame(json!({ "op": "git.status" }))).await)["ok"].clone();
                    let second = frame_outcome(ask(frame(json!({ "op": "git.diff", "cwd": "/x" }))).await);
                    Ok(json!([first, second]))
                }),
            )
            .await;
        assert_eq!(outcome(got, |v| v), f["served"]["result"]);
        assert_eq!(json!(heard.borrow().clone()), f["served"]["heard"]);
        assert_eq!(json!(w.dialled.borrow().clone()), f["served"]["dialled"]);
        assert_eq!(json!(w.roads_asked.borrow().clone()), f["served"]["roadsAsked"]);
        // An unbrokered machine: its daemon dialled at its road's url with its token, and closed after.
        let w = World::new();
        let area = w.area();
        *w.road.borrow_mut() = Ok(reach());
        let d = Daemon::new(|fr| frame(json!({ "ok": true, "op": fr["op"] })));
        w.dials.borrow_mut().push_back(Ok(d.clone()));
        let got = area
            .with_daemon(
                &entry(&rec(json!({})), &FakeMachine::new("m1")),
                work(|ask| async move { ask(frame(json!({ "op": "git.status" }))).await.map(Value::Object) }),
            )
            .await;
        assert_eq!(outcome(got, |v| v), f["unbrokered"]["result"]);
        assert_eq!(json!(d.heard()), f["unbrokered"]["heard"]);
        assert_eq!(
            json!(w.dialled.borrow().iter().map(|(u, t)| json!({ "url": u, "token": t })).collect::<Vec<_>>()),
            f["unbrokered"]["dialled"]
        );
        assert_eq!(json!(d.closes.get()), f["unbrokered"]["closes"]);
        assert_eq!(json!(w.listeners.borrow().len()), f["unbrokered"]["listeners"]);
    });
}

// D15, TS: channels.test.ts:211, :226
#[test]
fn d15_brokered_table() {
    run(async {
        for c in fx("brokered").as_array().unwrap() {
            let w = World::new();
            w.broker.set(c["ready"] == json!(true));
            w.has_door.set(c["door"] != json!(false));
            if c["joined"] == json!(true) {
                w.joined.borrow_mut().insert(c["place"].as_str().unwrap().into());
            }
            let area = w.area();
            let m =
                if c["served"] == json!(true) { serving(Rc::default(), |_| frame(json!({ "ok": true }))) } else { FakeMachine::new("m1") };
            let mut r = rec(json!({ "kind": c["kind"] }));
            if let Some(p) = c["place"].as_str() {
                r["place"] = json!(p);
            }
            assert_eq!(json!(area.brokered(&entry(&r, &m))), c["expected"], "{}", c["name"]);
        }
    });
}

// D16, TS: channels.test.ts:162
#[test]
fn d16_only_the_git_host_ops_carry_a_token() {
    run(async {
        let now = now_of(&fx("with_daemon"));
        for part in ["a git push and a git pr", "every op of the set", "a version above the least"] {
            let c = brokered_case(part);
            assert_eq!(hello_case(&c, now).await.to_string(), c.to_string(), "{part}");
        }
    });
}

// D16
#[test]
fn d16_constants_match_the_ts() {
    run(async {
        World::new().area();
    });
    let c = fx("constants");
    assert_eq!(json!(HELLO_WAIT_MS), c["HELLO_WAIT_MS"]);
    assert_eq!(json!(ORIGIN_READ_MS), c["ORIGIN_READ_MS"]);
    assert_eq!(json!(OP_TOKEN_OPS), c["OP_TOKEN_OPS"]);
    assert_eq!(json!(BLOCKED_READS), c["BLOCKED_READS"]);
}

// D16, TS: channels.test.ts:202
#[test]
fn d16_no_thread_is_a_usage_refusal() {
    run(async {
        let now = now_of(&fx("with_daemon"));
        let c = brokered_case("no thread is refused");
        let got = hello_case(&c, now).await;
        assert_eq!(got.to_string(), c.to_string());
        // The refusal in its words, whole: the sentence, its fix and its kind.
        assert_eq!(
            got["result"]["ok"][0]["error"],
            json!({
                "message": "alpha has no thread yet, so wsp cannot reach GitHub for it. Start a thread on it first.",
                "fix": "Start a thread on it first.",
                "kind": "usage",
            })
        );
    });
}

// D16, TS: channels.test.ts:189
#[test]
fn d16_an_old_or_silent_daemon_gets_no_token() {
    run(async {
        let now = now_of(&fx("with_daemon"));
        for part in ["a daemon from before op tokens", "a hello naming no number"] {
            let c = brokered_case(part);
            let got = hello_case(&c, now).await;
            assert_eq!(got.to_string(), c.to_string(), "{part}");
            assert_eq!(got["minted"], json!([]), "{part}: no device is minted");
        }
    });
}

// D16, TS: channels.test.ts:180
#[test]
fn d16_the_token_is_revoked_after_a_refusal() {
    run(async {
        let now = now_of(&fx("with_daemon"));
        let c = brokered_case("a refused op's token");
        let got = hello_case(&c, now).await;
        assert_eq!(got.to_string(), c.to_string());
        assert_eq!(got["liveAtOp"], json!([true]));
        assert_eq!(got["revoked"], json!(["dev_1"]));
    });
}

// D16
#[test]
fn d16_hello_waits_two_seconds_on_the_clock() {
    run(async {
        let f = fx("with_daemon");
        let w = World::new();
        w.broker.set(true);
        w.has_door.set(true);
        *w.road.borrow_mut() = Ok(reach());
        w.threads.borrow_mut().insert("th_new".into(), ThreadRecord { workspace_id: "ws1".into(), ..Default::default() });
        let area = w.area();
        let d = Daemon::ok();
        w.dials.borrow_mut().push_back(Ok(d.clone()));
        let done = Rc::new(Cell::new(false));
        let (flag, a, e) = (done.clone(), area.clone(), entry(&rec(json!({})), &FakeMachine::new("m1")));
        let task = tokio::task::spawn_local(async move {
            let r = a.with_daemon(&e, work(|ask| async move { ask(frame(json!({ "op": "git.push" }))).await.map(Value::Object) })).await;
            flag.set(true);
            r
        });
        settle().await;
        w.advance(1_999);
        settle().await;
        let (heard_at, settled_at) = (d.heard().len(), done.get());
        w.advance(1);
        settle().await;
        task.await.unwrap().expect("the op goes without a token");
        let got = json!({ "heardAt1999": heard_at, "settledAt1999": settled_at, "heardAfter": d.heard(), "minted": w.door.minted.borrow().clone() });
        assert_eq!(got.to_string(), f["hello_wait"].to_string());
    });
}

// D16
#[test]
fn d16_a_failed_revoke_warns_and_the_reply_stands() {
    run(async {
        let f = fx("with_daemon");
        let w = World::new();
        w.broker.set(true);
        w.has_door.set(true);
        w.turn_road.set(Some(ScopedRoad::Relayed));
        w.clock.advance(now_of(&f));
        w.door.fail_revoke.set(true);
        *w.road.borrow_mut() = Ok(reach());
        w.threads.borrow_mut().insert("th_new".into(), ThreadRecord { workspace_id: "ws1".into(), ..Default::default() });
        let area = w.area();
        w.dials.borrow_mut().push_back(Ok(Daemon::new(|_| frame(json!({ "ok": true, "v": 1 })))));
        let (a, e) = (area.clone(), entry(&rec(json!({ "name": "pricing" })), &FakeMachine::new("m1")));
        let task = tokio::task::spawn_local(async move {
            a.with_daemon(&e, work(|ask| async move { ask(frame(json!({ "op": "git.pr" }))).await.map(Value::Object) })).await
        });
        settle().await;
        let hello = w.listeners.borrow()[0].clone();
        hello(json!({ "type": "daemon.hello", "version": fx("constants")["PIPE_DAEMON_VERSION"].clone() }));
        let got = task.await.unwrap();
        let out = json!({ "result": outcome(got, |v| v), "warns": w.warns(), "minted": w.door.minted.borrow().clone(), "revoked": w.door.revoked.borrow().clone() });
        assert_eq!(out.to_string(), f["failed_revoke"].to_string());
    });
}

// D16, TS: channels.test.ts:559 (channels.ts:180-185, which the TS area does not expose; the table is what those lines say)
#[test]
fn d16_thread_on_picks_the_named_thread_of_this_workspace_else_the_newest() {
    run(async {
        let w = World::new();
        for (th, ws) in [("th_old", "ws1"), ("th_new", "ws1"), ("th_other", "ws2")] {
            w.threads.borrow_mut().insert(th.into(), ThreadRecord { workspace_id: ws.into(), ..Default::default() });
        }
        let area = w.area();
        assert_eq!(area.thread_on("ws1", Some(&json!("th_old"))), Some("th_old".into()), "a named thread of this workspace wins");
        assert_eq!(
            area.thread_on("ws1", Some(&json!("th_other"))),
            Some("th_new".into()),
            "a thread of another workspace is ignored, the newest here stands"
        );
        assert_eq!(area.thread_on("ws1", Some(&json!("th_gone"))), Some("th_new".into()), "an unknown thread is ignored");
        assert_eq!(area.thread_on("ws1", Some(&json!(7))), Some("th_new".into()), "a name that is no string is ignored");
        assert_eq!(area.thread_on("ws1", None), Some("th_new".into()), "none named: the last thread of the workspace in map order");
        assert_eq!(area.thread_on("ws2", None), Some("th_other".into()));
        assert_eq!(area.thread_on("ws3", Some(&json!("th_old"))), None, "a workspace with no thread has none");
    });
}

// D17, TS: slate-box-server.test.ts:242, slate-box-http.test.ts:186
#[test]
fn d17_stdio_road_table() {
    run(async {
        let f = fx("stdio");
        let consts = fx("constants");
        let default_need = consts["STDIO_DAEMON_VERSION"].as_i64().unwrap();
        let cases = f["cases"].as_array().unwrap();
        // Unserved: the machine's daemon dialled with the listeners as its events.
        {
            let c = &cases[0];
            let w = World::new();
            let area = w.area();
            *w.road.borrow_mut() = Ok(reach());
            let d = Daemon::new(|fr| {
                if fr["op"] == json!("stdio.fail") {
                    frame(json!({ "ok": false, "code": "bad", "error": "the daemon refused" }))
                } else {
                    frame(json!({ "ok": true, "op": fr["op"] }))
                }
            });
            w.dials.borrow_mut().push_back(Ok(d.clone()));
            let road = area.stdio_road(&entry(&rec(json!({})), &FakeMachine::new("m1")), default_need).await.expect("the road opens");
            let (a, b): (Rc<RefCell<Vec<Value>>>, Rc<RefCell<Vec<Value>>>) = (Rc::default(), Rc::default());
            let off_a = (road.listen)(on_event(a.clone()));
            (road.listen)(on_event(b.clone()));
            let hear = w.listeners.borrow()[0].clone();
            hear(json!({ "type": "stdio.data", "n": 1 }));
            off_a();
            hear(json!({ "type": "stdio.data", "n": 2 }));
            let sent = outcome((road.send)(frame(json!({ "op": "stdio.open" }))).await, Value::Object);
            let refused = outcome((road.send)(frame(json!({ "op": "stdio.fail" }))).await, Value::Object);
            let ended = Rc::new(Cell::new(false));
            let (chan, flag) = (road.channel.clone(), ended.clone());
            tokio::task::spawn_local(async move {
                chan.closed().await;
                flag.set(true);
            });
            road.channel.close();
            settle().await;
            assert_eq!(sent, c["sent"]);
            assert_eq!(refused, c["refused"]);
            assert_eq!(json!(a.borrow().clone()), c["heardA"], "a listener taken off hears nothing more");
            assert_eq!(json!(b.borrow().clone()), c["heardB"]);
            assert_eq!(json!(w.dialled.borrow().iter().map(|(u, t)| json!({ "url": u, "token": t })).collect::<Vec<_>>()), c["dialled"]);
            assert_eq!(json!(d.heard()), c["heard"]);
            assert_eq!(json!(d.closes.get()), c["closes"]);
            assert_eq!(json!(ended.get()), c["closedSettles"]);
        }
        // Served: the computer's link, its version, its frames.
        for c in &cases[1..] {
            let name = c["name"].as_str().unwrap();
            let w = World::new();
            w.has_door.set(c["door"] != json!(false));
            w.names.borrow_mut().insert("pl_x".into(), "Hetzner box".into());
            if let Some(v) = c["report"]["daemonVersion"].as_i64() {
                w.reports.borrow_mut().insert("pl_x".into(), serde_json::from_value::<PlaceReport>(report_json(Some(v)).unwrap()).unwrap());
            }
            let link = Daemon::new(serving_reply);
            if c["link"] != json!(false) {
                w.links.borrow_mut().insert("pl_x".into(), link.clone());
            }
            let area = w.area();
            let frames: Rc<RefCell<Vec<Value>>> = Rc::default();
            let m = serving(frames.clone(), serving_reply);
            let mut r = rec(json!({ "kind": "place" }));
            if let Some(p) = c["place"].as_str() {
                r["place"] = json!(p);
            }
            let need = c["need"].as_i64().unwrap_or(default_need);
            let got = match area.stdio_road(&entry(&r, &m), need).await {
                Err(e) => json!({ "error": err_json(&e) }),
                Ok(road) => {
                    let heard: Rc<RefCell<Vec<Value>>> = Rc::default();
                    (road.listen)(on_event(heard.clone()));
                    let hear = w.link_listeners.borrow()[0].1.clone();
                    hear(json!({ "type": "stdio.data", "n": 1 }));
                    let sent = outcome((road.send)(frame(json!({ "op": "stdio.open" }))).await, Value::Object);
                    let refused = outcome((road.send)(frame(json!({ "op": "stdio.fail" }))).await, Value::Object);
                    road.channel.close();
                    json!({ "ok": { "sent": sent, "refused": refused, "heard": heard.borrow().clone(), "frames": frames.borrow().clone(), "linkCloses": link.closes.get(), "linkHeard": link.heard() } })
                }
            };
            let out = json!({
                "name": c["name"], "place": c["place"], "door": c["door"], "report": c["report"], "link": c["link"], "need": c["need"],
                "road": got, "reportsAsked": w.reports_asked.borrow().clone(),
            });
            assert_eq!(out.to_string(), c.to_string(), "{name}");
        }
    });
}

// D18, TS: pull-request.test.ts:215, :231, :610
#[test]
fn d18_on_this_computer_without_a_daemon_is_no_host_cli() {
    run(async {
        let f = fx("on_this_computer");
        let w = World::new();
        let area = w.area();
        let got = area.on_this_computer::<Value>(Box::new(|_, _| Box::pin(async { Ok(json!("never")) }))).await;
        assert_eq!(outcome(got, |v| v), f["none"]);
        assert_eq!(w.local_roads_asked.get(), 0, "no road is asked");
    });
}

// D18, TS: pull-request.test.ts:215, :610
#[test]
fn d18_on_this_computer_dials_its_own_daemon_in_its_home_and_closes_it() {
    run(async {
        let f = fx("on_this_computer");
        let w = World::new();
        w.local_daemon.set(true);
        *w.local_road.borrow_mut() = Ok(reach());
        let area = w.area();
        let d = Daemon::new(|fr| frame(json!({ "ok": true, "op": fr["op"] })));
        w.dials.borrow_mut().push_back(Ok(d.clone()));
        let got = area
            .on_this_computer(Box::new(|ask, home| {
                Box::pin(
                    async move { Ok(json!({ "home": home, "reply": ask(frame(json!({ "op": "gh.pr" }))).await.map(Value::Object)? })) },
                )
            }))
            .await;
        assert_eq!(outcome(got, |v| v), f["done"]["result"]);
        assert_eq!(
            json!(w.dialled.borrow().iter().map(|(u, t)| json!({ "url": u, "token": t })).collect::<Vec<_>>()),
            f["done"]["dialled"]
        );
        assert_eq!(json!(d.heard()), f["done"]["heard"]);
        assert_eq!(json!(d.closes.get()), f["done"]["closes"]);
    });
}

// D18, TS: pull-request.test.ts:231
#[test]
fn d18_on_this_computer_carries_a_refusal_with_its_code_and_a_missing_token() {
    run(async {
        let f = fx("on_this_computer");
        let w = World::new();
        w.local_daemon.set(true);
        *w.local_road.borrow_mut() = Ok(reach());
        let area = w.area();
        let d = Daemon::new(|_| frame(json!({ "ok": false, "code": "no-host-cli", "error": "no gh on this computer" })));
        w.dials.borrow_mut().push_back(Ok(d.clone()));
        let got = area
            .on_this_computer(Box::new(|ask, _| Box::pin(async move { ask(frame(json!({ "op": "gh.pr" }))).await.map(Value::Object) })))
            .await;
        assert_eq!(outcome(got, |v| v), f["refused"]["result"]);
        assert_eq!(json!(d.closes.get()), f["refused"]["closes"]);
        // A road with no token is not answering yet, in this computer's name, before any dial.
        let w = World::new();
        w.local_daemon.set(true);
        *w.local_road.borrow_mut() = Ok(serde_json::from_value(json!({ "url": "wss://here", "expiresAt": 1 })).unwrap());
        let area = w.area();
        let got = area.on_this_computer::<Value>(Box::new(|_, _| Box::pin(async { Ok(json!("never")) }))).await;
        assert_eq!(outcome(got, |v| v), f["noToken"]["result"]);
        assert_eq!(json!(w.dialled.borrow().len()), json!(0));
    });
}

// D19, TS: places-settings.test.ts:441
#[test]
fn d19_place_guard_refuses_at_the_limit() {
    run(async {
        let f = fx("guard");
        for c in f["cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let w = World::new();
            w.has_door.set(c["door"] != json!(false));
            w.clock.advance(f["now"].as_i64().unwrap());
            *w.place_rows.borrow_mut() =
                f["rows"].as_array().unwrap().iter().map(|r| serde_json::from_value::<PlaceView>(r.clone()).unwrap()).collect();
            *w.spend.borrow_mut() =
                c["spend"].as_array().unwrap().iter().map(|s| (s["place"].as_str().unwrap().to_string(), s["todayUsd"].as_f64())).collect();
            let area = w.area();
            let got = host_outcome(area.place_guard(c["place"].as_str().unwrap()).await);
            assert_eq!(got, c["result"], "{name}");
            let asked: Vec<Value> = w.spend_asked.borrow().iter().map(|(ids, now)| json!({ "ids": ids, "now": now })).collect();
            assert_eq!(json!(asked), c["spendAsked"], "{name}: what the spend was asked over");
        }
    });
}

// D20
#[test]
fn d20_place_refuses_a_blocked_computer() {
    run(async {
        let f = fx("refuses");
        for c in f["cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let w = blocked_world(&c["reports"], c["door"] != json!(false));
            let area = w.area();
            let got = host_outcome(area.place_refuses(c["place"].as_str()).await);
            assert_eq!(got, c["result"], "{name}");
            assert_eq!(json!(w.reports_asked.borrow().clone()), c["reportsAsked"], "{name}: reports asked");
        }
        let w = blocked_world(&f["cases"][0]["reports"], true);
        let area = w.area();
        assert_eq!(area.blocked_line(Some("pl_x")).await, Some(f["sentences"]["blocked"].as_str().unwrap().to_string()));
        assert_eq!(area.blocked_line(None).await, None);
    });
}

// D20, TS: box-folder.test.ts:564
#[test]
fn d20_copy_blocked_skips_a_folder() {
    run(async {
        let f = fx("refuses");
        for c in f["copy"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let w = blocked_world(&c["reports"], true);
            let area = w.area();
            let mut r = rec(json!({ "kind": c["kind"] }));
            if let Some(p) = c["place"].as_str() {
                r["place"] = json!(p);
            }
            let got = host_outcome(area.copy_blocked(&entry(&r, &FakeMachine::new("m1"))).await);
            assert_eq!(got, c["result"], "{name}");
            assert_eq!(json!(w.reports_asked.borrow().clone()), c["reportsAsked"], "{name}: reports asked");
        }
    });
}
