// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_stop.md. TS cases: owed-box.test.ts, box-stop.test.ts, stop-past-grace.test.ts, thread-cap.test.ts,
//! sub-queue.test.ts, subagents.test.ts (task stops), permission-relay.test.ts (a caller the stop cannot reach).
//!
//! Not tested here: K5 (a send waiting on an owed end is given up) and the wake of sends behind a running turn
//! (K1 with a box). Both read state only the launch road writes (`held_sends`, `behind_turn`, `waits_for`), and
//! `SessionsArea::stops` is `pub(crate)`, so a test outside the crate cannot set or read it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_stop/world.rs");

// ---- holds -----------------------------------------------------------------------------------------------------

// K1: box-stop.test.ts:304 (a second Stop keeps the first Stop's hold)
#[test]
fn a_second_hold_joins_the_first() {
    run_local(async {
        let s = scene();
        let (g1, open1) = gate();
        let (g2, open2) = gate();
        assert!(!s.area.stop_holds(THREAD));
        s.area.hold_next(THREAD, g1, None);
        assert!(s.area.stop_holds(THREAD));
        s.area.hold_next(THREAD, g2, Some("hetzner"));
        open2();
        settle().await;
        assert!(s.area.stop_holds(THREAD), "the second gate settling alone does not release the first's hold");
        open1();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K1
#[test]
fn a_first_hold_settling_alone_keeps_the_second() {
    run_local(async {
        let s = scene();
        let (g1, open1) = gate();
        let (g2, open2) = gate();
        s.area.hold_next(THREAD, g1, None);
        s.area.hold_next(THREAD, g2, None);
        open1();
        settle().await;
        assert!(s.area.stop_holds(THREAD), "the thread's gate is the second, joined with the first");
        open2();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K1
#[test]
fn the_hold_is_forgotten_when_its_gate_settles() {
    run_local(async {
        let s = scene();
        let (g, open) = gate();
        s.area.hold_next(THREAD, g, Some("hetzner"));
        settle().await;
        assert!(s.area.stop_holds(THREAD));
        assert!(!s.area.stop_holds("thr-other"), "a hold is by thread");
        open();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// ---- does the computer hear the stop -----------------------------------------------------------------------------

// K2: box-stop.test.ts:112
#[test]
fn a_computer_away_does_not_hear() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        assert!(s.area.stop_unheard("place-1", None).await);
        assert_eq!(s.host.hooks.called("ping"), Vec::<String>::new(), "nothing is asked of a computer known away");
        assert_eq!(s.host.clock.pending(), 0);
    });
}

// K2
#[test]
fn a_computer_with_no_link_does_not_hear() {
    run_local(async {
        let s = scene();
        set_link(&s, false);
        set_away(&s, Rc::new(Cell::new(false)));
        assert!(s.area.stop_unheard("place-1", None).await);
        assert_eq!(s.host.hooks.called("ping"), Vec::<String>::new());
        assert_eq!(s.host.clock.pending(), 0);
    });
}

// K2: box-stop.test.ts:185
#[test]
fn a_ping_answered_with_no_agent_stop_hears() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        let (_absent, offs) = absent_listener(&s);
        let answer = ping_by_hand(&s);
        let said = spawn({
            let area = s.area.clone();
            async move { area.stop_unheard("place-1", None).await }
        });
        settle().await;
        assert_eq!(*said.borrow(), None, "waiting on the ping");
        answer(Ok(()));
        settle().await;
        assert_eq!(*said.borrow(), Some(false));
        assert_eq!(offs.get(), 1, "the listener is taken off");
        assert_eq!(s.host.clock.pending(), 0, "the quiet timer is cleared");
        assert_eq!(s.host.hooks.called("ping"), vec!["ping(place-1)".to_string()]);
    });
}

// K2: box-stop.test.ts:197
#[test]
fn no_answer_in_stop_reach_ms_does_not_hear() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        let (_absent, offs) = absent_listener(&s);
        let _answer = ping_by_hand(&s);
        let said = spawn({
            let area = s.area.clone();
            async move { area.stop_unheard("place-1", None).await }
        });
        settle().await;
        s.host.clock.advance(STOP_REACH_MS - 1);
        settle().await;
        assert_eq!(*said.borrow(), None, "not before the time is up");
        s.host.clock.advance(1);
        settle().await;
        assert_eq!(*said.borrow(), Some(true));
        assert_eq!(offs.get(), 1);
        assert_eq!(s.host.clock.pending(), 0);
    });
}

