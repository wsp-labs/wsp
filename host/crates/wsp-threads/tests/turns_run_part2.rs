// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `turns_run.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("turns_run/world.rs");

// N18
#[test]
fn a_checkpoint_another_turn_replaced_stays_when_the_first_lands() {
    run_local(async {
        let (host, entry) = world();
        let (tx, rx) = oneshot::channel::<()>();
        let landing: Joined = rx.map(|_| ()).boxed_local().shared();
        let l = landing.clone();
        host.hooks.set::<dyn Fn(&Entry, CheckpointAsk) -> Joined>(
            "TurnsCtx::keep_checkpoint",
            Rc::new(move |_e: &Entry, _a: CheckpointAsk| l.clone()),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        r.send(session_end());
        let (_other_tx, other_rx) = oneshot::channel::<()>();
        let other: Joined = other_rx.map(|_| ()).boxed_local().shared();
        host.checkpoints_landing.borrow_mut().insert(THREAD.to_string(), other);
        tx.send(()).unwrap();
        settle().await;
        assert!(host.checkpoints_landing.borrow().contains_key(THREAD), "the later turn's checkpoint is not taken off");
    });
}

// N18: a turn with nothing to keep
#[test]
fn a_turn_with_nothing_held_and_no_anchor_keeps_a_bare_checkpoint() {
    run_local(async {
        let (host, entry) = world();
        let asks: Rc<RefCell<Vec<CheckpointAsk>>> = Rc::default();
        let a = asks.clone();
        host.hooks.set::<dyn Fn(&Entry, CheckpointAsk) -> Joined>(
            "TurnsCtx::keep_checkpoint",
            Rc::new(move |_e: &Entry, ask: CheckpointAsk| {
                a.borrow_mut().push(ask);
                done()
            }),
        );
        let turn = turn_row(json!({ "claudeSessionId": "agent-1" }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        r.send(session_end());
        settle().await;
        assert_eq!(
            asks.borrow().clone(),
            vec![CheckpointAsk { session_id: "agent-1".into(), thread_id: THREAD.into(), turn_id: TURN.into(), anchor: None, kept: None }],
            "the agent's session id, else the row's"
        );
        assert_eq!(host.recorded_of("session.context"), Vec::<Value>::new());
    });
}

// N3: turns.ts:118-121
#[test]
fn a_context_the_result_named_is_not_written_again() {
    run_local(async {
        let r = rig();
        r.send(json!({ "type": "turn.usage", "sessionId": SESS, "tokens": 1200, "context": 5000 }));
        r.send(done_event(json!({ "status": "completed", "text": "ok", "tokens": { "input": 10, "output": 5, "context": 5000 } })));
        r.send(session_end());
        settle().await;
        assert_eq!(r.host.recorded_of("session.context"), Vec::<Value>::new());
    });
}

// N3: a closing host runs what the turn registered
#[test]
fn a_closing_host_writes_what_a_running_turn_held() {
    run_local(async {
        let r = rig();
        r.send(json!({ "type": "turn.usage", "sessionId": SESS, "tokens": 1200, "context": 3000 }));
        let writes: Vec<Rc<dyn Fn()>> = r.host.held_at_close.borrow().clone();
        assert_eq!(writes.len(), 1);
        for w in writes {
            w();
        }
        assert_eq!(
            r.host.recorded_of("session.context"),
            vec![
                json!({ "type": "session.context", "workspaceId": W, "sessionId": SESS, "turnId": TURN, "threadId": THREAD, "context": 3000 })
            ],
            "no window key where the agent named none"
        );
    });
}

// ---- row, changes and the snapshot (N14, N15, N19, N20) ----------------------------------------------------------

// N14: turns.ts:651-657
#[test]
fn a_shared_daemon_turn_names_its_group() {
    run_local(async {
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry) -> bool>("TurnsCtx::shared_daemon", Rc::new(|_e: &Entry| true));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::with(SESS, |s| s.pid = Some(4321)), turn, |_| {});
        settle().await;
        assert_eq!(r.pid(), Some(4321));
        assert_eq!(
            host.hooks.calls(),
            vec!["idle_hold(w1)", "arm_disk_watch", "shared_daemon", "arm_roots_recheck", "port_roots_moved(w1)", "persist_sessions(w1)"]
        );
        let mut group = IndexMap::new();
        group.insert(4321i64, THREAD.to_string());
        let mut groups = IndexMap::new();
        groups.insert(W.to_string(), group.clone());
        assert_eq!(*host.turn_groups.borrow(), groups);

        let second = turn_row(json!({ "id": "turn-2", "threadId": "t2" }));
        let slot = Rc::new(RefCell::new(None));
        let session = FakeSession::with("sess-2", |s| s.pid = Some(77));
        let mut t = turn_start(&entry, &second, &session, &slot);
        t.thread_id = "t2".into();
        t.turn_id = "turn-2".into();
        TurnsArea::new(host.clone()).run_turn(t).expect("a second turn");
        group.insert(77i64, "t2".to_string());
        groups.insert(W.to_string(), group);
        assert_eq!(*host.turn_groups.borrow(), groups, "a second turn joins the workspace's group");
    });
}

// N14
#[test]
fn a_turn_on_a_private_daemon_or_with_no_pid_names_no_group() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.pid = Some(4321)), |_| {});
        settle().await;
        assert_eq!(
            r.host.hooks.calls(),
            vec!["idle_hold(w1)", "arm_disk_watch", "shared_daemon", "port_roots_moved(w1)", "persist_sessions(w1)"]
        );
        assert!(r.host.turn_groups.borrow().is_empty());

        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry) -> bool>("TurnsCtx::shared_daemon", Rc::new(|_e: &Entry| true));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        settle().await;
        assert_eq!(host.hooks.calls(), vec!["idle_hold(w1)", "arm_disk_watch", "port_roots_moved(w1)", "persist_sessions(w1)"]);
        assert!(host.turn_groups.borrow().is_empty());
    });
}

