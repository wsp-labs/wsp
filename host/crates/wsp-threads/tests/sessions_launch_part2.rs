// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `sessions_launch.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_launch/world.rs");

// L11, L17
#[test]
fn a_fork_readies_its_session_on_the_sources_computer_and_records_its_history_first() {
    run_local(async {
        let r = rig();
        let dropped = Rc::new(Cell::new(false));
        let readied: Rc<RefCell<Vec<(String, ForkTurn, String)>>> = Rc::new(RefCell::new(Vec::new()));
        let rd = readied.clone();
        let d = dropped.clone();
        r.host
            .hooks
            .set::<dyn Fn(Arc<dyn HarnessAdapter>, &str, ForkTurn, &str) -> LocalBoxFuture<'static, Result<ForkedSession, HostError>>>(
                "SessionsCtx::ready_fork",
                Rc::new(move |_a: Arc<dyn HarnessAdapter>, s: &str, turn: ForkTurn, h: &str| {
                    rd.borrow_mut().push((s.to_string(), turn, h.to_string()));
                    let d = d.clone();
                    async move {
                        let drop: Box<dyn FnOnce() -> LocalBoxFuture<'static, Result<(), String>>> = Box::new(move || {
                            d.set(true);
                            async { Ok(()) }.boxed_local()
                        });
                        Ok(ForkedSession::Resume { resume: "copy-1".to_string(), drop })
                    }
                    .boxed_local()
                }),
            );
        let copied = Rc::new(RefCell::new(Vec::<String>::new()));
        let c = copied.clone();
        r.host.hooks.set::<dyn Fn(&ForkPlan, &str, &str)>(
            "SessionsCtx::record_copies",
            Rc::new(move |p: &ForkPlan, w: &str, t: &str| c.borrow_mut().push(format!("{}:{w}:{t}", p.source.thread_id))),
        );
        let mut p = prepared(&r);
        p.plan = Some(plan());
        p.fork_turn = Some(ForkTurn::Anchor("a1".to_string()));
        ok(r.run(p).await);
        assert_eq!(*readied.borrow(), vec![("src-sess".to_string(), ForkTurn::Anchor("a1".to_string()), "claude".to_string())]);
        let calls = r.calls();
        assert!(at(&calls, "land_images") < at(&calls, "ready_fork"));
        assert!(at(&calls, "ready_fork") < at(&calls, "copy_images"));
        assert_eq!(r.host.hooks.called("copy_images"), vec!["copy_images(w1, t1)".to_string()]);
        assert!(
            at(&calls, "copy_images") < at(&calls, "record_copies") && at(&calls, "record_copies") < at(&calls, "run_turn"),
            "the copied history opens the transcript"
        );
        assert_eq!(*copied.borrow(), vec!["src:w1:t1".to_string()]);
        let t = r.take_turn(0);
        assert_eq!(t.resume, Some("copy-1".to_string()), "the turn resumes the copy");
        assert_eq!(t.row.borrow().view.claude_session_id, Some("copy-1".to_string()));
        assert_eq!(r.host.records.borrow()["t1"].forked_from, Some(plan().forked_from));
        assert!(!dropped.get(), "a fork that ran keeps its copy");
        assert_eq!(r.host.hooks.called("drop_sent_images"), Vec::<String>::new());
    });
}

// L11, L18: a fork the start makes itself opens with the fork, resumes nothing and keeps no process
#[test]
fn a_fork_the_start_makes_itself_opens_with_the_fork_and_keeps_no_process() {
    run_local(async {
        let r = rig();
        r.host
            .hooks
            .set::<dyn Fn(Arc<dyn HarnessAdapter>, &str, ForkTurn, &str) -> LocalBoxFuture<'static, Result<ForkedSession, HostError>>>(
                "SessionsCtx::ready_fork",
                Rc::new(|_a: Arc<dyn HarnessAdapter>, _s: &str, _t: ForkTurn, _h: &str| {
                    async { Ok(ForkedSession::Fork { session: "src-sess".to_string(), turn: "anchor-1".to_string() }) }.boxed_local()
                }),
            );
        let mut p = prepared(&r);
        p.plan = Some(plan());
        p.fork_turn = Some(ForkTurn::Anchor("a1".to_string()));
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("take_kept"), vec!["take_kept(t1, None)".to_string()]);
        let t = r.take_turn(0);
        assert_eq!(t.resume, None);
        assert!(t.keep.is_none(), "a fork is a launch of its own");
        open(t);
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!(starts[0].fork, Some(("src-sess".to_string(), "anchor-1".to_string())));
        assert_eq!(starts[0].resume, None);
        assert!(!starts[0].keep);
    });
}

// Z5: a fork that never ran leaves no copy and no image of its history
#[test]
fn a_fork_that_never_ran_drops_its_copy_and_its_sent_images() {
    run_local(async {
        let r = rig();
        let dropped = Rc::new(Cell::new(false));
        readies_a_copy(&r, &dropped, false);
        let sent: Rc<RefCell<Vec<(String, Vec<String>)>>> = Rc::new(RefCell::new(Vec::new()));
        let s = sent.clone();
        r.host.hooks.set::<dyn Fn(&str, &[String]) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::drop_sent_images",
            Rc::new(move |w: &str, ids: &[String]| {
                s.borrow_mut().push((w.to_string(), ids.to_vec()));
                async { Ok(()) }.boxed_local()
            }),
        );
        run_turn_fails(&r, "the agent would not start");
        let mut p = prepared(&r);
        p.plan = Some(plan());
        p.fork_turn = Some(ForkTurn::Anchor("a1".to_string()));
        err(r.run(p).await);
        settle().await;
        assert!(dropped.get(), "the copy is removed");
        assert_eq!(*sent.borrow(), vec![("w1".to_string(), vec!["t1".to_string()])]);
    });
}

// Z5: the copy that would not go is warned, in the source's words
#[test]
fn a_copy_that_would_not_be_removed_is_warned() {
    run_local(async {
        let warned = console::capture();
        let r = rig();
        let dropped = Rc::new(Cell::new(false));
        readies_a_copy(&r, &dropped, true);
        run_turn_fails(&r, "the agent would not start");
        let mut p = prepared(&r);
        p.plan = Some(plan());
        p.fork_turn = Some(ForkTurn::Anchor("a1".to_string()));
        err(r.run(p).await);
        settle().await;
        assert_eq!(
            warned.lines(),
            vec![format!("the copy a fork of thread {} made was not removed: the machine would not answer", thread_word("src"))]
        );
    });
}