// K2
#[test]
fn an_absent_event_does_not_hear() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        let (absent, offs) = absent_listener(&s);
        let answer = ping_by_hand(&s);
        let said = spawn({
            let area = s.area.clone();
            async move { area.stop_unheard("place-1", None).await }
        });
        settle().await;
        assert_eq!(*said.borrow(), None);
        let f = absent.borrow().clone().expect("the host listens for the place going absent");
        f();
        settle().await;
        assert_eq!(*said.borrow(), Some(true));
        assert_eq!(offs.get(), 1);
        assert_eq!(s.host.clock.pending(), 0);
        answer(Ok(()));
        settle().await;
        assert_eq!(*said.borrow(), Some(true), "it settles once: a later ping answer changes nothing");
    });
}

// K2
#[test]
fn a_failed_ping_does_not_hear() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        let (_absent, offs) = absent_listener(&s);
        let answer = ping_by_hand(&s);
        let said = spawn({
            let area = s.area.clone();
            async move { area.stop_unheard("place-1", None).await }
        });
        settle().await;
        answer(Err(fail("the link closed")));
        settle().await;
        assert_eq!(*said.borrow(), Some(true));
        assert_eq!(offs.get(), 1);
        assert_eq!(s.host.clock.pending(), 0);
    });
}

// K2: box-stop.test.ts:185 (the agent's stop lands first, so the computer was heard)
#[test]
fn an_agent_stop_that_settles_first_decides_by_whether_the_place_is_away() {
    run_local(async {
        for goes_away in [false, true] {
            let s = scene();
            set_link(&s, true);
            let away = Rc::new(Cell::new(false));
            set_away(&s, away.clone());
            let (_absent, offs) = absent_listener(&s);
            let answer = ping_by_hand(&s);
            let (stopping, stopped) = gate();
            let said = spawn({
                let area = s.area.clone();
                async move { area.stop_unheard("place-1", Some(stopping)).await }
            });
            settle().await;
            answer(Ok(()));
            settle().await;
            assert_eq!(*said.borrow(), None, "an answered ping with an agent stop out settles nothing");
            s.host.clock.advance(STOP_REACH_MS * 10);
            settle().await;
            assert_eq!(*said.borrow(), None, "an answered ping clears the quiet timer");
            away.set(goes_away);
            stopped();
            settle().await;
            assert_eq!(*said.borrow(), Some(goes_away));
            assert_eq!(offs.get(), 1);
        }
    });
}

// K2
#[test]
fn an_unanswered_ping_and_an_agent_stop_that_lands_first_was_heard() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        let (_absent, _offs) = absent_listener(&s);
        let _answer = ping_by_hand(&s);
        let (stopping, stopped) = gate();
        let said = spawn({
            let area = s.area.clone();
            async move { area.stop_unheard("place-1", Some(stopping)).await }
        });
        settle().await;
        stopped();
        settle().await;
        assert_eq!(*said.borrow(), Some(false));
        assert_eq!(s.host.clock.pending(), 0);
    });
}

// ---- finding the row ----------------------------------------------------------------------------------------------

