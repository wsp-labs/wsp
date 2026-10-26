// SPDX-License-Identifier: AGPL-3.0-only
//! channels_ptys (spec/channels_ptys.md): whose shell is whose, the kill guard, the thread tags of a workspace's shells
//! and the port roots of a local workspace. Every expected value is the TS's: oracle/channels_ptys.oracle.ts ran
//! `channelsArea` (channels.ts:280-580) over the same scenarios and wrote tests/fixtures/oracle/channels_ptys.json,
//! inputs beside answers; the two `proc_scopes_terminal_*` cases are test/proc-scopes.test.ts:195 and :211 with the
//! literal expectations that file holds.
//!
//! TS cases ported here (test/...):
//!   channels.test.ts reaches the area through the host; the pty cases below are local-workspace.test.ts:262 (roots),
//!   :369 (owners), :391 (tags named on a list), :405 (pty.thread), :419 (tags after a restart), :437 (a deleted thread's
//!   shells), :453 and :472 (another folder's shell), :484 (a replaced daemon), :505 (a late list), :520 and :561 (the
//!   recheck and the folder), :612 (the kill guard); box-panes.test.ts:84 (a box's kill, here = false);
//!   deleted-workspace-state.test.ts:20 and :38; box-settle.test.ts:172; proc-scopes.test.ts:195 and :211.
#![allow(dead_code, unused_imports)]
mod common;
include!("channels_ptys/world.rs");

#[test]
fn channels_area_registers_its_two_deleted_listeners() {
    run(async {
        let w = World::new();
        let area = w.area();
        assert_eq!(w.deleted.borrow().len(), 2);
        assert!(area.daemon_notes.borrow().is_empty());
        assert!(area.turn_groups.borrow().is_empty());
        assert!(area.port_pids.borrow().is_empty());
    });
}

// Y1
#[test]
fn y1_frame_lists() {
    let f = fx("lists");
    same(ChannelsArea::guest_road_frames(), &f["GUEST_ROAD_FRAMES"]);
    same(strs(&GUEST_EVENTS), &f["GUEST_EVENTS"]);
    same(strs(&PTY_EVENTS), &f["PTY_EVENTS"]);
    same(strs(&COMPUTER_WATCHES), &f["COMPUTER_WATCHES"]);
    same(strs(&COMPUTER_PROCS), &f["COMPUTER_PROCS"]);
    same(strs(&WORKSPACE_FRAMES), &f["WORKSPACE_FRAMES"]);
    assert_eq!(json!(PORT_ROOTS_RECHECK_MS), f["PORT_ROOTS_RECHECK_MS"]);
}

// Y2, TS: test/local-workspace.test.ts:369, :453
#[test]
fn y2_an_owner_sees_and_drives_only_its_ptys() {
    run(async {
        let f = fx("owners");
        let w = World::new();
        let area = w.area();
        let d = Shared::new();
        let own = |owner: &str| area.held_to_owner(owner, d.channel());
        let (a, b, here) = (own("ws_a"), own("ws_b"), own("pl_here"));
        let mut made = vec![];
        for c in [&a, &b, &here] {
            made.push(send(c, json!({ "id": null, "op": "pty.create" })).await);
        }
        same(json!(made), &f["created"]);
        let lists = json!({ "a": ids(&a).await, "b": ids(&b).await, "here": ids(&here).await });
        same(lists, &f["lists"]);
        // A reload dials a fresh channel of the same owner and still finds its own; an exited shell stays listed.
        d.held.borrow_mut().insert("pty_1".into(), true);
        same(ids(&own("ws_a")).await, &f["reloaded"]);
        send(&a, json!({ "id": null, "op": "pty.kill", "ptyId": "pty_1" })).await;
        let after_kill = json!({ "a": ids(&a).await, "b": ids(&b).await });
        same(after_kill, &f["afterKill"]);
        // Another owner's shells: every op by id is told there is no such pty, and none reaches the daemon.
        send(&own("ws_a"), json!({ "id": null, "op": "pty.create" })).await;
        let before = d.heard().len();
        let mut refused = vec![];
        for pty in ["pty_2", "pty_3"] {
            for op in PTY_BY_ID {
                let fr = by_id(7, op, json!(pty));
                let reply = send(&a, fr.clone()).await;
                refused.push(json!({ "frame": fr, "reply": reply }));
            }
        }
        same(json!(refused), &f["refused"]);
        same(json!(d.heard()[before..].to_vec()), &f["reachedDaemon"]);
        let mine = ids(&a).await[0].as_str().unwrap().to_string();
        same(json!(mine), &f["mine"]);
        let mut answered = vec![];
        for op in PTY_BY_ID {
            answered.push(json!({ "op": op, "ok": send(&a, by_id(7, op, json!(mine))).await["ok"] }));
        }
        same(json!(answered), &f["answered"]);
        // A ptyId that is no string is read as its string; a frame naming none passes.
        let odd =
            vec![send(&a, json!({ "id": 8, "op": "pty.attach", "ptyId": 2 })).await, send(&a, json!({ "id": 9, "op": "ping" })).await];
        same(json!(odd), &f["odd"]);
    });
}

