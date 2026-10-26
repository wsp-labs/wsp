// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `sessions_stop.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_stop/world.rs");

// K11, K13, K16: stop-past-grace.test.ts:96, box-stop.test.ts:185
#[test]
fn a_stop_ends_the_turn_then_its_group() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("a process this thread started still runs".to_string())));
        let h = handle(&s, false, false);
        probe_holds(&h, &s.area);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let said = spawn({
            let area = s.area.clone();
            async move { area.interrupt_row(KEY, None, None).await }
        });
        settle().await;
        assert_eq!(
            *s.log.borrow(),
            strs(&[
                "cap_stopping(sess-1)",
                "release",
                "drop_queued(turn-1)",
                "stop_under(thr-ab12cd34)",
                "try_stopped(turn-1)",
                "interrupt(holds=true)",
            ]),
            "the next turn is held before the agent is stopped, and the group is not ended while the turn runs"
        );
        assert!(said.borrow().is_none(), "the stop waits for the harness to finish");
        assert!(s.area.stop_holds(THREAD));
        h.finish();
        settle().await;
        let r = said.borrow_mut().take().expect("the stop answers once the turn finished");
        assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "a process this thread started still runs" }));
        assert_eq!(s.log.borrow()[6..].to_vec(), strs(&["end_thread(thr-ab12cd34, remove=false, away=false)", "release"]),);
        assert!(!s.area.stop_holds(THREAD), "the hold is released once the end is over");
    });
}

// K16, K18: box-stop.test.ts:185 (the hold is released however the end went)
#[test]
fn a_failed_group_end_still_releases_the_hold() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Err(fail("the daemon would not answer")));
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        let e = r.expect_err("the end failed, so the stop fails");
        assert_eq!(e.message, "the daemon would not answer");
        assert!(!s.area.stop_holds(THREAD));
        assert_eq!(s.log.borrow().last(), Some(&"release".to_string()));
    });
}

// K12: box-stop.test.ts:102 (a second Stop on the stopped turn)
#[test]
fn a_row_not_running_is_not_running() {
    run_local(async {
        let s = scene();
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), status: "completed", ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert_eq!(h.calls(), Vec::<String>::new());
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K12
#[test]
fn a_running_row_with_no_road_to_its_process_is_not_running() {
    run_local(async {
        let s = scene();
        let r = put(&s, Spec { end: false, ..spec() });
        assert!(r.borrow().handle.is_none());
        let said = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(said), json!({ "outcome": "not-running" }));
        assert!(!s.log.borrow().iter().any(|l| l.starts_with("end(")), "nothing ended the row");
    });
}

// K12: owed-box.test.ts:41 (a line's try has its own hour)
#[test]
fn a_lines_try_is_not_an_unanswered_row_to_stop_here() {
    run_local(async {
        let s = scene();
        s.host.hooks.set::<dyn Fn(&Row) -> bool>("SessionsCtx::line_try", Rc::new(|_r: &Row| true));
        put(&s, Spec { unanswered: true, ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert!(!s.log.borrow().iter().any(|l| l.starts_with("end(")));
    });
}

// K12, K16: box-stop.test.ts:185 (a thread whose turn is over still has its group)
#[test]
fn a_finished_thread_has_its_group_ended() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("a process this thread started still runs".to_string())));
        put(&s, Spec { status: "completed", ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running", "left": "a process this thread started still runs" }));
        assert_eq!(s.log.borrow()[5..].to_vec(), strs(&["end_thread(thr-ab12cd34, remove=false, away=false)", "release"]));
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K12: a running row with no handle and a group end but no end of its own reads the same
#[test]
fn a_running_row_with_no_handle_and_no_end_has_its_group_ended() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("left words".to_string())));
        put(&s, Spec { end: false, ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running", "left": "left words" }));
    });
}

