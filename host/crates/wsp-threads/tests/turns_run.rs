// SPDX-License-Identifier: AGPL-3.0-only
//! spec/turns_run.md. TS cases: session-row.test.ts:145, permission-relay.test.ts:179-420 and :646-700,
//! turn-changes.test.ts:170 and :483-510, thread-keeps-process.test.ts:92. The event switch is turns_forward's:
//! events here only open an ask, name an anchor, report what the agent held or give a reply.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("turns_run/world.rs");

// ---- the row (N1, N2, N11-N13) -----------------------------------------------------------------------------------

// N1, N2, N13: session-row.test.ts:145
#[test]
fn a_new_turn_holds_one_row_under_the_sessions_id_and_drops_its_turn_key() {
    run_local(async {
        let (host, entry) = world();
        host.add_row("other", row(view("other", W, json!({ "threadId": "t0" })), "turn-0"));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let asked = TurnAsked {
            prompt: "write it".into(),
            effort: Some("high".into()),
            typed: None,
            request_id: None,
            owed: None,
            awaits_prompt: None,
        };
        turn.borrow_mut().asked = Some(asked.clone());
        let session = FakeSession::with(SESS, |s| {
            s.run = Some("run-1".into());
            s.from = Some(40);
        });
        let r = rig_on(&host, &entry, session, turn.clone(), |t| {
            t.notify = Some(vec!["thr_lead".into()]);
            t.notify_by = Some(scope());
            t.notify_road = Some(WorkspaceOrigin::Paired);
            t.turn_token = Some("tok-1".into());
            t.scope_device_id = Some("dev-1".into());
            t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c0".into())), cwd: "/work/w1".into() });
        });
        settle().await;
        assert_eq!(host.rows.borrow().keys().cloned().collect::<Vec<_>>(), vec!["other".to_string(), SESS.to_string()]);
        assert!(Rc::ptr_eq(&r.row(), &turn));
        let held = turn.borrow();
        assert_eq!(held.turn_id, TURN);
        assert_eq!(held.notify, Some(vec!["thr_lead".to_string()]));
        assert_eq!(held.notify_by, Some(scope()));
        assert_eq!(held.notify_road, Some(WorkspaceOrigin::Paired));
        assert_eq!(held.turn_token.as_deref(), Some("tok-1"));
        assert_eq!(held.scope_device_id.as_deref(), Some("dev-1"));
        assert_eq!(held.run.as_deref(), Some("run-1"));
        assert_eq!(held.from, Some(40));
        assert_eq!(held.asked, Some(asked));
        assert_eq!(held.snapshot.as_deref(), Some("c0"));
        assert_eq!(held.pid, None);
        assert_eq!(held.handle.clone().expect("the handle").id(), SESS);
        assert!(held.end.is_some() && held.calls.is_some());
        assert_eq!(held.turn_live, Some(TurnLive::default()));
        assert!(held.launch.is_none() && held.replaces.is_none() && !held.unanswered && !held.queued);
        drop(held);
        assert_eq!(json_of(&turn), doc(SESS, json!({})));
        assert_eq!(
            (r.handle.id(), r.handle.workspace_id(), r.handle.turn_id(), r.handle.outcome()),
            (SESS.to_string(), W.to_string(), TURN.to_string(), SessionStartOutcome::Started)
        );
        assert_eq!(host.hooks.calls(), vec!["idle_hold(w1)", "arm_disk_watch", "port_roots_moved(w1)", "persist_sessions(w1)"]);
    });
}

// N2: turns.ts:128
#[test]
fn the_row_rides_the_live_cell_the_turn_was_handed() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let live = TurnLive { reply: Some(TurnStatus::Completed), told: Some(1), waited: num(5_000), ..TurnLive::default() };
        turn.borrow_mut().turn_live = Some(live.clone());
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        assert_eq!(r.row().borrow().turn_live, Some(live));
        r.send(ask_event("ask_1"));
        host.clock.advance(2_000);
        r.send(close_event("ask_1", "allowed"));
        assert_eq!(live_of(&turn).waited, num(7_000), "the span adds to what the cell already held");
        assert_eq!(live_of(&turn).waiting_since, None);
        assert_eq!(live_of(&turn).reply, Some(TurnStatus::Completed));
        assert_eq!(live_of(&turn).told, Some(1));
    });
}

// N11, N13: turns.ts:597, 625
#[test]
fn a_reopened_row_keeps_the_id_a_client_holds() {
    run_local(async {
        let (host, entry) = world();
        host.add_row("a", row(view("a", W, json!({ "threadId": "t0" })), "turn-0"));
        let old = row(view("client-id", W, json!({ "threadId": THREAD })), "turn-1");
        old.borrow_mut().turn_token = Some("old-token".into());
        host.add_row("client-id", old);
        host.add_row("z", row(view("z", W, json!({ "threadId": "t9" })), "turn-9"));
        let turn = turn_row(json!({ "id": "client-id" }));
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| t.written = Some(TurnWritten::default()));
        settle().await;
        assert_eq!(
            host.rows.borrow().keys().cloned().collect::<Vec<_>>(),
            vec!["a", "client-id", "z"],
            "the row replaced the old one in its place"
        );
        let held = host.rows.borrow()["client-id"].clone();
        assert!(Rc::ptr_eq(&held, &turn));
        assert_eq!(held.borrow().turn_token, None, "the old row is replaced whole");
        assert_eq!(held.borrow().turn_id, TURN);
        assert_eq!(r.handle.id(), "client-id");
        assert_eq!(json_of(&turn)["id"], "client-id");
    });
}

