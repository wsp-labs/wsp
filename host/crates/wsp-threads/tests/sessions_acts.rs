// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_acts.md. TS cases: runtime-steer.test.ts, steer-images.test.ts, permission-relay.test.ts,
//! thread-access.test.ts, session-title.test.ts, thread-marks.test.ts, aside.test.ts, session-run.test.ts
//! (the whole-runtime cases, ported at the seam).
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_acts/world.rs");

// C1, C3: runtime-steer.test.ts:232, :286
#[test]
fn a_steer_is_kept_only_when_accepted() {
    run_local(async {
        let (host, area) = host();
        host.ids.uuids.borrow_mut().extend(["steer-1".to_string(), "steer-2".to_string()]);
        let steers = watch_steers(&host);
        let (taken, missed) = (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
        put(
            &host,
            "sess-1",
            "w1",
            json!({ "threadId": "t1" }),
            Some(StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&taken) }),
        );
        put(
            &host,
            "sess-2",
            "w1",
            json!({ "threadId": "t2" }),
            Some(StubHandle { steer: Some(SteerTaken::NotRunning), ..StubHandle::new(&missed) }),
        );

        let got = area.steer_row("sess-1", steer_ask("and say pineapple", Some("req_2"), None), None).await.expect("a steer");
        assert_eq!(json_of(&got), json!({ "outcome": "accepted" }));
        assert_eq!(*taken.borrow(), vec!["steer(and say pineapple, Some(\"steer-1\"), [])".to_string()], "a fresh steer id, no images");
        assert_eq!(
            *steers.borrow(),
            vec![("sess-1".to_string(), "and say pineapple".to_string(), Some("req_2".to_string()), "steer-1".to_string())]
        );

        // The adapter says the line missed the turn: answered as it came, nothing kept.
        let late = area.steer_row("sess-2", steer_ask("racing", None, None), None).await.expect("a steer");
        assert_eq!(json_of(&late), json!({ "outcome": "not-running" }));
        assert_eq!(*missed.borrow(), vec!["steer(racing, Some(\"steer-2\"), [])".to_string()]);
        assert_eq!(steers.borrow().len(), 1, "nothing is recorded for a steer that missed");
    });
}

// C1: runtime-steer.test.ts:281, :286, :443
#[test]
fn steer_on_a_row_not_running_is_not_running() {
    run_local(async {
        let (host, area) = host();
        let steers = watch_steers(&host);
        let log = Rc::new(RefCell::new(Vec::new()));
        let road = || StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&log) };
        put(&host, "ended", "w1", json!({ "threadId": "t1", "status": "completed" }), Some(road()));
        put(&host, "launching", "w1", json!({ "threadId": "t2" }), None);
        put(&host, "plain", "w1", json!({ "threadId": "t3" }), Some(StubHandle::new(&log)));
        let outcome = |name: &'static str| {
            let area = area.clone();
            async move { json_of(&area.steer_row(name, steer_ask("too late", None, None), None).await.expect("an answer")) }
        };
        assert_eq!(outcome("ended").await, json!({ "outcome": "not-running" }), "a turn that ended");
        assert_eq!(outcome("launching").await, json!({ "outcome": "not-running" }), "a turn whose process this host does not hold");
        assert_eq!(outcome("plain").await, json!({ "outcome": "unsupported" }), "a harness that takes no message mid-turn");
        assert_eq!(outcome("nope").await, json!({ "outcome": "not-found" }));
        assert_eq!(*log.borrow(), Vec::<String>::new(), "no handle was asked");
        assert!(steers.borrow().is_empty());
    });
}

// C0, C1: permission-relay.test.ts:423
#[test]
fn a_steer_of_a_row_the_caller_does_not_reach_is_not_found() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        put(
            &host,
            "sess-1",
            "w1",
            json!({ "threadId": "t1" }),
            Some(StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&log) }),
        );
        let seen = Reached::default();
        unreached(&host, seen.clone());
        let got = area.steer_row("sess-1", steer_ask("x", None, None), Some(scoped("t9"))).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "not-found" }));
        assert_eq!(*seen.borrow(), vec![(Some("t1".to_string()), "w1".to_string(), None, false)]);
        assert_eq!(*log.borrow(), Vec::<String>::new());
    });
}