// N15, N19, N20: turn-changes.test.ts:170, 483
#[test]
fn the_snapshot_rides_the_row_until_the_changes_are_read() {
    run_local(async {
        let (host, entry) = world();
        let asks: Rc<RefCell<Vec<TurnChangesAsk>>> = Rc::default();
        let (read_tx, read_rx) = oneshot::channel::<bool>();
        changes_hook(&host, &asks, Rc::new(RefCell::new(Some(read_rx))));
        let (snap_tx, snapshot) = taking();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
            t.snapshot = Some(snapshot);
            t.turn_token = Some("tok-1".into());
        });
        settle().await;
        assert_eq!(r.snapshot(), None, "no commit on the row while the snapshot is still being taken");
        let persists = host.hooks.called("persist_sessions").len();
        snap_tx.send(Some("c1".into())).unwrap();
        settle().await;
        assert_eq!(r.snapshot().as_deref(), Some("c1"));
        assert_eq!(host.hooks.called("persist_sessions").len(), persists + 1);

        r.send(done_event(json!({ "status": "completed", "text": "ok" })));
        settle().await;
        assert_eq!(asks.borrow().clone(), vec![changes_ask("c1", T0, None)]);
        r.send(session_end());
        settle().await;
        assert_eq!(asks.borrow().len(), 1, "the changes are read once");

        finish(&r, json!({ "status": "completed", "text": "ok" }));
        settle().await;
        assert_eq!(r.snapshot().as_deref(), Some("c1"), "the commit stays while the read is out");
        assert_eq!(r.token(), None, "the token goes with the exit");
        let persists = host.hooks.called("persist_sessions").len();
        read_tx.send(false).unwrap();
        settle().await;
        assert_eq!(r.snapshot(), None, "a read that lands after the end takes the commit off");
        assert_eq!(host.hooks.called("persist_sessions").len(), persists + 1);
    });
}