// Y2, TS: test/local-workspace.test.ts:505
#[test]
fn y2_a_list_drops_only_what_it_was_asked_over() {
    run(async {
        let f = fx("list_over");
        let w = World::new();
        let area = w.area();
        let d = Shared::new();
        let own = || area.held_to_owner("ws_a", d.channel());
        let (one, two) = (own(), own());
        d.list_held.set(true);
        let asked = one.clone();
        let late = tokio::task::spawn_local(async move { ids(&asked).await });
        settle().await;
        let shell = created(&two).await;
        d.list_held.set(false);
        let late = late.await.unwrap();
        same(json!(shell), &f["shell"]);
        same(send(&two, json!({ "id": 5, "op": "pty.attach", "ptyId": shell })).await, &f["attach"]);
        same(json!({ "late": late, "one": ids(&one).await }), &f["lists"]);
        // The daemon forgets one of two: the next list drops that id from the owner's set and keeps the other.
        let second = created(&one).await;
        same(json!(second), &f["second"]);
        d.held.borrow_mut().shift_remove(&shell);
        let list = ids(&one).await;
        let forgotten = send(&one, json!({ "id": 6, "op": "pty.attach", "ptyId": shell })).await;
        let kept = send(&one, json!({ "id": 7, "op": "pty.attach", "ptyId": second })).await;
        same(json!({ "list": list, "attachForgotten": forgotten, "attachKept": kept }), &f["dropped"]);
    });
}

// Y2: a refused reply passes whole and changes nothing.
#[test]
fn y2_a_refused_reply_passes_whole_and_changes_nothing() {
    run(async {
        let f = fx("list_over");
        let w = World::new();
        let area = w.area();
        let refusing = Daemon::new(|fr| match fr["op"].as_str().unwrap() {
            "pty.create" => frame(json!({ "id": null, "ok": false, "error": "no room" })),
            "pty.list" => frame(json!({ "id": null, "ok": false, "error": "busy" })),
            _ => frame(json!({ "id": null, "ok": true, "ptyId": "pty_x" })),
        });
        let r = area.held_to_owner("ws_r", refusing.clone());
        same(send(&r, json!({ "id": 1, "op": "pty.create" })).await, &f["refusedCreate"]);
        same(send(&r, json!({ "id": 2, "op": "pty.attach", "ptyId": "pty_x" })).await, &f["refusedAttach"]);
        same(send(&r, json!({ "id": 3, "op": "pty.list" })).await, &f["refusedList"]);
        same(json!(refusing.heard()), &f["refusedHeard"]);
    });
}

// Y2, TS: test/local-workspace.test.ts:484
#[test]
fn y2_a_replaced_daemon_forgets_every_owner() {
    run(async {
        let f = fx("replaced");
        let w = World::new();
        let area = w.area();
        let d = Shared::new();
        let own = |owner: &str| area.held_to_owner(owner, d.channel());
        let (a, b) = (own("ws_a"), own("ws_b"));
        let first = json!([created(&a).await, created(&b).await]);
        same(first, &f["first"]);
        d.restart();
        area.shared_daemon_replaced();
        same(json!(created(&b).await), &f["bAgain"]);
        same(ids(&own("ws_a")).await, &f["aList"]);
        let before = d.heard().len();
        same(send(&a, json!({ "id": 3, "op": "pty.attach", "ptyId": "pty_1" })).await, &f["aAttach"]);
        same(json!(d.heard()[before..].to_vec()), &f["reached"]);
        same(ids(&b).await, &f["bList"]);
        same(send(&b, json!({ "id": 4, "op": "pty.attach", "ptyId": "pty_1" })).await, &f["bAttach"]);
    });
}

// Y3, TS: test/local-workspace.test.ts:612, test/box-panes.test.ts:84
#[test]
fn y3_kill_guard_here_and_elsewhere() {
    run(async {
        let f = fx("kill_guard");
        let w = guard_world(&f);
        let area = w.area();
        let inner = Daemon::ok();
        let asked: Vec<Value> = f["asked"].as_array().unwrap().clone();
        for here in [true, false] {
            for fr in &asked {
                send(&area.kill_guarded(inner.clone(), here), fr.clone()).await;
            }
        }
        // Another op passes whole, key order and all.
        send(&area.kill_guarded(inner.clone(), true), json!({ "id": 4, "op": "pty.list", "protect": [9] })).await;
        send(&area.kill_guarded(inner.clone(), false), json!({ "id": 5, "op": "ping" })).await;
        same(json!(inner.heard()), &f["heard"]);
        // The agent the run recorded is gone, then the app: read at each kill.
        w.agent_pids.borrow_mut().remove("run1");
        let gone = Daemon::ok();
        send(&area.kill_guarded(gone.clone(), true), json!({ "id": 6, "op": "proc.kill", "pid": 53 })).await;
        w.alive.borrow_mut().remove(&4999);
        send(&area.kill_guarded(gone.clone(), true), json!({ "id": 7, "op": "proc.kill", "pid": 54 })).await;
        same(json!(gone.heard()), &f["afterAgentGone"]);
        // A host with no state path has no app, and no running turn means no agents.
        let bare = guard_world(&f);
        bare.state_path.set(false);
        bare.rows.borrow_mut().clear();
        let bare_area = bare.area();
        let none = Daemon::ok();
        send(&bare_area.kill_guarded(none.clone(), true), json!({ "id": 8, "op": "proc.kill", "pid": 55 })).await;
        same(json!(none.heard()), &f["noState"]);
    });
}

