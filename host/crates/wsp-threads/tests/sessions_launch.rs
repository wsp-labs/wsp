// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_launch.md. TS cases: session-held.test.ts:15, session-opening.test.ts:50, owed-box.test.ts:56,
//! runtime-steer.test.ts:512-643, thread-cap.test.ts:111, 566-645, 1060, session-images.test.ts:88-381,
//! folder-threads.test.ts:637, 1038, thread-keeps-process.test.ts:92, session-mcp.test.ts:44, lost-session-line.test.ts:102,
//! replaces.test.ts:42, first-run.test.ts:128 (host package). Each test builds a `Prepared` by hand (the state
//! sessions_start leaves) and calls `launch_prepared`.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_launch/world.rs");

// ===============================================================================================================
// L1, L3: the row, held before the machine is asked

// L1, L3: session-held.test.ts:15
#[test]
fn the_row_is_held_before_the_machine_is_asked() {
    run_local(async {
        let r = rig();
        let handle = ok(r.run(prepared(&r)).await);
        let seen = r.seen.borrow();
        assert_eq!(seen.len(), 1, "the agent's lists were asked once");
        let first = &seen[0];
        assert_eq!(first.rows.len(), 1);
        let (key, row, v) = &first.rows[0];
        assert_eq!(key, "turn-1");
        assert_eq!(row.turn_id, "turn-1");
        assert_eq!(*v, held_view(json!({})));
        assert!(row.launch.is_some(), "the row carries the launch others wait on");
        assert!(row.handle.is_none());
        assert_eq!((row.replaces.clone(), row.notify.clone(), row.notify_by.clone(), row.notify_road), (None, None, None, None));
        assert_eq!(first.emitted, vec![held_event()], "every subscriber heard the thread held, with the request it answers");
        assert!(at(&first.calls, "cap_hold") < at(&first.calls, "catalog_on"), "the slot is taken before the machine is asked");
        assert_eq!(r.host.hooks.called("cap_hold"), vec!["cap_hold(turn-1, None)".to_string()]);
        // The row's view is the turn's view: one object the turn writes through.
        assert!(Rc::ptr_eq(&r.row("turn-1"), &r.turns.borrow()[0].row));
        assert_eq!(handle.id(), "turn-1");
        assert_eq!(handle.outcome(), SessionStartOutcome::Started);
    });
}

// L3: no TS test (the TS spreads replaces, notify, notifyBy and notifyRoad onto the held row)
#[test]
fn the_held_row_carries_who_it_replaces_and_who_it_tells() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.replaces = Some("thr_old".to_string());
        p.notify = Some(vec!["lead".to_string()]);
        p.notify_by = Some(scope("lead"));
        p.notify_road = Some(WorkspaceOrigin::Relayed);
        r.host.add_record("thr_old", "w1", thread_record(json!({ "harness": "claude" })));
        r.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>>(
            "LaunchCtx::preferences",
            Rc::new(|| async { Ok(prefs()) }.boxed_local()),
        );
        ok(r.run(p).await);
        let seen = r.seen.borrow();
        let (_, row, _) = &seen[0].rows[0];
        assert_eq!(row.replaces, Some("thr_old".to_string()));
        assert_eq!(row.notify, Some(vec!["lead".to_string()]));
        assert_eq!(row.notify_by, Some(scope("lead")));
        assert_eq!(row.notify_road, Some(WorkspaceOrigin::Relayed));
    });
}

// L1: session-opening.test.ts:42 (a row keeps the head of the opening, the transcript keeps it whole)
#[test]
fn the_row_keeps_the_head_of_a_long_opening_and_the_turn_the_whole() {
    run_local(async {
        let r = rig();
        let opening =
            format!("Build the ticket, and say what you find. {}", "Read every comment first, then the code it names. ".repeat(700));
        let mut p = prepared(&r);
        p.opts.prompt = opening.clone();
        ok(r.run(p).await);
        let held = r.seen.borrow()[0].rows[0].2.clone();
        let kept = held.prompt.clone().expect("the row has a prompt");
        assert_eq!(kept, kept_opening(&opening));
        assert!(kept.len() <= OPENING_KEPT && opening.starts_with(&kept));
        let t = r.take_turn(0);
        assert_eq!(t.opening.prompt, opening, "the transcript's start keeps the message whole");
        assert_eq!(t.row.borrow().asked.as_ref().unwrap().prompt, opening);
    });
}

// L1: no TS test (the TS view literal)
#[test]
fn the_held_view_carries_what_the_start_knew() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opts.started_by = Some(SessionOrigin::Cli);
        p.opts.attempt = Some("2".to_string());
        p.opts.context_window = Some("1m".to_string());
        p.tree = TreeOf { parent_thread_id: Some("lead".to_string()), root_thread_id: Some("lead".to_string()) };
        p.opens = false;
        p.resume = Some("sess-0".to_string());
        ok(r.run(p).await);
        let held = r.seen.borrow()[0].rows[0].2.clone();
        let expected = held_view(
            json!({ "startedBy": "cli", "attempt": "2", "contextWindow": "1m", "parentThreadId": "lead", "rootThreadId": "lead", "claudeSessionId": "sess-0" }),
        );
        assert_eq!(held, expected);
    });
}

// L1: a person's title stands as the person's
#[test]
fn a_persons_title_is_the_rows_title() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.title = Some("Landing fix".to_string());
        ok(r.run(p).await);
        let held = r.seen.borrow()[0].rows[0].2.clone();
        assert_eq!(held, held_view(json!({ "harnessTitle": "Landing fix", "titleSource": "person" })));
        assert_eq!(r.take_turn(0).opening.title, Some("Landing fix".to_string()));
    });
}

// L1, L17: an outside conversation's own name stands as a generated one
#[test]
fn an_outside_conversations_name_stands_as_an_auto_title() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opts.outside = Some(OutsideOpening {
            harness: "claude".to_string(),
            id: "out-1".to_string(),
            copy: false,
            project: "p1".to_string(),
            cwd: None,
            title: "  An outside talk\n".to_string(),
            earlier: Vec::new(),
        });
        p.resume = Some("out-1".to_string());
        ok(r.run(p).await);
        let held = r.seen.borrow()[0].rows[0].2.clone();
        assert_eq!(
            held,
            held_view(json!({ "harnessTitle": title_line("  An outside talk\n"), "titleSource": "auto", "claudeSessionId": "out-1" }))
        );
    });
}

