// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `sessions_acts.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_acts/world.rs");

// C10, C11: session-title.test.ts:726, :737, :748, :760
#[test]
fn rename_with_no_session_is_no_session() {
    run_local(async {
        // The store answers no-session, or fails with its own line: the rows keep the title they had.
        for (outcome, want) in [
            (SessionRenameWrite::NoSession, json!({ "outcome": "no-session" })),
            (
                SessionRenameWrite::Failed { error: "database is locked".into() },
                json!({ "outcome": "failed", "error": "database is locked" }),
            ),
        ] {
            let (host, area) = renaming_host();
            let wrote = writes(&host, outcome);
            let got = area.rename_row("turn-2", "the name", None).await.expect("an answer");
            assert_eq!(json_of(&got), want);
            assert_eq!(wrote.borrow().len(), 1);
            assert_eq!(titles(&host)[1], (Some("old".to_string()), None));
            assert_eq!(calls(&host, &["persist_sessions", "push_head"]), Vec::<String>::new());
        }

        // A thread whose harness never named a session has nothing to name, and the machine is not asked.
        let (host, area) = renaming_host();
        put(&host, "bare", "w1", json!({ "threadId": "t5", "status": "completed" }), None);
        let wrote = writes(&host, SessionRenameWrite::Written);
        let got = area.rename_row("bare", "the name", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "no-session" }));
        assert_eq!(*wrote.borrow(), Vec::<(String, String)>::new());

        // No road to write a name is unsupported, and it is said before the missing session is.
        adapters(&host, StubAdapter::default());
        let got = area.rename_row("bare", "the name", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "unsupported" }));
        assert_eq!(*wrote.borrow(), Vec::<(String, String)>::new());
    });
}

// C10: session-title.test.ts:773, :780, :790
#[test]
fn a_blank_rename_is_refused() {
    run_local(async {
        let (host, area) = renaming_host();
        let wrote = writes(&host, SessionRenameWrite::Written);
        // The blank name is refused before the row is looked up.
        let err = area.rename_row("no-such-session", "   ", None).await.expect_err("a refusal");
        assert_eq!(said(&err), (EMPTY_TITLE_LINE.to_string(), None, None));
        assert_eq!(calls(&host, &["entry_of_row", "launch_adapter_for"]), Vec::<String>::new());

        let none = area.rename_row("no-such-session", "the name", None).await.expect("an answer");
        assert_eq!(json_of(&none), json!({ "outcome": "not-found" }));

        let seen = Reached::default();
        unreached(&host, seen.clone());
        let none = area.rename_row("turn-1", "the name", Some(scoped("t9"))).await.expect("an answer");
        assert_eq!(json_of(&none), json!({ "outcome": "not-found" }));
        host.hooks.set::<EntryOfRow>(
            "SessionsCtx::entry_of_row",
            Rc::new(|_at: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| {
                async move { Ok(Some(entry("w1", json!({ "phase": "napping" })) as Entry)) }.boxed_local()
            }),
        );
        let err = area.rename_row("turn-1", "the name", None).await.expect_err("a refusal");
        let rec = record_in(&entry("w1", json!({ "phase": "napping" })));
        let state = workspace_state(&WorkspaceStateInput { phase: rec.phase, machine_state: None, reach: None, wake_refused: None });
        assert_eq!(said(&err), (action_refusal(&state, "rename", rec.gone.as_deref(), Some(&rec.name)).expect("a refusal"), None, None));
        assert_eq!(calls(&host, &["copy_blocked"]), Vec::<String>::new(), "the workspace's state is asked before the copy");
        assert_eq!(*wrote.borrow(), Vec::<(String, String)>::new(), "the machine is asked nothing");
    });
}

// C10: the copy check comes first and its refusal is the caller's
#[test]
fn rename_stops_at_a_copy_refusal() {
    run_local(async {
        let (host, area) = renaming_host();
        let wrote = writes(&host, SessionRenameWrite::Written);
        host.hooks.set::<dyn Fn(&Entry) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::copy_blocked",
            Rc::new(|_e: &Entry| async move { Err(refusal("the copy is still landing", Some(Kind::Conflict))) }.boxed_local()),
        );
        let err = area.rename_row("turn-2", "the name", None).await.expect_err("a refusal");
        assert_eq!(said(&err), ("the copy is still landing".to_string(), None, Some(Kind::Conflict)));
        assert_eq!(calls(&host, &["launch_adapter_for"]), Vec::<String>::new());
        assert_eq!(*wrote.borrow(), Vec::<(String, String)>::new());
    });
}