// N11: turns.ts:597
#[test]
fn a_view_with_no_id_or_the_turns_own_takes_the_sessions_id() {
    for id in ["", TURN] {
        run_local(async move {
            let (host, entry) = world();
            let turn = turn_row(json!({ "id": id }));
            if id == TURN {
                launching(&host, &turn);
            }
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
            assert_eq!(host.rows.borrow().keys().cloned().collect::<Vec<_>>(), vec![SESS.to_string()], "view id {id:?}");
            assert_eq!(r.handle.id(), SESS);
            assert_eq!(json_of(&turn)["id"], SESS);
        });
    }
}

// N12: turns.ts:601-608
#[test]
fn a_resumed_turn_takes_over_its_rows_opener_prompt_and_cost() {
    run_local(async {
        let (host, entry) = world();
        host.add_row(
            SESS,
            row(view(SESS, W, json!({ "threadId": THREAD, "startedBy": "person", "prompt": "the first brief", "costUsd": 0.5 })), "turn-0"),
        );
        let turn = turn_row(json!({ "startedBy": "cli", "prompt": "go on" }));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| t.resume = Some(SESS.into()));
        assert_eq!(json_of(&turn), doc(SESS, json!({ "startedBy": "person", "prompt": "the first brief", "costUsd": 0.5 })));
        assert_eq!(host.rows.borrow().keys().cloned().collect::<Vec<_>>(), vec![SESS.to_string()]);
        assert!(Rc::ptr_eq(&r.row(), &turn), "the new row took the old one's place");
    });
}

// N12
#[test]
fn a_resumed_row_that_names_nothing_leaves_the_view_alone() {
    run_local(async {
        let (host, entry) = world();
        host.add_row(SESS, row(view(SESS, W, json!({ "threadId": THREAD })), "turn-0"));
        let turn = turn_row(json!({ "startedBy": "cli", "prompt": "go on" }));
        launching(&host, &turn);
        rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| t.resume = Some(SESS.into()));
        assert_eq!(json_of(&turn), doc(SESS, json!({ "startedBy": "cli", "prompt": "go on" })));
    });
}

// N12
#[test]
fn a_turn_that_does_not_resume_takes_over_nothing() {
    run_local(async {
        let (host, entry) = world();
        host.add_row(
            SESS,
            row(view(SESS, W, json!({ "threadId": THREAD, "startedBy": "person", "prompt": "the first brief", "costUsd": 0.5 })), "turn-0"),
        );
        let turn = turn_row(json!({ "startedBy": "cli", "prompt": "go on" }));
        launching(&host, &turn);
        rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |_| {});
        assert_eq!(json_of(&turn), doc(SESS, json!({ "startedBy": "cli", "prompt": "go on" })));
    });
}

// ---- opening and the idle hold (N10) -----------------------------------------------------------------------------

// N10: turns.ts:571-580
#[test]
fn a_launch_that_throws_releases_the_idle_hold_and_fails() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let waiting = Rc::new(Cell::new(true));
        let seen: Rc<Cell<Option<bool>>> = Rc::new(Cell::new(None));
        let slot = Rc::new(RefCell::new(None));
        let mut t = turn_start(&entry, &turn, &FakeSession::new(SESS), &slot);
        t.waiting = Some(waiting.clone());
        let (s, w) = (seen.clone(), waiting.clone());
        t.open = Box::new(move |_f: OnEvent| {
            s.set(Some(w.get()));
            Err(HostError::with_kind("the launch was refused", Kind::Usage))
        });
        let err = TurnsArea::new(host.clone()).run_turn(t).err().expect("the launch fails");
        settle().await;
        assert_eq!(said(&err), ("the launch was refused".to_string(), None, Some(Kind::Usage)));
        assert_eq!(seen.get(), Some(false), "the waiting box was read again before the launch");
        assert_eq!(host.hooks.calls(), vec!["idle_hold(w1)", "idle_release(w1)"]);
        assert_eq!(host.rows.borrow().keys().cloned().collect::<Vec<_>>(), vec![TURN.to_string()]);
        assert_eq!(json_of(&turn)["id"], TURN);
    });
}

// N10: turns.ts:581-587
#[test]
fn the_hold_is_released_when_the_turn_finishes() {
    run_local(async {
        let r = rig();
        settle().await;
        finish(&r, json!({ "status": "completed", "text": "done" }));
        settle().await;
        assert_eq!(
            only(&r.host, &["idle_hold", "arm_disk_watch", "idle_release", "read_disk"]),
            vec!["idle_hold(w1)", "arm_disk_watch", "idle_release(w1)", "read_disk"]
        );
    });
}