// K12
#[test]
fn another_turn_running_is_not_what_was_stopped() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("must not be said".to_string())));
        put(&s, Spec { status: "completed", ..spec() });
        let other = handle(&s, true, false);
        put(&s, Spec { key: "sess-2", turn: "turn-2", handle: Some(other.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert!(!s.log.borrow().iter().any(|l| l.starts_with("end_thread")), "the group is not ended under the turn that runs");
        assert_eq!(other.calls(), Vec::<String>::new());
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K12
#[test]
fn another_turn_launching_is_not_what_was_stopped_either() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("must not be said".to_string())));
        put(&s, Spec { status: "completed", ..spec() });
        let (launch, _open) = gate();
        put(&s, Spec { key: "sess-2", turn: "turn-2", launch: Some(launch), end: false, ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert!(!s.log.borrow().iter().any(|l| l.starts_with("end_thread")));
    });
}

// K10: sessions.ts:155 (a turn let through and still reaching its machine)
#[test]
fn a_turn_still_launching_is_stopped_once_it_is_there() {
    run_local(async {
        let s = scene();
        let (launch, open) = gate();
        let launching = put(&s, Spec { launch: Some(launch), ..spec() });
        let h = handle(&s, true, false);
        let rows = s.host.rows.clone();
        let said = spawn({
            let area = s.area.clone();
            async move { area.end_turn(launching, None, Rc::new(move || rows.borrow().get(KEY).cloned())).await }
        });
        settle().await;
        assert!(said.borrow().is_none());
        assert_eq!(h.calls(), Vec::<String>::new(), "nothing to stop until the launch settles");
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        open();
        settle().await;
        let ended = said.borrow_mut().take().expect("the end answers once the launch settled").expect("an end");
        assert_eq!(ended, Ended { outcome: SessionInterruptOutcome::Accepted, left: None });
        assert_eq!(h.calls(), strs(&["interrupt"]), "the row is read again and its handle stopped");
    });
}

// ---- no agent stop ------------------------------------------------------------------------------------------------

// K15: sessions.ts:181 (a row a restart left running, its machine silent)
#[test]
fn no_agent_stop_and_no_group_ends_the_row_stopped_and_unreached() {
    run_local(async {
        let s = scene();
        put(&s, Spec { unanswered: true, ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert!(s.log.borrow().contains(&format!("end({TURN_STOPPED_LINE}, Some(true), Some(true))")), "{:?}", s.log.borrow());
    });
}

// K15
#[test]
fn no_agent_stop_with_a_group_ends_the_row_stopped_and_answers_the_groups_words() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("left words".to_string())));
        put(&s, spec());
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "left words" }));
        let log = s.log.borrow().clone();
        let end = log
            .iter()
            .position(|l| *l == format!("end({TURN_STOPPED_LINE}, Some(true), None)"))
            .unwrap_or_else(|| panic!("the row ends stopped, not unreached: {log:?}"));
        let group = log.iter().position(|l| l.starts_with("end_thread")).expect("the group is ended");
        assert!(end < group, "the row first, then the group");
        assert!(!s.area.stop_holds(THREAD));
    });
}

// ---- a computer that does not hear --------------------------------------------------------------------------------

// K14: box-stop.test.ts:120, owed-box.test.ts:41
#[test]
fn a_stop_on_a_box_away_owes_the_end_and_holds_the_next_turn() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        group_end(&s, |o| Ok(Some(if o.away { "the box is away".to_string() } else { "the box answered".to_string() })));
        let (paid, pay) = gate();
        owes(&s, Some(Some(paid)));
        let h = handle(&s, false, true);
        probe_holds(&h, &s.area);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "the box is away" }));
        assert_eq!(
            *s.log.borrow(),
            strs(&[
                "cap_stopping(sess-1)",
                "release",
                "drop_queued(turn-1)",
                "stop_under(thr-ab12cd34)",
                "try_stopped(turn-1)",
                "interrupt(holds=true)",
                "end_thread(thr-ab12cd34, remove=false, away=true)",
                "end(the box is away, Some(true), Some(true))",
                "end_owed(thr-ab12cd34)",
                "release",
            ]),
            "the agent is still asked to stop, the group is ended away, the row settles, then the owed end is read"
        );
        assert_eq!(s.host.rows.borrow()[KEY].borrow().view.status, SessionStatus::Interrupted);
        assert_eq!(s.host.hooks.called("place_name_of"), vec!["place_name_of(place-1)".to_string()], "the hold names the computer");
        assert!(s.area.stop_holds(THREAD), "the next turn waits for the end the computer owes");
        pay();
        settle().await;
        assert!(!s.area.stop_holds(THREAD), "and goes once it is paid");
    });
}