// Y4
#[test]
fn y4_app_main_table() {
    run(async {
        let f = fx("app_main");
        let path = format!("/state/{}/{}", f["lockPath"][0].as_str().unwrap(), f["lockPath"][1].as_str().unwrap());
        for case in f["cases"].as_array().unwrap() {
            let w = World::new();
            w.state_path.set(case["statePath"] == json!(true));
            *w.hostname.borrow_mut() = f["host"].as_str().unwrap().to_string();
            if let Some(link) = case["link"].as_str() {
                w.fs_links.borrow_mut().insert(path.clone(), link.to_string());
            }
            for pid in case["alive"].as_array().unwrap() {
                w.alive.borrow_mut().insert(pid.as_i64().unwrap());
            }
            let area = w.area();
            let name = case["name"].as_str().unwrap();
            assert_eq!(json!(area.app_main()), case["expected"], "{name}");
            let read: Vec<String> = w.link_reads.borrow().clone();
            let wanted: Vec<String> = if case["statePath"] == json!(true) { vec![path.clone()] } else { vec![] };
            assert_eq!(read, wanted, "{name}: the link read");
        }
    });
}

// Y5, TS: test/local-workspace.test.ts:391
#[test]
fn y5_a_create_tags_its_thread() {
    run(async {
        let f = fx("create_tags");
        let w = World::new();
        let area = w.area();
        let d = Shared::new();
        let pane = area.thread_tagged("ws_a", d.channel());
        // Each write runs before the next send (the TS's awaits give it the turn), so each is the tags as they stood.
        let mut replies = vec![];
        for fr in [
            json!({ "id": 1, "op": "pty.create", "thread": "th_a" }),
            json!({ "id": 2, "op": "pty.create", "thread": "th_b" }),
            json!({ "id": 3, "op": "pty.create", "thread": 7 }),
            json!({ "id": 4, "op": "pty.create" }),
            json!({ "id": 5, "op": "pty.create", "thread": "" }),
        ] {
            replies.push(send(&pane, fr).await);
            settle().await;
        }
        settle().await;
        same(json!(replies), &f["replies"]);
        let heard_creates: Vec<Value> = d.heard().into_iter().filter(|h| h["op"] == json!("pty.create")).collect();
        same(json!(heard_creates), &f["heardCreates"]);
        same(json!({ "ops": w.store.ops(), "doc": doc(&w, "ws_a") }), &f["afterCreates"]);
        same(send(&pane, json!({ "id": 6, "op": "pty.list" })).await, &f["list"]);
        let before_kill = w.store.ops.borrow().len();
        same(send(&pane, json!({ "id": 7, "op": "pty.kill", "ptyId": "pty_4" })).await, &f["killUntagged"]);
        settle().await;
        assert_eq!(json!(w.store.ops.borrow().len() - before_kill), f["writesByUntaggedKill"]);
        same(send(&pane, json!({ "id": 8, "op": "pty.kill", "ptyId": "pty_1" })).await, &f["kill"]);
        settle().await;
        same(send(&pane, json!({ "id": 9, "op": "pty.kill", "ptyId": "pty_1" })).await, &f["refusedKill"]);
        settle().await;
        let list = send(&pane, json!({ "id": 10, "op": "pty.list" })).await;
        let ops: Vec<Value> = w.store.ops.borrow()[before_kill..].to_vec();
        same(json!({ "ops": ops, "doc": doc(&w, "ws_a"), "list": list }), &f["afterKill"]);
        same(w.warns(), &f["warns"]);
    });
}

// Y5, TS: test/local-workspace.test.ts:405
#[test]
fn y5_pty_thread_hands_a_shell_over() {
    run(async {
        let f = fx("handover");
        let w = World::new();
        let area = w.area();
        let d = Shared::new();
        let mk = |id: &str| area.held_to_owner(id, area.thread_tagged(id, d.channel()));
        let pane = mk("ws_a");
        let page = send(&pane, json!({ "id": null, "op": "pty.create", "thread": "" })).await["ptyId"].as_str().unwrap().to_string();
        let theirs = created(&mk("ws_b")).await;
        settle().await;
        same(json!([page, theirs]), &json!([f["page"], f["theirs"]]));
        let moved_before = w.moved.get();
        let ops_before = w.store.ops.borrow().len();
        let handed = send(&pane, json!({ "id": 3, "op": "pty.thread", "ptyId": page, "thread": "th_new" })).await;
        settle().await;
        same(handed, &f["handed"]);
        assert_eq!(json!(w.moved.get() - moved_before), f["movedByHandover"]);
        let writes: Vec<Value> = w.store.ops.borrow()[ops_before..].to_vec();
        same(json!(writes), &f["writes"]);
        same(send(&pane, json!({ "id": 4, "op": "pty.thread", "ptyId": theirs, "thread": "th_new" })).await, &f["notOwned"]);
        same(send(&pane, json!({ "id": 5, "op": "pty.thread", "ptyId": page })).await, &f["noThread"]);
        same(send(&pane, json!({ "id": 6, "op": "pty.thread", "thread": "th_x" })).await, &f["noPty"]);
        same(send(&pane, json!({ "id": 7, "op": "pty.thread", "ptyId": 7, "thread": "th_x" })).await, &f["numberPty"]);
        same(send(&pane, json!({ "id": 8, "op": "pty.thread", "ptyId": page, "thread": 7 })).await, &f["numberThread"]);
        settle().await;
        let rows = send(&mk("ws_a"), json!({ "id": 9, "op": "pty.list" })).await["ptys"].clone();
        same(rows, &f["rows"]);
        let on_daemon: Vec<Value> = d.heard().into_iter().filter(|h| h["op"] == json!("pty.thread")).collect();
        same(json!(on_daemon), &f["heardThread"]);
        // A bare tagged channel retags any id, listed or not; the next list forgets it.
        let bare = area.thread_tagged("ws_c", d.channel());
        same(send(&bare, json!({ "id": 10, "op": "pty.thread", "ptyId": "ghost", "thread": "th_g" })).await, &f["ghost"]);
        settle().await;
        same(doc(&w, "ws_c"), &f["ghostDoc"]);
        same(send(&bare, json!({ "id": 11, "op": "pty.list" })).await, &f["ghostList"]);
        settle().await;
        same(doc(&w, "ws_c"), &f["ghostAfter"]);
    });
}