// N10: turns.ts:585
#[test]
fn a_disk_that_does_not_read_after_the_turn_is_warned() {
    run_local(async {
        let lines = console::capture();
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&Entry) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "TurnsCtx::read_disk",
            Rc::new(|_e: &Entry| async { Err(failed("the disk is full")) }.boxed_local()),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        finish(&r, json!({ "status": "completed" }));
        settle().await;
        assert_eq!(lines.lines(), vec!["disk of w1 not read after its turn: the disk is full".to_string()]);
    });
}

// N10: turns.ts:575
#[test]
fn a_kept_process_that_still_says_waiting_reads_the_box_again() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let waiting = Rc::new(Cell::new(true));
        let w = waiting.clone();
        rig_on(&host, &entry, FakeSession::new(SESS), turn, |t| t.waiting = Some(w));
        assert!(!waiting.get());
    });
}

// ---- asks (N4-N8) ------------------------------------------------------------------------------------------------

// N4, N6, N8: permission-relay.test.ts:336, 351, 320
#[test]
fn an_open_ask_marks_the_row_asking_and_counts_the_time_on_the_person() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({ "claudeSessionId": "agent-sess" }));
        let waiting = Rc::new(Cell::new(false));
        turn.borrow_mut().turn_live = Some(TurnLive::default());
        let w = waiting.clone();
        let state = state_on(&host, &entry, &turn, |t| t.waiting = Some(w));
        state.forward(adapter_event(ask_event("ask_1")));
        settle().await;
        assert_eq!(turn.borrow().view.asking, Some(asking(&ask_doc("ask_1"))));
        let lead = host.lead_asks.borrow()[THREAD].clone();
        let mut named = ask_doc("ask_1");
        named["options"][2]["label"] = json!(permission_mode_option_label("acceptEdits"));
        assert_eq!(serde_json::to_value(&lead).unwrap(), named);
        assert!(waiting.get());
        assert_eq!((live_of(&turn).waiting_since, live_of(&turn).waited), (num(T0), num(0)));
        assert_eq!(host.hooks.called("persist_sessions").len(), 1);

        host.clock.advance(3 * MIN);
        state.forward(adapter_event(ask_event("ask_2")));
        assert_eq!(turn.borrow().view.asking, Some(asking(&ask_doc("ask_1"))), "the oldest open ask leads");
        assert_eq!(live_of(&turn).waiting_since, num(T0), "the span began at the first open");
        host.clock.advance(2 * MIN);
        state.close_ask("ask_1", PermissionOutcome::Allowed, Some("allow"));
        state.close_ask("ask_1", PermissionOutcome::Allowed, Some("allow"));
        settle().await;
        assert_eq!(turn.borrow().view.asking, Some(asking(&ask_doc("ask_2"))));
        assert_eq!(host.lead_asks.borrow()[THREAD].ask_id, "ask_2");
        assert!(waiting.get());
        assert_eq!(live_of(&turn).waiting_since, num(T0));
        assert_eq!(
            host.recorded_of("session.permission.closed"),
            vec![on_agent_session(closed_row("ask_1", "allowed", Some("allow")))],
            "a closed ask closes once"
        );

        host.clock.advance(15 * MIN);
        assert_eq!(state.waited_so_far(), (20 * MIN) as f64, "the open span counts to now");
        let reply = result(json!({ "status": "completed", "text": "ok" }));
        assert_eq!(state.with_waited(reply.clone()), result(json!({ "status": "completed", "text": "ok", "waitedMs": 20 * MIN })));

        state.close_ask("ask_2", PermissionOutcome::Cancelled, None);
        settle().await;
        assert_eq!(turn.borrow().view.asking, None);
        assert!(host.lead_asks.borrow().get(THREAD).is_none());
        assert!(!waiting.get());
        assert_eq!((live_of(&turn).waited, live_of(&turn).waiting_since), (num(20 * MIN), None));
        assert_eq!(
            host.recorded_of("session.permission.closed"),
            vec![on_agent_session(closed_row("ask_1", "allowed", Some("allow"))), on_agent_session(closed_row("ask_2", "cancelled", None))]
        );
        assert_eq!(host.hooks.called("persist_sessions").len(), 4, "one write for each open and each close");
        host.clock.advance(10 * MIN);
        assert_eq!(state.waited_so_far(), (20 * MIN) as f64, "a closed span stops counting");
    });
}

// N8
#[test]
fn a_reply_carries_waited_ms_only_where_the_turn_waited() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        let state = state_on(&host, &entry, &turn, |_| {});
        let reply = result(json!({ "status": "completed", "text": "ok" }));
        assert_eq!(state.waited_so_far(), 0.0);
        assert_eq!(state.with_waited(reply.clone()), reply);
    });
}

// N4, N8: turns.ts:176-179
#[test]
fn a_span_open_at_the_restart_is_counted_from_where_it_began() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        turn.borrow_mut().turn_live = Some(TurnLive { waited: num(1_000), waiting_since: num(T0 - 4_000), ..TurnLive::default() });
        let state = state_on(&host, &entry, &turn, |_| {});
        assert_eq!(state.waited_so_far(), 5_000.0);
        state.forward(adapter_event(ask_event("ask_1")));
        assert_eq!(live_of(&turn).waiting_since, num(T0 - 4_000), "an open span is kept, not restarted");
        host.clock.advance(1_000);
        state.close_ask("ask_1", PermissionOutcome::Denied, Some("deny"));
        assert_eq!((live_of(&turn).waited, live_of(&turn).waiting_since), (num(6_000), None));
    });
}