// N19, N20: turn-changes.test.ts:170
#[test]
fn the_commit_comes_off_at_the_end_when_the_read_was_already_in() {
    run_local(async {
        let (host, entry) = world();
        let asks: Rc<RefCell<Vec<TurnChangesAsk>>> = Rc::default();
        changes_hook(&host, &asks, Rc::default());
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
            t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c1".into())), cwd: "/work/w1".into() })
        });
        r.send(done_event(json!({ "status": "completed" })));
        settle().await;
        assert_eq!(asks.borrow().len(), 1);
        assert_eq!(r.snapshot().as_deref(), Some("c1"), "a read that lands before the end leaves the row alone");
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(r.snapshot(), None);
    });
}

// N19: turn-changes.test.ts:483
#[test]
fn a_read_that_fails_as_the_host_closes_leaves_the_commit_on_disk() {
    run_local(async {
        for (read, closing, written) in [(false, true, false), (true, true, true), (false, false, true)] {
            let (host, entry) = world();
            let asks: Rc<RefCell<Vec<TurnChangesAsk>>> = Rc::default();
            let (read_tx, read_rx) = oneshot::channel::<bool>();
            changes_hook(&host, &asks, Rc::new(RefCell::new(Some(read_rx))));
            let turn = turn_row(json!({}));
            launching(&host, &turn);
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
                t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c1".into())), cwd: "/work/w1".into() })
            });
            r.send(done_event(json!({ "status": "completed" })));
            finish(&r, json!({ "status": "completed" }));
            settle().await;
            host.closing.set(closing);
            let persists = host.hooks.called("persist_sessions").len();
            read_tx.send(read).unwrap();
            settle().await;
            let wrote = host.hooks.called("persist_sessions").len() - persists;
            assert_eq!(wrote, usize::from(written), "read {read}, closing {closing}");
        }
    });
}

// N19
#[test]
fn no_changes_are_read_without_a_snapshot_or_where_they_were_written() {
    run_local(async {
        for (snapshot, written) in [
            (None, false),
            (Some(TurnSnapshot { from: SnapshotFrom::Known(None), cwd: "/work/w1".into() }), false),
            (Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c1".into())), cwd: "/work/w1".into() }), true),
        ] {
            let (host, entry) = world();
            let asks: Rc<RefCell<Vec<TurnChangesAsk>>> = Rc::default();
            changes_hook(&host, &asks, Rc::default());
            let turn = turn_row(json!({}));
            launching(&host, &turn);
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
                t.snapshot = snapshot;
                if written {
                    t.written = Some(TurnWritten { changes: true, ..TurnWritten::default() });
                }
            });
            r.send(done_event(json!({ "status": "completed" })));
            r.send(session_end());
            settle().await;
            assert_eq!(host.hooks.called("read_turn_changes"), Vec::<String>::new());
        }
    });
}

// N19: turns.ts:88
#[test]
fn the_changes_ask_names_the_start_and_the_edits_only_where_the_harness_reports_them() {
    run_local(async {
        for (reports, expected_wrote) in [(true, Some(BTreeSet::<String>::new())), (false, None)] {
            let (host, entry) = world();
            let asks: Rc<RefCell<Vec<TurnChangesAsk>>> = Rc::default();
            changes_hook(&host, &asks, Rc::default());
            let turn = turn_row(json!({}));
            turn.borrow_mut().view.started_at = None;
            launching(&host, &turn);
            host.clock.advance(5_000);
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
                t.reports_edits = reports;
                t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c1".into())), cwd: "/work/w1".into() });
            });
            r.send(done_event(json!({ "status": "completed" })));
            settle().await;
            assert_eq!(
                asks.borrow().clone(),
                vec![changes_ask("c1", T0 + 5_000, expected_wrote)],
                "reports edits {reports}: a view with no start reads the clock"
            );
        }
    });
}