// Y5
#[test]
fn y5_a_list_names_and_drops() {
    run(async {
        let f = fx("list_names");
        let w = World::new();
        w.store.seed("pty-threads", "ws_a", json!({ "pty_1": "th_a", "pty_2": "th_b", "pty_9": "th_gone" }));
        let area = w.area();
        let d = Daemon::new(|fr| match fr["op"].as_str().unwrap() {
            "pty.list" => frame(
                json!({ "id": null, "ok": true, "ptys": [{ "id": "pty_1", "pid": 100, "cols": 80 }, { "id": "pty_2", "pid": 101, "cols": 80 }, { "id": "pty_3", "pid": 102, "cols": 80 }] }),
            ),
            _ => frame(json!({ "id": null, "ok": false, "error": "no" })),
        });
        let pane = area.thread_tagged("ws_a", d);
        same(send(&pane, json!({ "id": 1, "op": "pty.list" })).await, &f["first"]);
        settle().await;
        same(json!({ "ops": w.store.ops(), "doc": doc(&w, "ws_a") }), &f["afterFirst"]);
        same(send(&pane, json!({ "id": 2, "op": "pty.list" })).await, &f["second"]);
        settle().await;
        assert_eq!(json!(w.store.ops.borrow().len()), f["writesAfterSecond"]);
        same(send(&pane, json!({ "id": 3, "op": "pty.kill", "ptyId": "pty_1" })).await, &f["refused"]);
        settle().await;
        assert_eq!(json!(w.store.ops.borrow().len()), f["writesAfterSecond"]);
        same(json!(w.store.reads.borrow().clone()), &f["reads"]);
    });
}

// Y5: a non-object, a failed read or a non-string value reads as no tag.
#[test]
fn y5_a_bad_tag_document_reads_as_no_tags() {
    run(async {
        let f = fx("tags_read");
        let w = World::new();
        w.store.seed("pty-threads", "ws_obj", json!({ "pty_1": "th_a", "pty_2": 7, "pty_3": null, "pty_4": ["x"], "pty_5": "" }));
        w.store.seed("pty-threads", "ws_str", json!("th_a"));
        w.store.seed("pty-threads", "ws_null", Value::Null);
        let area = w.area();
        let lister = || {
            Daemon::new(|fr| match fr["op"].as_str().unwrap() {
                "pty.list" => frame(
                    json!({ "id": null, "ok": true, "ptys": (1..=5).map(|i| json!({ "id": format!("pty_{i}"), "pid": 99 + i, "cols": 80 })).collect::<Vec<_>>() }),
                ),
                _ => ok(),
            })
        };
        for id in ["ws_obj", "ws_str", "ws_null", "ws_none"] {
            same(send(&area.thread_tagged(id, lister()), json!({ "id": 1, "op": "pty.list" })).await, &f[id]);
        }
        *w.store.fail_get.borrow_mut() = Some("disk gone".into());
        same(send(&area.thread_tagged("ws_failed", lister()), json!({ "id": 1, "op": "pty.list" })).await, &f["ws_failed"]);
    });
}

// Y6
#[test]
fn y6_tag_writes_run_in_turn_last_wins() {
    run(async {
        let f = fx("writes_in_turn");
        let w = World::new();
        let area = w.area();
        let d = Shared::new();
        let pane = area.thread_tagged("ws_a", d.channel());
        w.store.held.set(true);
        send(&pane, json!({ "id": 1, "op": "pty.create", "thread": "th_a" })).await;
        settle().await;
        send(&pane, json!({ "id": 2, "op": "pty.create", "thread": "th_b" })).await;
        send(&pane, json!({ "id": 3, "op": "pty.kill", "ptyId": "pty_1" })).await;
        settle().await;
        same(json!({ "ops": w.store.ops(), "doc": doc(&w, "ws_a") }), &f["whileHeld"]);
        assert!(area.tag_write("ws_a").is_some(), "a write is out while the store holds it");
        w.store.held.set(false);
        settle().await;
        same(json!({ "ops": w.store.ops(), "doc": doc(&w, "ws_a") }), &f["afterRelease"]);
        // The last tag goes: the document goes with it.
        send(&pane, json!({ "id": 4, "op": "pty.kill", "ptyId": "pty_2" })).await;
        settle().await;
        same(json!({ "ops": w.store.ops(), "doc": doc(&w, "ws_a") }), &f["last"]);
        same(w.warns(), &f["warns"]);
        assert!(area.tag_write("ws_a").is_none(), "no write is out once the last settled");
    });
}