// L12: runtime-steer.test.ts:512
#[test]
fn a_steering_harness_steers_into_the_running_turn() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "steer-1".into()];
        let h0 = steering(&r, |_| {});
        steer_answers(&r, true);
        let mut p = steering_send(&r);
        p.launch.scoped_device_id = Some("dev-1".to_string());
        let handle = ok(r.run(p).await);
        assert_eq!(handle.outcome(), SessionStartOutcome::Steered);
        assert_eq!(handle.id(), "sess-0", "the caller holds the running turn");
        assert_eq!(handle.turn_id(), "turn-0");
        assert_eq!(*h0.steered.borrow(), vec![("end with STEERED".to_string(), Some("steer-1".to_string()))]);
        let calls = r.calls();
        assert!(at(&calls, "keep_steer") < at(&calls, "steer_answered"), "the turn keeps the message before it is written");
        assert_eq!(r.host.hooks.called("keep_steer"), vec!["keep_steer(steer-1)".to_string()]);
        assert_eq!(r.host.hooks.called("steer_answered"), vec!["steer_answered(sess-0, steer-1, true)".to_string()]);
        assert!(r.turns.borrow().is_empty(), "no second turn");
        assert_eq!(r.row_keys(), vec!["sess-0".to_string()]);
        assert_eq!(r.emitted(), Vec::<Value>::new());
        assert_eq!(r.host.hooks.called("cap_left"), vec!["cap_left(turn-1)".to_string()], "a steer holds no slot");
        settle().await;
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()], "and keeps no token");
        assert_eq!(r.host.clock.pending(), 0);
    });
}

// L12: runtime-steer.test.ts:624 (a turn that has answered takes no message)
#[test]
fn a_turn_that_has_answered_takes_no_steer_and_the_send_queues_for_the_next() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        let h0 = steering(&r, |_| {});
        r.host.rows.borrow()["sess-0"].borrow_mut().turn_live = Some(TurnLive { reply: Some(TurnStatus::Completed), ..Default::default() });
        steer_answers(&r, true);
        let join = r.spawn(steering_send(&r));
        settle().await;
        assert_eq!(
            *h0.steered.borrow(),
            Vec::<(String, Option<String>)>::new(),
            "steering it would put the words behind a reply already read"
        );
        assert_eq!(r.emitted(), vec![queued_event("end with STEERED", "req_2")]);
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        let handle = ok(join.await.expect("the launch ran"));
        assert_eq!(handle.outcome(), SessionStartOutcome::Queued);
        assert_eq!(r.host.hooks.called("keep_steer"), Vec::<String>::new());
    });
}

// L12: files an image-less steer cannot carry wait for the turn to end
#[test]
fn files_the_running_turn_cannot_take_queue_the_send() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        let h0 = steering(&r, |_| {});
        steer_answers(&r, true);
        let mut p = steering_send(&r);
        p.records = vec![record_of("image/png", 5)];
        p.opts.attachments = Some(vec![png()]);
        let join = r.spawn(p);
        settle().await;
        assert_eq!(*h0.steered.borrow(), Vec::<(String, Option<String>)>::new(), "this agent takes no image into a running turn");
        assert_eq!(r.emitted(), vec![queued_event("end with STEERED", "req_2")]);
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        assert_eq!(ok(join.await.expect("the launch ran")).outcome(), SessionStartOutcome::Queued);
    });
}

// L12: runtime-steer.test.ts:286 (the adapter says the line missed the turn: not running)
#[test]
fn a_steer_the_turn_never_took_queues_behind_it() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "steer-1".into()];
        let h0 = steering(&r, |h| *h.steer_answer.borrow_mut() = Ok(SteerTaken::NotRunning));
        steer_answers(&r, true);
        let join = r.spawn(steering_send(&r));
        settle().await;
        assert_eq!(
            r.host.hooks.called("steer_answered"),
            vec!["steer_answered(sess-0, steer-1, false)".to_string()],
            "the answer says the line was not taken"
        );
        assert_eq!(r.emitted(), vec![queued_event("end with STEERED", "req_2")]);
        end_turn(&r.host, "sess-0", &h0);
        settle().await;
        assert_eq!(ok(join.await.expect("the launch ran")).outcome(), SessionStartOutcome::Queued);
    });
}

// L12: no TS test (a write that threw on an agent that tells no unread messages falls to the person)
#[test]
fn a_lost_steer_falls_to_the_person() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "steer-1".into()];
        steering(&r, |h| *h.steer_answer.borrow_mut() = Err("the computer did not answer".to_string()));
        steer_answers(&r, true);
        let e = err(r.run(steering_send(&r)).await);
        assert_eq!(
            said(&e),
            ("the computer did not answer".to_string(), None, None),
            "no kind: no retry reads it as a computer that did not answer"
        );
        assert_eq!(r.host.hooks.called("keep_steer"), vec!["keep_steer(steer-1)".to_string()]);
        assert_eq!(r.host.hooks.called("steer_lost"), vec!["steer_lost(steer-1)".to_string()]);
        assert_eq!(r.host.hooks.called("steer_answered"), Vec::<String>::new());
        assert_eq!(r.row_keys(), vec!["sess-0".to_string()]);
        assert_eq!(r.emitted(), Vec::<Value>::new(), "a send that never held its row says no end");
        assert!(r.turns.borrow().is_empty());
    });
}

// L12: an agent that tells unread messages keeps the line the failed write may have landed
#[test]
fn a_write_that_threw_on_an_agent_that_tells_unread_is_left_to_its_unread_list() {
    run_local(async {
        let r = rig_with(adapter(|a| a.steers = true));
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "steer-1".into()];
        steering(&r, |h| {
            *h.steer_answer.borrow_mut() = Err("the computer did not answer".to_string());
            h.unread = Some(true);
        });
        steer_answers(&r, true);
        let handle = ok(r.run(steering_send(&r)).await);
        assert_eq!(handle.outcome(), SessionStartOutcome::Steered);
        assert_eq!(r.host.hooks.called("steer_lost"), Vec::<String>::new());
        assert_eq!(r.host.hooks.called("steer_answered"), vec!["steer_answered(sess-0, steer-1, true)".to_string()]);
    });
}