// N15: turns.ts:659-667
#[test]
fn a_snapshot_that_answers_nothing_or_after_the_row_moved_on_writes_nothing() {
    run_local(async {
        let (host, entry) = world();
        let (tx, snapshot) = taking();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| t.snapshot = Some(snapshot));
        settle().await;
        let persists = host.hooks.called("persist_sessions").len();
        tx.send(None).unwrap();
        settle().await;
        assert_eq!((r.snapshot(), host.hooks.called("persist_sessions").len()), (None, persists));

        for stale in ["not running", "another turn"] {
            let (host, entry) = world();
            let (tx, snapshot) = taking();
            let turn = turn_row(json!({}));
            launching(&host, &turn);
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| t.snapshot = Some(snapshot));
            settle().await;
            if stale == "not running" {
                turn.borrow_mut().view.status = SessionStatus::Completed;
            } else {
                host.rows.borrow()[SESS].borrow_mut().turn_id = "turn-2".into();
            }
            let persists = host.hooks.called("persist_sessions").len();
            tx.send(Some("c1".into())).unwrap();
            settle().await;
            assert_eq!((r.snapshot(), host.hooks.called("persist_sessions").len()), (None, persists), "{stale}");
        }
    });
}

// N20: turns.ts:672-673
#[test]
fn a_row_another_turn_took_over_keeps_its_snapshot_and_token() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| {
            t.turn_token = Some("tok-1".into());
            t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c1".into())), cwd: "/work/w1".into() });
        });
        {
            let rows = host.rows.borrow();
            let mut row = rows[SESS].borrow_mut();
            row.turn_id = "turn-2".into();
            row.turn_token = Some("tok-2".into());
            row.snapshot = Some("c9".into());
        }
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!((r.token(), r.snapshot()), (Some("tok-2".to_string()), Some("c9".to_string())));
    });
}

// N21: thread-keeps-process.test.ts:92
#[test]
fn a_kept_agent_keeps_its_token_and_device() {
    run_local(async {
        let (host, entry) = world();
        let holds = hold_hook(&host, true);
        let closes = Arc::new(Mutex::new(Vec::new()));
        let turn = turn_row(json!({ "claudeSessionId": "agent-1" }));
        launching(&host, &turn);
        let waiting = Rc::new(Cell::new(false));
        let w = waiting.clone();
        let r = rig_on(&host, &entry, keeping(&closes, Some(99)), turn, |t| {
            t.keep = Some(kept_launch());
            t.turn_token = Some("tok-1".into());
            t.scope_device_id = Some("dev-1".into());
            t.waiting = Some(w);
        });
        finish(&r, json!({ "status": "completed", "text": "ok" }));
        settle().await;
        let holds = holds.borrow();
        assert_eq!(holds.len(), 1);
        let (thread, hold) = &holds[0];
        assert_eq!(thread, THREAD);
        assert_eq!(hold.workspace_id, W);
        assert_eq!(hold.launch, kept_launch());
        assert_eq!(hold.session.as_deref(), Some("agent-1"));
        assert_eq!(hold.turn_token, "tok-1");
        assert_eq!(hold.pid, Some(99));
        assert_eq!(hold.scope_device_id.as_deref(), Some("dev-1"));
        assert!(Rc::ptr_eq(&hold.waiting, &waiting));
        assert_eq!(*closes.lock().unwrap(), Vec::<bool>::new(), "a kept agent is not closed");
        assert_eq!(host.hooks.called("revoke_device"), Vec::<String>::new(), "a kept agent keeps its device");
        assert_eq!(r.token(), None, "the row no longer names the token");
    });
}