// N5: permission-relay.test.ts:179
#[test]
fn a_mode_option_reads_the_catalogs_label() {
    run_local(async {
        let (host, entry) = world();
        host.hooks.set::<dyn Fn(&str, &str) -> Option<String>>(
            "TurnsCtx::mode_label",
            Rc::new(|h: &str, m: &str| if (h, m) == ("claude", "acceptEdits") { Some("Accept edits".to_string()) } else { None }),
        );
        let turn = turn_row(json!({}));
        let state = state_on(&host, &entry, &turn, |_| {});
        let mut ask = ask_doc("ask_1");
        ask["options"] = json!([
            { "id": "allow", "label": "Allow", "effect": "allow" },
            { "id": "mode:acceptEdits", "label": "acceptEdits", "effect": "mode", "mode": "acceptEdits" },
            { "id": "mode:plan", "label": "plan", "effect": "mode", "mode": "plan" },
            { "id": "mode:odd", "label": "Odd", "effect": "mode" },
            { "id": "deny", "label": "Deny", "effect": "deny" },
        ]);
        let named = state.named(&serde_json::from_value(ask.clone()).expect("an ask"));
        let mut expected = ask.clone();
        expected["options"][1]["label"] = json!(permission_mode_option_label("Accept edits"));
        expected["options"][2]["label"] = json!(permission_mode_option_label("plan"));
        assert_eq!(serde_json::to_value(&named).unwrap(), expected);
        assert_eq!(host.hooks.called("mode_label"), vec!["mode_label(claude, acceptEdits)", "mode_label(claude, plan)"]);
    });
}

// N7: permission-relay.test.ts:206, 288, 303
#[test]
fn answer_allows_denies_and_carries_a_reason_only_on_a_deny() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.answers = true), |_| {});
        for id in ["ask_1", "ask_2", "ask_3", "ask_4", "ask_5"] {
            r.send(ask_event(id));
        }
        let ask = |id: &str, option: &str, reason: Option<&str>| {
            r.handle.answer(id.into(), option.into(), reason.map(String::from)).expect("an answer road")
        };
        let reason = "Run only the thread-rows file";
        for (id, option, reason) in [
            ("ask_1", "allow", None),
            ("ask_2", "deny", Some("  Run only the thread-rows file ")),
            ("ask_3", "deny", Some("   ")),
            ("ask_4", "mode:acceptEdits", None),
            ("ask_5", "allow", Some("because")),
        ] {
            assert_eq!(ask(id, option, reason).await.ok(), Some(answered(SessionAnswerOutcome::Answered)), "{id}");
        }
        assert_eq!(
            r.session.calls(),
            vec![
                format!("answer(ask_1, allow, Allowed, {}, \"\")", denied_line(None)),
                format!("answer(ask_2, deny, Denied, {}, {:?})", denied_line(Some(reason)), reason),
                format!("answer(ask_3, deny, Denied, {}, \"\")", denied_line(None)),
                format!("answer(ask_4, mode:acceptEdits, Allowed, {}, \"\")", denied_line(None)),
                format!("answer(ask_5, allow, Allowed, {}, \"\")", denied_line(None)),
            ]
        );
    });
}

// N7: permission-relay.test.ts:268, 303
#[test]
fn a_question_takes_a_pick_naming_several_and_an_answer_typed_in_other() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.answers = true), |_| {});
        r.send(question_event("ask_q"));
        r.send(question_event("ask_t"));
        let both = r.handle.answer("ask_q".into(), "q0:o0|q0:o1".into(), None).expect("a road").await;
        assert_eq!(both.ok(), Some(answered(SessionAnswerOutcome::Answered)));
        let typed = r.handle.answer("ask_t".into(), "q0:other:neither%2C%20C".into(), None).expect("a road").await;
        assert_eq!(typed.ok(), Some(answered(SessionAnswerOutcome::Answered)));
        assert_eq!(
            r.session.calls(),
            vec![
                format!("answer(ask_q, q0:o0|q0:o1, Allowed, {}, \"\")", denied_line(None)),
                format!("answer(ask_t, q0:other:neither%2C%20C, Allowed, {}, \"\")", denied_line(None)),
            ]
        );
    });
}

// N7: permission-relay.test.ts:206, 411
#[test]
fn answer_on_a_closed_ask_is_gone() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.answers = true), |_| {});
        r.send(ask_event("ask_1"));
        r.send(close_event("ask_1", "allowed"));
        for id in ["ask_1", "ask_9"] {
            let got = r.handle.answer(id.into(), "allow".into(), None).expect("a road").await;
            assert_eq!(got.ok(), Some(answered(SessionAnswerOutcome::Gone)), "{id}");
        }
        assert_eq!(r.session.calls(), Vec::<String>::new(), "nothing reaches the harness for an ask not open");
        r.send(ask_event("ask_2"));
        *r.session.answer_answer.lock().unwrap() = AnswerOutcome::Gone;
        let got = r.handle.answer("ask_2".into(), "allow".into(), None).expect("a road").await;
        assert_eq!(got.ok(), Some(answered(SessionAnswerOutcome::Gone)), "a harness that says gone reads gone");
    });
}