// K14: box-stop.test.ts:225 (a second Stop keeps the first Stop's hold)
#[test]
fn a_second_stop_on_the_stopped_turn_keeps_the_owed_hold() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        group_end(&s, |o| Ok(if o.away { Some("the box is away".to_string()) } else { None }));
        let (paid, pay) = gate();
        owes(&s, Some(Some(paid)));
        let h = handle(&s, false, true);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let first = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(first), json!({ "outcome": "accepted", "left": "the box is away" }));
        let second = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(second), json!({ "outcome": "not-running" }));
        assert_eq!(h.calls(), strs(&["interrupt"]), "the second stop asks the agent nothing");
        assert!(s.area.stop_holds(THREAD), "the first stop's hold stands through the second");
        pay();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K14: a kind that owes nothing, or has no owed end, holds nothing past the stop
#[test]
fn an_away_stop_where_nothing_is_owed_holds_nothing() {
    run_local(async {
        for owed in [None, Some(None)] {
            let s = scene();
            on_box(&s);
            set_link(&s, true);
            set_away(&s, Rc::new(Cell::new(true)));
            group_end(&s, |_o| Ok(Some("the box is away".to_string())));
            owes(&s, owed);
            let h = handle(&s, false, true);
            put(&s, Spec { handle: Some(h), ..spec() });
            let r = s.area.interrupt_row(KEY, None, None).await;
            assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "the box is away" }));
            assert!(!s.area.stop_holds(THREAD));
        }
    });
}

// K14: no words from the away end read as stopped and unreached
#[test]
fn an_away_end_with_no_words_ends_the_row_stopped_and_unreached() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        group_end(&s, |_o| Ok(None));
        owes(&s, Some(None));
        let h = handle(&s, false, true);
        put(&s, Spec { handle: Some(h), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert!(s.log.borrow().contains(&format!("end({TURN_STOPPED_LINE}, Some(true), Some(true))")), "{:?}", s.log.borrow());
    });
}

// K14, K2: box-stop.test.ts:197 (the box stops answering with its socket still open)
#[test]
fn a_computer_that_stops_answering_takes_the_away_road() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        group_end(&s, |o| Ok(Some(if o.away { "the box is slow".to_string() } else { "the box answered".to_string() })));
        let (paid, pay) = gate();
        owes(&s, Some(Some(paid)));
        let _answer = ping_by_hand(&s);
        let h = handle(&s, false, true);
        put(&s, Spec { handle: Some(h), ..spec() });
        let said = spawn({
            let area = s.area.clone();
            async move { area.interrupt_row(KEY, None, None).await }
        });
        settle().await;
        assert!(said.borrow().is_none(), "the stop waits for the computer for STOP_REACH_MS");
        s.host.clock.advance(STOP_REACH_MS);
        settle().await;
        let r = said.borrow_mut().take().expect("the stop answers once the time is up");
        assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "the box is slow" }));
        assert_eq!(s.host.rows.borrow()[KEY].borrow().view.status, SessionStatus::Interrupted);
        assert!(s.area.stop_holds(THREAD));
        pay();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K16: box-stop.test.ts:185 (a box that answers is stopped as it was: the agent, then the group, nothing owed)