// K3: box-stop.test.ts:102
#[test]
fn a_row_nobody_holds_is_not_found() {
    run_local(async {
        let s = scene();
        let r = s.area.interrupt_row("sess_nobody", None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-found" }));
        assert_eq!(s.host.hooks.called("entry_of_row"), Vec::<String>::new());
        assert_eq!(*s.log.borrow(), strs(&["cap_stopping(sess_nobody)", "release"]));
    });
}

// K3: thread-cap.test.ts:467 (a held turn is named by its turn id, its row under its launch's id)
#[test]
fn a_row_is_found_by_its_turn_id() {
    run_local(async {
        let s = scene();
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(TURN, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(h.calls(), strs(&["interrupt"]));
        assert!(s.log.borrow().contains(&"try_stopped(turn-1)".to_string()));
    });
}

// K4: permission-relay.test.ts:434
#[test]
fn a_row_the_caller_cannot_reach_reads_not_found() {
    run_local(async {
        let s = scene();
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let outside = Caller::Scoped {
            origin: WorkspaceOrigin::Here,
            by: serde_json::from_value(json!({ "kind": "thread", "threadId": "beside", "workspaceId": "w1", "rootThreadId": "beside" }))
                .expect("a scope"),
        };
        let seen: Rc<RefCell<Vec<Option<Caller>>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = seen.clone();
        let ats: Rc<RefCell<Vec<RowAt>>> = Rc::new(RefCell::new(Vec::new()));
        let at_sink = ats.clone();
        s.host
            .hooks
            .set::<dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>>(
                "SessionsCtx::entry_of_row",
                Rc::new(move |at: &RowAt, o: Option<&Caller>, _talk: Option<TreeTalk>, _b: bool| {
                    at_sink.borrow_mut().push(at.clone());
                    sink.borrow_mut().push(o.cloned());
                    async move { Ok(None) }.boxed_local()
                }),
            );
        let r = s.area.interrupt_row(KEY, Some(outside.clone()), None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-found" }), "the same answer as a row nobody holds");
        assert_eq!(*seen.borrow(), vec![Some(outside)]);
        assert_eq!(*ats.borrow(), vec![RowAt { thread_id: Some(THREAD.to_string()), workspace_id: "w1".to_string() }]);
        assert_eq!(s.host.hooks.called("entry_of_row"), vec![format!("entry_of_row(Some(\"{THREAD}\"), w1, false)")]);
        assert_eq!(h.calls(), Vec::<String>::new(), "the agent hears nothing");
        assert_eq!(*s.log.borrow(), strs(&["cap_stopping(sess-1)", "release"]));
    });
}

// K3, K9: session-row.test.ts (a thread listed off its record), thread-cap.test.ts:467
#[test]
fn a_trimmed_thread_is_stopped_by_its_record() {
    run_local(async {
        let s = scene();
        s.host.add_record(THREAD, "w1", thread_record(json!({ "harness": "claude", "ended": "completed" })));
        set_under(&s, vec!["child-thr".to_string()]);
        let ats: Rc<RefCell<Vec<RowAt>>> = Rc::new(RefCell::new(Vec::new()));
        let at_sink = ats.clone();
        let host = s.host.clone();
        s.host
            .hooks
            .set::<dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>>(
                "SessionsCtx::entry_of_row",
                Rc::new(move |at: &RowAt, _o: Option<&Caller>, _talk: Option<TreeTalk>, _b: bool| {
                    at_sink.borrow_mut().push(at.clone());
                    let live = host.live(&at.workspace_id);
                    async move { Ok(live) }.boxed_local()
                }),
            );
        let r = s.area.interrupt_row(THREAD, None, None).await;
        assert_eq!(
            answered(r),
            json!({ "outcome": "not-running", "under": ["child-thr"] }),
            "a thread nothing runs, its tree stopped under it"
        );
        let asked = RowAt { thread_id: Some(THREAD.to_string()), workspace_id: "w1".to_string() };
        let calls = ats.borrow();
        assert!(!calls.is_empty() && calls.iter().all(|c| *c == asked), "the trimmed row names the thread and its workspace: {calls:?}");
        assert_eq!(
            *s.log.borrow(),
            strs(&[
                "cap_stopping(thr-ab12cd34)",
                "release",
                "drop_queued(thr-ab12cd34)",
                "stop_under(thr-ab12cd34)",
                "try_stopped(thr-ab12cd34)",
                "release",
            ]),
            "the thread id is the turn"
        );
    });
}

// K4: a trimmed thread whose workspace the caller does not reach reads the same absence
#[test]
fn a_trimmed_thread_the_caller_cannot_reach_reads_not_found() {
    run_local(async {
        let s = scene();
        s.host.add_record(THREAD, "w1", thread_record(json!({ "harness": "claude" })));
        s.host
            .hooks
            .set::<dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>>(
                "SessionsCtx::entry_of_row",
                Rc::new(|_a: &RowAt, _o: Option<&Caller>, _talk: Option<TreeTalk>, _b: bool| async move { Ok(None) }.boxed_local()),
            );
        let r = s.area.interrupt_row(THREAD, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-found" }));
        assert_eq!(*s.log.borrow(), strs(&["cap_stopping(thr-ab12cd34)", "release"]));
    });
}

// ---- one subagent -------------------------------------------------------------------------------------------------

// K6: subagents.test.ts:428
#[test]
fn a_task_stop_stops_only_the_subagent() {
    run_local(async {
        let s = scene();
        let h = handle_with_tasks(&s, Ok(TaskStop::Accepted));
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(h.calls(), strs(&["stop_task(a1)"]), "the turn itself is not stopped");
        assert_eq!(*s.log.borrow(), Vec::<String>::new(), "no cap stop, no tree, no end of the row");
        assert_eq!(s.host.rows.borrow()[KEY].borrow().view.status, SessionStatus::Running);
    });
}

// K6: subagents.test.ts:428 (the outcome as it came)
#[test]
fn a_task_stop_the_harness_answers_not_running_comes_back_as_it_came() {
    run_local(async {
        let s = scene();
        let h = handle_with_tasks(&s, Ok(TaskStop::NotRunning));
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert_eq!(h.calls(), strs(&["stop_task(a1)"]));
    });
}

// K6: subagents.test.ts:441
#[test]
fn a_refused_task_stop_comes_back_in_the_agents_words() {
    run_local(async {
        let s = scene();
        s.host.hooks.set::<dyn Fn(&str) -> String>("SessionsCtx::agent_label", Rc::new(|_h: &str| "Claude Code".to_string()));
        let h = handle_with_tasks(&s, Ok(TaskStop::Refused { error: "No task found with ID: zz9".to_string() }));
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("zz9".to_string())).await;
        assert_eq!(
            answered(r),
            json!({ "outcome": "refused", "error": task_stop_refused_line("Claude Code", "No task found with ID: zz9") })
        );
        assert_eq!(h.calls(), strs(&["stop_task(zz9)"]));
        assert_eq!(s.host.hooks.called("agent_label"), vec!["agent_label(claude)".to_string()]);
    });
}

// K6: subagents.test.ts:446
#[test]
fn a_harness_with_no_task_stop_is_unsupported() {
    run_local(async {
        let s = scene();
        s.host.hooks.set::<dyn Fn(&str) -> String>("SessionsCtx::agent_label", Rc::new(|_h: &str| "Claude Code".to_string()));
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "unsupported", "error": task_stop_unsupported_line("Claude Code") }));
        assert_eq!(h.calls(), Vec::<String>::new());
        assert_eq!(s.host.rows.borrow()[KEY].borrow().view.status, SessionStatus::Running, "the turn runs on");
    });
}