// C13: thread-marks.test.ts:42
#[test]
fn read_marks_the_thread_read_now() {
    run_local(async {
        let (host, area) = host();
        let marked: Rc<RefCell<Vec<(Vec<String>, Value, Option<Caller>)>>> = Rc::default();
        let seen = marked.clone();
        host.hooks.set::<dyn Fn(&[String], StampsOf, Option<&Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::mark",
            Rc::new(move |ids: &[String], stamps: StampsOf, o: Option<&Caller>| {
                let stamped = ids.iter().map(|id| Value::Object(stamps(id))).collect::<Vec<_>>();
                seen.borrow_mut().push((ids.to_vec(), Value::Array(stamped), o.cloned()));
                async move { Ok(()) }.boxed_local()
            }),
        );
        host.clock.advance(1_234);
        area.read_thread("t1", Some(scoped("t1"))).await.expect("a read");
        let marked = marked.borrow();
        assert_eq!(marked.len(), 1);
        assert_eq!(marked[0].0, vec!["t1".to_string()]);
        assert_eq!(marked[0].2, Some(scoped("t1")), "the caller is handed on");
        let stamps = marked[0].1.as_array().expect("one stamp set per thread");
        assert_eq!(stamps.len(), 1);
        let keys: Vec<&String> = stamps[0].as_object().expect("stamps").keys().collect();
        assert_eq!(keys, vec!["readAt"], "the read stamp alone");
        assert_eq!(stamps[0]["readAt"].as_f64(), Some((T0 + 1_234) as f64), "now on the host's clock");
    });
}

// C14, C15: aside.test.ts:117, :272, :286
#[test]
fn aside_asks_the_latest_session_with_what_it_ran_at() {
    run_local(async {
        let (host, area) = aside_host();
        let asked_for = adapters(&host, with_aside());
        // The vault's own record, as `opts.vault()` answers it (sessions.ts:1199): the adapter is handed its server values.
        let record: IndexMap<String, String> = IndexMap::from([("VAULT_URL".to_string(), "http://x".to_string())]);
        let vault = wsp_adapters::port::ServerValues { entries: None, config: Some(record.clone()) };
        host.hooks.set::<dyn Fn() -> IndexMap<String, String>>("LaunchCtx::vault", Rc::new(move || record.clone()));
        let vault_seen: Rc<RefCell<Vec<Option<wsp_adapters::port::ServerValues>>>> = Rc::default();
        let log = vault_seen.clone();
        host.hooks.set::<LaunchAdapterFor>(
            "SessionsCtx::launch_adapter_for",
            Rc::new(move |_e: &Entry, h: &str, v: Option<wsp_adapters::port::ServerValues>| {
                log.borrow_mut().push(v);
                let built = BuiltAdapter { harness: h.to_string(), adapter: Arc::new(with_aside()) };
                async move { Ok(built) }.boxed_local()
            }),
        );
        let asked = aside_answers(&host, ANSWER);
        let history = host.recorded_json();

        let got = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "text": ANSWER }));
        assert_eq!(*vault_seen.borrow(), vec![Some(vault)], "the vault's servers go with the adapter");
        assert!(asked_for.borrow().is_empty(), "the bare adapter is the only one built");
        {
            let asked = asked.borrow();
            assert_eq!(asked.len(), 1);
            let q = &asked[0];
            // The row named is the thread's first; the question goes at the latest row's session, folder, model and effort.
            assert_eq!(
                (q.session.as_str(), q.question.as_str(), q.cut.as_deref(), q.cwd.as_deref(), q.model.as_deref(), q.effort.as_deref()),
                ("SESS", "what did I last ask?", None, Some("/root/b"), Some("claude-sonnet-5"), Some("high"))
            );
            assert_eq!((q.context_window.clone(), q.fast), (None, None));
            assert!(q.on_text.is_none() && q.mcp_servers.is_none() && q.server_values.is_none());
        }
        assert_eq!(host.recorded_json(), history, "nothing of it is kept");
        assert_eq!((host.passed.borrow().len(), host.emitted.borrow().len()), (0, 0));

        // The window and fast ride along, fast only where true.
        {
            let rows = host.rows.borrow();
            let mut latest = rows["turn-b"].borrow_mut();
            latest.view.context_window = Some("1m".into());
            latest.view.fast = Some(true);
        }
        area.aside_row("turn-a", "again?", None, None).await.expect("an answer");
        host.rows.borrow()["turn-b"].borrow_mut().view.fast = Some(false);
        area.aside_row("turn-a", "and again?", None, None).await.expect("an answer");
        let asked = asked.borrow();
        assert_eq!((asked[1].context_window.as_deref(), asked[1].fast), (Some("1m"), Some(true)));
        assert_eq!((asked[2].context_window.as_deref(), asked[2].fast), (Some("1m"), None));
    });
}