// Y6
#[test]
fn y6_a_failed_tag_write_warns_and_the_next_runs() {
    run(async {
        let f = fx("failed_write");
        let w = World::new();
        let area = w.area();
        let d = Shared::new();
        let pane = area.thread_tagged("ws_a", d.channel());
        w.store.fail_puts.borrow_mut().push_back("disk full".into());
        send(&pane, json!({ "id": 1, "op": "pty.create", "thread": "th_a" })).await;
        settle().await;
        same(json!({ "ops": w.store.ops(), "doc": doc(&w, "ws_a"), "warns": w.warns() }), &f["afterFailure"]);
        send(&pane, json!({ "id": 2, "op": "pty.create", "thread": "th_b" })).await;
        settle().await;
        same(
            json!({ "ops": w.store.ops(), "doc": doc(&w, "ws_a"), "warns": w.warns() }),
            &json!({ "ops": f["ops"], "doc": f["doc"], "warns": f["warns"] }),
        );
    });
}

// Y7, TS: test/deleted-workspace-state.test.ts:20
#[test]
fn y7_deleted_workspace_tags_go_with_it() {
    run(async {
        let f = fx("deleted");
        let w = World::new();
        let area = w.area();
        for ws in ["ws_gone", "ws_kept"] {
            send(&area.thread_tagged(ws, lists_pty7()), json!({ "id": null, "op": "pty.create", "thread": "th_a" })).await;
        }
        settle().await;
        let docs =
            |w: &World| json!({ "ws_gone": w.store.doc("pty-threads", "ws_gone"), "ws_kept": w.store.doc("pty-threads", "ws_kept") });
        same(json!({ "doc": docs(&w), "ops": w.store.ops() }), &f["before"]);
        w.store.ops.borrow_mut().clear();
        w.delete_workspace_event("ws_gone");
        settle().await;
        let writes: Vec<&str> = ["ws_gone", "ws_kept"].into_iter().filter(|ws| area.tag_write(ws).is_some()).collect();
        same(json!({ "doc": docs(&w), "ops": w.store.ops(), "writes": writes }), &f["afterDelete"]);
        // Held no more: a document that appeared since is read afresh for the deleted one, and not for the other.
        w.store.seed("pty-threads", "ws_gone", json!({ "pty_7": "th_z" }));
        w.store.seed("pty-threads", "ws_kept", json!({ "pty_1": "th_changed" }));
        let listed = |ws: &str| {
            let pane = area.thread_tagged(ws, lists_pty7());
            async move { send(&pane, json!({ "id": null, "op": "pty.list" })).await["ptys"].clone() }
        };
        same(json!({ "ws_gone": listed("ws_gone").await, "ws_kept": listed("ws_kept").await }), &f["reread"]);
    });
}

// Y7, TS: test/deleted-workspace-state.test.ts:38
#[test]
fn y7_a_reply_after_the_delete_writes_nothing() {
    run(async {
        let f = fx("deleted")["late"].clone();
        let w = World::new();
        let area = w.area();
        let slow = Daemon::new(|_| frame(json!({ "id": null, "ok": true, "ptyId": "pty_late" })));
        slow.hold_next.set(true);
        let pane = area.thread_tagged("ws_gone", slow.clone());
        let opening =
            tokio::task::spawn_local(async move { send(&pane, json!({ "id": null, "op": "pty.create", "thread": "th_a" })).await });
        settle().await;
        w.delete_workspace_event("ws_gone");
        settle().await;
        same(w.store.ops(), &f["opsAtDelete"]);
        slow.land();
        opening.await.unwrap();
        settle().await;
        let writes: Vec<&str> = if area.tag_write("ws_gone").is_some() { vec!["ws_gone"] } else { vec![] };
        same(json!({ "ops": w.store.ops(), "doc": doc(&w, "ws_gone"), "writes": writes }), &f["afterLate"]);
        assert!(area.thread_terminals().is_empty());
    });
}

// Y7, TS: test/local-workspace.test.ts:419
#[test]
fn y7_tags_survive_a_restart() {
    run(async {
        let f = fx("deleted")["restart"].clone();
        let first = World::new();
        let area = first.area();
        let d = Shared::new();
        let pane = area.thread_tagged("ws_c", d.channel());
        send(&pane, json!({ "id": null, "op": "pty.create", "thread": "th_a" })).await;
        send(&pane, json!({ "id": null, "op": "pty.create", "thread": "th_b" })).await;
        send(&pane, json!({ "id": null, "op": "pty.thread", "ptyId": "pty_2", "thread": "th_c" })).await;
        settle().await;
        same(doc(&first, "ws_c"), &f["doc"]);
        let second = first.again();
        let area2 = second.area();
        let rows = send(&area2.thread_tagged("ws_c", d.channel()), json!({ "id": null, "op": "pty.list" })).await["ptys"].clone();
        same(rows, &f["restarted"]);
    });
}

// Y8
#[test]
fn y8_thread_terminals_from_tags_read() {
    run(async {
        let f = fx("terminals");
        let w = World::new();
        let area = w.area();
        set_ptys(&area, "ws_a", &[("pty_1", 4242), ("pty_2", 4343), ("pty_3", 4444), ("pty_4", 4545)]);
        set_ptys(&area, "ws_b", &[("pty_9", 77)]);
        w.store.seed("pty-threads", "ws_a", json!({ "pty_1": "th_a", "pty_2": "", "pty_4": "th_b" }));
        w.store.seed("pty-threads", "ws_b", json!({ "pty_9": "th_x" }));
        same(terminals(area.thread_terminals()), &f["unread"]);
        let pane = area.thread_tagged("ws_a", Shared::new().channel());
        send(&pane, json!({ "id": 1, "op": "pty.thread", "ptyId": "pty_3", "thread": "th_c" })).await;
        settle().await;
        same(terminals(area.thread_terminals()), &f["afterOneRead"]);
        send(
            &area.thread_tagged("ws_b", Shared::new().channel()),
            json!({ "id": 2, "op": "pty.thread", "ptyId": "pty_8", "thread": "th_y" }),
        )
        .await;
        settle().await;
        same(terminals(area.thread_terminals()), &f["afterTwoRead"]);
    });
}