// L12: runtime-steer.test.ts:643 (another thread's turn is no reason to wait)
#[test]
fn a_turn_on_another_thread_holds_nothing_back() {
    run_local(async {
        let r = rig();
        add_running(&r.host, "sess-9", "turn-9", "other", json!({}), |_| {});
        ok(r.run(prepared(&r)).await);
        assert_eq!(r.emitted_of("session.queued"), Vec::<Value>::new());
        assert_eq!(r.take_turn(0).outcome, SessionStartOutcome::Started);
    });
}

// L13: no TS test (behind and setup each record a row)
#[test]
fn behind_and_setup_each_record_a_session_behind_row() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opts.behind = Some("the branch is two commits behind".to_string());
        p.opts.setup = Some("ran pnpm install".to_string());
        ok(r.run(p).await);
        let row = |text: &str| json!({ "type": "session.behind", "workspaceId": "w1", "sessionId": "turn-1", "turnId": "turn-1", "threadId": "t1", "text": text });
        assert_eq!(r.host.recorded_json(), vec![row("the branch is two commits behind"), row("ran pnpm install")]);
    });
}

// L14: folder-threads.test.ts:637 (Claude Code keeps its session)
#[test]
fn a_gone_worktree_moves_the_turn_and_says_so() {
    run_local(async {
        let r = rig();
        let e = r.host.add_entry(entry("w3", gone_worktree()));
        project_folder(&r);
        ran_in_the_worktree(&r);
        let mut p = prepared(&r);
        p.workspace_id = "w3".to_string();
        p.entry = e;
        p.opens = false;
        p.resume = Some("sess-claude".to_string());
        ok(r.run(p).await);
        assert_eq!(
            r.host.recorded_of("session.moved"),
            vec![
                json!({ "type": "session.moved", "workspaceId": "w3", "sessionId": "sess-claude", "turnId": "turn-1", "threadId": "t1", "from": "/wt/feat-gone", "to": "/proj", "branch": "feat/gone" })
            ],
            "claude keys its sessions to the project: no fresh"
        );
        let t = r.take_turn(0);
        assert_eq!(t.resume, Some("sess-claude".to_string()));
        assert_eq!(t.row.borrow().view.claude_session_id, Some("sess-claude".to_string()));
        assert_eq!(t.row.borrow().view.cwd, Some("/proj".to_string()));
        open(t);
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!((starts[0].cwd.clone(), starts[0].resume.clone()), (Some("/proj".to_string()), Some("sess-claude".to_string())));
    });
}

// L14: folder-threads.test.ts:637 (another agent opens a fresh one)
#[test]
fn an_agent_that_keys_sessions_to_the_folder_opens_a_fresh_one_in_the_project() {
    run_local(async {
        let codex = adapter(|_| {});
        let r = rig_with(codex.clone());
        let e = r.host.add_entry(entry("w3", gone_worktree()));
        project_folder(&r);
        ran_in_the_worktree(&r);
        let mut p = prepared(&r);
        p.workspace_id = "w3".to_string();
        p.entry = e;
        p.opens = false;
        p.opts.harness = Some("codex".to_string());
        p.built = BuiltAdapter { harness: "codex".to_string(), adapter: codex };
        p.resume = Some("sess-codex".to_string());
        ok(r.run(p).await);
        assert_eq!(
            r.host.recorded_of("session.moved"),
            vec![
                json!({ "type": "session.moved", "workspaceId": "w3", "sessionId": "sess-codex", "turnId": "turn-1", "threadId": "t1", "from": "/wt/feat-gone", "to": "/proj", "branch": "feat/gone", "fresh": true })
            ]
        );
        let t = r.take_turn(0);
        assert_eq!(t.resume, None, "the session is dropped");
        assert_eq!(t.row.borrow().view.claude_session_id, None, "and the view's with it");
        open(t);
        assert_eq!(r.adapter.starts.lock().unwrap()[0].resume, None);
    });
}

// L14: folder-threads.test.ts:1038 (a worktree removed before any thread ran in it)
#[test]
fn a_worktree_removed_before_its_first_thread_ran_moves_that_thread_with_no_session() {
    run_local(async {
        let r = rig();
        let e = r
            .host
            .add_entry(entry("w3", json!({ "worktree": { "path": "/wt/feat-bare", "branch": "feat/bare", "made": true, "gone": true } })));
        project_folder(&r);
        let mut p = prepared(&r);
        p.workspace_id = "w3".to_string();
        p.entry = e;
        p.gone_now = Some("/wt/feat-bare".to_string());
        ok(r.run(p).await);
        assert_eq!(
            r.host.recorded_of("session.moved"),
            vec![
                json!({ "type": "session.moved", "workspaceId": "w3", "sessionId": "turn-1", "turnId": "turn-1", "threadId": "t1", "from": "/wt/feat-bare", "to": "/proj", "branch": "feat/bare" })
            ]
        );
        open(r.take_turn(0));
        assert_eq!(r.adapter.starts.lock().unwrap()[0].cwd, Some("/proj".to_string()));
    });
}

// L14: a worktree that is there is no move
#[test]
fn a_folder_that_did_not_change_says_no_move() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        r.host.hooks.set::<dyn Fn(&str, &str) -> Option<String>>(
            "TrimmedCtx::folder_of",
            Rc::new(|_w: &str, _s: &str| Some("/work/w1".to_string())),
        );
        ok(r.run(p).await);
        assert_eq!(r.host.recorded_of("session.moved"), Vec::<Value>::new());
    });
}

// L13, L15: the picks, the folder and the session land on the row the start wrote
#[test]
fn the_picks_the_folder_and_the_session_land_on_the_row() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opts.model = Some("claude-haiku-5-5".to_string());
        p.opts.effort = Some("low".to_string());
        p.opts.permission_mode = Some("default".to_string());
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        ok(r.run(p).await);
        let t = r.take_turn(0);
        let v = t.row.borrow().view.clone();
        assert_eq!(
            v,
            held_view(
                json!({ "model": "claude-haiku-5-5", "effort": "low", "permissionMode": "default", "cwd": "/work/w1", "claudeSessionId": "sess-1" })
            )
        );
        assert!(Rc::ptr_eq(&t.row, &r.row("turn-1")));
    });
}