// C0: runtime-steer.test.ts:485
#[test]
fn a_steer_on_a_workspace_that_will_not_send_is_refused() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        put(
            &host,
            "sess-1",
            "w1",
            json!({ "threadId": "t1" }),
            Some(StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&log) }),
        );
        for phase in ["pausing", "napping"] {
            host.add_entry(entry("w1", json!({ "phase": phase })));
            let err = area.steer_row("sess-1", steer_ask("x", None, None), None).await.expect_err("a refusal");
            assert_eq!(said(&err), (send_line(&host, true), None, None), "{phase}: the composer's sentence, no kind");
        }
        assert_eq!(*log.borrow(), Vec::<String>::new());
    });
}

// C2: steer-images.test.ts:105
#[test]
fn steer_refuses_files_the_harness_cannot_take() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        put(
            &host,
            "sess-1",
            "w1",
            json!({ "threadId": "t1" }),
            Some(StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&log) }),
        );
        let asked = adapters(&host, StubAdapter::default());
        let steers = watch_steers(&host);
        let refused = |files: Vec<Attachment>| {
            let area = area.clone();
            async move {
                said(&area.steer_row("sess-1", steer_ask("and this?", Some("req_2"), Some(files)), None).await.expect_err("a refusal"))
            }
        };
        assert_eq!(refused(vec![png()]).await, ("claude takes no image into a running turn".to_string(), None, None));
        assert_eq!(
            refused(vec![notes()]).await,
            ("a file that is not an image goes only with a message that starts a turn".to_string(), None, None)
        );
        assert_eq!(
            refused(vec![png(); 6]).await,
            ("only 5 files fit one message; this one carries 6".to_string(), None, None),
            "the caps come first"
        );
        assert!(asked.borrow().contains(&"adapter_for:claude".to_string()), "the row's own agent is asked whether its steer reads images");
        assert_eq!(*log.borrow(), Vec::<String>::new(), "nothing reaches the turn");
        assert!(steers.borrow().is_empty(), "nothing reaches the transcript");
    });
}

// C2, C3: steer-images.test.ts:67
#[test]
fn a_steer_hands_its_images_inline() {
    run_local(async {
        let (host, area) = host();
        host.ids.uuids.borrow_mut().push("steer-1".to_string());
        let log = Rc::new(RefCell::new(Vec::new()));
        put(
            &host,
            "sess-1",
            "w1",
            json!({ "threadId": "t1" }),
            Some(StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&log) }),
        );
        adapters(&host, StubAdapter { steers_images: true, ..StubAdapter::default() });
        let steers = watch_steers(&host);
        let got =
            area.steer_row("sess-1", steer_ask("what is in this picture?", Some("req_2"), Some(vec![png()])), None).await.expect("a steer");
        assert_eq!(json_of(&got), json!({ "outcome": "accepted" }));
        let images = json!([{ "mediaType": "image/png", "bytes": png().bytes }]);
        assert_eq!(*log.borrow(), vec![format!("steer(what is in this picture?, Some(\"steer-1\"), {images})")]);
        assert_eq!(steers.borrow().len(), 1);
    });
}