// Y9, TS: test/box-settle.test.ts:172, test/local-workspace.test.ts:437
#[test]
fn y9_end_thread_shells() {
    run(async {
        let f = fx("end_shells");
        let w = World::new();
        let area = w.area();
        w.store.seed("pty-threads", "ws_a", json!({ "pty_1": "th_gone", "pty_2": "th_kept", "pty_3": "th_gone", "pty_4": "th_gone" }));
        w.store.seed("pty-tokens", "ws_a", json!({ "pty_1": "dev_1", "pty_2": "dev_2", "pty_4": "dev_4" }));
        let served_frames: Rc<RefCell<Vec<Value>>> = Rc::default();
        let machine = FakeMachine::new("m1");
        let log = served_frames.clone();
        *machine.served.borrow_mut() = Some(Rc::new(move |fr: Frame| {
            log.borrow_mut().push(Value::Object(fr.clone()));
            Box::pin(async move {
                Ok(match fr.get("ptyId").and_then(Value::as_str) {
                    Some("pty_3") => frame(json!({ "id": null, "ok": false, "error": "no such pty: pty_3" })),
                    Some("pty_4") => frame(json!({ "ok": false, "code": "busy" })),
                    _ => ok(),
                })
            })
        }));
        let entry = live_with(
            record(
                &json!({ "id": "ws_a", "name": "alpha", "machineId": "m1", "phase": "running", "kind": "place", "project": "p1", "place": "pl_1" }),
            ),
            machine,
        );
        area.end_thread_shells(&entry, "th_gone").await.expect("the shells end");
        settle().await;
        area.end_thread_shells(&entry, "th_nobody").await.expect("a thread with no shells");
        settle().await;
        let revoked = json!(w.door.revoked.borrow().clone());
        let out = json!({
            "served": served_frames.borrow().clone(),
            "revoked": revoked,
            "warns": w.warns(),
            "tags": doc(&w, "ws_a"),
            "tokens": { "ws_a": w.store.doc("pty-tokens", "ws_a") },
        });
        same(out, &f);
    });
}

// Y10, TS: test/local-workspace.test.ts:262
#[test]
fn y10_port_roots_sorted_and_pruned() {
    run(async {
        let f = fx("roots");
        let w = World::new();
        let area = w.area();
        w.groups.borrow_mut().extend([3100, 3200]);
        set_groups(&area, "ws_a", &[(3200, "th_2"), (2900, "th_3"), (3100, "th_1")]);
        set_ptys(&area, "ws_a", &[("pty_1", 4242), ("pty_2", 3100), ("pty_0", 17)]);
        set_groups(&area, "ws_b", &[(2800, "th_4")]);
        set_ptys(&area, "ws_c", &[("pty_5", 5)]);
        set_ptys(&area, "ws_b", &[]);
        let mut watched = vec![];
        for ws in ["ws_a", "ws_b", "ws_c", "ws_none"] {
            watched.push(watch_roots(&area, ws).await);
        }
        same(json!(watched), &f["watched"]);
        // The same reading, asked straight.
        let roots = json!({ "ws_a": area.port_roots_of("ws_a"), "ws_b": area.port_roots_of("ws_b"), "ws_c": area.port_roots_of("ws_c"), "ws_none": area.port_roots_of("ws_none") });
        same(roots, &f["roots"]);
        same(groups_of(&area), &f["groupsAfter"]);
    });
}

// Y10, TS: test/local-workspace.test.ts:520, :561
#[test]
fn y10_recheck_runs_every_5s_and_stops() {
    run(async {
        let f = fx("recheck");
        let w = World::new();
        let area = w.area();
        w.groups.borrow_mut().extend([3100, 3200, 3300]);
        set_groups(&area, "ws_a", &[(3100, "th_1"), (3200, "th_2")]);
        set_groups(&area, "ws_b", &[(3300, "th_3")]);
        let inner = Daemon::new(|_| frame(json!({ "id": null, "ok": true, "ports": [] })));
        let wa = area.rooted_ports("ws_a", "/work/ws_a", inner.clone());
        send(&wa, json!({ "id": null, "op": "ports.watch" })).await;
        let snap = |name: &str| {
            let watches: Vec<Value> =
                inner.heard().into_iter().filter(|h| h["op"] == json!("ports.watch")).map(|h| h["roots"].clone()).collect();
            let mut groups = serde_json::Map::new();
            for (ws, g) in area.turn_groups.borrow().iter() {
                groups.insert(ws.clone(), json!(g.keys().collect::<Vec<_>>()));
            }
            json!({ "name": name, "moved": w.moved.get(), "armed": w.state.with(|s| s.roots_recheck.is_some()), "watches": watches, "groups": groups })
        };
        let mut steps = vec![snap("watching")];
        area.arm_roots_recheck();
        area.arm_roots_recheck();
        steps.push(
            json!({ "name": "armed twice, one stands", "same": w.clock.pending() == 1 && w.state.with(|s| s.roots_recheck.is_some()) }),
        );
        w.advance(4_999);
        settle().await;
        steps.push(snap("4999 ms"));
        w.advance(1);
        settle().await;
        steps.push(snap("5000 ms, everything still stands"));
        w.groups.borrow_mut().remove(&3200);
        w.advance(5_000);
        settle().await;
        steps.push(snap("10000 ms, one group of ws_a gone"));
        w.groups.borrow_mut().remove(&3100);
        w.groups.borrow_mut().remove(&3300);
        w.advance(5_000);
        settle().await;
        steps.push(snap("15000 ms, every group gone"));
        w.advance(5_000);
        settle().await;
        steps.push(snap("20000 ms, nothing left to move"));
        assert_eq!(w.clock.pending(), 0, "the interval stopped itself");
        set_groups(&area, "ws_z", &[(3100, "th_9")]);
        w.groups.borrow_mut().insert(3100);
        area.arm_roots_recheck();
        steps.push(json!({ "name": "armed again after it stopped", "armed": w.state.with(|s| s.roots_recheck.is_some()) }));
        same(json!(steps), &f);
    });
}