// N7: permission-relay.test.ts:411
#[test]
fn answer_naming_no_option_is_no_option() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.answers = true), |_| {});
        r.send(ask_event("ask_1"));
        for pick in ["mode:bypassPermissions", "allow|nope"] {
            let got = r.handle.answer("ask_1".into(), pick.into(), None).expect("a road").await;
            assert_eq!(got.ok(), Some(answered(SessionAnswerOutcome::NoOption)), "{pick}");
        }
        assert_eq!(r.session.calls(), Vec::<String>::new());
    });
}

// N7: turns.ts:271-279
#[test]
fn an_open_ask_with_no_harness_road_is_gone_after_its_option_is_picked() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        let state = state_on(&host, &entry, &turn, |_| {});
        state.forward(adapter_event(ask_event("ask_1")));
        assert_eq!(state.answer("ask_1", "nope", None).await, answered(SessionAnswerOutcome::NoOption));
        assert_eq!(state.answer("ask_1", "allow", None).await, answered(SessionAnswerOutcome::Gone));
    });
}

// N4, N7: turns.ts:271, 626
#[test]
fn an_ask_raised_while_the_session_opens_is_answered_through_its_road() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let waiting = Rc::new(Cell::new(false));
        let session = FakeSession::with(SESS, |s| s.answers = true);
        let slot = Rc::new(RefCell::new(None));
        let mut t = turn_start(&entry, &turn, &session, &slot);
        t.waiting = Some(waiting.clone());
        let s = session.clone();
        t.open = Box::new(move |f: OnEvent| {
            f(adapter_event(ask_event("ask_1")));
            Ok(s as Arc<dyn HarnessSession>)
        });
        let handle = TurnsArea::new(host.clone()).run_turn(t).expect("the turn opens");
        assert!(waiting.get(), "the ask raised in the launch holds the box");
        assert_eq!(turn.borrow().view.asking, Some(asking(&ask_doc("ask_1"))));
        let got = handle.answer("ask_1".into(), "allow".into(), None).expect("a road").await;
        assert_eq!(got.ok(), Some(answered(SessionAnswerOutcome::Answered)));
        assert_eq!(session.calls(), vec![format!("answer(ask_1, allow, Allowed, {}, \"\")", denied_line(None))]);
    });
}

// N4: turns.ts:207-209, forward's turn.tasks
#[test]
fn work_still_running_holds_the_waiting_box_like_an_open_ask() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        let waiting = Rc::new(Cell::new(false));
        let w = waiting.clone();
        let state = state_on(&host, &entry, &turn, |t| t.waiting = Some(w));
        state.forward(adapter_event(json!({ "type": "turn.tasks", "sessionId": SESS, "running": 2 })));
        assert!(waiting.get());
        state.forward(adapter_event(ask_event("ask_1")));
        state.forward(adapter_event(json!({ "type": "turn.tasks", "sessionId": SESS, "running": 0 })));
        assert!(waiting.get(), "an open ask still holds it");
        state.close_ask("ask_1", PermissionOutcome::Allowed, None);
        assert!(!waiting.get());
    });
}

// N9: turns.ts:194-205
#[test]
fn after_told_sends_nothing_twice_and_the_rest_of_a_held_end() {
    run_local(async {
        let done = result(json!({ "status": "completed", "text": "All done.", "costUsd": 0.5, "error": "late warning" }));
        let outcome = result(json!({ "status": "completed", "costUsd": 0.5 }));

        let (_h, none_told) = told_state(None);
        assert_eq!(none_told.after_told(&done, true), None, "a held end with no line told sends nothing");
        assert_eq!(none_told.after_told(&done, false), Some(done.clone()), "an unheld end with none told sends the end whole");

        let same = told(TurnStatus::Completed, Some("All done."), Some("All done."), false);
        let (_h, s) = told_state(same.clone());
        assert_eq!(s.after_told(&done, false), None, "the same body and status, nothing promised: nothing again");
        let (_h, s) = told_state(told(TurnStatus::Completed, Some("All done."), Some("All done."), true));
        assert_eq!(s.after_told(&done, false), Some(outcome.clone()), "a promised line sends the outcome alone, no text and no error");

        let (_h, s) = told_state(told(TurnStatus::Completed, Some("something else"), Some("something else"), false));
        assert_eq!(s.after_told(&done, false), Some(done.clone()), "an unheld end whose body differs goes whole");

        let stopped = result(json!({ "status": "interrupted", "text": "All done." }));
        let (_h, s) = told_state(same.clone());
        assert_eq!(s.after_told(&stopped, false), Some(result(json!({ "status": "interrupted" }))), "a new status sends the outcome alone");

        let held_end = result(json!({ "status": "completed", "text": "All done.\n\nTask a finished.\n", "costUsd": 0.5 }));
        let (_h, s) = told_state(same.clone());
        assert_eq!(
            s.after_told(&held_end, true),
            Some(result(json!({ "status": "completed", "text": "Task a finished.", "costUsd": 0.5 }))),
            "a held end sends what follows the told words"
        );
        assert_eq!(s.after_told(&done, true), None, "a held end that is only the told words, same status: nothing");
        let elsewhere = result(json!({ "status": "completed", "text": "Something unrelated." }));
        assert_eq!(s.after_told(&elsewhere, true), None, "a held end that does not start with the told words: status matches, nothing");
        let (_h, s) = told_state(told(TurnStatus::Completed, Some("All done."), None, false));
        assert_eq!(s.after_told(&held_end, true), None, "no told words, no rest");
        let (_h, s) = told_state(told(TurnStatus::Failed, Some("All done."), Some("All done."), false));
        assert_eq!(s.after_told(&done, true), Some(outcome), "a held end of another status than the told one sends the outcome");
    });
}