// C15
#[test]
fn aside_carries_the_cut_a_rewind_left_owing() {
    run_local(async {
        let (host, area) = aside_host();
        adapters(&host, with_aside());
        thread_with_record(&host, "t1", json!({ "harness": "claude", "cutOwed": "turn-x" }));
        let asked = aside_answers(&host, ANSWER);
        area.aside_row("turn-a", "what is this?", None, None).await.expect("an answer");
        assert_eq!(asked.borrow()[0].cut.as_deref(), Some("turn-x"));
    });
}

// C14: aside.test.ts:236
#[test]
fn a_blank_aside_and_an_unreached_row_are_refused() {
    run_local(async {
        let (host, area) = aside_host();
        adapters(&host, with_aside());
        let asked = aside_answers(&host, ANSWER);
        let err = area.aside_row("turn-a", "   ", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (BLANK_ASIDE_LINE.to_string(), None, None));
        assert_eq!(calls(&host, &["entry_of_row"]), Vec::<String>::new());

        let err = area.aside_row("sess_nothing", "hi", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), said(&not_found_refusal("no session sess_nothing")));

        let seen = Reached::default();
        unreached(&host, seen.clone());
        let err = area.aside_row("turn-a", "hi", Some(scoped("t9")), None).await.expect_err("a refusal");
        assert_eq!(said(&err), said(&not_found_refusal("no session turn-a")));

        host.hooks.set::<EntryOfRow>(
            "SessionsCtx::entry_of_row",
            Rc::new(|_at: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| {
                async move { Ok(Some(entry("w1", json!({ "phase": "pausing" })) as Entry)) }.boxed_local()
            }),
        );
        let err = area.aside_row("turn-a", "hi", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (send_line_for("pausing"), None, None), "the send refusal, with no name in it");
        assert!(asked.borrow().is_empty());
    });
}

// C15: aside.test.ts:236
#[test]
fn an_aside_with_no_road_or_no_session_is_refused() {
    run_local(async {
        let (host, area) = aside_host();
        adapters(&host, StubAdapter::default());
        let asked = aside_answers(&host, ANSWER);
        let err = area.aside_row("turn-a", "anyone there?", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (aside_unsupported_line("claude"), None, None));

        adapters(&host, with_aside());
        host.rows.borrow()["turn-b"].borrow_mut().view.claude_session_id = None;
        let err = area.aside_row("turn-a", "anyone there?", None, None).await.expect_err("a refusal");
        assert_eq!(
            said(&err),
            (ASIDE_NO_SESSION_LINE.to_string(), None, None),
            "the latest row names no session, whatever the row asked for holds"
        );

        // No road and no session: the road is said first.
        adapters(&host, StubAdapter::default());
        let err = area.aside_row("turn-a", "anyone there?", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (aside_unsupported_line("claude"), None, None));
        assert!(asked.borrow().is_empty(), "nothing is asked");
    });
}