// Y11, TS: test/local-workspace.test.ts:262, test/box-panels.test.ts:661
#[test]
fn y11_rooted_ports_watch_and_push() {
    run(async {
        let f = fx("rooted");
        let w = World::new();
        let area = w.area();
        w.groups.borrow_mut().insert(3100);
        set_groups(&area, "ws_a", &[(3100, "th_1")]);
        set_ptys(&area, "ws_a", &[]);
        let refuse = Rc::new(Cell::new(false));
        let creates = Cell::new(0);
        let refusing = refuse.clone();
        let inner = Daemon::new(move |fr| match fr["op"].as_str().unwrap() {
            "pty.create" => {
                creates.set(creates.get() + 1);
                if refusing.get() {
                    return frame(json!({ "id": null, "ok": false, "error": "no room", "pid": 1 }));
                }
                frame(
                    json!({ "id": null, "ok": true, "ptyId": format!("pty_{}", creates.get()), "pid": fr.get("pid").cloned().unwrap_or(json!(4242)) }),
                )
            }
            "pty.list" => {
                if refusing.get() {
                    return frame(json!({ "id": null, "ok": false, "error": "busy" }));
                }
                frame(
                    json!({ "id": null, "ok": true, "ptys": [{ "id": "pty_1" }, { "id": "pty_2", "exited": true }, { "id": "pty_3", "exited": false }] }),
                )
            }
            _ => frame(json!({ "id": null, "ok": true, "ports": [] })),
        });
        let c = area.rooted_ports("ws_a", "/work/ws_a", inner.clone());
        let mut steps: Vec<Value> = vec![];
        macro_rules! step {
            ($name:expr, $body:expr) => {{
                let before = inner.heard().len();
                let moved = w.moved.get();
                let reply: Value = $body;
                settle().await;
                let ptys = pty_pids(&area.port_pids.borrow()["ws_a"]);
                steps.push(json!({ "name": $name, "reply": reply, "heard": inner.heard()[before..].to_vec(), "moved": w.moved.get() - moved, "ptys": ptys }));
            }};
        }
        let set = |id: &str, pid: i64| {
            area.port_pids.borrow_mut().get_mut("ws_a").unwrap().insert(id.to_string(), pid);
        };
        let unset = |id: &str| {
            area.port_pids.borrow_mut().get_mut("ws_a").unwrap().shift_remove(id);
        };
        step!("before a watch, a move names nothing", {
            area.port_roots_moved("ws_a");
            Value::Null
        });
        step!("a watch names the roots, the folder after the frame's own keys", send(&c, json!({ "id": 1, "op": "ports.watch" })).await);
        step!("a move with the same roots names nothing", {
            area.port_roots_moved("ws_a");
            Value::Null
        });
        step!("a terminal with a pid joins and the roots are named again", send(&c, json!({ "id": 2, "op": "pty.create" })).await);
        step!(
            "a watch carrying roots and folder of its own keeps their slots",
            send(&c, json!({ "id": 3, "op": "ports.watch", "roots": [9], "folder": "/elsewhere", "extra": true })).await
        );
        step!("a second watch with nothing changed still carries the roots", send(&c, json!({ "id": 4, "op": "ports.watch" })).await);
        step!("a terminal with no numeric pid is no root", send(&c, json!({ "id": 5, "op": "pty.create", "pid": "x" })).await);
        refuse.set(true);
        step!("a refused create joins nothing", send(&c, json!({ "id": 6, "op": "pty.create" })).await);
        step!("a refused list drops nothing", {
            set("pty_8", 4848);
            send(&c, json!({ "id": 7, "op": "pty.list" })).await
        });
        refuse.set(false);
        step!("a list that holds every pty it was given standing moves nothing", {
            unset("pty_8");
            send(&c, json!({ "id": 8, "op": "pty.list" })).await
        });
        step!("a pty the host holds that the daemon does not list is dropped and the roots are named again", {
            set("pty_8", 4848);
            area.port_roots_moved("ws_a");
            settle().await;
            send(&c, json!({ "id": 9, "op": "pty.list" })).await
        });
        step!("a pty listed as exited is dropped and the roots are named again", {
            set("pty_2", 4949);
            area.port_roots_moved("ws_a");
            settle().await;
            send(&c, json!({ "id": 10, "op": "pty.list" })).await
        });
        step!("another op passes whole", send(&c, json!({ "id": 11, "op": "fs.list", "path": "/" })).await);
        // The oracle's refused steps used the main daemon's frames, which carry the refused create's frame too.
        for (i, step) in steps.iter().enumerate() {
            assert_eq!(step.to_string(), f["steps"][i].to_string(), "step {i}: {}", step["name"]);
        }
        assert_eq!(steps.len(), f["steps"].as_array().unwrap().len());
    });
}