// ---- the handle (N16) --------------------------------------------------------------------------------------------

// N16: turns.ts:617-633
#[test]
fn the_handle_has_only_the_roads_the_session_has() {
    run_local(async {
        let plain = rig();
        assert!(plain.handle.steer("go".into(), None, vec![]).is_none());
        assert!(plain.handle.answer("a".into(), "allow".into(), None).is_none());
        assert!(plain.handle.set_access("plan".into()).is_none());
        assert!(plain.handle.stop_task("task-1".into()).is_none());
        assert_eq!(plain.handle.tells_unread(), Some(false));
        assert_eq!(plain.handle.notice(), None);

        let full = rig_with(
            FakeSession::with(SESS, |s| {
                s.steers = true;
                s.answers = true;
                s.sets_access = true;
                s.stops_tasks = true;
                s.tells_unread = Some(true);
                *s.task_answer.lock().unwrap() = TaskStop::Refused { error: "no such task".into() };
            }),
            |_| {},
        );
        let steered = full.handle.steer("go on".into(), Some("m1".into()), vec![]).expect("a steer road").await.ok();
        assert_eq!(steered, Some(SteerTaken::Accepted));
        *full.session.steer_answer.lock().unwrap() = Ok(SteerOutcome::NotRunning);
        let steered = full.handle.steer("again".into(), None, vec![]).expect("a steer road").await.ok();
        assert_eq!(steered, Some(SteerTaken::NotRunning));
        let stop = full.handle.stop_task("task-1".into()).expect("a stop road").await.ok();
        assert_eq!(stop, Some(TaskStop::Refused { error: "no such task".into() }));
        assert_eq!(full.handle.tells_unread(), Some(true));
        assert_eq!(full.session.calls(), vec!["steer(go on, Some(\"m1\"))", "steer(again, None)", "stop_task(task-1)"]);
    });
}

// N16
#[test]
fn the_handle_reads_the_row_as_it_stands_and_finishes_with_the_session() {
    run_local(async {
        let r = rig();
        let mut copy = r.handle.view();
        copy.prompt = Some("changed in the copy".into());
        assert_eq!(r.turn.borrow().view.prompt, None, "view() hands a copy");
        r.turn.borrow_mut().view.claude_session_id = Some("agent-1".into());
        assert_eq!(r.handle.view().claude_session_id.as_deref(), Some("agent-1"));
        let done = result(json!({ "status": "completed", "text": "ok" }));
        r.session.finish(done.clone());
        settle().await;
        assert_eq!(r.handle.finished().await, done);
    });
}

// N16: permission-relay.test.ts:646, 679
#[test]
fn set_access_moves_the_row_only_when_set() {
    run_local(async {
        for (outcome, moved, expected_row) in [
            (AccessOutcome::Set, AccessMove::Set, "acceptEdits"),
            (AccessOutcome::Refused, AccessMove::Refused, "bypassPermissions"),
            (AccessOutcome::Gone, AccessMove::Gone, "bypassPermissions"),
        ] {
            let r = rig_with(
                FakeSession::with(SESS, |s| {
                    s.sets_access = true;
                    *s.access_answer.lock().unwrap() = outcome;
                }),
                |_| {},
            );
            assert_eq!(r.handle.view().permission_mode.as_deref(), Some("bypassPermissions"));
            let got = r.handle.set_access("acceptEdits".into()).expect("an access road").await.ok();
            assert_eq!(got, Some(moved));
            assert_eq!(r.handle.view().permission_mode.as_deref(), Some(expected_row), "{outcome:?}");
            assert_eq!(r.session.calls(), vec!["set_access(acceptEdits)"]);
        }
    });
}

// N16: stop-past-grace.test.ts:45 (the interrupt reaching the harness; the TERM and KILL are the adapter's)
#[test]
fn interrupt_marks_the_turn_stopped_and_reaches_the_session() {
    run_local(async {
        let r = rig();
        r.handle.interrupt().await;
        assert_eq!(r.session.calls(), vec!["interrupt"]);
        finish(&r, json!({ "status": "interrupted" }));
        settle().await;
        assert_eq!(only(&r.host, &["send_back"]), vec!["send_back(true)"]);
    });
}