// L16: thread-keeps-process.test.ts:92 (the second send runs on the process the first left up, under its token)
#[test]
fn a_turn_runs_on_the_threads_kept_process_at_the_same_launch() {
    run_local(async {
        let r = rig();
        *r.catalog.borrow_mut() = catalog("harness", Some("2.1.300"), json!([]));
        let agent = Arc::new(FakeKept::default());
        let waiting: Waiting = Rc::new(Cell::new(false));
        let kept_launch_of = kept_launch("kept-fixed", json!({ "model": "claude-haiku-5-5" }));
        let taken = kept_process(Box::new(SharedKept(agent.clone())), Some("kept-dev"), kept_launch_of.clone(), waiting.clone());
        let asks = takes_kept(&r, Some(taken));
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        p.opts.model = Some("claude-haiku-5-5".to_string());
        p.launch.scoped_device_id = Some("dev-1".to_string());
        p.waiting = Rc::new(Cell::new(false));
        ok(r.run(p).await);
        let fixed = expected_fixed(&r, Some(Some("2.1.300")), None);
        let picks = json!({ "model": "claude-haiku-5-5" });
        assert_eq!(
            *asks.borrow(),
            vec![(
                "t1".to_string(),
                Some(kept_launch(&fixed, picks.clone())),
                Some("sess-1".to_string()),
                Some(kept_launch(&fixed, picks))
            )]
        );
        let t = r.take_turn(0);
        assert_eq!(t.turn_token, Some("kept-token".to_string()), "the token its first turn was launched with");
        assert_eq!(t.scope_device_id, Some("kept-dev".to_string()));
        assert_eq!(t.keep, Some(kept_launch_of));
        assert!(Rc::ptr_eq(t.waiting.as_ref().unwrap(), &waiting), "the kept process's own waiting box");
        settle().await;
        assert_eq!(
            r.host.hooks.called("revoke_device"),
            vec!["revoke_device(dev-1)".to_string()],
            "this send's own device goes unused and is taken away"
        );
        assert_eq!(r.emitted(), vec![held_event()], "a kept process says no starting");
        assert_eq!(r.host.hooks.called("end_kept"), Vec::<String>::new());
        open(t);
        assert!(r.adapter.starts.lock().unwrap().is_empty(), "the adapter is not asked to start");
        let nexts = agent.nexts.lock().unwrap();
        assert_eq!((nexts.len(), nexts[0].prompt.as_str(), nexts[0].images.len(), nexts[0].after), (1, "fix it", 0, false));
    });
}

// L16: a cold launch keeps its launch key for the next turn, matched on what this send named
#[test]
fn a_cold_turn_carries_its_launch_key_and_asks_for_what_it_named() {
    run_local(async {
        let r = rig();
        *r.catalog.borrow_mut() = catalog("harness", Some("2.1.300"), json!([]));
        r.host.hooks.set::<dyn Fn(HarnessCatalog, &Preferences, &str) -> DefaultsOn>(
            "LaunchCtx::defaults_on",
            Rc::new(|c: HarnessCatalog, _p: &Preferences, _project: &str| DefaultsOn {
                catalog: c,
                open_model: None,
                open_effort: Some("high".to_string()),
            }),
        );
        let asks = takes_kept(&r, None);
        let mut p = prepared(&r);
        p.opts.model = Some("claude-haiku-5-5".to_string());
        p.opts.permission_mode = Some("default".to_string());
        p.opts.mcp_servers = Some(IndexMap::from([(
            "extra".to_string(),
            McpServerSpec { command: "/bin/x".to_string(), args: Vec::new(), no_slate: None },
        )]));
        p.launch.scoped_device_id = Some("dev-1".to_string());
        ok(r.run(p).await);
        let named = json!({ "extra": { "command": "/bin/x", "args": [] } });
        let fixed = expected_fixed(&r, Some(Some("2.1.300")), Some(&named));
        let picks = json!({ "model": "claude-haiku-5-5", "effort": "high", "permissionMode": "default" });
        let asked = json!({ "model": "claude-haiku-5-5", "permissionMode": "default" });
        assert_eq!(
            *asks.borrow(),
            vec![("t1".to_string(), Some(kept_launch(&fixed, asked)), None, Some(kept_launch(&fixed, picks.clone())))],
            "an effort the send left out is the thread's own"
        );
        let t = r.take_turn(0);
        assert_eq!(t.turn_token, Some("tok-1".to_string()));
        assert_eq!(t.scope_device_id, Some("dev-1".to_string()));
        assert_eq!(t.keep, Some(kept_launch(&fixed, picks)));
        open(t);
        let starts = r.adapter.starts.lock().unwrap();
        assert!(starts[0].keep, "the agent is asked to keep its process");
        assert_eq!(starts[0].version, Some("2.1.300".to_string()));
        assert_eq!(
            (starts[0].model.clone(), starts[0].effort.clone(), starts[0].permission_mode.clone()),
            (Some("claude-haiku-5-5".to_string()), Some("high".to_string()), Some("default".to_string()))
        );
    });
}

// L16: a kind that keeps no agents carries no key
#[test]
fn a_kind_that_keeps_no_agents_carries_no_launch_key() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Entry) -> bool>("LaunchCtx::keeps_agents", Rc::new(|_e: &Entry| false));
        let asks = takes_kept(&r, None);
        ok(r.run(prepared(&r)).await);
        assert_eq!(*asks.borrow(), vec![("t1".to_string(), None, None, None)]);
        let t = r.take_turn(0);
        assert!(t.keep.is_none());
        open(t);
        assert!(!r.adapter.starts.lock().unwrap()[0].keep);
    });
}

// Z6, Z1: a kept process taken for a start that never opened is ended
#[test]
fn a_kept_process_taken_for_a_start_that_fails_is_ended() {
    run_local(async {
        let r = rig();
        let taken = kept_process(Box::new(FakeKept::default()), None, kept_launch("kept-fixed", json!({})), Rc::new(Cell::new(false)));
        takes_kept(&r, Some(taken));
        run_turn_fails(&r, "the agent would not start");
        let e = err(r.run(prepared(&r)).await);
        assert_eq!(e.message, "the agent would not start");
        assert_eq!(r.host.hooks.called("end_kept"), vec!["end_kept(t1)".to_string()]);
    });
}

// ===============================================================================================================
// L17, L18: the transcript's opening and what the agent is started with