#[test]
fn a_computer_that_answers_hears_the_stop() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        group_end(&s, |o| Ok(Some(if o.away { "the box is away".to_string() } else { "a process still runs".to_string() })));
        owes(&s, Some(Some(done())));
        let answer = ping_by_hand(&s);
        answer(Ok(()));
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "a process still runs" }));
        let log = s.log.borrow().clone();
        assert!(log.contains(&"end_thread(thr-ab12cd34, remove=false, away=false)".to_string()), "{log:?}");
        assert!(
            !log.iter().any(|l| l.starts_with("end(") || l.starts_with("end_owed")),
            "the turn ends itself and nothing is owed: {log:?}"
        );
        assert_eq!(
            s.host.rows.borrow()[KEY].borrow().view.status,
            SessionStatus::Running,
            "the row is settled by the turn, not by the stop"
        );
        assert!(!s.area.stop_holds(THREAD));
    });
}

// ---- the log and the cap stop ------------------------------------------------------------------------------------

// K17: a cap stop is opened by a stop with no task and released however it ends
#[test]
fn a_stop_with_no_task_opens_a_cap_stop_and_releases_it_whichever_way_it_ends() {
    run_local(async {
        let s = scene();
        let _ = s.area.interrupt_row("sess_nobody", None, None).await;
        assert_eq!(*s.log.borrow(), strs(&["cap_stopping(sess_nobody)", "release"]));

        let s = scene();
        group_end(&s, |_o| Err(fail("the daemon would not answer")));
        put(&s, Spec { status: "completed", ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert!(r.is_err());
        let log = s.log.borrow().clone();
        assert_eq!(log.first(), Some(&"cap_stopping(sess-1)".to_string()));
        assert_eq!(log.last(), Some(&"release".to_string()), "released when the stop failed too");

        let s = scene();
        let h = handle_with_tasks(&s, Ok(TaskStop::Accepted));
        put(&s, Spec { handle: Some(h), ..spec() });
        let _ = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(s.host.hooks.called("cap_stopping"), Vec::<String>::new(), "a subagent stop opens none");
    });
}

// K18: box-stop.test.ts:102
#[test]
fn every_stop_logs_one_line() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        group_end(&s, |o| Ok(if o.away { Some("the box is away".to_string()) } else { None }));
        owes(&s, Some(Some(done())));
        let h = handle(&s, false, true);
        put(&s, Spec { handle: Some(h), ..spec() });
        let cap = console::capture();
        let first = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(first), json!({ "outcome": "accepted", "left": "the box is away" }));
        let second = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(second), json!({ "outcome": "not-running" }));
        let third = s.area.interrupt_row("sess_nobody", None, None).await;
        assert_eq!(answered(third), json!({ "outcome": "not-found" }));
        assert_eq!(
            stop_lines(&cap),
            strs(&[
                "stop on thread thr-ab12: accepted (the box is away)",
                "stop on thread thr-ab12: not-running",
                "stop on thread sess_nob: not-found",
            ])
        );
    });
}