// N21: turns.ts:680-684
#[test]
fn a_kept_agent_with_no_pid_or_no_waiting_box_is_offered_with_a_fresh_box() {
    run_local(async {
        let (host, entry) = world();
        let holds = hold_hook(&host, true);
        let closes = Arc::new(Mutex::new(Vec::new()));
        let turn = turn_row(json!({ "claudeSessionId": "agent-1" }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, keeping(&closes, None), turn, |t| {
            t.keep = Some(kept_launch());
            t.turn_token = Some("tok-1".into());
        });
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        let holds = holds.borrow();
        assert_eq!(holds.len(), 1);
        assert_eq!((holds[0].1.pid, holds[0].1.scope_device_id.clone(), holds[0].1.waiting.get()), (None, None, false));
    });
}

// N21, N22: thread-keeps-process.test.ts:196
#[test]
fn an_agent_not_kept_is_closed_and_its_device_revoked() {
    run_local(async {
        // The runtime refusing the hold: the device goes with the turn.
        let (host, entry) = world();
        let holds = hold_hook(&host, false);
        let closes = Arc::new(Mutex::new(Vec::new()));
        let turn = turn_row(json!({ "claudeSessionId": "agent-1" }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, keeping(&closes, None), turn, |t| {
            t.keep = Some(kept_launch());
            t.turn_token = Some("tok-1".into());
            t.scope_device_id = Some("dev-1".into());
        });
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(holds.borrow().len(), 0, "the runtime refused the hold");
        assert_eq!(host.hooks.called("hold_kept"), vec!["hold_kept(t1)"]);
        assert_eq!(*closes.lock().unwrap(), vec![false], "the agent handed back is closed");
        assert_eq!(host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)"]);

        // A turn that cannot be kept: ended here, no token, or no agent session. The agent is closed, never offered.
        for variant in ["ended", "no token", "no session"] {
            let (host, entry) = world();
            let holds = hold_hook(&host, true);
            let closes = Arc::new(Mutex::new(Vec::new()));
            let turn = turn_row(if variant == "no session" { json!({}) } else { json!({ "claudeSessionId": "agent-1" }) });
            launching(&host, &turn);
            let r = rig_on(&host, &entry, keeping(&closes, None), turn, |t| {
                t.keep = Some(kept_launch());
                t.scope_device_id = Some("dev-1".into());
                if variant != "no token" {
                    t.turn_token = Some("tok-1".into());
                }
            });
            if variant == "ended" {
                r.end("cut", false, false);
            }
            finish(&r, json!({ "status": "completed" }));
            settle().await;
            assert_eq!(holds.borrow().len(), 0, "{variant}");
            assert_eq!(*closes.lock().unwrap(), vec![false], "{variant}: the agent is closed once");
            assert_eq!(host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)"], "{variant}");
        }
    });
}

// N21, N22
#[test]
fn a_turn_launched_with_no_keep_leaves_the_agent_alone_and_revokes_its_device() {
    run_local(async {
        let (host, entry) = world();
        let holds = hold_hook(&host, true);
        let closes = Arc::new(Mutex::new(Vec::new()));
        let turn = turn_row(json!({ "claudeSessionId": "agent-1" }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, keeping(&closes, None), turn, |t| {
            t.turn_token = Some("tok-1".into());
            t.scope_device_id = Some("dev-1".into());
        });
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(holds.borrow().len(), 0);
        assert_eq!(*closes.lock().unwrap(), Vec::<bool>::new());
        assert!(r.session.kept.lock().unwrap().is_some(), "the session was never asked for its agent");
        assert_eq!(host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)"]);
    });
}

// N22: turns.ts:687-691
#[test]
fn a_device_that_will_not_go_is_warned() {
    run_local(async {
        let lines = console::capture();
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "TurnsCtx::revoke_device",
            Rc::new(|_d: &str| async { Err(failed("the door is shut")) }.boxed_local()),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| t.scope_device_id = Some("dev-1".into()));
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(lines.lines(), vec![format!("the token of thread {} was not taken away: the door is shut", thread_word(THREAD))]);
    });
}

// N20: turns.ts:672
#[test]
fn the_turns_token_comes_off_its_row_at_the_exit_and_a_later_ones_stays() {
    run_local(async {
        let r = rig_with(FakeSession::new(SESS), |t| t.turn_token = Some("tok-1".into()));
        assert_eq!(r.token().as_deref(), Some("tok-1"));
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(r.token(), None);

        let r = rig_with(FakeSession::new(SESS), |t| t.turn_token = Some("tok-1".into()));
        r.host.rows.borrow()[SESS].borrow_mut().turn_token = Some("tok-2".into());
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(r.token().as_deref(), Some("tok-2"));
    });
}

// N23: turns.ts:693-707
#[test]
fn a_process_that_exits_with_no_result_reads_its_status_words() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        r.send(json!({ "type": "turn.delta", "sessionId": SESS, "kind": "tool_use", "text": "{}", "toolName": "Bash", "toolUseId": "toolu_1" }));
        let ask: PermissionAsk = serde_json::from_value(ask_doc("ask_1")).unwrap();
        host.lead_asks.borrow_mut().insert(THREAD.to_string(), ask);
        assert_eq!(r.calls_open(), 1);
        settle().await;
        let mark = host.hooks.calls().len();
        host.clock.advance(9_000);
        finish(&r, json!({ "status": "failed" }));
        settle().await;
        let lines = turn_lines(&result(json!({ "status": "failed" })), None);
        let mut expected = doc(SESS, json!({ "status": "failed", "endedAt": T0 + 9_000 }));
        for (k, v) in serde_json::to_value(&lines).unwrap().as_object().unwrap() {
            expected[k] = v.clone();
        }
        assert_eq!(json_of(&turn), expected);
        assert!(lines.failure.is_some());
        assert_eq!(r.calls_open(), 0, "its own calls follow nobody now");
        assert!(host.lead_asks.borrow().get(THREAD).is_none(), "nobody waits behind it any more");
        let calls = after(&host, mark);
        assert_eq!(calls.iter().filter(|c| *c == "end_snooze_for").count(), 1, "a failure ends the snooze");
        assert_eq!(calls.iter().filter(|c| *c == "port_roots_moved(w1)").count(), 1);
        assert_eq!(calls.iter().filter(|c| c.starts_with("persist_sessions")).count(), 1);
    });
}