// C15: aside.test.ts:131
#[test]
fn aside_streams_text_only_with_an_ask_id() {
    run_local(async {
        let (host, area) = aside_host();
        adapters(&host, with_aside());
        let asked = aside_answers(&host, ANSWER);
        let got = area.aside_row("turn-a", "what did I last ask?", None, Some("ask-1".into())).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "text": ANSWER }));
        let piece = |text: &str| json!({ "type": "aside.text", "workspaceId": "w1", "threadId": "t1", "askId": "ask-1", "text": text });
        assert_eq!(
            *host.passed.borrow(),
            vec![piece("You are in "), piece("/root/b")],
            "each piece passed on the bus under the question's id"
        );
        assert_eq!(host.recorded_json(), Vec::<Value>::new(), "none of them kept for a replay");
        assert!(asked.borrow()[0].on_text.is_some());

        // Asked with no id, the pieces go nowhere.
        host.passed.borrow_mut().clear();
        area.aside_row("turn-a", "again?", None, None).await.expect("an answer");
        assert!(asked.borrow()[1].on_text.is_none());
        assert_eq!(*host.passed.borrow(), Vec::<Value>::new());
    });
}

// C16: aside.test.ts:313
#[test]
fn an_empty_aside_answer_is_refused() {
    run_local(async {
        let (host, area) = aside_host();
        adapters(&host, with_aside());
        aside_answers(&host, " \n");
        let err = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (ASIDE_EMPTY_LINE.to_string(), None, None));
    });
}

// C17: aside.test.ts:152
#[test]
fn an_aside_with_servers_revokes_its_token_after() {
    run_local(async {
        let (host, area, s) = servers_host(full_servers(), full_servers(), false, "t1");
        let got = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "text": ANSWER }));
        assert_eq!(
            calls(&host, &ORDER),
            vec![
                "launch_adapter_for(claude)".to_string(),
                "thread_launch(t1, t1, true)".to_string(),
                "adapter_for(claude)".to_string(),
                "server_values_for(claude, /root/b)".to_string(),
                "aside".to_string(),
                "revoke_device(dev-1)".to_string(),
            ],
            "a launch is minted for the thread, its device revoked once the answer is in"
        );
        assert_eq!(*s.revoked.borrow(), vec!["dev-1".to_string()]);
        let asks = s.asks.borrow();
        assert_eq!(asks.len(), 1);
        assert_eq!(
            asks[0].turn_env.as_ref().and_then(|e| e.get("WSP_TOKEN")).map(String::as_str),
            Some("tok"),
            "the copy is built with the launch's pair"
        );
        let asked = s.asked.borrow();
        assert_eq!(
            json_of(&asked[0].mcp_servers),
            json!({ "wsp": { "command": "node", "args": ["/opt/wsp/dist/bin.js", "mcp", "--scoped"] } }),
            "the wsp server a turn of the thread gets"
        );
        assert_eq!(
            asked[0].server_values,
            Some(wsp_adapters::port::ServerValues { entries: None, config: None }),
            "server values read in the folder the thread ran in"
        );
        assert_eq!(*s.values_for.borrow(), vec![("claude".to_string(), "/root/b".to_string())]);
    });
}

// C17: aside.test.ts:295
#[test]
fn a_thread_another_thread_started_is_served_with_no_slate() {
    run_local(async {
        let (_host, area, s) = servers_host(full_servers(), full_servers(), false, "t0");
        area.aside_row("turn-a", "how is it going?", None, None).await.expect("an answer");
        let asked = s.asked.borrow();
        assert_eq!(
            json_of(&asked[0].mcp_servers),
            json!({ "wsp": { "command": "node", "args": ["/opt/wsp/dist/bin.js", "mcp", "--scoped", "--no-slate"], "noSlate": true } })
        );
    });
}

// C17: aside.test.ts:152
#[test]
fn a_token_that_will_not_go_is_warned_and_the_answer_stands() {
    run_local(async {
        let (host, area, _s) = servers_host(full_servers(), full_servers(), false, "t1");
        host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "LaunchCtx::revoke_device",
            Rc::new(|_d: &str| async move { Err(refusal("the door is shut", None)) }.boxed_local()),
        );
        let warned = wsp_core::console::capture();
        let got = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "text": ANSWER }));
        assert_eq!(
            warned.lines(),
            vec![format!("the token of a side question on thread {} was not taken away: the door is shut", thread_word("t1"))]
        );
    });
}