// L17: an outside conversation's earlier rows go in once, ahead of the prompt
#[test]
fn an_outside_conversations_earlier_rows_are_recorded_before_the_turn() {
    run_local(async {
        let r = rig();
        let earlier = |who: &str, text: &str, at: i64| -> SessionEarlierEvent {
            serde_json::from_value(json!({ "workspaceId": "", "sessionId": "", "who": who, "text": text, "at": at }))
                .expect("an earlier row")
        };
        let mut p = prepared(&r);
        p.opts.outside = Some(OutsideOpening {
            harness: "claude".to_string(),
            id: "out-1".to_string(),
            copy: false,
            project: "p1".to_string(),
            cwd: None,
            title: "talk".to_string(),
            earlier: vec![earlier("person", "hello", 1700), earlier("agent", "hi there", 1800)],
        });
        p.resume = Some("out-1".to_string());
        ok(r.run(p).await);
        let row = |who: &str, text: &str, at: i64| json!({ "type": "session.earlier", "workspaceId": "w1", "sessionId": "out-1", "threadId": "t1", "who": who, "text": text, "at": at });
        assert_eq!(r.host.recorded_json(), vec![row("person", "hello", 1700), row("agent", "hi there", 1800)]);
        assert!(at(&r.calls(), "run_turn") > 0);
    });
}

// L18: session-mcp.test.ts:49 (servers named on a start reach an adapter that renders them)
#[test]
fn the_launch_carries_picks_servers_and_the_rest_of_what_the_start_named() {
    run_local(async {
        let r = rig_with(adapter(|a| a.mcp = true));
        *r.catalog.borrow_mut() = catalog("harness", Some("2.1.300"), json!([]));
        r.host.hooks.set::<dyn Fn(&Entry, &str) -> LocalBoxFuture<'static, Result<bool, HostError>>>(
            "SessionsCtx::limit_details_due",
            Rc::new(|_e: &Entry, _h: &str| async { Ok(true) }.boxed_local()),
        );
        let wsp = McpServerSpec {
            command: "/usr/local/bin/node".to_string(),
            args: vec!["/opt/wsp/bin.js".to_string(), "mcp".to_string()],
            no_slate: None,
        };
        let extra = McpServerSpec { command: "/bin/x".to_string(), args: Vec::new(), no_slate: None };
        let mut p = prepared(&r);
        p.title = Some("Landing fix".to_string());
        p.opts.model = Some("claude-haiku-5-5".to_string());
        p.opts.effort = Some("low".to_string());
        p.opts.permission_mode = Some("default".to_string());
        p.opts.context_window = Some("1m".to_string());
        p.mcp_servers = Some(IndexMap::from([("wsp".to_string(), wsp), ("extra".to_string(), extra)]));
        ok(r.run(p).await);
        open(r.take_turn(0));
        let starts = r.adapter.starts.lock().unwrap();
        let s = &starts[0];
        assert_eq!(s.prompt, "fix it");
        assert_eq!((s.resume.clone(), s.copy, s.fork.clone()), (None, false, None));
        assert_eq!(s.cwd, Some("/work/w1".to_string()));
        assert_eq!(
            (s.model.clone(), s.effort.clone(), s.permission_mode.clone(), s.context_window.clone(), s.fast),
            (Some("claude-haiku-5-5".to_string()), Some("low".to_string()), Some("default".to_string()), Some("1m".to_string()), None)
        );
        assert_eq!(s.title, Some("Landing fix".to_string()));
        assert!(s.limit_details);
        assert_eq!(s.version, Some("2.1.300".to_string()));
        assert!(s.keep);
        assert!(s.seed.is_none(), "a new session has nothing to seed");
        let servers = s.mcp_servers.clone().expect("servers");
        let adapter_spec = |command: &str, args: &[&str]| wsp_adapters::port::McpServerSpec {
            command: command.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            no_slate: None,
        };
        assert_eq!(servers.keys().cloned().collect::<Vec<_>>(), vec!["wsp".to_string(), "extra".to_string()]);
        assert_eq!(servers["wsp"], adapter_spec("/usr/local/bin/node", &["/opt/wsp/bin.js", "mcp"]));
        assert_eq!(servers["extra"], adapter_spec("/bin/x", &[]));
    });
}

// L18: the table's version is not the agent's: only a catalog the harness answered names one
#[test]
fn a_catalog_read_off_the_table_names_no_version() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        open(r.take_turn(0));
        let starts = r.adapter.starts.lock().unwrap();
        assert_eq!((starts[0].version.clone(), starts[0].limit_details, starts[0].mcp_servers.is_none()), (None, false, true));
    });
}