// N23
#[test]
fn only_a_failed_exit_ends_a_snooze_and_every_status_is_the_views() {
    run_local(async {
        for status in ["completed", "interrupted", "failed"] {
            let r = rig();
            finish(&r, json!({ "status": status }));
            settle().await;
            assert_eq!(json_of(&r.turn)["status"], status);
            assert_eq!(r.host.hooks.called("end_snooze_for").len(), usize::from(status == "failed"), "{status}");
        }
    });
}

// N23
#[test]
fn a_reply_given_before_the_exit_keeps_its_lines_at_the_settle() {
    run_local(async {
        let r = rig();
        let reply = json!({ "status": "completed", "text": "Fixed it.\nAll files written." });
        r.send(done_event(reply.clone()));
        let before = json_of(&r.turn);
        finish(&r, reply.clone());
        settle().await;
        let lines = turn_lines(&result(reply), None);
        assert!(lines.last_line.is_some());
        let mut expected = before;
        expected["status"] = json!("completed");
        expected["endedAt"] = json!(T0);
        assert_eq!(json_of(&r.turn), expected, "the lines the reply wrote stay as they were");
        assert_eq!(expected["lastLine"], json!(lines.last_line.unwrap()));
    });
}

// N23
#[test]
fn an_end_time_already_set_is_not_set_again() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({ "endedAt": T0 + 1 }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        host.clock.advance(9_000);
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(json_of(&turn)["endedAt"], json!(T0 + 1));
    });
}

// N23: permission-relay.test.ts:691
#[test]
fn a_pick_landed_on_the_record_rides_the_ended_row() {
    run_local(async {
        let (host, entry) = world();
        host.add_record(THREAD, W, thread_record(json!({ "harness": "claude", "permissionMode": "dontAsk" })));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        assert_eq!(turn.borrow().view.permission_mode.as_deref(), Some("bypassPermissions"));
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(turn.borrow().view.permission_mode.as_deref(), Some("dontAsk"));

        let (host, entry) = world();
        host.add_record(THREAD, W, thread_record(json!({ "harness": "claude" })));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(turn.borrow().view.permission_mode.as_deref(), Some("bypassPermissions"), "a record with no mode changes nothing");
    });
}