// C4, C5, trap #1776: permission-relay.test.ts:229, :244
#[test]
fn answer_finds_the_running_row_by_the_agents_session_id() {
    run_local(async {
        let (host, area) = host();
        let (old, new, keyed, other) = (
            Rc::new(RefCell::new(Vec::new())),
            Rc::new(RefCell::new(Vec::new())),
            Rc::new(RefCell::new(Vec::new())),
            Rc::new(RefCell::new(Vec::new())),
        );
        let road = |log: &Rc<RefCell<Vec<String>>>| StubHandle { answer: Some(SessionAnswerOutcome::Answered), ..StubHandle::new(log) };
        // The row keyed by the agent's id has ended; the turn that asks runs under a key of its own.
        put(&host, "agent-1", "w1", json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "agent-1" }), Some(road(&old)));
        put(&host, "turn-2", "w1", json!({ "threadId": "t1", "claudeSessionId": "agent-1" }), Some(road(&new)));
        // A running row keyed by the id wins over another running row that carries the id as its session.
        put(&host, "k", "w1", json!({ "threadId": "t2", "claudeSessionId": "z" }), Some(road(&keyed)));
        put(&host, "turn-3", "w1", json!({ "threadId": "t2", "claudeSessionId": "k" }), Some(road(&other)));
        let ask = |id: &str| AnswerAsk { ask_id: "ask_1".into(), option_id: id.into(), reason: None };

        let got = area.answer_row("agent-1", ask("allow"), None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "answered" }));
        assert_eq!(
            (old.borrow().len(), new.borrow().clone()),
            (0, vec!["answer(ask_1, allow, None)".to_string()]),
            "the running row that carries the id, not the ended row keyed by it"
        );
        let got = area.answer_row("k", ask("allow"), None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "answered" }));
        assert_eq!((keyed.borrow().len(), other.borrow().len()), (1, 0));
    });
}

// C4: permission-relay.test.ts:411
#[test]
fn answer_falls_back_to_the_keyed_row_and_hands_back_the_outcome_whole() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        // Nothing runs under the id, so the keyed row, ended as it is, takes the answer.
        put(
            &host,
            "turn-1",
            "w1",
            json!({ "threadId": "t1", "status": "completed" }),
            Some(StubHandle { answer: Some(SessionAnswerOutcome::NoOption), ..StubHandle::new(&log) }),
        );
        let ask = AnswerAsk {
            ask_id: "ask_1".into(),
            option_id: "mode:bypassPermissions".into(),
            reason: Some("Run only the thread-rows file".into()),
        };
        let got = area.answer_row("turn-1", ask, None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "no-option" }));
        assert_eq!(*log.borrow(), vec!["answer(ask_1, mode:bypassPermissions, Some(\"Run only the thread-rows file\"))".to_string()]);
        let none = area
            .answer_row("s_nope", AnswerAsk { ask_id: "a".into(), option_id: "o".into(), reason: None }, None)
            .await
            .expect("an answer");
        assert_eq!(json_of(&none), json!({ "outcome": "not-found" }));
    });
}

// C5: permission-relay.test.ts:216
#[test]
fn answer_without_a_handle_is_gone() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        put(&host, "no-process", "w1", json!({ "threadId": "t1" }), None);
        put(&host, "no-road", "w1", json!({ "threadId": "t2" }), Some(StubHandle::new(&log)));
        let ask = || AnswerAsk { ask_id: "ask_1".into(), option_id: "allow".into(), reason: None };
        assert_eq!(json_of(&area.answer_row("no-process", ask(), None).await.expect("an answer")), json!({ "outcome": "gone" }));
        assert_eq!(json_of(&area.answer_row("no-road", ask(), None).await.expect("an answer")), json!({ "outcome": "unsupported" }));
        assert_eq!(*log.borrow(), Vec::<String>::new());
    });
}

// C0, C1: permission-relay.test.ts:380, :423
#[test]
fn answer_of_another_trees_row_is_not_found_and_a_paused_workspace_refuses() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        put(
            &host,
            "sess-1",
            "w1",
            json!({ "threadId": "t1" }),
            Some(StubHandle { answer: Some(SessionAnswerOutcome::Answered), ..StubHandle::new(&log) }),
        );
        let ask = || AnswerAsk { ask_id: "ask_1".into(), option_id: "allow".into(), reason: None };
        host.add_entry(entry("w1", json!({ "phase": "napping" })));
        let err = area.answer_row("sess-1", ask(), None).await.expect_err("a refusal");
        assert_eq!(said(&err), (send_line(&host, true), None, None));
        host.add_entry(entry("w1", json!({})));
        let seen = Reached::default();
        unreached(&host, seen.clone());
        let got = area.answer_row("sess-1", ask(), Some(scoped("t9"))).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "not-found" }));
        assert_eq!(*seen.borrow(), vec![(Some("t1".to_string()), "w1".to_string(), None, false)]);
        assert_eq!(*log.borrow(), Vec::<String>::new(), "the harness hears nothing");
    });
}