// K18: subagents.test.ts:428
#[test]
fn a_task_stop_logs_the_subagent_beside_the_thread() {
    run_local(async {
        let s = scene();
        let h = handle_with_tasks(&s, Ok(TaskStop::Accepted));
        put(&s, Spec { handle: Some(h), ..spec() });
        let cap = console::capture();
        let r = s.area.interrupt_row(KEY, None, Some("task_0123456789".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(stop_lines(&cap), strs(&["stop on thread thr-ab12 subagent task_012: accepted"]));
    });
}

// K18: a failure is logged with the outcome `failed: <message>` and is the caller's
#[test]
fn a_failed_stop_logs_it_and_fails() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Err(fail("the daemon would not answer")));
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h), ..spec() });
        let cap = console::capture();
        let r = s.area.interrupt_row(KEY, None, None).await;
        let e = r.expect_err("the stop fails");
        assert_eq!(said(&e), ("the daemon would not answer".to_string(), None, None));
        assert_eq!(stop_lines(&cap), strs(&["stop on thread thr-ab12: failed: the daemon would not answer"]), "no `left`, one line");
        assert_eq!(s.log.borrow().last(), Some(&"release".to_string()));
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K18: the thread is the row's, else the id asked; the api's interrupt is this stop
#[test]
fn the_api_interrupt_names_the_thread_of_the_turn_it_was_asked_by() {
    run_local(async {
        let s = scene();
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let cap = console::capture();
        let api: &dyn SessionsApi = &s.area;
        let r = api.interrupt(TURN, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(h.calls(), strs(&["interrupt"]));
        assert_eq!(stop_lines(&cap), strs(&["stop on thread thr-ab12: accepted"]), "asked by the turn id, logged by the row's thread");
    });
}

// ---- a line's try ------------------------------------------------------------------------------------------------

// K19, K14: owed-box.test.ts:41
#[test]
fn a_lines_try_is_ended_with_no_tree_and_no_mark() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        s.host.hooks.set::<dyn Fn(&Row) -> bool>("SessionsCtx::line_try", Rc::new(|_r: &Row| true));
        group_end(&s, |o| Ok(if o.away { Some("the box is away".to_string()) } else { None }));
        let (paid, pay) = gate();
        owes(&s, Some(Some(paid)));
        let h = handle(&s, false, true);
        probe_holds(&h, &s.area);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let cap = console::capture();
        s.area.stop_try(KEY).await.expect("the try is stopped");
        assert_eq!(
            *s.log.borrow(),
            strs(&[
                "interrupt(holds=true)",
                "end_thread(thr-ab12cd34, remove=false, away=true)",
                "end(the box is away, Some(true), Some(true))",
                "end_owed(thr-ab12cd34)",
            ]),
            "no tree, no cap mark, no try_stopped, no drop of a queued turn"
        );
        assert_eq!(s.host.rows.borrow()[KEY].borrow().view.status, SessionStatus::Interrupted, "the row settles");
        assert!(s.area.stop_holds(THREAD), "the end is owed to the computer's next link");
        assert_eq!(cap.lines(), Vec::<String>::new(), "a line's try is not logged as a stop");
        pay();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K19: sessions.ts:1482
#[test]
fn a_lines_try_with_no_row_is_left_alone() {
    run_local(async {
        let s = scene();
        s.area.stop_try("sess_nobody").await.expect("nothing to stop");
        assert_eq!(s.host.hooks.calls(), Vec::<String>::new());
        assert_eq!(*s.log.borrow(), Vec::<String>::new());
    });
}

// K5: owed-box.test.ts (a send waiting on an end owed to a computer that is away is given up by a stop on its row)
#[test]
fn a_send_waiting_on_an_owed_end_is_given_up() {
    run_local(async {
        let s = scene();
        put(&s, Spec { key: "held-turn", turn: "held-turn", handle: None, end: false, ..spec() });
        let given_up = Rc::new(Cell::new(0u32));
        let g = given_up.clone();
        let line = wsp_words::words::computer::send_given_up_line("aws-box");
        let said = line.clone();
        s.area.stops.borrow_mut().held_sends.insert(
            "held-turn".to_string(),
            HeldSend {
                thread_id: THREAD.to_string(),
                give_up: Rc::new(move || {
                    g.set(g.get() + 1);
                    said.clone()
                }),
            },
        );
        let answer = answered(s.area.interrupt_row("held-turn", None, None).await);
        assert_eq!(answer, json!({ "outcome": "accepted", "left": line }));
        assert_eq!(given_up.get(), 1, "the send is given up once");
        assert!(
            !s.log.borrow().iter().any(|c| c.starts_with("end(") || c.starts_with("try_stopped")),
            "nothing running is stopped: {:?}",
            s.log.borrow()
        );
        assert_eq!(s.host.hooks.called("stop_under"), Vec::<String>::new(), "the tree is not walked for a send that ran nothing");

        // With a task named, the held send is not what is asked about.
        let task = answered(s.area.interrupt_row("held-turn", None, Some("a1".into())).await);
        assert_eq!(task, json!({ "outcome": "not-running" }));
        assert_eq!(given_up.get(), 1);
    });
}