// K6: a harness that has the road and says it is unsupported
#[test]
fn a_task_stop_the_harness_calls_unsupported_is_unsupported() {
    run_local(async {
        let s = scene();
        s.host.hooks.set::<dyn Fn(&str) -> String>("SessionsCtx::agent_label", Rc::new(|_h: &str| "Claude Code".to_string()));
        let h = handle_with_tasks(&s, Ok(TaskStop::Unsupported));
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "unsupported", "error": task_stop_unsupported_line("Claude Code") }));
        assert_eq!(h.calls(), strs(&["stop_task(a1)"]));
    });
}

// K6: subagents.test.ts:456
#[test]
fn a_task_on_a_turn_not_running_is_not_running() {
    run_local(async {
        let s = scene();
        let h = handle_with_tasks(&s, Ok(TaskStop::Accepted));
        put(&s, Spec { handle: Some(h.clone()), status: "completed", ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert_eq!(h.calls(), Vec::<String>::new(), "a turn that is over has no child to stop");
    });
}

// K6: a running row with no handle has no road to its subagents
#[test]
fn a_task_on_a_row_with_no_handle_is_not_running() {
    run_local(async {
        let s = scene();
        put(&s, spec());
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert_eq!(*s.log.borrow(), Vec::<String>::new());
    });
}

// ---- held and queued turns, and the tree under -------------------------------------------------------------------

// K7: thread-cap.test.ts:445 (a stop at a held turn: marked, never launches)
#[test]
fn a_held_turn_is_marked_and_never_launches() {
    run_local(async {
        let s = scene();
        let wait = thread_wait();
        set_held(&s, "turn-held", held_waiting(wait.clone()));
        let (launch, open) = gate();
        put(&s, Spec { key: "turn-held", turn: "turn-held", launch: Some(launch), end: false, ..spec() });
        let said = spawn({
            let area = s.area.clone();
            let l = s.log.clone();
            async move { area.stop_turn(Rc::new(move || say(&l, "marked")), "turn-held", None, None).await }
        });
        settle().await;
        let line = cap_stopped_line(Some(&wait));
        assert_eq!(
            *s.log.borrow(),
            vec![
                format!("cap_stop(turn-held, {line})"),
                "marked".to_string(),
                "drop_queued(turn-held)".to_string(),
                format!("stop_under({THREAD})")
            ],
            "marked before anything waits"
        );
        assert!(said.borrow().is_none(), "the stop waits for the held turn's launch to settle");
        open();
        settle().await;
        let r = said.borrow_mut().take().expect("the stop answers once the launch settled");
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(s.log.borrow().len(), 4, "no try, no agent stop, no end of the group");
    });
}

// K7: thread-cap.test.ts:949 (a held thread stopped before its first look has no wait to say)
#[test]
fn a_held_turn_that_has_not_looked_is_marked_with_no_wait() {
    run_local(async {
        let s = scene();
        set_held(&s, "turn-held", held());
        let (launch, open) = gate();
        put(&s, Spec { key: "turn-held", turn: "turn-held", launch: Some(launch), end: false, ..spec() });
        open();
        let r = s.area.stop_turn(Rc::new(|| {}), "turn-held", None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(s.log.borrow()[0], format!("cap_stop(turn-held, {})", cap_stopped_line(None)));
    });
}

// K8: thread-cap.test.ts:922 (a lead stopped while its detached child is held)
#[test]
fn a_stop_on_a_held_turn_takes_the_tree_under_it() {
    run_local(async {
        let s = scene();
        set_held(&s, "turn-held", held());
        set_under(&s, vec!["child-thr".to_string()]);
        let (launch, open) = gate();
        put(&s, Spec { key: "turn-held", turn: "turn-held", launch: Some(launch), end: false, ..spec() });
        open();
        let r = s.area.stop_turn(Rc::new(|| {}), "turn-held", None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted", "under": ["child-thr"] }));
    });
}

// K7: sub-queue.test.ts:285 (a turn queued on its machine is dropped)
#[test]
fn a_queued_turn_is_dropped() {
    run_local(async {
        let s = scene();
        set_queued(&s, true);
        put(&s, Spec { end: false, ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(
            *s.log.borrow(),
            strs(&["cap_stopping(sess-1)", "release", "drop_queued(turn-1)", "release"]),
            "nothing under it, no try, no stop of an agent"
        );
    });
}

// K8: sub-queue.test.ts:285
#[test]
fn a_stop_takes_the_tree_under_first() {
    run_local(async {
        let s = scene();
        set_under(&s, vec!["sub-thr".to_string()]);
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted", "under": ["sub-thr"] }));
        let log = s.log.borrow().clone();
        let at = |what: &str| log.iter().position(|l| l == what).unwrap_or_else(|| panic!("{what} is in {log:?}"));
        assert!(at("stop_under(thr-ab12cd34)") < at("try_stopped(turn-1)"));
        assert!(at("stop_under(thr-ab12cd34)") < at("interrupt"), "the children go first");
    });
}

// K8: thread-cap.test.ts:922 (the lead itself may already be over)
#[test]
fn the_tree_under_rides_an_answer_for_a_turn_that_is_over() {
    run_local(async {
        let s = scene();
        set_under(&s, vec!["sub-thr".to_string()]);
        put(&s, Spec { status: "completed", ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running", "under": ["sub-thr"] }));
    });
}

// ---- ending the turn and its group --------------------------------------------------------------------------------

// K9, K13, K17: stop-past-grace.test.ts:96 (accepted, nothing left)
#[test]
fn a_stop_with_no_group_to_end_stops_the_turn_alone() {
    run_local(async {
        let s = scene();
        let h = handle(&s, true, false);
        probe_holds(&h, &s.area);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }), "no `left` where the kind ends no group");
        assert_eq!(
            *s.log.borrow(),
            strs(&[
                "cap_stopping(sess-1)",
                "release",
                "drop_queued(turn-1)",
                "stop_under(thr-ab12cd34)",
                "try_stopped(turn-1)",
                "interrupt(holds=false)",
                "release",
            ])
        );
        assert!(!s.area.stop_holds(THREAD));
    });
}