// N24: turns.ts:709-723
#[test]
fn a_turns_own_end_reads_the_branch_then_the_pull_request_then_the_lead() {
    run_local(async {
        let r = rig();
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        let mut expected = chain();
        expected.push("pull_request_state");
        assert_eq!(only(&r.host, CHAIN), expected);
    });
}

// N24
#[test]
fn the_root_workspace_settles_again_when_its_pull_request_is_not_open() {
    run_local(async {
        for (state, root, settles) in [
            (Some("merged"), None, vec!["w1"]),
            (Some("open"), None, vec![]),
            (None, None, vec![]),
            (Some("closed"), Some("root-1"), vec!["w2"]),
        ] {
            let (host, entry) = world();
            host.add_entry(entry_for("w2"));
            host.add_row("root-sess", row(view("root-sess", "w2", json!({ "threadId": "root-1", "startedAt": T0 })), "turn-r"));
            host.hooks.set::<dyn Fn(&Entry) -> Option<String>>(
                "TurnsCtx::pull_request_state",
                Rc::new(
                    move |e: &Entry| if root.is_some() && id_of(e) == "w1" { Some("open".to_string()) } else { state.map(String::from) },
                ),
            );
            let settled: Rc<RefCell<Vec<String>>> = Rc::default();
            let s = settled.clone();
            host.hooks.set::<dyn Fn(&Entry) -> LocalBoxFuture<'static, Result<(), HostError>>>(
                "TurnsCtx::settle_tree",
                Rc::new(move |e: &Entry| {
                    s.borrow_mut().push(id_of(e));
                    async { Ok(()) }.boxed_local()
                }),
            );
            let extra = match root {
                Some(id) => json!({ "rootThreadId": id }),
                None => json!({}),
            };
            let turn = turn_row(extra);
            launching(&host, &turn);
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
            finish(&r, json!({ "status": "completed" }));
            settle().await;
            assert_eq!(*settled.borrow(), settles.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "pr {state:?}, root {root:?}");
        }
    });
}

// N24: turns.ts:716-722
#[test]
fn a_failed_branch_read_is_warned_and_the_rest_of_the_chain_stops() {
    run_local(async {
        let lines = console::capture();
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry, bool) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "TurnsCtx::read_checkout",
            Rc::new(|_e: &Entry, _f: bool| async { Err(failed("git is gone")) }.boxed_local()),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(
            only(&host, CHAIN),
            vec!["refresh_title(true)", "read_checkout(true)", "pull_request_state"],
            "the settle-again check still runs"
        );
        assert_eq!(lines.lines(), vec!["the branch of w1 was not read at a turn's end: git is gone".to_string()]);
    });
}

// N24: turns.ts:709
#[test]
fn a_turn_ended_here_reads_no_title_and_no_branch() {
    run_local(async {
        let r = rig();
        r.end("cut", false, false);
        finish(&r, json!({ "status": "completed", "text": "ok" }));
        settle().await;
        assert_eq!(only(&r.host, CHAIN), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("take_review"), Vec::<String>::new(), "a turn ended here takes no review");
        assert_eq!(json_of(&r.turn)["status"], "running", "the status is the cut's to set");
        assert_eq!(json_of(&r.turn)["endedAt"], json!(T0), "the end time is set all the same");
    });
}