// C6, C8: thread-keeps-agent.test.ts:133
#[test]
fn access_on_a_trimmed_thread_moves_its_record() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t9", json!({ "harness": "codex", "permissionMode": "read-only", "rootThreadId": "t9" }));
        let asked = adapters(&host, StubAdapter::default());
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

        let got = area.access_row("t9", "danger-full-access", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "set" }));
        assert_eq!(*seen.borrow(), vec![(Some("t9".to_string()), "w1".to_string())], "reached as the thread of its record");
        assert_eq!(*asked.borrow(), vec!["codex".to_string()], "the record's agent");
        assert_eq!(
            record_of(&host, "t9"),
            Some(ThreadRecord {
                workspace_id: "w1".into(),
                ..thread_record(json!({ "harness": "codex", "permissionMode": "danger-full-access", "rootThreadId": "t9" }))
            })
        );
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), vec!["persist_sessions(w1)".to_string(), "push_head(t9)".to_string()]);

        let none = area.access_row("thr_nobody", "danger-full-access", None).await.expect("an answer");
        assert_eq!(json_of(&none), json!({ "outcome": "not-found" }));
    });
}

// C6, C9: permission-relay.test.ts:646, :664
#[test]
fn access_while_running_lands_on_the_record_and_the_row_follows() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t1", json!({ "harness": "claude", "permissionMode": "bypassPermissions" }));
        let log = Rc::new(RefCell::new(Vec::new()));
        put(
            &host,
            "turn-1",
            "w1",
            json!({ "threadId": "t1", "permissionMode": "bypassPermissions" }),
            Some(StubHandle { access: Some(AccessMove::Set), ..StubHandle::new(&log) }),
        );
        adapters(&host, StubAdapter::default());

        let got = area.access_row("turn-1", "acceptEdits", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "set" }));
        assert_eq!(*log.borrow(), vec!["set_access(acceptEdits)".to_string()]);
        assert_eq!(
            record_of(&host, "t1"),
            Some(ThreadRecord {
                workspace_id: "w1".into(),
                ..thread_record(json!({ "harness": "claude", "permissionMode": "acceptEdits" }))
            })
        );
        // The turn moved by the harness writes its own row; this road leaves a running row to the turn.
        assert_eq!(host.rows.borrow()["turn-1"].borrow().view.permission_mode.as_deref(), Some("bypassPermissions"));
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), vec!["persist_sessions(w1)".to_string(), "push_head(t1)".to_string()]);
    });
}

// C9: permission-relay.test.ts:664, :679, :691
#[test]
fn access_a_cli_refuses_is_unsupported() {
    run_local(async {
        for (road, name, want, handle_calls) in [
            (Some(AccessMove::Refused), "a CLI that refuses the mode", "unsupported", 1),
            (None, "a harness with no access road", "unsupported", 0),
            (Some(AccessMove::Gone), "a handle that says gone", "not-running", 1),
        ] {
            let (host, area) = host();
            thread_with_record(&host, "t1", json!({ "harness": "claude", "permissionMode": "bypassPermissions" }));
            let log = Rc::new(RefCell::new(Vec::new()));
            put(
                &host,
                "turn-1",
                "w1",
                json!({ "threadId": "t1", "permissionMode": "bypassPermissions" }),
                Some(StubHandle { access: road, ..StubHandle::new(&log) }),
            );
            adapters(&host, StubAdapter::default());
            let got = area.access_row("turn-1", "dontAsk", None).await.expect("an answer");
            assert_eq!(json_of(&got), json!({ "outcome": want }), "{name}");
            assert_eq!(log.borrow().len(), handle_calls, "{name}");
            // The pick stands for the thread's next turn all the same.
            assert_eq!(record_of(&host, "t1").map(|h| h.permission_mode), Some(Some("dontAsk".to_string())), "{name}");
            assert_eq!(
                host.rows.borrow()["turn-1"].borrow().view.permission_mode.as_deref(),
                Some("bypassPermissions"),
                "{name}: the running turn keeps its mode"
            );
        }
    });
}