// L1: no title of its own, the one the thread carries
#[test]
fn a_thread_that_has_a_title_keeps_it_on_the_new_row() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> (Option<String>, Option<TitleSource>)>(
            "SessionsCtx::carried_title",
            Rc::new(|_t: &str| (Some("Earlier name".to_string()), Some(TitleSource::Person))),
        );
        ok(r.run(prepared(&r)).await);
        let held = r.seen.borrow()[0].rows[0].2.clone();
        assert_eq!(held, held_view(json!({ "harnessTitle": "Earlier name", "titleSource": "person" })));
        assert_eq!(r.host.hooks.called("carried_title"), vec!["carried_title(t1)".to_string()]);
    });
}

// L3: runtime-steer.test.ts:539 (a send into a thread whose turn runs holds no row of its own until the thread is free)
#[test]
fn a_send_into_a_running_thread_holds_no_second_row() {
    run_local(async {
        let r = rig();
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        p.opts.prompt = "two".to_string();
        p.opts.request_id = Some("req_2".to_string());
        let join = r.spawn(p);
        settle().await;
        assert_eq!(r.row_keys(), vec!["sess-0".to_string()], "nothing held while the thread runs");
        assert_eq!(r.host.hooks.called("cap_hold"), Vec::<String>::new());
        assert_eq!(r.emitted(), vec![queued_event("two", "req_2")], "the wait is said at once, and no held is");
        assert!(r.turns.borrow().is_empty());
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(r.row_keys(), vec!["sess-0".to_string(), "turn-1".to_string()]);
        assert_eq!(r.host.hooks.called("cap_hold"), vec!["cap_hold(turn-1, None)".to_string()]);
        // The thread's own session is what the turn resumes: read off its latest row once the thread was this send's.
        let t = r.take_turn(0);
        assert_eq!(t.resume, Some("sess-old".to_string()));
        assert_eq!(t.outcome, SessionStartOutcome::Queued);
        assert_eq!(handle.outcome(), SessionStartOutcome::Queued);
        let mut held = held_event();
        held["requestId"] = json!("req_2");
        assert_eq!(
            r.emitted(),
            vec![queued_event("two", "req_2"), held],
            "held after the wait with this send's request id (sessions.ts:521), queued said once"
        );
        assert_eq!(t.row.borrow().view.claude_session_id, Some("sess-old".to_string()));
    });
}

// L3, L20: the launch settles once the handover is done
#[test]
fn the_launch_others_wait_on_settles_at_the_handover() {
    run_local(async {
        let r = rig();
        let tx = stall_landing(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        let launch = r.row("turn-1").borrow().launch.clone().expect("a launch");
        assert!(launch.clone().now_or_never().is_none(), "the launch is open while the machine is asked");
        tx.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert!(launch.now_or_never().is_some(), "the launch settled");
    });
}

// ===============================================================================================================
// L4, L19: saying the agent is starting

// L4: first-run.test.ts:128 (host package)
#[test]
fn starting_is_said_once_after_four_seconds_with_what_the_first_run_installs() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry, &str) -> LocalBoxFuture<'static, Result<bool, HostError>>>(
            "SessionsCtx::first_run_here",
            Rc::new(|_e: &Entry, _h: &str| async { Ok(true) }.boxed_local()),
        );
        let tx = stall_landing(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        r.host.clock.advance(AGENT_STARTING_MS as i64 - 1);
        settle().await;
        assert_eq!(r.emitted(), vec![held_event()], "nothing before the time");
        r.host.clock.advance(1);
        settle().await;
        let mut starting = starting_event();
        starting["installs"] = json!(true);
        assert_eq!(r.emitted(), vec![held_event(), starting.clone()]);
        assert_eq!(r.host.hooks.called("first_run_here"), vec!["first_run_here(claude)".to_string()]);
        tx.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.emitted(), vec![held_event(), starting], "the handover says nothing a second time");
    });
}

// L4: first-run.test.ts:173 (an agent that is not a wrapper says no installs)
#[test]
fn the_starting_line_names_no_install_where_there_is_none() {
    run_local(async {
        let r = rig();
        let tx = stall_landing(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        r.host.clock.advance(AGENT_STARTING_MS as i64);
        settle().await;
        assert_eq!(r.emitted(), vec![held_event(), starting_event()]);
        tx.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
    });
}

// L4: the turn's own session.start hushes it
#[test]
fn a_session_start_of_this_turn_hushes_the_starting_line() {
    run_local(async {
        let r = rig();
        let listeners: Rc<RefCell<Vec<(String, Rc<dyn Fn()>)>>> = Rc::new(RefCell::new(Vec::new()));
        let offs = Rc::new(Cell::new(0u32));
        let (l, o) = (listeners.clone(), offs.clone());
        r.host.hooks.set::<dyn Fn(&str, Rc<dyn Fn()>) -> Rc<dyn Fn()>>(
            "SessionsCtx::on_turn_settled",
            Rc::new(move |turn_id: &str, f: Rc<dyn Fn()>| {
                l.borrow_mut().push((turn_id.to_string(), f));
                let o = o.clone();
                Rc::new(move || o.set(o.get() + 1)) as Rc<dyn Fn()>
            }),
        );
        let tx = stall_landing(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        assert_eq!(listeners.borrow().len(), 1);
        assert_eq!(listeners.borrow()[0].0, "turn-1");
        assert_eq!(r.host.clock.pending(), 1, "the four-second timer");
        let f = listeners.borrow()[0].1.clone();
        f();
        assert_eq!(offs.get(), 1, "the listener is taken off");
        assert_eq!(r.host.clock.pending(), 0, "the timer is cancelled");
        r.host.clock.advance(AGENT_STARTING_MS as i64);
        settle().await;
        tx.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.emitted(), vec![held_event()]);
    });
}

// L19: the handover says the agent is starting at once, off a new process
#[test]
fn a_new_process_says_starting_at_the_handover() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        settle().await;
        assert_eq!(r.emitted(), vec![held_event(), starting_event()]);
        assert_eq!(r.host.clock.pending(), 0, "the timer is cancelled once it is said");
    });
}