// C17
#[test]
fn an_aside_that_fails_still_takes_the_token_back() {
    run_local(async {
        let (host, area, s) = servers_host(full_servers(), full_servers(), false, "t1");
        host.hooks.set::<AsideHook>(
            "SessionsCtx::aside",
            Rc::new(|_a: &dyn HarnessAdapter, _q: AsideQuestion| async move { Err(refusal("the harness fell over", None)) }.boxed_local()),
        );
        let err = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect_err("the failure");
        assert_eq!(said(&err), ("the harness fell over".to_string(), None, None));
        assert_eq!(*s.revoked.borrow(), vec!["dev-1".to_string()]);
    });
}

// C17: aside.test.ts:180
#[test]
fn a_harness_that_loads_no_servers_is_asked_bare() {
    run_local(async {
        let (host, area, s) = servers_host(with_aside(), full_servers(), false, "t1");
        let got = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "text": ANSWER }));
        assert_eq!(calls(&host, &ORDER), vec!["launch_adapter_for(claude)".to_string(), "aside".to_string()], "no launch, no token");
        let asked = s.asked.borrow();
        assert!(asked[0].mcp_servers.is_none() && asked[0].server_values.is_none());
        assert!(s.revoked.borrow().is_empty());
    });
}

// C17
#[test]
fn a_brokered_workspace_is_asked_through_a_launch_even_for_a_harness_with_no_servers() {
    run_local(async {
        let (host, area, s) = servers_host(with_aside(), with_aside(), true, "t1");
        area.aside_row("turn-a", "what did I last ask?", None, None).await.expect("an answer");
        assert_eq!(
            calls(&host, &ORDER),
            vec![
                "launch_adapter_for(claude)".to_string(),
                "thread_launch(t1, t1, true)".to_string(),
                "adapter_for(claude)".to_string(),
                "aside".to_string(),
                "revoke_device(dev-1)".to_string(),
            ],
            "a harness that loads no servers reads no server values"
        );
        let asked = s.asked.borrow();
        assert!(asked[0].mcp_servers.is_none() && asked[0].server_values.is_none());
    });
}

// C18: session-run.test.ts:75
#[test]
fn run_is_the_persons_alone() {
    run_local(async {
        let (host, area) = run_host();
        let err = area.run_step(step(json!({})), Some(scoped("t1"))).await.expect_err("a refusal");
        assert_eq!(said(&err), (wsp_wire::views::session_events::RUN_PERSONS_LINE.to_string(), None, None));
        assert_eq!(calls(&host, &["ready"]), vec!["ready".to_string()], "ready first, then the scope");
        assert_eq!(calls(&host, &["entry_of_row", "open_thread"]), Vec::<String>::new());
        assert_eq!(host.recorded_json(), Vec::<Value>::new());

        // A thread with no row and no record, and one the caller does not reach, are no thread.
        let err = area.run_step(step(json!({ "threadId": "nope" })), None).await.expect_err("a refusal");
        assert_eq!(said(&err), said(&not_found_refusal(&format!("no thread {}", thread_word("nope")))));
        let seen = Reached::default();
        unreached(&host, seen.clone());
        let err = area.run_step(step(json!({})), None).await.expect_err("a refusal");
        assert_eq!(said(&err), said(&not_found_refusal(&format!("no thread {}", thread_word("t1")))));
        assert_eq!(*seen.borrow(), vec![(Some("t1".to_string()), "w1".to_string(), None, false)]);
        assert_eq!(host.recorded_json(), Vec::<Value>::new());
    });
}

// C18: the workspace comes off the thread's record where no row holds it
#[test]
fn a_run_on_a_thread_known_by_its_record_alone_is_recorded() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t7", json!({ "harness": "claude" }));
        let seen: Rc<RefCell<Vec<(Option<String>, String)>>> = Rc::default();
        host.hooks.set::<EntryOfRow>(
            "SessionsCtx::entry_of_row",
            Rc::new({
                let seen: Rc<RefCell<Vec<(Option<String>, String)>>> = Rc::clone(&seen);
                move |at: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| {
                    seen.borrow_mut().push((at.thread_id.clone(), at.workspace_id.clone()));
                    async move { Ok(Some(entry("w1", json!({})) as Entry)) }.boxed_local()
                }
            }),
        );
        let got = area.run_step(step(json!({ "threadId": "t7", "turnId": "turn-7" })), None).await.expect("a run");
        assert_eq!(*seen.borrow(), vec![(Some("t7".to_string()), "w1".to_string())]);
        // No row and no session: the step's own turn names the session.
        assert_eq!(host.recorded_of("session.run")[0]["sessionId"], "turn-7");
        assert_eq!(got.session_id, "turn-7");
    });
}