// ---- end (N17) ---------------------------------------------------------------------------------------------------

// N17: permission-relay.test.ts:351, 362
#[test]
fn end_closes_open_asks_settles_the_cut_and_interrupts_once() {
    run_local(async {
        let (host, entry) = world();
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let waiting = Rc::new(Cell::new(false));
        let w = waiting.clone();
        let r = rig_on(&host, &entry, FakeSession::with(SESS, |s| s.answers = true), turn.clone(), |t| {
            t.waiting = Some(w);
            t.snapshot = Some(TurnSnapshot { from: SnapshotFrom::Known(Some("c0".into())), cwd: "/work/w1".into() });
        });
        r.send(ask_event("ask_1"));
        r.send(ask_event("ask_2"));
        settle().await;
        assert_eq!(r.snapshot().as_deref(), Some("c0"));
        let mark = host.hooks.calls().len();

        r.end("the machine is paused", true, true);
        settle().await;
        assert_eq!(
            host.recorded_of("session.permission.closed"),
            vec![closed_row("ask_1", "cancelled", None), closed_row("ask_2", "cancelled", None)]
        );
        assert_eq!(turn.borrow().view.asking, None);
        assert!(host.lead_asks.borrow().get(THREAD).is_none());
        assert!(!waiting.get());
        assert_eq!(r.snapshot(), None, "the launch's commit comes off the row");
        assert_eq!(r.session.calls(), vec!["interrupt"]);
        let calls = after(&host, mark);
        assert_eq!(only(&host, &["settle_cut"]), vec!["settle_cut(the machine is paused, true, true)"]);
        assert_eq!(calls.iter().filter(|c| c.starts_with("persist_sessions")).count(), 3, "two closes and the end");
        assert_eq!(calls.iter().filter(|c| c.starts_with("slates_turn_ended")).collect::<Vec<_>>(), vec!["slates_turn_ended(t1, turn-1)"]);
        assert_eq!(calls.iter().filter(|c| *c == "keep_checkpoint").count(), 1);

        r.send(ask_event("ask_3"));
        assert_eq!(turn.borrow().view.asking, None, "once ended the turn's events are dropped");
        let mark = host.hooks.calls().len();
        r.end("again", false, false);
        settle().await;
        assert_eq!(after(&host, mark), Vec::<String>::new());
        assert_eq!(r.session.calls(), vec!["interrupt"]);

        finish(&r, json!({ "status": "interrupted" }));
        settle().await;
        assert_eq!(turn.borrow().view.status, SessionStatus::Running, "an end here leaves the status to the cut");
        assert_eq!(only(&host, &["send_back"]), vec!["send_back(true)"]);
    });
}

// N17
#[test]
fn end_after_the_turn_settled_does_nothing() {
    run_local(async {
        let r = rig();
        finish(&r, json!({ "status": "completed", "text": "ok" }));
        settle().await;
        assert_eq!(r.turn.borrow().view.status, SessionStatus::Completed);
        let mark = r.host.hooks.calls().len();
        r.end("late", true, false);
        settle().await;
        assert_eq!(after(&r.host, mark), Vec::<String>::new());
        assert_eq!(r.session.calls(), Vec::<String>::new());

        let other = rig();
        other.turn.borrow_mut().view.status = SessionStatus::Interrupted;
        let mark = other.host.hooks.calls().len();
        other.end("late", true, false);
        settle().await;
        assert_eq!(after(&other.host, mark), Vec::<String>::new(), "a view no longer running is not ended");
        assert_eq!(other.session.calls(), Vec::<String>::new());
    });
}

// N17: RUNTIME-AGENTS.md #1646
#[test]
fn end_hands_settle_cut_the_turns_own_row_fields() {
    run_local(async {
        for with_all in [true, false] {
            let (host, entry) = world();
            let cut: Rc<RefCell<Vec<Row>>> = Rc::default();
            let c = cut.clone();
            host.hooks.set::<dyn Fn(&Row, &str, bool, bool)>(
                "TurnsCtx::settle_cut",
                Rc::new(move |row: &Row, _r: &str, _s: bool, _u: bool| c.borrow_mut().push(row.clone())),
            );
            let turn = turn_row(json!({}));
            launching(&host, &turn);
            let asked =
                TurnAsked { prompt: "write it".into(), effort: None, typed: None, request_id: None, owed: None, awaits_prompt: None };
            if with_all {
                turn.borrow_mut().asked = Some(asked.clone());
            }
            let r = rig_on(&host, &entry, FakeSession::new(SESS), turn.clone(), |t| {
                if with_all {
                    t.notify = Some(vec!["thr_lead".into()]);
                    t.notify_by = Some(scope());
                    t.notify_road = Some(WorkspaceOrigin::Relayed);
                }
            });
            r.end("cut", false, false);
            let rows = cut.borrow();
            assert_eq!(rows.len(), 1);
            assert!(Rc::ptr_eq(&rows[0], &turn));
            let row = rows[0].borrow();
            assert_eq!(row.turn_id, TURN);
            assert!(row.turn_live.is_some(), "the turn's live state");
            if with_all {
                assert_eq!(row.notify, Some(vec!["thr_lead".to_string()]));
                assert_eq!(row.notify_by, Some(scope()));
                assert_eq!(row.notify_road, Some(WorkspaceOrigin::Relayed));
                assert_eq!(row.asked, Some(asked));
            } else {
                assert_eq!((row.notify.clone(), row.notify_by.clone(), row.notify_road), (None, None, None));
                assert!(row.asked.is_none());
            }
            assert_eq!(host.hooks.called("settle_cut"), vec!["settle_cut(cut, false, false)"]);
        }
    });
}