// L4, L7: a send that queues says nothing of starting
#[test]
fn a_queued_send_never_says_starting() {
    run_local(async {
        let r = rig();
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        let join = r.spawn(p);
        settle().await;
        r.host.clock.advance(AGENT_STARTING_MS as i64);
        settle().await;
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1")]);
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        ok(join.await.expect("the launch ran"));
        settle().await;
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1"), held_event()]);
    });
}

// ===============================================================================================================
// L5: before the loop

// L5, Z1-Z5: no TS test (a local workspace whose named folder is gone)
#[test]
fn a_start_that_fails_before_its_launch_leaves_no_row_and_ends_the_wait() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        let mut p = prepared(&r);
        p.opts.cwd = Some("/gone/dir".to_string());
        p.launch.scoped_device_id = Some("dev-1".to_string());
        let join = r.spawn(p);
        settle().await;
        assert_eq!(r.seen.borrow().len(), 1, "the agent's lists are asked once, before the folder is read (sessions.ts:569-571)");
        let e = err(join.await.expect("the launch ran"));
        let line = no_cwd_line(&home_shortened("/gone/dir", None));
        assert_eq!(said(&e), (line.clone(), None, Some(Kind::Usage)));
        assert_eq!(r.row_keys(), Vec::<String>::new(), "no row is left");
        assert_eq!(r.emitted(), vec![held_event(), end_event(&line)], "the wait ends on the bus");
        assert_eq!(r.host.recorded_json(), Vec::<Value>::new(), "no end with no start in the transcript");
        assert_eq!(r.host.clock.pending(), 0);
        assert!(r.host.hooks.called("cap_left").iter().all(|c| c == "cap_left(turn-1)") && !r.host.hooks.called("cap_left").is_empty());
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()]);
        assert_eq!(r.host.hooks.called("drop_images"), Vec::<String>::new());
        assert!(r.turns.borrow().is_empty());
    });
}

// Z3, Z4: no TS test (a send waiting on a launch finds the thread as it now is)
#[test]
fn a_send_waiting_on_a_start_that_fails_goes_on_after_the_row_is_gone() {
    run_local(async {
        let r = rig();
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "turn-2".into(), "minted-2".into()];
        let (tx, rx) = oneshot::channel::<()>();
        let rx = Rc::new(RefCell::new(Some(rx)));
        r.host.hooks.set::<dyn Fn(&Entry, Option<&str>) -> LocalBoxFuture<'static, Result<String, HostError>>>(
            "LaunchCtx::thread_folder",
            Rc::new(move |_e: &Entry, cwd: Option<&str>| {
                let rx = rx.borrow_mut().take();
                let folder = cwd.map(String::from).unwrap_or_else(|| "/work/w1".to_string());
                async move {
                    if let Some(rx) = rx {
                        let _ = rx.await;
                    }
                    Ok(folder)
                }
                .boxed_local()
            }),
        );
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        let mut a = prepared(&r);
        a.opts.cwd = Some("/gone/dir".to_string());
        let first = r.spawn(a);
        settle().await;
        let mut b = prepared(&r);
        b.opens = false;
        b.cleared = false;
        b.opts.thread = Some("t1".to_string());
        b.opts.prompt = "second".to_string();
        b.opts.request_id = Some("req_2".to_string());
        let second = r.spawn(b);
        settle().await;
        assert_eq!(r.emitted(), vec![held_event(), queued_event("second", "req_2")], "the second waits on the first's launch");
        tx.send(()).unwrap();
        settle().await;
        let line = no_cwd_line(&home_shortened("/gone/dir", None));
        assert_eq!(err(first.await.expect("the launch ran")).message, line);
        let handle = ok(second.await.expect("the launch ran"));
        assert_eq!(handle.turn_id(), "turn-2");
        let events = r.emitted();
        assert_eq!(events[2], end_event(&line), "the first's end is out before the second holds its row");
        assert_eq!(events[3], json!({ "type": "session.held", "workspaceId": "w1", "threadId": "t1", "requestId": "req_2" }));
        assert_eq!(r.take_turn(0).outcome, SessionStartOutcome::Queued);
    });
}

// L5: the cwd check reads this computer's disk only for a local workspace
#[test]
fn a_cloud_workspace_does_not_look_for_the_folder_on_this_disk() {
    run_local(async {
        let r = rig();
        let cloud = r.host.add_entry(entry("w2", json!({ "kind": "cloud" })));
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        let mut p = prepared(&r);
        p.workspace_id = "w2".to_string();
        p.entry = cloud;
        p.opts.cwd = Some("/gone/dir".to_string());
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("folder_exists"), Vec::<String>::new());
        assert_eq!(r.take_turn(0).turn_id, "turn-1");
    });
}