// C19: session-run.test.ts:58
#[test]
fn an_ended_run_is_answered_from_the_transcript() {
    run_local(async {
        let (host, area) = run_host();
        host.recorded.borrow_mut().push(event(run_event(json!({ "runId": "run-2", "state": "exited", "exitCode": 130, "at": T0 - 5 }))));
        host.recorded.borrow_mut().push(event(run_event(json!({ "runId": "run-3", "state": "running" }))));
        let before = host.recorded_json();

        // run-2 ended; another window reports it lost, and it stays as it ended.
        let got = area.run_step(step(json!({ "runId": "run-2", "state": "lost" })), None).await.expect("a run");
        let mut want = run_event(json!({ "runId": "run-2", "state": "exited", "exitCode": 130, "at": T0 - 5 }));
        want.as_object_mut().expect("an object").remove("type");
        assert_eq!(json_of(&got), want);
        assert_eq!(host.recorded_json(), before, "nothing recorded");

        // A run the transcript holds only as running is not an ending: its end is recorded.
        area.run_step(step(json!({ "runId": "run-3", "state": "exited", "exitCode": 0 })), None).await.expect("a run");
        assert_eq!(host.recorded_json().len(), before.len() + 1);
        assert_eq!(host.recorded_json().last().expect("an event")["state"], "exited");
    });
}

// C20: session-run.test.ts:34, :58
#[test]
fn a_run_step_is_recorded_with_its_tail() {
    run_local(async {
        let (host, area) = run_host();
        let started = area.run_step(step(json!({ "ptyId": "pty_7" })), None).await.expect("a run");
        let mut want = run_event(json!({ "ptyId": "pty_7" }));
        assert_eq!(host.recorded_json(), vec![want.clone()], "recorded as the event, the agent's own session on it");
        want.as_object_mut().expect("an object").remove("type");
        want["at"] = json!(T0);
        assert_eq!(json_of(&started), want, "answered with the time it was recorded");

        host.clock.advance(10);
        let long = (0..40_000).map(|i| format!("y {i}")).collect::<Vec<_>>().join("\n");
        let ended = area
            .run_step(step(json!({ "runId": "run-2", "state": "exited", "exitCode": 130, "output": long })), None)
            .await
            .expect("a run");
        let tail = run_output_tail(&long, RUN_OUTPUT_MAX_CHARS as usize);
        assert!(tail.ends_with("y 39999") && tail.len() <= RUN_OUTPUT_MAX_CHARS as usize);
        let recorded = host.recorded_of("session.run");
        assert_eq!(
            recorded[1],
            run_event(json!({ "runId": "run-2", "state": "exited", "exitCode": 130, "output": tail })),
            "no ptyId on a step that carried none"
        );
        assert_eq!(json_of(&ended)["output"], json!(tail));
        assert_eq!(json_of(&ended)["at"], json!(T0 + 10));
    });
}

// C20: the session is the latest row's agent session, else its id, else the step's turn
#[test]
fn a_run_names_the_latest_rows_session_else_its_id() {
    run_local(async {
        let (host, area) = host();
        put(&host, "old", "w1", json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "agent-old", "startedAt": T0 }), None);
        put(&host, "new", "w1", json!({ "threadId": "t1", "status": "completed", "startedAt": T0 + 5 }), None);
        area.run_step(step(json!({ "runId": "a" })), None).await.expect("a run");
        host.rows.borrow()["new"].borrow_mut().view.claude_session_id = Some("agent-new".into());
        area.run_step(step(json!({ "runId": "b" })), None).await.expect("a run");
        let sessions: Vec<Value> = host.recorded_of("session.run").iter().map(|e| e["sessionId"].clone()).collect();
        assert_eq!(sessions, vec![json!("new"), json!("agent-new")], "the latest row's own id where it has no agent session");
    });
}