// L18: lost-session-line.test.ts:102 (a resume seeds the new session with the thread so far, this turn left out)
#[test]
fn the_seed_is_the_threads_earlier_turns_without_this_one() {
    run_local(async {
        let r = rig();
        let base =
            |kind: &str, turn: &str| json!({ "type": kind, "workspaceId": "w1", "sessionId": "sess-1", "turnId": turn, "threadId": "t1" });
        let events = vec![
            event(merged(base("session.start", "turn-0"), json!({ "prompt": "Remember the word lantern.", "agent": "claude" }))),
            event(merged(base("session.start", "turn-1"), json!({ "prompt": "what was the word?", "agent": "claude" }))),
        ];
        let served = events.clone();
        r.host.hooks.set::<dyn Fn(&str, &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>>(
            "SessionsCtx::open_thread",
            Rc::new(move |_w: &str, _t: &str| {
                let served = served.clone();
                async move { Ok(served) }.boxed_local()
            }),
        );
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        ok(r.run(p).await);
        open(r.take_turn(0));
        let seed = r.adapter.starts.lock().unwrap()[0].seed.clone().expect("a resume carries its seed");
        let got = seed().await.expect("the seed reads");
        let earlier: Vec<SessionEvent> =
            events.iter().filter(|e| serde_json::to_value(e).unwrap()["turnId"] != "turn-1").cloned().collect();
        let expected = thread_seed(&thread_messages(&earlier, "t1"));
        assert_eq!(got, expected);
        assert_ne!(got, thread_seed(&thread_messages(&events, "t1")), "this turn's own message follows, so it is left out");
        assert_eq!(r.host.hooks.called("open_thread"), vec!["open_thread(w1, t1)".to_string()]);
    });
}

// L18, L20: the whole handover
#[test]
fn run_turn_is_handed_the_whole_turn() {
    run_local(async {
        let r = rig_with(adapter(|a| a.reports_edits = true));
        let waiting: Waiting = Rc::new(Cell::new(false));
        let mut p = prepared(&r);
        p.waiting = waiting.clone();
        p.title = Some("Landing".to_string());
        p.notify = Some(vec!["lead".to_string()]);
        p.notify_by = Some(scope("lead"));
        p.notify_road = Some(WorkspaceOrigin::Relayed);
        p.launch.scoped_device_id = Some("dev-1".to_string());
        p.records = vec![record_of("image/png", 5)];
        p.opts.attachments = Some(vec![png()]);
        p.opts.via_slate = true;
        p.opts.after_limit = Some(1234.0);
        p.opts.effort = Some("low".to_string());
        p.opts.owed = true;
        ok(r.run(p).await);
        let t = r.take_turn(0);
        assert_eq!((id_of(&t.entry), t.thread_id.as_str(), t.turn_id.as_str()), ("w1".to_string(), "t1", "turn-1"));
        assert_eq!(
            (t.notify.clone(), t.notify_by.clone(), t.notify_road),
            (Some(vec!["lead".to_string()]), Some(scope("lead")), Some(WorkspaceOrigin::Relayed))
        );
        assert_eq!(
            (t.turn_token.clone(), t.scope_device_id.clone(), t.outcome),
            (Some("tok-1".to_string()), Some("dev-1".to_string()), SessionStartOutcome::Started)
        );
        assert_eq!(
            t.opening,
            Opening {
                prompt: "fix it".to_string(),
                request_id: Some("req_1".to_string()),
                via_slate: true,
                after_cut: false,
                after_limit: Some(1234.0),
                opens_thread: true,
                title: Some("Landing".to_string()),
                attachments: Some(vec![record_of("image/png", 5)]),
            }
        );
        let asked = t.row.borrow().asked.clone().expect("asked");
        assert_eq!(
            asked,
            TurnAsked {
                prompt: "fix it".to_string(),
                effort: Some("low".to_string()),
                typed: None,
                request_id: Some("req_1".to_string()),
                owed: Some(true),
                awaits_prompt: None
            }
        );
        assert_eq!(
            (t.resume.clone(), t.images_dir.clone(), t.written.is_none(), t.row.borrow().turn_live.is_none()),
            (None, None, true, true)
        );
        assert!(t.reports_edits, "the adapter says it reports edits");
        assert!(Rc::ptr_eq(t.waiting.as_ref().unwrap(), &waiting));
    });
}

// L18: a send that is not an opening says so
#[test]
fn the_opening_names_a_thread_only_for_an_opening() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.opens = false;
        ok(r.run(p).await);
        assert!(!r.take_turn(0).opening.opens_thread);
    });
}

// ===============================================================================================================
// L20: the thread's record

// L20
#[test]
fn the_record_is_made_at_the_first_turn_only() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.tree = TreeOf { parent_thread_id: Some("lead".to_string()), root_thread_id: Some("lead".to_string()) };
        p.opts.permission_mode = Some("default".to_string());
        p.clouds = Some(vec!["acme".to_string()]);
        ok(r.run(p).await);
        let expected = ThreadRecord {
            harness: "claude".to_string(),
            permission_mode: Some("default".to_string()),
            parent_thread_id: Some("lead".to_string()),
            root_thread_id: Some("lead".to_string()),
            worked: Some(thread_ran(&[])),
            clouds: Some(vec!["acme".to_string()]),
            ..Default::default()
        };
        assert_eq!(thread_record_of(&r, "t1"), ThreadRecord { workspace_id: "w1".to_string(), ..expected });
    });
}

// L20: a thread with a record keeps it; the clouds are told on it and a limit resume is taken off with its timer
#[test]
fn a_thread_that_has_a_record_keeps_it_and_takes_the_clouds_and_drops_a_limit_resume() {
    run_local(async {
        let r = rig();
        r.host.add_record(
            "t1",
            "w1",
            thread_record(
                json!({ "harness": "claude", "readAt": 5, "pinnedAt": 9, "limitResume": { "at": 99, "turnId": "turn-0" }, "worked": true }),
            ),
        );
        let mut p = prepared(&r);
        p.opens = false;
        p.clouds = Some(vec!["acme".to_string()]);
        ok(r.run(p).await);
        let expected = thread_record(json!({ "harness": "claude", "readAt": 5, "pinnedAt": 9, "worked": true, "clouds": ["acme"] }));
        assert_eq!(thread_record_of(&r, "t1"), ThreadRecord { workspace_id: "w1".to_string(), ..expected });
        assert_eq!(r.host.hooks.called("cancel_resume_timer"), vec!["cancel_resume_timer(t1)".to_string()]);
    });
}

// L20: a thread with no limit to resume leaves its timer alone
#[test]
fn a_thread_with_no_limit_resume_leaves_the_timer_alone() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        assert_eq!(r.host.hooks.called("cancel_resume_timer"), Vec::<String>::new());
    });
}

// L20: no TS test (the files folder is kept once)
#[test]
fn files_folder_is_kept_once() {
    run_local(async {
        let r = rig();
        *r.host.ids.uuids.borrow_mut() = vec!["turn-1".into(), "minted-1".into(), "turn-2".into(), "minted-2".into()];
        record_landings(&r, Vec::new(), vec!["/work/w1/.wsp-files/t1/req_1/report.pdf"]);
        let mut p = prepared(&r);
        p.opts.attachments = Some(vec![pdf()]);
        ok(r.run(p).await);
        assert_eq!(r.host.records.borrow()["t1"].files_in, Some(vec!["/work/w1".to_string()]));
        let h1 = r.handles.borrow()[0].clone();
        end_turn(&r.host, "turn-1", &h1);
        let mut again = prepared(&r);
        again.opens = false;
        again.opts.thread = Some("t1".to_string());
        again.opts.attachments = Some(vec![pdf()]);
        ok(r.run(again).await);
        assert_eq!(r.host.records.borrow()["t1"].files_in, Some(vec!["/work/w1".to_string()]), "the same folder is not added twice");
    });
}

// L20: the thread says it worked off the rows that ended
#[test]
fn a_thread_from_before_its_record_gets_one_off_its_ended_rows() {
    run_local(async {
        let r = rig();
        r.host.add_row(
            "sess-0",
            row(
                view(
                    "sess-0",
                    "w1",
                    json!({ "threadId": "t1", "status": "completed", "startedAt": T0 - 5000, "claudeSessionId": "sess-0" }),
                ),
                "turn-0",
            ),
        );
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-0".to_string());
        ok(r.run(p).await);
        let ended =
            [view("sess-0", "w1", json!({ "threadId": "t1", "status": "completed", "startedAt": T0 - 5000, "claudeSessionId": "sess-0" }))];
        assert_eq!(
            thread_record_of(&r, "t1"),
            ThreadRecord {
                workspace_id: "w1".to_string(),
                harness: "claude".to_string(),
                worked: Some(thread_ran(&ended)),
                ..Default::default()
            }
        );
    });
}