// L5: a pick the lists do not carry is refused before this send waits on anything
#[test]
fn a_refused_pick_fails_the_start_before_anything_lands() {
    run_local(async {
        let r = rig();
        *r.catalog.borrow_mut() = catalog("harness", Some("2.1.300"), json!([{ "value": "haiku", "label": "Haiku" }]));
        r.host.hooks.set::<dyn Fn(Option<&HarnessCatalog>, Option<&str>) -> Option<HostError>>(
            "LaunchCtx::not_run_refused",
            Rc::new(|_c: Option<&HarnessCatalog>, _m: Option<&str>| {
                Some(HostError::with_kind("no-such has not run on this computer", Kind::Usage))
            }),
        );
        let mut p = prepared(&r);
        p.opts.model = Some("no-such".to_string());
        let e = err(r.run(p).await);
        assert_eq!(said(&e), ("no-such has not run on this computer".to_string(), None, Some(Kind::Usage)));
        assert_eq!(r.host.hooks.called("thread_folder"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("land_images"), Vec::<String>::new());
        assert_eq!(r.row_keys(), Vec::<String>::new());
        assert_eq!(r.emitted(), vec![held_event(), end_event("no-such has not run on this computer")]);
        assert!(r.adapter.starts.lock().unwrap().is_empty());
    });
}

// L6: owed-box.test.ts:41
#[test]
fn a_send_behind_a_stops_owed_end_waits_and_says_the_computer() {
    run_local(async {
        let r = rig();
        let paid = owed_hooks(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        assert!(r.area.stop_holds("t1"), "the owed end holds the thread's next turn");
        let mut queued = queued_event("fix it", "req_1");
        queued["waitsFor"] = json!("acme's laptop");
        assert_eq!(r.emitted(), vec![held_event(), queued], "held first, then the wait said once with the computer");
        assert_eq!(r.row("turn-1").borrow().view.waits_for, Some("acme's laptop".to_string()), "the row says what it waits for");
        assert_eq!(r.host.hooks.called("cap_hold").len(), 1);
        assert!(r.turns.borrow().is_empty());
        paid.send(()).unwrap();
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(handle.outcome(), SessionStartOutcome::Queued);
        let t = r.take_turn(0);
        assert_eq!(t.outcome, SessionStartOutcome::Queued);
        assert_eq!(t.row.borrow().view.waits_for, None, "the row's wait is taken off");
        assert!(!r.area.stop_holds("t1"), "the hold is forgotten once its gate settles");
        assert_eq!(r.emitted_of("session.queued").len(), 1);
    });
}

// L6, K5: owed-box.test.ts:41 (a stop on the waiting send gives it up)
#[test]
fn a_stop_gives_up_a_send_waiting_on_an_owed_end() {
    run_local(async {
        let r = rig();
        let _paid = owed_hooks(&r);
        let join = r.spawn(prepared(&r));
        settle().await;
        let stopped = r.area.interrupt_row("turn-1", None, None).await.expect("the stop answers");
        let line = send_given_up_line("acme's laptop");
        assert_eq!(
            stopped,
            SessionInterruptResult { outcome: SessionInterruptOutcome::Accepted, under: None, error: None, left: Some(line.clone()) }
        );
        settle().await;
        let e = err(join.await.expect("the launch ran"));
        assert_eq!(said(&e), (line.clone(), None, None));
        assert_eq!(r.row_keys(), Vec::<String>::new());
        let events = r.emitted();
        assert_eq!(events.last().cloned(), Some(end_event(&line)));
        assert!(r.turns.borrow().is_empty());
    });
}

// ===============================================================================================================
// L7: waiting for another send's launch

// L7: runtime-steer.test.ts:570
#[test]
fn two_sends_queued_behind_one_turn_run_in_order_and_each_says_so_once() {
    run_local(async {
        let r = rig();
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "turn-2".into(), "minted-2".into()];
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        let mut a = prepared(&r);
        send_into_running(&r, &mut a);
        a.opts.prompt = "two".to_string();
        a.opts.request_id = Some("req_2".to_string());
        let mut b = prepared(&r);
        send_into_running(&r, &mut b);
        b.opts.prompt = "three".to_string();
        b.opts.request_id = Some("req_3".to_string());
        let (ja, jb) = (r.spawn(a), r.spawn(b));
        settle().await;
        assert_eq!(r.emitted(), vec![queued_event("two", "req_2"), queued_event("three", "req_3")]);
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        let first = ok(ja.await.expect("the launch ran"));
        assert_eq!(first.outcome(), SessionStartOutcome::Queued);
        assert_eq!(r.turns.borrow().len(), 1, "the second send waits for the first's turn");
        assert_eq!(r.turns.borrow()[0].opening.prompt, "two");
        let h1 = r.handles.borrow()[0].clone();
        end_turn(&r.host, "turn-1", &h1);
        settle().await;
        let second = ok(jb.await.expect("the launch ran"));
        assert_eq!(second.outcome(), SessionStartOutcome::Queued);
        let prompts: Vec<String> = r.turns.borrow().iter().map(|t| t.opening.prompt.clone()).collect();
        assert_eq!(prompts, vec!["two".to_string(), "three".to_string()]);
        let held: Vec<Value> = r.emitted_of("session.held");
        assert_eq!(held.iter().map(|e| e["requestId"].clone()).collect::<Vec<_>>(), vec![json!("req_2"), json!("req_3")]);
        assert_eq!(r.emitted_of("session.queued").len(), 2, "the second wait of the second send is not said again");
    });
}

// L7: another send's turn still launching is waited for, and said once
#[test]
fn a_send_waits_for_the_launch_of_a_turn_ahead_of_it() {
    run_local(async {
        let r = rig();
        let (open_launch, launch) = gate();
        add_launching(&r.host, "turn-0", "t1", launch);
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        let join = r.spawn(p);
        settle().await;
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1")]);
        assert_eq!(r.row_keys(), vec!["turn-0".to_string()]);
        // The turn ahead is up: its row has its handle. This send now waits behind a running turn and says nothing more.
        let up = r.host.rows.borrow()["turn-0"].clone();
        let h0 = Rc::new(Handle::new("turn-0", SessionStartOutcome::Started, up.clone()));
        up.borrow_mut().handle = Some(h0.clone() as Rc<dyn SessionHandle>);
        open_launch.send(()).unwrap();
        settle().await;
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1")]);
        assert!(r.turns.borrow().is_empty());
        end_turn(&r.host, "turn-0", &h0);
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(handle.outcome(), SessionStartOutcome::Queued);
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1"), held_event()]);
    });
}

// L7: a steering harness waiting on a launching turn stays started and says nothing
#[test]
fn a_steering_harness_waiting_on_a_launching_turn_stays_started() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        let (open_launch, launch) = gate();
        add_launching(&r.host, "turn-0", "t1", launch);
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        let join = r.spawn(p);
        settle().await;
        assert_eq!(r.emitted(), Vec::<Value>::new());
        r.host.rows.borrow()["turn-0"].borrow_mut().view.status = serde_json::from_value(json!("completed")).unwrap();
        open_launch.send(()).unwrap();
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(handle.outcome(), SessionStartOutcome::Started);
        assert_eq!(r.take_turn(0).outcome, SessionStartOutcome::Started);
        assert_eq!(r.emitted_of("session.queued"), Vec::<Value>::new());
    });
}