// N18
#[test]
fn the_turns_end_drops_every_undo_on_its_workspace_and_says_so() {
    run_local(async {
        let (host, entry) = world();
        host.add_record("t1", W, thread_record(rewound()));
        host.add_record("t3", "w2", thread_record(rewound()));
        host.add_record("t4", W, thread_record(json!({ "harness": "claude" })));
        host.add_record("t2", W, thread_record(rewound()));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        settle().await;
        let mark = host.hooks.calls().len();
        r.send(session_end());
        settle().await;
        let rewound_of = |t: &str| host.records.borrow()[t].rewound.clone();
        assert_eq!((rewound_of("t1"), rewound_of("t2"), rewound_of("t4")), (None, None, None));
        assert!(rewound_of("t3").is_some(), "another workspace's undo stays");
        assert_eq!(host.emitted.borrow().clone(), vec![json!({ "type": "thread.marked", "workspaceId": W, "threadIds": ["t1", "t2"] })]);
        assert_eq!(after(&host, mark).iter().filter(|c| c.starts_with("persist_sessions")).count(), 1);

        let (host, entry) = world();
        host.add_record("t4", W, thread_record(json!({ "harness": "claude" })));
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        settle().await;
        let mark = host.hooks.calls().len();
        r.send(session_end());
        settle().await;
        assert_eq!(host.emitted.borrow().clone(), Vec::<Value>::new());
        assert_eq!(after(&host, mark).iter().filter(|c| c.starts_with("persist_sessions")).count(), 0);
    });
}

// N18: runs once however many ends
#[test]
fn the_turn_is_over_once() {
    run_local(async {
        let r = rig();
        r.send(session_end());
        settle().await;
        let mark = r.host.hooks.calls().len();
        r.end("late", false, false);
        r.send(session_end());
        settle().await;
        assert_eq!(after(&r.host, mark).iter().filter(|c| c.starts_with("slates_turn_ended") || *c == "keep_checkpoint").count(), 0);
        assert_eq!(r.host.hooks.called("slates_turn_ended"), vec!["slates_turn_ended(t1, turn-1)"]);
    });
}

// N18, N3: turns.ts:132-154, 118
#[test]
fn the_turns_end_keeps_a_checkpoint_and_writes_what_the_agent_held() {
    run_local(async {
        let (host, entry) = world();
        let asks: Rc<RefCell<Vec<CheckpointAsk>>> = Rc::default();
        let (tx, rx) = oneshot::channel::<()>();
        let landing: Joined = rx.map(|_| ()).boxed_local().shared();
        let (a, l) = (asks.clone(), landing.clone());
        host.hooks.set::<dyn Fn(&Entry, CheckpointAsk) -> Joined>(
            "TurnsCtx::keep_checkpoint",
            Rc::new(move |_e: &Entry, ask: CheckpointAsk| {
                a.borrow_mut().push(ask);
                l.clone()
            }),
        );
        let turn = turn_row(json!({}));
        launching(&host, &turn);
        let r = rig_on(&host, &entry, FakeSession::new(SESS), turn, |_| {});
        assert_eq!(host.held_at_close.borrow().len(), 1, "the turn holds a close-time write while it runs");
        r.send(json!({ "type": "turn.usage", "sessionId": SESS, "tokens": 1200, "context": 5000, "window": 200000 }));
        r.send(json!({ "type": "turn.anchor", "sessionId": SESS, "anchor": "a-1", "kept": "cannot cut" }));
        assert_eq!(host.recorded_of("session.context"), Vec::<Value>::new(), "nothing is written while the turn runs");
        r.send(session_end());
        settle().await;
        assert_eq!(
            asks.borrow().clone(),
            vec![CheckpointAsk {
                session_id: SESS.into(),
                thread_id: THREAD.into(),
                turn_id: TURN.into(),
                anchor: Some("a-1".into()),
                kept: Some("cannot cut".into())
            }]
        );
        assert_eq!(
            host.recorded_of("session.context"),
            vec![
                json!({ "type": "session.context", "workspaceId": W, "sessionId": SESS, "turnId": TURN, "threadId": THREAD, "context": 5000, "window": 200000 })
            ]
        );
        assert_eq!(host.held_at_close.borrow().len(), 0);
        assert!(host.checkpoints_landing.borrow().contains_key(THREAD), "the thread's landing checkpoint is set");
        tx.send(()).unwrap();
        settle().await;
        assert!(!host.checkpoints_landing.borrow().contains_key(THREAD), "and cleared when it lands");
    });
}