// ===============================================================================================================
// L21: after the handover

// L21: replaces.test.ts:32
#[test]
fn a_restart_settles_the_thread_it_replaces_and_pushes_its_head() {
    run_local(async {
        let r = rig();
        r.host.add_record("thr_old", "w1", thread_record(json!({ "harness": "claude" })));
        r.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>>(
            "LaunchCtx::preferences",
            Rc::new(|| async { Ok(prefs()) }.boxed_local()),
        );
        let marked: Rc<RefCell<Vec<(Vec<String>, ThreadStamps)>>> = Rc::new(RefCell::new(Vec::new()));
        let m = marked.clone();
        r.host.hooks.set::<dyn Fn(&[String], StampsOf, Option<&Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::mark",
            Rc::new(move |ids: &[String], stamps: StampsOf, _o: Option<&Caller>| {
                m.borrow_mut().push((ids.to_vec(), stamps("thr_old")));
                async { Ok(()) }.boxed_local()
            }),
        );
        let mut p = prepared(&r);
        p.replaces = Some("thr_old".to_string());
        ok(r.run(p).await);
        let stamps = json!({ "readAt": T0 as f64, "settledAt": T0 as f64, "settleNamedAt": T0 as f64 });
        assert_eq!(
            *marked.borrow(),
            vec![(vec!["thr_old".to_string()], stamps.as_object().unwrap().clone())],
            "settled as a settle by the person would"
        );
        assert_eq!(r.host.hooks.called("push_head"), vec!["push_head(thr_old)".to_string()]);
        assert!(at(&r.calls(), "run_turn") < at(&r.calls(), "mark") && at(&r.calls(), "mark") < at(&r.calls(), "push_head"));
        assert_eq!(r.host.records.borrow()["t1"].replaces, Some("thr_old".to_string()));
    });
}

// L21: a settle that failed is warned and the start still answers
#[test]
fn a_restart_whose_settle_fails_is_warned_and_still_answers() {
    run_local(async {
        let warned = console::capture();
        let r = rig();
        r.host.add_record("thr_old", "w1", thread_record(json!({ "harness": "claude" })));
        r.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>>(
            "LaunchCtx::preferences",
            Rc::new(|| async { Ok(prefs()) }.boxed_local()),
        );
        r.host.hooks.set::<dyn Fn(&[String], StampsOf, Option<&Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::mark",
            Rc::new(|_ids: &[String], _s: StampsOf, _o: Option<&Caller>| {
                async { Err(HostError::new("the store would not write")) }.boxed_local()
            }),
        );
        let mut p = prepared(&r);
        p.replaces = Some("thr_old".to_string());
        let handle = ok(r.run(p).await);
        assert_eq!(handle.turn_id(), "turn-1");
        assert_eq!(
            warned.lines(),
            vec![format!("thread {} was not settled after its restart: the store would not write", thread_word("thr_old"))]
        );
        assert_eq!(r.host.hooks.called("push_head"), vec!["push_head(thr_old)".to_string()], "the head is pushed all the same");
    });
}

// L21: a new session remembers where it ran
#[test]
fn a_new_session_remembers_the_target_and_a_resume_does_not() {
    run_local(async {
        let r = rig();
        ok(r.run(prepared(&r)).await);
        assert_eq!(r.host.hooks.called("remember_target"), vec!["remember_target".to_string()]);
        let r = rig();
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        ok(r.run(p).await);
        assert_eq!(r.host.hooks.called("remember_target"), Vec::<String>::new());
    });
}

// L21: a target that was not remembered is no failed start
#[test]
fn a_target_that_was_not_remembered_is_warned_and_the_start_still_answers() {
    run_local(async {
        let warned = console::capture();
        let r = rig();
        r.host.hooks.set::<dyn Fn(&WorkspaceRecord) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::remember_target",
            Rc::new(|_r: &WorkspaceRecord| async { Err(HostError::new("the store is full")) }.boxed_local()),
        );
        let handle = ok(r.run(prepared(&r)).await);
        assert_eq!(handle.outcome(), SessionStartOutcome::Started);
        assert_eq!(warned.lines(), vec!["last target for w1 not remembered: the store is full".to_string()]);
    });
}

// L21: the handle answers with the start's notice
#[test]
fn the_handle_answers_with_the_starts_notice() {
    run_local(async {
        let r = rig();
        let mut p = prepared(&r);
        p.notice = Some("the machine was started again".to_string());
        let handle = ok(r.run(p).await);
        assert_eq!(handle.notice(), Some("the machine was started again".to_string()));
        assert_eq!(
            (handle.id(), handle.turn_id(), handle.outcome()),
            ("turn-1".to_string(), "turn-1".to_string(), SessionStartOutcome::Started)
        );
        let r = rig();
        assert_eq!(ok(r.run(prepared(&r)).await).notice(), None);
    });
}

// L21, L20: the keep of the images the send carried, once the turn is handed over
#[test]
fn the_images_a_send_carried_are_kept_once_it_is_handed_over() {
    run_local(async {
        let r = rig();
        let kept: Rc<RefCell<Vec<(String, String, Option<String>, Vec<Attachment>)>>> = Rc::new(RefCell::new(Vec::new()));
        let k = kept.clone();
        r.host.hooks.set::<dyn Fn(&str, &str, Option<&str>, Vec<Attachment>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::keep_sent_images",
            Rc::new(move |w: &str, t: &str, req: Option<&str>, a: Vec<Attachment>| {
                k.borrow_mut().push((w.to_string(), t.to_string(), req.map(String::from), a));
                async { Ok(()) }.boxed_local()
            }),
        );
        let mut p = prepared(&r);
        p.opts.attachments = Some(vec![png()]);
        ok(r.run(p).await);
        settle().await;
        assert_eq!(*kept.borrow(), vec![("w1".to_string(), "t1".to_string(), Some("req_1".to_string()), vec![png()])]);
        assert!(at(&r.calls(), "run_turn") < at(&r.calls(), "keep_sent_images"));
    });
}

// ===============================================================================================================
// Z1-Z3: a start that never reached its machine