// L3, L7: thread-cap.test.ts:78 (a turn a running thread waits on runs in that thread's slot)
#[test]
fn the_slot_of_a_thread_waiting_on_this_send_is_lent_to_it() {
    run_local(async {
        async fn lender(origin: Option<Caller>, on_held: bool, followed: Option<bool>) -> String {
            let r = rig();
            add_running(&r.host, "lead-sess", "lead-turn", "lead", json!({}), |_| {});
            let mut p = prepared(&r);
            p.origin = origin;
            if on_held {
                p.opts.on_held = Some(Rc::new(|_h: HeldStart| {}));
            }
            p.opts.followed = followed;
            ok(r.run(p).await);
            r.host.hooks.called("cap_hold").remove(0)
        }
        assert_eq!(
            lender(Some(caller_in("lead")), false, None).await,
            "cap_hold(turn-1, Some(\"lead-turn\"))",
            "a start the caller waits on"
        );
        assert_eq!(lender(Some(caller_in("lead")), true, Some(true)).await, "cap_hold(turn-1, Some(\"lead-turn\"))", "a follow");
        assert_eq!(lender(Some(caller_in("lead")), true, None).await, "cap_hold(turn-1, None)", "answered held: nobody waits on it");
        assert_eq!(lender(Some(caller_in("t1")), false, None).await, "cap_hold(turn-1, None)", "a thread never lends itself");
        assert_eq!(lender(None, false, None).await, "cap_hold(turn-1, None)", "a person lends nothing");
    });
}

// L7: the lender's slot goes to the turn ahead too
#[test]
fn a_send_waiting_on_a_launching_turn_lends_its_slot_to_that_turn() {
    run_local(async {
        let r = rig();
        add_running(&r.host, "lead-sess", "lead-turn", "lead", json!({}), |_| {});
        let (open_launch, launch) = gate();
        add_launching(&r.host, "turn-0", "t1", launch);
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        p.origin = Some(caller_in("lead"));
        let join = r.spawn(p);
        settle().await;
        assert_eq!(r.host.hooks.called("cap_lend"), vec!["cap_lend(turn-0, lead-turn)".to_string()]);
        r.host.rows.borrow()["turn-0"].borrow_mut().view.status = serde_json::from_value(json!("completed")).unwrap();
        open_launch.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.host.hooks.called("cap_hold"), vec!["cap_hold(turn-1, Some(\"lead-turn\"))".to_string()]);
    });
}

// L8: no TS test (a workspace deleted during the wait fails the start in its words)
#[test]
fn a_workspace_deleted_during_a_wait_fails_the_start_in_its_words() {
    run_local(async {
        let r = rig();
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        p.launch.scoped_device_id = Some("dev-1".to_string());
        let join = r.spawn(p);
        settle().await;
        r.host.entries.borrow_mut().shift_remove("w1");
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        let e = err(join.await.expect("the launch ran"));
        assert_eq!(e.message, deleted_before_start_line("w1"));
        assert_eq!(r.row_keys(), vec!["sess-0".to_string()], "a send that never held its row leaves none");
        assert_eq!(r.emitted(), vec![queued_event("fix it", "req_1")], "and says no end of a turn that never was");
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()]);
        assert!(r.turns.borrow().is_empty());
    });
}

// L8: the checks run again after the wait: the copy's block, the mounts and the kept process
#[test]
fn the_checks_run_once_for_a_free_thread_and_again_after_a_wait() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        assert_eq!(r.host.hooks.called("copy_blocked"), Vec::<String>::new(), "cleared: the start made them already");
        let r = rig();
        let h0 = add_running(&r.host, "sess-0", "turn-0", "t1", json!({}), |_| {});
        let mut p = prepared(&r);
        send_into_running(&r, &mut p);
        let join = r.spawn(p);
        settle().await;
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        ok(join.await.expect("the launch ran"));
        let calls = r.calls();
        assert_eq!(r.host.hooks.called("copy_blocked").len(), 1);
        assert_eq!(r.host.hooks.called("worktree_mounted").len(), 1);
        assert_eq!(r.host.hooks.called("kept_check"), vec!["kept_check(t1)".to_string()]);
        assert!(at(&calls, "copy_blocked") < at(&calls, "worktree_mounted") && at(&calls, "worktree_mounted") < at(&calls, "kept_check"));
    });
}

// ===============================================================================================================
// L9: a host write in flight on the resumed session

// L9
#[test]
fn a_host_write_in_flight_on_the_resumed_session_is_waited_out() {
    run_local(async {
        let r = rig();
        let (done_writing, writing) = gate();
        let asked = Rc::new(Cell::new(0u32));
        let a = asked.clone();
        r.host.hooks.set::<dyn Fn(&str) -> Option<Joined>>(
            "SessionsCtx::host_writes",
            Rc::new(move |_s: &str| {
                a.set(a.get() + 1);
                if a.get() == 1 {
                    Some(writing.clone())
                } else {
                    None
                }
            }),
        );
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        let join = r.spawn(p);
        settle().await;
        assert!(r.turns.borrow().is_empty(), "nothing launches over the write");
        assert_eq!(r.host.hooks.called("land_images"), Vec::<String>::new());
        done_writing.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(
            r.host.hooks.called("host_writes"),
            vec!["host_writes(sess-1)".to_string(); 3],
            "the loop reads it on each pass, the one that breaks after landing included (sessions.ts:649)"
        );
        assert_eq!(r.take_turn(0).resume, Some("sess-1".to_string()));
        assert_eq!(
            r.host.hooks.called("copy_blocked").len(),
            0,
            "a host write wait leaves the checks cleared as they were (sessions.ts:653-656)"
        );
    });
}