// N25: turns.ts:724
#[test]
fn the_images_of_a_turn_are_dropped_however_it_ended() {
    run_local(async {
        for how in ["completed", "failed", "ended here"] {
            let r = rig_with(FakeSession::new(SESS), |t| t.images_dir = Some("/tmp/img-1".into()));
            if how == "ended here" {
                r.end("cut", false, false);
            }
            finish(&r, json!({ "status": if how == "failed" { "failed" } else { "completed" } }));
            settle().await;
            assert_eq!(r.host.hooks.called("drop_images"), vec!["drop_images(/tmp/img-1)"], "{how}");
        }
        let r = rig();
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(r.host.hooks.called("drop_images"), Vec::<String>::new());
    });
}

// N26: turns.ts:731-737
#[test]
fn a_completed_turn_takes_its_review() {
    run_local(async {
        let r = rig();
        finish(&r, json!({ "status": "completed", "text": "All done." }));
        settle().await;
        assert_eq!(only(&r.host, &["send_back", "take_review"]), vec!["send_back(false)", "take_review(t1, All done.)"]);

        let r = rig();
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(r.host.hooks.called("take_review"), vec!["take_review(t1, )"], "no text reads as empty");

        for status in ["failed", "interrupted"] {
            let r = rig();
            finish(&r, json!({ "status": status, "text": "x" }));
            settle().await;
            assert_eq!(r.host.hooks.called("take_review"), Vec::<String>::new(), "{status}");
        }
    });
}

// N26
#[test]
fn a_review_that_does_not_read_is_warned() {
    run_local(async {
        let lines = console::capture();
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry, &str, &str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "TurnsCtx::take_review",
            Rc::new(|_e: &Entry, _t: &str, _x: &str| async { Err(failed("the host went")) }.boxed_local()),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        finish(&r, json!({ "status": "completed", "text": "ok" }));
        settle().await;
        assert_eq!(lines.lines(), vec!["the review in w1 was not read: the host went".to_string()]);
    });
}

// N26: turns.ts:738-741
#[test]
fn a_failed_finish_settles_failed() {
    run_local(async {
        let r = rig();
        r.session.fail();
        settle().await;
        assert_eq!(json_of(&r.turn)["status"], "failed");
        assert_eq!(only(&r.host, &["end_snooze_for", "send_back", "take_review"]), vec!["end_snooze_for", "send_back(false)"]);
    });
}

// N26: turns.ts:730
#[test]
fn send_back_carries_the_unread_ids_and_whether_the_turn_was_stopped_and_only_the_turns_own_row() {
    run_local(async {
        let (host, entry) = world();
        let sent: Rc<RefCell<Vec<(Row, Vec<String>, bool)>>> = Rc::default();
        let s = sent.clone();
        host.hooks.set::<dyn Fn(&Row, Vec<String>, bool)>(
            "TurnsCtx::send_back",
            Rc::new(move |row: &Row, unread: Vec<String>, stopped: bool| s.borrow_mut().push((row.clone(), unread, stopped))),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let asked = TurnAsked { prompt: "x".into(), effort: None, typed: None, request_id: None, owed: None, awaits_prompt: None };
        turn.borrow_mut().asked = Some(asked.clone());
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| {
            t.notify = Some(vec!["thr_lead".into()]);
        });
        r.send(json!({ "type": "turn.unread", "sessionId": SESS, "ids": ["m1", "m2"] }));
        r.handle.interrupt().await;
        finish(&r, json!({ "status": "interrupted" }));
        settle().await;
        let sent = sent.borrow();
        assert_eq!(sent.len(), 1);
        let (row, unread, stopped) = &sent[0];
        assert_eq!((unread.clone(), *stopped), (vec!["m1".to_string(), "m2".to_string()], true));
        assert!(Rc::ptr_eq(row, &turn));
        assert!(Rc::ptr_eq(row, &r.row()));
        assert_eq!(row.borrow().turn_id, TURN);
        assert!(row.borrow().turn_live.is_some());
        assert_eq!(
            (row.borrow().notify.clone(), row.borrow().asked.clone()),
            (Some(vec!["thr_lead".to_string()]), Some(asked)),
            "the row is the turn's whole row"
        );
    });
}