// C9: permission-relay.test.ts:664
#[test]
fn access_on_a_turn_this_host_holds_no_process_for_is_not_running() {
    run_local(async {
        let (host, area) = host();
        let launching = row(view("turn-1", "w1", json!({ "threadId": "t1" })), "turn-1");
        launching.borrow_mut().launch = Some(done());
        host.add_row("turn-1", launching);
        adapters(&host, StubAdapter::default());
        let got = area.access_row("turn-1", "dontAsk", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "not-running" }));
        assert_eq!(
            record_of(&host, "t1").map(|h| (h.workspace_id, h.harness, h.permission_mode)),
            Some(("w1".to_string(), "claude".to_string(), Some("dontAsk".to_string()))),
            "a thread with no record gets one off its row"
        );
    });
}

// C6, C9: permission-relay.test.ts:747
#[test]
fn access_between_turns_sets_the_threads_latest_row() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t1", json!({ "harness": "claude", "permissionMode": "bypassPermissions" }));
        // Map order and start order differ: the row asked for by thread id is the last in the map, and the row
        // the thread's latest turn left is the one started last.
        put(
            &host,
            "r-a",
            "w1",
            json!({ "threadId": "t1", "status": "completed", "startedAt": T0 + 100, "permissionMode": "bypassPermissions" }),
            None,
        );
        put(
            &host,
            "r-b",
            "w1",
            json!({ "threadId": "t1", "status": "completed", "startedAt": T0, "harness": "codex", "permissionMode": "bypassPermissions" }),
            None,
        );
        let asked = adapters(&host, StubAdapter::default());

        let got = area.access_row("t1", "dontAsk", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "set" }));
        assert_eq!(*asked.borrow(), vec!["codex".to_string()], "the last row of the thread in map order names the agent");
        assert_eq!(host.rows.borrow()["r-a"].borrow().view.permission_mode.as_deref(), Some("dontAsk"), "the latest row says the new mode");
        assert_eq!(host.rows.borrow()["r-b"].borrow().view.permission_mode.as_deref(), Some("bypassPermissions"));
        assert_eq!(record_of(&host, "t1").map(|h| h.permission_mode), Some(Some("dontAsk".to_string())));
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), vec!["persist_sessions(w1)".to_string(), "push_head(t1)".to_string()]);
    });
}

// C9
#[test]
fn a_row_with_no_thread_takes_the_mode_on_itself() {
    run_local(async {
        let (host, area) = host();
        put(&host, "solo", "w1", json!({ "status": "completed", "permissionMode": "default" }), None);
        adapters(&host, StubAdapter::default());
        let got = area.access_row("solo", "dontAsk", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "set" }));
        assert_eq!(host.rows.borrow()["solo"].borrow().view.permission_mode.as_deref(), Some("dontAsk"));
        assert_eq!(host.records.borrow().len(), 0, "no thread, no record");
        assert_eq!(
            calls(&host, &["persist_sessions", "push_head"]),
            vec!["persist_sessions(w1)".to_string(), "push_head(solo)".to_string()]
        );
    });
}

// C7: permission-relay.test.ts:738, thread-access.test.ts:96
#[test]
fn access_refuses_a_mode_the_catalog_does_not_carry() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t1", json!({ "harness": "claude", "permissionMode": "default" }));
        let log = Rc::new(RefCell::new(Vec::new()));
        put(
            &host,
            "turn-1",
            "w1",
            json!({ "threadId": "t1" }),
            Some(StubHandle { access: Some(AccessMove::Set), ..StubHandle::new(&log) }),
        );
        adapters(&host, StubAdapter::default());
        host.hooks.set::<CatalogOn>(
            "LaunchCtx::catalog_on",
            Rc::new(|_h: &str, _e: &Entry, _a: &dyn HarnessAdapter| async move { Ok(Some(claude_catalog())) }.boxed_local()),
        );

        let err = area.access_row("turn-1", "sideways", None).await.expect_err("a refusal");
        let want = start_picks(
            Some(&claude_catalog()),
            &StartPicks { permission_mode: Some("sideways".into()), ..StartPicks::default() },
            false,
            None,
        )
        .expect_err("the catalog refuses");
        assert_eq!(want.kind, "usage");
        assert_eq!(said(&err), (want.message, None, Some(Kind::Usage)), "in the words a start refuses it with");
        assert_eq!(*log.borrow(), Vec::<String>::new());
        assert_eq!(record_of(&host, "t1").map(|h| h.permission_mode), Some(Some("default".to_string())), "nothing moved");
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), Vec::<String>::new());
        assert!(calls(&host, &["catalog_on"]).contains(&"catalog_on(claude)".to_string()));

        let ok = area.access_row("turn-1", "acceptEdits", None).await.expect("a mode the catalog carries");
        assert_eq!(json_of(&ok), json!({ "outcome": "set" }));
    });
}