// L10: thread-cap.test.ts:111
#[test]
fn a_full_computer_holds_the_turn_and_tells_the_caller_once() {
    run_local(async {
        let r = rig();
        let (open_one, one) = gate();
        let (open_two, two) = gate();
        let gates = Rc::new(RefCell::new(vec![one, two]));
        let asks: Rc<RefCell<Vec<(CapAt, ThreadCapWait)>>> = Rc::new(RefCell::new(Vec::new()));
        let looks = Rc::new(Cell::new(0u32));
        let (l, w) = (looks.clone(), full_wait());
        r.host.hooks.set::<dyn Fn(&WorkspaceRecord, &str) -> Option<ThreadCapWait>>(
            "SessionsCtx::cap_full",
            Rc::new(move |_r: &WorkspaceRecord, _t: &str| {
                l.set(l.get() + 1);
                if l.get() <= 2 {
                    Some(w.clone())
                } else {
                    None
                }
            }),
        );
        let (g, a) = (gates.clone(), asks.clone());
        r.host.hooks.set::<dyn Fn(CapAt, ThreadCapWait) -> Joined>(
            "SessionsCtx::cap_wait",
            Rc::new(move |ask: CapAt, full: ThreadCapWait| {
                a.borrow_mut().push((ask, full));
                g.borrow_mut().remove(0)
            }),
        );
        let told: Rc<RefCell<Vec<HeldStart>>> = Rc::new(RefCell::new(Vec::new()));
        let t = told.clone();
        let mut p = prepared(&r);
        p.opts.on_held = Some(Rc::new(move |h: HeldStart| t.borrow_mut().push(h)));
        let join = r.spawn(p);
        settle().await;
        assert_eq!(*asks.borrow(), vec![(ask_of(None), full_wait())]);
        assert_eq!(
            *told.borrow(),
            vec![HeldStart {
                view: held_view(json!({ "capped": serde_json::to_value(full_wait()).unwrap() })),
                turn_id: "turn-1".to_string(),
                wait: full_wait()
            }]
        );
        assert_eq!(r.host.clock.pending(), 0, "a held turn is hushed: no starting line comes out of it");
        assert_eq!(r.host.hooks.called("cap_left"), Vec::<String>::new());
        assert!(r.turns.borrow().is_empty());
        assert_eq!(r.emitted(), vec![held_event()]);
        open_one.send(()).unwrap();
        settle().await;
        assert_eq!(asks.borrow().len(), 2, "it looks again and waits again");
        assert_eq!(told.borrow().len(), 1, "the caller is told once");
        assert_eq!(r.host.hooks.called("copy_blocked").len(), 1, "the checks run again after a wait");
        open_two.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.host.hooks.called("cap_left"), vec!["cap_left(turn-1)".to_string()], "let through once");
        assert_eq!(r.host.hooks.called("cap_full").len(), 3);
        assert!(at(&r.calls(), "cap_left") < at(&r.calls(), "land_images"), "the slot is taken before anything lands");
        assert_eq!(r.emitted(), vec![held_event()], "a hushed start says nothing of starting");
    });
}

// L10: a send that wakes its lead is let through a full computer
#[test]
fn a_send_that_wakes_its_lead_is_never_held_by_a_full_computer() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&WorkspaceRecord, &str) -> Option<ThreadCapWait>>(
            "SessionsCtx::cap_full",
            Rc::new(|_r: &WorkspaceRecord, _t: &str| Some(full_wait())),
        );
        let mut p = prepared(&r);
        p.opts.wakes_lead = true;
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("cap_full"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("cap_left"), vec!["cap_left(turn-1)".to_string()]);
        assert_eq!(r.take_turn(0).turn_id, "turn-1");
    });
}

// L10: stop marks pending are waited for, then the look is made again
#[test]
fn stops_pending_on_the_turn_are_waited_for_before_anything_else() {
    run_local(async {
        let r = rig();
        let (done, stop) = gate();
        let looked = Rc::new(Cell::new(0u32));
        let l = looked.clone();
        r.host.hooks.set::<dyn Fn(&str) -> Option<CapHeld>>(
            "SessionsCtx::cap_held",
            Rc::new(move |_t: &str| {
                l.set(l.get() + 1);
                if l.get() == 1 {
                    Some(CapHeld { stops: vec![stop.clone()], ..blank_hold() })
                } else {
                    None
                }
            }),
        );
        let join = r.spawn(prepared(&r));
        settle().await;
        assert_eq!(r.host.hooks.called("cap_full"), Vec::<String>::new(), "a mark is read before the computer is");
        assert!(r.turns.borrow().is_empty());
        done.send(()).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        assert_eq!(r.host.hooks.called("copy_blocked").len(), 1, "every check again");
        assert_eq!(r.host.hooks.called("cap_full").len(), 1);
    });
}

// L10, Z1-Z3: thread-cap.test.ts:583 (a held start that is stopped fails with the stop's words)
#[test]
fn a_stop_mark_fails_the_start_with_its_words_and_it_never_launches() {
    run_local(async {
        let r = rig();
        let stopped = "stopped before it started, while mac was full";
        let looked = Rc::new(Cell::new(0u32));
        let l = looked.clone();
        r.host.hooks.set::<dyn Fn(&str) -> Option<CapHeld>>(
            "SessionsCtx::cap_held",
            Rc::new(move |_t: &str| {
                l.set(l.get() + 1);
                if l.get() >= 2 {
                    Some(CapHeld { stopped: Some(stopped.to_string()), ..blank_hold() })
                } else {
                    None
                }
            }),
        );
        let mut p = prepared(&r);
        p.opts.on_held = Some(Rc::new(|_h: HeldStart| {}));
        r.host.hooks.set::<dyn Fn(&WorkspaceRecord, &str) -> Option<ThreadCapWait>>(
            "SessionsCtx::cap_full",
            Rc::new(|_r: &WorkspaceRecord, _t: &str| Some(full_wait())),
        );
        let e = err(r.run(p).await);
        assert_eq!(said(&e), (stopped.to_string(), None, None));
        assert!(r.turns.borrow().is_empty());
        assert_eq!(r.row_keys(), Vec::<String>::new());
        assert_eq!(r.emitted(), vec![held_event(), end_event(stopped)]);
        assert_eq!(r.host.hooks.called("land_images"), Vec::<String>::new());
    });
}

// ===============================================================================================================
// L11: the landing