// Y11: a list drops every pty not listed standing, an exited one among them, and names the roots again.
#[test]
fn y11_a_list_drops_exited_ptys() {
    run(async {
        let f = fx("list_exited");
        let w = World::new();
        let area = w.area();
        set_ptys(&area, "ws_a", &[("pty_1", 4242), ("pty_2", 4949), ("pty_8", 4848)]);
        let inner = Daemon::new(|fr| match fr["op"].as_str().unwrap() {
            "pty.list" => frame(
                json!({ "id": null, "ok": true, "ptys": [{ "id": "pty_1" }, { "id": "pty_2", "exited": true }, { "id": "pty_3", "exited": false }] }),
            ),
            _ => frame(json!({ "id": null, "ok": true, "ports": [] })),
        });
        let c = area.rooted_ports("ws_a", "/work/ws_a", inner.clone());
        send(&c, json!({ "id": 1, "op": "ports.watch" })).await;
        let moved = w.moved.get();
        let reply = send(&c, json!({ "id": 2, "op": "pty.list" })).await;
        settle().await;
        let ptys = pty_pids(&area.port_pids.borrow()["ws_a"]);
        same(json!({ "reply": reply, "heard": inner.heard(), "moved": w.moved.get() - moved, "ptys": ptys }), &f);
    });
}

// Y11: the push leaves at close, and when the channel's own end settles.
#[test]
fn y11_the_push_leaves_at_close_and_at_the_channels_end() {
    run(async {
        let f = fx("rooted");
        let w = World::new();
        let area = w.area();
        w.groups.borrow_mut().insert(3100);
        set_groups(&area, "ws_a", &[(3100, "th_1")]);
        set_ptys(&area, "ws_a", &[("pty_1", 4242)]);
        for (how, expected) in [("close", &f["closed"]), ("end", &f["ended"])] {
            let ch = Daemon::new(|_| frame(json!({ "id": null, "ok": true, "ports": [] })));
            let rc = area.rooted_ports("ws_a", "/work/ws_a", ch.clone());
            send(&rc, json!({ "id": 1, "op": "ports.watch" })).await;
            let mv = |id: &str, pid: i64| {
                area.port_pids.borrow_mut().get_mut("ws_a").unwrap().insert(id.to_string(), pid);
                area.port_roots_moved("ws_a");
            };
            mv("pty_20", 2020);
            settle().await;
            let while_watching: Vec<Value> = ch.heard()[1..].to_vec();
            if how == "close" {
                rc.close();
            } else {
                ch.end();
            }
            settle().await;
            mv("pty_21", 2121);
            settle().await;
            let after: Vec<Value> = ch.heard()[1 + while_watching.len()..].to_vec();
            area.port_pids.borrow_mut().get_mut("ws_a").unwrap().shift_remove("pty_20");
            area.port_pids.borrow_mut().get_mut("ws_a").unwrap().shift_remove("pty_21");
            same(json!({ "whileWatching": while_watching, "afterLeaving": after }), expected);
        }
    });
}

// TS: test/proc-scopes.test.ts:195
#[test]
fn proc_scopes_terminal_follows_its_thread_and_a_watching_channel_hears_each_move() {
    run(async {
        let (_w, area, scopes) = both_areas();
        let watching = Daemon::ok();
        let of = scopes.clone();
        let scoped = scopes.scoped_procs(Rc::new(move || of.scopes_here()), watching.clone());
        send(&scoped, json!({ "id": 1, "op": "proc.watch" })).await;
        let pane = opened(&area, "ws_a", shells(&[4242, 4343]));
        send(&pane, json!({ "id": 2, "op": "pty.create", "thread": "th_a" })).await;
        send(&pane, json!({ "id": 3, "op": "pty.create" })).await;
        settle().await;
        assert_eq!(terminals_of(&scopes.scopes_here(), "th_a"), vec![4242]);
        assert!(!all_pids(&scopes.scopes_here()).contains(&4343));
        send(&pane, json!({ "id": 4, "op": "pty.thread", "ptyId": "pty_1", "thread": "th_b" })).await;
        settle().await;
        assert_eq!((terminals_of(&scopes.scopes_here(), "th_a"), terminals_of(&scopes.scopes_here(), "th_b")), (vec![], vec![4242]));
        let told: Vec<Value> = watching.heard().into_iter().filter(|f| f["op"] == json!("proc.scopes")).collect();
        let last = told.last().expect("the watching channel was told");
        let named: Vec<ProcScope> = serde_json::from_value(last["scopes"].clone()).unwrap();
        assert_eq!(terminals_of(&named, "th_b"), vec![4242]);
        assert_eq!(terminals_of(&named, "th_a"), Vec::<i64>::new());
    });
}

// TS: test/proc-scopes.test.ts:211
#[test]
fn proc_scopes_terminal_on_a_box_is_named_to_the_boxs_daemon_never_this_computers() {
    run(async {
        let (_w, area, scopes) = both_areas();
        let pane = opened(&area, "ws_box", shells(&[77]));
        send(&pane, json!({ "id": 1, "op": "pty.create", "thread": "th_box" })).await;
        settle().await;
        assert_eq!(terminals_of(&scopes.scopes_on("pl_1"), "th_box"), vec![77]);
        assert!(!all_pids(&scopes.scopes_here()).contains(&77));
    });
}