// C0, C6: permission-relay.test.ts:711, :747
#[test]
fn access_of_a_row_not_reached_or_a_workspace_that_will_not_send() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t1", json!({ "harness": "claude", "permissionMode": "default" }));
        put(&host, "turn-1", "w1", json!({ "threadId": "t1", "status": "completed", "permissionMode": "default" }), None);
        adapters(&host, StubAdapter::default());
        let none = area.access_row("s_nope", "dontAsk", None).await.expect("an answer");
        assert_eq!(json_of(&none), json!({ "outcome": "not-found" }));

        host.add_entry(entry("w1", json!({ "phase": "pausing" })));
        let err = area.access_row("turn-1", "dontAsk", None).await.expect_err("a refusal");
        assert_eq!(said(&err), (send_line(&host, true), None, None));
        host.add_entry(entry("w1", json!({})));

        let seen = Reached::default();
        unreached(&host, seen.clone());
        let got = area.access_row("turn-1", "dontAsk", Some(scoped("t9"))).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "not-found" }));
        assert_eq!(host.rows.borrow()["turn-1"].borrow().view.permission_mode.as_deref(), Some("default"), "moves nothing");
        assert_eq!(record_of(&host, "t1").map(|h| h.permission_mode), Some(Some("default".to_string())));
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), Vec::<String>::new());
    });
}

// C10, C11, C12: session-title.test.ts:689, :801
#[test]
fn rename_names_every_row_of_the_session() {
    run_local(async {
        let (host, area) = renaming_host();
        let ms: Rc<RefCell<Vec<u64>>> = Rc::default();
        let seen = ms.clone();
        host.hooks.set::<dyn Fn(&Entry, u64) -> Arc<dyn HarnessExec>>(
            "TurnsCtx::harness_exec",
            Rc::new(move |_e: &Entry, ms: u64| {
                seen.borrow_mut().push(ms);
                Arc::new(NoExec) as Arc<dyn HarnessExec>
            }),
        );
        let wrote = writes(&host, SessionRenameWrite::Written);

        let got = area.rename_row("turn-2", "  the name he typed in wsp  ", None).await.expect("a rename");
        assert_eq!(json_of(&got), json!({ "outcome": "renamed" }));
        assert_eq!(
            *wrote.borrow(),
            vec![("SESS".to_string(), "the name he typed in wsp".to_string())],
            "the harness's own session id, the trimmed name"
        );
        let named = Some("the name he typed in wsp".to_string());
        assert_eq!(
            titles(&host),
            vec![
                (named.clone(), Some("person".to_string())),
                (named, Some("person".to_string())),
                (Some("keep".to_string()), None),
                (Some("elsewhere".to_string()), None),
            ],
            "every row of that workspace and session; no other session, no other workspace"
        );
        assert_eq!(
            calls(&host, &["write_session", "persist_sessions", "push_head"]),
            vec![
                "write_session(SESS, the name he typed in wsp)".to_string(),
                "persist_sessions(w1)".to_string(),
                "push_head(t1)".to_string()
            ]
        );
        assert!(
            ms.borrow().iter().all(|m| *m == wsp_core::types::events::SESSION_TITLE_TIMEOUT_MS),
            "the exec the write runs on is bounded by the title timeout"
        );
    });
}