// L11, L18: session-images.test.ts:289 (an image on the file road lands in this send's own folder)
#[test]
fn images_land_in_a_folder_of_their_own_per_send() {
    run_local(async {
        let r = rig_with(adapter(|a| a.attachments = Some(AttachmentRoad::File)));
        let folder = "/work/w1";
        let dir = turn_images_dir(folder, "t1", Some("req_1"), "minted-1");
        let sent = format!("{dir}/1.png");
        let landed_image = TurnImage { media_type: "image/png".to_string(), bytes: "iVBORw0=".to_string(), path: Some(sent.clone()) };
        let files_dir = send_files_dir(folder, "t1", Some("req_1"), "minted-1");
        let file_path = format!("{files_dir}/report.pdf");
        let (images, files) = record_landings(&r, vec![landed_image.clone()], vec![file_path.as_str()]);
        let mut p = prepared(&r);
        p.opts.prompt = "what is the first heading?".to_string();
        p.opts.attachments = Some(vec![png(), pdf()]);
        p.records = vec![record_of("image/png", 5), record_of("application/pdf", 5)];
        ok(r.run(p).await);
        assert_eq!(
            *images.borrow(),
            vec![(folder.to_string(), dir.clone(), vec![png()])],
            "the images only, in a folder keyed by thread, request and the send's own id"
        );
        assert_eq!(*files.borrow(), vec![(folder.to_string(), files_dir, vec![pdf()])], "the other files only");
        let t = r.take_turn(0);
        assert_eq!(t.images_dir, Some(dir));
        let handed = attached_files_prompt("what is the first heading?", &[file_path.as_str()]);
        assert_ne!(handed, "what is the first heading?");
        let asked = t.row.borrow().asked.clone().unwrap();
        assert_eq!(
            asked,
            TurnAsked {
                prompt: handed.clone(),
                effort: None,
                typed: Some("what is the first heading?".to_string()),
                request_id: Some("req_1".to_string()),
                owed: None,
                awaits_prompt: None
            }
        );
        assert_eq!(t.opening.prompt, "what is the first heading?", "the transcript keeps the words the person typed");
        assert_eq!(t.opening.attachments, Some(vec![record_of("image/png", 5), record_of("application/pdf", 5)]));
        assert_eq!(r.host.hooks.called("cap_left").len(), 1);
        assert!(at(&r.calls(), "land_images") < at(&r.calls(), "land_files"));
        assert_eq!(r.host.records.borrow()["t1"].files_in, Some(vec![folder.to_string()]), "the files folder is kept on the thread");
        assert_eq!(r.row("turn-1").borrow().view.prompt, Some("what is the first heading?".to_string()));
        open(t);
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!(starts[0].prompt, handed);
        assert_eq!(starts[0].images, vec![landed_image]);
    });
}

// L11: session-images.test.ts:373 (a message with no image asks the machine for nothing)
#[test]
fn a_message_with_no_attachment_lands_nothing_and_hands_the_adapter_no_image() {
    run_local(async {
        let r = rig();
        let (images, files) = record_landings(&r, Vec::new(), Vec::new());
        ok(r.run(prepared(&r)).await);
        assert_eq!(images.borrow().len(), 1);
        assert_eq!(images.borrow()[0].2, Vec::<Attachment>::new());
        assert_eq!(files.borrow()[0].2, Vec::<Attachment>::new());
        let t = r.take_turn(0);
        assert_eq!(t.images_dir, None);
        let asked = t.row.borrow().asked.clone().unwrap();
        assert_eq!(
            asked,
            TurnAsked {
                prompt: "fix it".to_string(),
                effort: None,
                typed: None,
                request_id: Some("req_1".to_string()),
                owed: None,
                awaits_prompt: None
            }
        );
        assert_eq!(t.opening.attachments, None);
        assert_eq!(r.host.records.borrow()["t1"].files_in, None);
        open(t);
        assert_eq!(r.adapter.starts.lock().unwrap()[0].images, Vec::<TurnImage>::new());
    });
}

// L11: session-images.test.ts:406 (two sends that wake together land in folders of their own)
#[test]
fn two_sends_never_share_a_folder_whatever_their_request_id() {
    run_local(async {
        let r = rig();
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "turn-2".into(), "minted-2".into()];
        let (images, _files) = record_landings(&r, Vec::new(), Vec::new());
        let mut a = prepared(&r);
        a.opts.request_id = None;
        a.opts.attachments = Some(vec![png()]);
        ok(r.run(a).await);
        let mut b = prepared(&r);
        b.thread_id = "t2".to_string();
        b.opts.request_id = Some("../../etc".to_string());
        b.opts.attachments = Some(vec![png()]);
        ok(r.run(b).await);
        let dirs: Vec<String> = images.borrow().iter().map(|i| i.1.clone()).collect();
        assert_eq!(
            dirs,
            vec![turn_images_dir("/work/w1", "t1", None, "minted-1"), turn_images_dir("/work/w1", "t2", Some("../../etc"), "minted-2")]
        );
        assert_ne!(dirs[0], dirs[1]);
        assert!(!dirs[1].contains(".."), "a request id shaped like a path is not one");
    });
}

// L11, Z5: session-images.test.ts:452 (a send that landed its images and is then refused takes them off the machine)
#[test]
fn images_landed_for_a_start_that_fails_are_taken_off_the_machine() {
    run_local(async {
        let r = rig_with(adapter(|a| a.attachments = Some(AttachmentRoad::File)));
        let dir = turn_images_dir("/work/w1", "t1", Some("req_1"), "minted-1");
        let image = TurnImage { media_type: "image/png".to_string(), bytes: "iVBORw0=".to_string(), path: Some(format!("{dir}/1.png")) };
        record_landings(&r, vec![image], Vec::new());
        run_turn_fails(&r, "the agent would not start");
        let mut p = prepared(&r);
        p.opts.attachments = Some(vec![png()]);
        let e = err(r.run(p).await);
        assert_eq!(e.message, "the agent would not start");
        assert_eq!(r.host.hooks.called("drop_images"), vec![format!("drop_images({dir})")]);
        assert_eq!(r.row_keys(), Vec::<String>::new());
        assert_eq!(r.emitted(), vec![held_event(), end_event("the agent would not start")]);
        assert!(r.host.hooks.called("cap_left").iter().all(|c| c == "cap_left(turn-1)"));
        assert_eq!(r.host.hooks.called("keep_sent_images"), Vec::<String>::new(), "nothing is kept for a turn that never ran");
        assert_eq!(r.host.records.borrow().get("t1").cloned(), None, "a launch that never opened leaves no record");
    });
}

// L11: the folder's snapshot is awaited unless the agent takes its prompt late
#[test]
fn the_snapshot_is_awaited_for_an_agent_that_takes_its_prompt_at_once() {
    run_local(async {
        let r = rig();
        let (done, taking) = {
            let (tx, rx) = oneshot::channel::<Option<String>>();
            let taking: Shared<LocalBoxFuture<'static, Option<String>>> =
                futures::FutureExt::map(rx, |r| r.unwrap_or(None)).boxed_local().shared();
            (tx, taking)
        };
        r.host.hooks.set::<dyn Fn(&Entry, &str) -> Shared<LocalBoxFuture<'static, Option<String>>>>(
            "SessionsCtx::snapshot_of",
            Rc::new(move |_e: &Entry, _cwd: &str| taking.clone()),
        );
        let join = r.spawn(prepared(&r));
        settle().await;
        assert!(r.turns.borrow().is_empty(), "the launch waits for the checkpoint the turn starts from");
        assert_eq!(r.host.hooks.called("snapshot_of"), vec!["snapshot_of(/work/w1)".to_string()]);
        done.send(Some("abc123".to_string())).unwrap();
        settle().await;
        ok(join.await.expect("the launch ran"));
        let t = r.take_turn(0);
        let snap = t.snapshot.clone().expect("a snapshot");
        assert_eq!(snap.cwd, "/work/w1");
        match snap.from {
            SnapshotFrom::Known(from) => assert_eq!(from, Some("abc123".to_string())),
            SnapshotFrom::Taking(_) => panic!("the snapshot was awaited, so it is known"),
        }
        open(t);
        assert!(!r.adapter.starts.lock().unwrap()[0].prompt_after);
    });
}