// Z1: the scoped device goes, and a refusal to take it away is warned
#[test]
fn a_failed_start_hands_its_token_back() {
    run_local(async {
        let r = rig();
        run_turn_fails(&r, "the agent would not start");
        let mut p = prepared(&r);
        p.launch.scoped_device_id = Some("dev-1".to_string());
        err(r.run(p).await);
        settle().await;
        assert_eq!(r.host.hooks.called("revoke_device"), vec!["revoke_device(dev-1)".to_string()]);
        assert_eq!(r.host.clock.pending(), 0, "hushed");
    });
}

// Z1
#[test]
fn a_token_that_would_not_be_taken_away_is_warned() {
    run_local(async {
        let warned = console::capture();
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "LaunchCtx::revoke_device",
            Rc::new(|_d: &str| async { Err(HostError::new("the daemon is away")) }.boxed_local()),
        );
        run_turn_fails(&r, "the agent would not start");
        let mut p = prepared(&r);
        p.launch.scoped_device_id = Some("dev-1".to_string());
        err(r.run(p).await);
        settle().await;
        assert_eq!(warned.lines(), vec![format!("the token of thread {} was not taken away: the daemon is away", thread_word("t1"))]);
    });
}

// Z2: thread-cap.test.ts:583 (a held start that fails tells its targets)
#[test]
fn a_held_start_that_fails_tells_its_targets() {
    run_local(async {
        let (r, ended, p) = told_held_rig(false, true);
        let stopped = "stopped before it started, while mac was full";
        err(r.run(p).await);
        assert_eq!(
            *ended.borrow(),
            vec![(
                true,
                1,
                vec!["lead-thread".to_string()],
                Some(scope("lead")),
                Some(WorkspaceOrigin::Relayed),
                result(json!({ "status": "failed", "error": stopped })),
                "turn-1:turn-1".to_string()
            )],
            "told while the row still stands and before the end goes out"
        );
        assert_eq!(r.row_keys(), vec!["sess-prev".to_string()]);
        assert_eq!(r.emitted(), vec![held_event(), end_event(stopped)]);
    });
}

// Z2: an owed start tells nobody
#[test]
fn an_owed_start_that_fails_tells_nobody() {
    run_local(async {
        let (r, ended, p) = told_held_rig(true, true);
        err(r.run(p).await);
        assert_eq!(ended.borrow().len(), 0);
        assert_eq!(r.emitted_of("session.end").len(), 1);
    });
}

// Z2: a start that was never told held tells nobody either
#[test]
fn a_start_never_told_held_tells_nobody_when_it_fails() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&str) -> bool>("LaunchCtx::folder_exists", Rc::new(|_p: &str| false));
        let told = Rc::new(Cell::new(0u32));
        let t = told.clone();
        r.host.hooks.set::<dyn Fn(&Row, &[String], Option<&ThreadScope>, Option<&WorkspaceOrigin>, TurnResult)>(
            "SessionsCtx::notify_end",
            Rc::new(move |_v: &Row, _n: &[String], _b: Option<&ThreadScope>, _r: Option<&WorkspaceOrigin>, _res: TurnResult| {
                t.set(t.get() + 1)
            }),
        );
        let mut p = prepared(&r);
        p.opts.cwd = Some("/gone/dir".to_string());
        p.notify = Some(vec!["lead-thread".to_string()]);
        err(r.run(p).await);
        assert_eq!(told.get(), 0);
    });
}

// Z3: thread-cap.test.ts:627 (a send answered held whose agent then fails leaves its reason on the thread)
#[test]
fn a_sender_told_held_whose_start_failed_leaves_the_reason_on_the_threads_latest_row() {
    run_local(async {
        let (r, _ended, p) = told_held_rig(false, true);
        let stopped = "stopped before it started, while mac was full";
        err(r.run(p).await);
        assert_eq!(r.host.rows.borrow()["sess-prev"].borrow().view.failure, Some(listed_failure(Some(stopped))));
        assert_eq!(r.host.hooks.called("persist_sessions"), vec!["persist_sessions(w1)".to_string()]);
    });
}

// Z3: a sender that waited heard the refusal, and the thread did not fail
#[test]
fn a_sender_that_waited_leaves_the_threads_latest_row_alone() {
    run_local(async {
        let (r, ended, p) = told_held_rig(false, false);
        err(r.run(p).await);
        assert_eq!(r.host.rows.borrow()["sess-prev"].borrow().view.failure, None);
        assert_eq!(r.host.hooks.called("persist_sessions"), Vec::<String>::new());
        assert_eq!(ended.borrow().len(), 1, "its targets are told all the same");
        assert_eq!(r.emitted_of("session.end").len(), 1);
    });
}

// L18, L21: launch.ts:150-165 (a resume reads what the machine holds for the session it lost, once, and says it)
#[test]
fn a_resume_reads_what_the_machine_holds_for_its_session_once_and_says_it() {
    run_local(async {
        let r = rig();
        r.host.hooks.set::<dyn Fn(&Arc<dyn HarnessAdapter>) -> Option<wsp_threads::threads::launch::SessionHeld>>(
            "SessionsCtx::session_held_of",
            Rc::new(|_a: &Arc<dyn HarnessAdapter>| {
                let held: wsp_threads::threads::launch::SessionHeld =
                    Rc::new(|session: String, _exec: Arc<dyn HarnessExec>| async move { Ok(format!("held {session}")) }.boxed_local());
                Some(held)
            }),
        );
        r.host.hooks.set::<dyn Fn(&Entry, u64) -> Arc<dyn HarnessExec>>(
            "SessionsCtx::harness_exec",
            Rc::new(|_e: &Entry, _ms: u64| Arc::new(NoExec) as Arc<dyn HarnessExec>),
        );
        let mut p = prepared(&r);
        p.opens = false;
        p.resume = Some("sess-1".to_string());
        ok(r.run(p).await);
        open(r.take_turn(0));
        let seed = r.adapter.starts.lock().unwrap()[0].seed.clone().expect("a resume carries its seed");
        let warned = console::capture();
        seed().await.expect("the seed reads");
        seed().await.expect("the seed reads again");
        settle().await;
        assert_eq!(
            warned.lines(),
            vec![wsp_core::types::internal::lost_session_log_line("sess-1", "t1", "w1", "held sess-1")],
            "the machine is read once, and its words are logged with the ids"
        );
    });
}