// L11: no TS test (sessions.ts:699 leaves the snapshot out where none was taken)
#[test]
fn a_snapshot_that_found_nothing_leaves_the_turn_none() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        assert!(r.take_turn(0).snapshot.is_none());
    });
}

// L11, L18: an agent that takes its prompt late does not wait for the snapshot, and its start waits on it
#[test]
fn a_late_prompt_agent_launches_without_the_snapshot_and_is_handed_it_as_the_wait() {
    run_local(async {
        let r = rig_with(adapter(|a| a.waits_prompt = true));
        let (done, taking) = {
            let (tx, rx) = oneshot::channel::<Option<String>>();
            let taking: Shared<LocalBoxFuture<'static, Option<String>>> =
                futures::FutureExt::map(rx, |r| r.unwrap_or(None)).boxed_local().shared();
            (tx, taking)
        };
        r.host.hooks.set::<dyn Fn(&Entry, &str) -> Shared<LocalBoxFuture<'static, Option<String>>>>(
            "SessionsCtx::snapshot_of",
            Rc::new(move |_e: &Entry, _cwd: &str| taking.clone()),
        );
        ok(r.run(prepared(&r)).await);
        let t = r.take_turn(0);
        let snap = t.snapshot.clone().expect("a snapshot");
        assert_eq!(snap.cwd, "/work/w1");
        let taking = match snap.from {
            SnapshotFrom::Taking(s) => s,
            SnapshotFrom::Known(_) => panic!("the snapshot is still being taken"),
        };
        done.send(Some("abc123".to_string())).unwrap();
        assert_eq!(taking.await, Some("abc123".to_string()));
        open(t);
        assert!(r.adapter.starts.lock().unwrap()[0].prompt_after, "the prompt goes after the snapshot");
    });
}

// L11: launch servers are read where the agent takes servers or denies them
#[test]
fn launch_servers_are_read_for_an_agent_that_takes_or_denies_servers() {
    run_local(async {
        let off = vec!["codex-apps".to_string()];
        let values =
            ServerValues { entries: None, config: Some(IndexMap::from([("mcp_servers.x.url".to_string(), "http://x".to_string())])) };
        for takes in [(true, false), (false, true)] {
            let r = rig_with(adapter(|a| {
                a.mcp = takes.0;
                a.denies = takes.1;
            }));
            let (o, v) = (off.clone(), values.clone());
            r.host
                .hooks
                .set::<dyn Fn(&Entry, &str, &dyn HarnessAdapter, &str) -> LocalBoxFuture<'static, Result<LaunchServers, HostError>>>(
                    "LaunchCtx::launch_servers_for",
                    Rc::new(move |_e: &Entry, _h: &str, _a: &dyn HarnessAdapter, _cwd: &str| {
                        let ls = LaunchServers { server_values: Some(v.clone()), off_servers: Some(o.clone()) };
                        async move { Ok(ls) }.boxed_local()
                    }),
                );
            ok(r.run(prepared(&r)).await);
            assert_eq!(r.host.hooks.called("launch_servers_for"), vec!["launch_servers_for(claude, /work/w1)".to_string()]);
            open(r.take_turn(0));
            let starts = r.adapter.starts.lock().unwrap();
            assert_eq!(starts[0].off_servers, Some(off.clone()));
            assert_eq!(starts[0].server_values, Some(values.clone()));
        }
        let r = rig();
        ok(r.run(prepared(&r)).await);
        assert_eq!(r.host.hooks.called("launch_servers_for"), Vec::<String>::new(), "an agent with no servers asks for none");
        open(r.take_turn(0));
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!((starts[0].off_servers.clone(), starts[0].server_values.clone()), (None, None));
    });
}

// L11: a send resuming pays a cut owed, and the opening says it came after one
#[test]
fn a_send_resuming_pays_the_cut_owed_and_resumes_the_session_it_made() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(Arc<dyn HarnessAdapter>, &str, &str, &str) -> LocalBoxFuture<'static, String>>(
            "SessionsCtx::pay_cut_owed",
            Rc::new(|_a: Arc<dyn HarnessAdapter>, _w: &str, _t: &str, _resume: &str| async { "sess-1b".to_string() }.boxed_local()),
        );
        r.host.hooks.set::<dyn Fn(&str, &str) -> bool>("SessionsCtx::cut_before", Rc::new(|_w: &str, _t: &str| true));
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("pay_cut_owed"), vec!["pay_cut_owed(w1, t1, sess-1)".to_string()]);
        let t = r.take_turn(0);
        assert_eq!(t.resume, Some("sess-1b".to_string()));
        assert!(t.opening.after_cut);
        assert_eq!(t.row.borrow().view.claude_session_id, Some("sess-1b".to_string()));
        open(t);
        assert_eq!(r.adapter.starts.lock().unwrap()[0].resume, Some("sess-1b".to_string()));
    });
}

// L11: a thread's opening pays no cut, and no cut is said
#[test]
fn an_opening_pays_no_cut_and_a_resume_with_none_owed_says_none() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opts.outside = Some(OutsideOpening {
            harness: "claude".to_string(),
            id: "out-1".to_string(),
            copy: true,
            project: "p1".to_string(),
            cwd: None,
            title: "talk".to_string(),
            earlier: Vec::new(),
        });
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("pay_cut_owed"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("cut_before"), Vec::<String>::new(), "no session to resume, no cut to read");
        let t = r.take_turn(0);
        assert!(!t.opening.after_cut);
        assert_eq!(t.resume, None);
        open(t);
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!(
            (starts[0].resume.clone(), starts[0].copy),
            (Some("out-1".to_string()), true),
            "a copy opens on the outside conversation's own id"
        );
    });
}
