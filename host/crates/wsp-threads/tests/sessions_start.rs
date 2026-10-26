// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_start.md. TS cases: verbs-send-notify.test.ts:91 (S2), conversations.test.ts:683 (S5, S16),
//! thread-warm-agent.test.ts:82, :186, :197, :213, :241 (S7, S8, S26), sub-spawn.test.ts:237 (S10),
//! replaces.test.ts:73, :96, :136 (S13, S20), box-lead.test.ts:183, :232 (S14), spawn-guard.test.ts:753, :767, :1233,
//! :1309, :1351 and runtime-notify.test.ts:283, :296, :474, :580 (S20, S21, invariant 3), sub-queue.test.ts:175 (S24),
//! runtime-index.test.ts:255, thread-keeps-agent.test.ts:89, thread-defaults.test.ts:185, session-title.test.ts:684,
//! box-nap.test.ts:867. Each case is ported at this unit's seam: `prepare` and `start_send` over a scripted host.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_start/world.rs");

// ---------------------------------------------------------------------------------------------------------------
// S1, S2, S3

// S1: sessions.ts:259
#[test]
fn preferences_are_the_held_copy_else_the_stores() {
    run_local(async {
        let fx = fx();
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.prefs, default_preferences());
        assert_eq!(order(&fx.host, &["ready", "preferences_held", "preferences"]), strings(&["ready", "preferences_held", "preferences"]));

        let fx = fx_on(Agent::default());
        let mut held = default_preferences();
        held.labs = true;
        let held_copy = held.clone();
        hook!(fx.host, "LaunchCtx::preferences_held", dyn Fn() -> Option<Preferences>, move || Some(held_copy.clone()));
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.prefs, held);
        assert_eq!(order(&fx.host, &["ready", "preferences_held", "preferences"]), strings(&["ready", "preferences_held"]));
    })
}

// S2: verbs-send-notify.test.ts:91
#[test]
fn a_start_sent_again_answers_the_turn_it_opened_and_starts_nothing() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let held = taken("t1");
        hook!(fx.host, "SessionsCtx::taken", dyn Fn(&str, &str) -> Option<Taken>, move |_w: &str, r: &str| if r == "req-1" {
            Some(held.clone())
        } else {
            None
        });
        let opened = Handle::new("sess-1", "turn-1");
        hook!(
            fx.host,
            "SessionsCtx::taken_turn",
            dyn Fn(&str, Taken) -> LocalBoxFuture<'static, Result<Option<Rc<dyn SessionHandle>>, HostError>>,
            move |_w: &str, _t: Taken| {
                let a = opened.clone();
                async move { Ok(Some(a)) }.boxed_local()
            }
        );
        let opts = StartOptions { thread: Some("t1".into()), request_id: Some("req-1".into()), ..go("second") };
        match fx.prepare(opts, None).await {
            Ok(Checked::Answered(h)) => {
                assert_eq!((h.id(), h.turn_id(), h.outcome()), ("sess-1".to_string(), "turn-1".to_string(), SessionStartOutcome::Started))
            }
            Ok(Checked::Launch(_)) => panic!("a start already taken is not launched again"),
            Err(e) => panic!("refused: {}", e.message),
        }
        assert_eq!(
            fx.host.hooks.calls(),
            strings(&[
                "ready",
                "preferences_held",
                "preferences",
                "taken(w1, req-1)",
                "entry_of_row(Some(\"t1\"), w1, false)",
                "taken_turn(w1)"
            ])
        );
        before_the_mint(&fx.host);
    })
}

// S2: verbs-send-notify.test.ts:91
#[test]
fn a_start_with_no_request_id_looks_nothing_up() {
    run_local(async {
        let fx = fx();
        fx.launches(go("hi"), None).await;
        assert_eq!(fx.host.hooks.called("taken"), Vec::<String>::new());
        assert_eq!(fx.host.hooks.called("taken_turn"), Vec::<String>::new());
    })
}

// S2, S12
#[test]
fn a_taken_start_the_caller_may_not_send_to_is_not_answered() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let held = taken("t1");
        hook!(fx.host, "SessionsCtx::taken", dyn Fn(&str, &str) -> Option<Taken>, move |_w: &str, _r: &str| Some(held.clone()));
        hook!(
            fx.host,
            "SessionsCtx::entry_of_row",
            dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>,
            |_a: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| async { Ok(None) }.boxed_local()
        );
        let opts = StartOptions { thread: Some("t1".into()), request_id: Some("req-1".into()), ..go("second") };
        let e = fx.refused(opts, None).await;
        assert_eq!(said(&e), said(&err("no thread t1 in this folder")));
        assert_eq!(fx.host.hooks.called("taken_turn"), Vec::<String>::new());
        assert_eq!(fx.host.hooks.called("entry_of_row").len(), 2, "once for the taken start, once for the send");
        before_the_mint(&fx.host);
    })
}

// S2
#[test]
fn a_taken_start_the_turn_cannot_answer_goes_on_as_a_new_start() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let held = taken("t1");
        hook!(fx.host, "SessionsCtx::taken", dyn Fn(&str, &str) -> Option<Taken>, move |_w: &str, _r: &str| Some(held.clone()));
        let opts = StartOptions { thread: Some("t1".into()), request_id: Some("req-1".into()), ..go("second") };
        let p = fx.launches(opts, None).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("t1", false));
        assert_eq!(order(&fx.host, &["taken_turn", "fork_opened"]), strings(&["taken_turn(w1)", "fork_opened(w1)"]));
    })
}

// S3
#[test]
fn what_fork_opened_hands_back_is_what_the_checks_read_and_carry() {
    run_local(async {
        let fx = fx();
        let plan = plan_with(false, Some("src-sess"));
        let handed = plan.clone();
        hook!(
            fx.host,
            "SessionsCtx::fork_opened",
            dyn Fn(&str, StartOptions, Option<&Caller>) -> LocalBoxFuture<'static, Result<(StartOptions, Option<ForkPlan>), HostError>>,
            move |_w: &str, o: StartOptions, _c: Option<&Caller>| {
                let plan = handed.clone();
                async move { Ok((StartOptions { prompt: format!("forked: {}", o.prompt), harness: Some("claude".into()), ..o }, Some(plan))) }.boxed_local()
            }
        );
        let p = fx.launches(go("on it"), None).await;
        assert_eq!(p.opts.prompt, "forked: on it");
        same_plan(&p.plan, &Some(plan));
        assert_eq!(order(&fx.host, &["fork_opened", "fork_turn_of"]), strings(&["fork_opened(w1)", "fork_turn_of(false)"]));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S4, S5, S6, S12

// S4, S12: runtime-index.test.ts:255, spawn-guard.test.ts:1289
#[test]
fn a_thread_of_another_workspace_is_no_thread_in_this_folder() {
    run_local(async {
        let line = said(&err("no thread t1 in this folder"));
        let named = StartOptions { thread: Some("t1".into()), ..go("x") };

        let fx = fx();
        assert_eq!(said(&fx.refused(named.clone(), None).await), line, "a thread nothing holds");
        assert_eq!(fx.host.hooks.called("entry_of"), Vec::<String>::new());
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        thread_in(&fx.host, "t1", "w2", "claude", "completed");
        assert_eq!(said(&fx.refused(named.clone(), None).await), line, "a thread whose rows are on another workspace");
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        fx.host.add_record("t1", "w2", thread_record(json!({ "harness": "claude" })));
        assert_eq!(said(&fx.refused(named.clone(), None).await), line, "a thread whose record is on another workspace");
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        hook!(
            fx.host,
            "SessionsCtx::entry_of_row",
            dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>,
            |_a: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| async { Ok(None) }.boxed_local()
        );
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        assert_eq!(said(&fx.refused(named, Some(child)).await), line, "a thread the caller's tree does not reach");
        before_the_mint(&fx.host);
    })
}

// S12
#[test]
fn an_opening_reads_its_entry_with_the_callers_origin_unless_it_may_open_there() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        for (opens_there, origin, read_with) in
            [(true, Some(child.clone()), None), (false, Some(child.clone()), Some(child.clone())), (true, None, None)]
        {
            let fx = fx();
            let live = fx.host.live("w1").expect("the workspace");
            let seen: Rc<RefCell<Vec<(String, Option<Caller>)>>> = Rc::new(RefCell::new(Vec::new()));
            let log = seen.clone();
            hook!(
                fx.host,
                "LaunchCtx::entry_of",
                dyn Fn(&str, Option<Caller>) -> LocalBoxFuture<'static, Result<Entry, HostError>>,
                move |w: &str, o: Option<Caller>| {
                    log.borrow_mut().push((w.to_string(), o));
                    let e = live.clone();
                    async move { Ok(e) }.boxed_local()
                }
            );
            hook!(
                fx.host,
                "SessionsCtx::opens_in",
                dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<bool, HostError>,
                move |_r: &WorkspaceRecord, _o: Option<&Caller>| Ok(opens_there)
            );
            fx.launches(go("hi"), origin).await;
            assert_eq!(*seen.borrow(), vec![("w1".to_string(), read_with)]);
        }
    })
}

// S12
#[test]
fn a_send_reads_its_entry_by_the_thread_and_the_workspace() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let seen: Rc<RefCell<Vec<(Option<String>, String, Option<Caller>, Option<TreeTalk>, bool)>>> = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let live = fx.host.live("w1").expect("the workspace");
        hook!(
            fx.host,
            "SessionsCtx::entry_of_row",
            dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>,
            move |at: &RowAt, o: Option<&Caller>, k: Option<TreeTalk>, b: bool| {
                log.borrow_mut().push((at.thread_id.clone(), at.workspace_id.clone(), o.cloned(), k, b));
                let e = live.clone();
                async move { Ok(Some(e)) }.boxed_local()
            }
        );
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        fx.launches(StartOptions { thread: Some("t1".into()), ..go("and more") }, Some(child.clone())).await;
        assert_eq!(*seen.borrow(), vec![(Some("t1".to_string()), "w1".to_string(), Some(child), Some(TreeTalk::Send), false)]);
        assert_eq!(fx.host.hooks.called("entry_of"), Vec::<String>::new(), "a send reads no opening's entry");
    })
}

// S5, S16: conversations.test.ts:683
#[test]
fn an_outside_conversation_opens_a_thread_only() {
    run_local(async {
        let want = said(&usage(RESUME_A_THREAD_LINE, RESUME_A_THREAD_FIX));

        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let e =
            fx.refused(StartOptions { thread: Some("t1".into()), outside: Some(outside("claude", "other", false)), ..go("x") }, None).await;
        assert_eq!(said(&e), want, "named beside a thread");
        before_the_mint(&fx.host);
        assert_eq!(fx.host.hooks.called("agent_off"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        let sent_into = StartOptions {
            r#as: Some(StartAs { thread_id: "t1".into(), turn_id: "turn-x".into() }),
            outside: Some(outside("claude", "other", false)),
            ..go("x")
        };
        let e = fx.refused(sent_into, None).await;
        assert_eq!(said(&e), want, "sent into a thread that has run");
        before_the_mint(&fx.host);
        assert_eq!(fx.host.hooks.called("agent_off"), Vec::<String>::new());
    })
}

// S6
#[test]
fn resume_is_the_latest_rows_session_else_the_transcripts_else_an_uncopied_outsides() {
    run_local(async {
        let named = StartOptions { thread: Some("t1".into()), ..go("more") };

        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        assert_eq!(fx.launches(named.clone(), None).await.resume, Some("cs-t1".to_string()), "the row's session");

        let fx = fx_on(Agent::default());
        fx.host.add_record("t1", "w1", thread_record(json!({ "harness": "claude" })));
        hook!(fx.host, "TrimmedCtx::started_as", dyn Fn(&str, &str) -> Option<String>, |_w: &str, t: &str| if t == "t1" {
            Some("sess-from-transcript".to_string())
        } else {
            None
        });
        assert_eq!(fx.launches(named, None).await.resume, Some("sess-from-transcript".to_string()), "the transcript's session");

        let fx = fx_on(Agent::default());
        let p = fx.launches(StartOptions { outside: Some(outside("claude", "outside-1", false)), ..go("go on") }, None).await;
        assert_eq!(p.resume, Some("outside-1".to_string()), "an outside conversation the start does not copy");

        let fx = fx_on(Agent::default());
        let p = fx.launches(StartOptions { outside: Some(outside("claude", "outside-1", true)), ..go("go on") }, None).await;
        assert_eq!(p.resume, None, "a copy resumes nothing");

        let fx = fx_on(Agent::default());
        assert_eq!(fx.launches(go("fresh"), None).await.resume, None);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S7, S8, S26

// S7, S8: thread-warm-agent.test.ts:82
#[test]
fn an_opening_send_claims_the_warm_agent() {
    run_local(async {
        let fx = fx();
        hook!(fx.host, "SessionsCtx::claim_warm", dyn Fn(&str, &str) -> Option<String>, |_w: &str, _h: &str| Some("thr-warm".to_string()));
        let claimed = RefCell::new(None);
        let p = match fx.prepare_with(&claimed, StartOptions { harness: Some("claude".into()), ..go("one") }, None).await {
            Ok(Checked::Launch(p)) => p,
            _ => panic!("the start was not prepared"),
        };
        assert_eq!((p.thread_id.as_str(), p.opens), ("thr-warm", true));
        assert_eq!(*claimed.borrow(), Some("thr-warm".to_string()));
        assert_eq!(fx.host.hooks.called("claim_warm"), strings(&["claim_warm(w1, claude)"]));
        assert_eq!(fx.host.ids.uuids.borrow().clone(), strings(&["thr-new"]), "no fresh id is drawn for it");
    })
}

// S7: thread-warm-agent.test.ts:197, :213, :241
#[test]
fn a_start_that_changes_the_launch_takes_no_warm_agent() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        let plan = plan_with(false, Some("src-sess"));
        let mut servers = IndexMap::new();
        servers.insert("mine".to_string(), McpServerSpec { command: "x".into(), args: Vec::new(), no_slate: None });
        let base = || StartOptions { harness: Some("claude".into()), ..go("one") };
        let variants: Vec<(&str, StartOptions, Option<Caller>, Option<ForkPlan>)> = vec![
            ("no agent named", go("one"), None, None),
            ("a thread's own start", base(), Some(child), None),
            ("a fork", base(), None, Some(plan)),
            ("a copy of an outside conversation", StartOptions { outside: Some(outside("claude", "o-1", true)), ..base() }, None, None),
            ("a title", StartOptions { title: Some("Named".into()), ..base() }, None, None),
            ("servers", StartOptions { mcp_servers: Some(servers), ..base() }, None, None),
            ("asks until stopped", StartOptions { asks_until_stopped: true, ..base() }, None, None),
        ];
        for (why, opts, origin, plan) in variants {
            let fx = fx_on(Agent { takes_servers: true, ..Default::default() });
            hook!(fx.host, "SessionsCtx::claim_warm", dyn Fn(&str, &str) -> Option<String>, |_w: &str, _h: &str| Some(
                "thr-warm".to_string()
            ));
            hook!(
                fx.host,
                "SessionsCtx::fork_opened",
                dyn Fn(&str, StartOptions, Option<&Caller>) -> LocalBoxFuture<'static, Result<(StartOptions, Option<ForkPlan>), HostError>>,
                move |_w: &str, o: StartOptions, _c: Option<&Caller>| {
                    let plan = plan.clone();
                    async move { Ok((o, plan)) }.boxed_local()
                }
            );
            let claimed = RefCell::new(None);
            let p = match fx.prepare_with(&claimed, opts, origin).await {
                Ok(Checked::Launch(p)) => p,
                _ => panic!("{why}: the start was not prepared"),
            };
            assert_eq!(fx.host.hooks.called("claim_warm"), Vec::<String>::new(), "{why}");
            assert_eq!(*claimed.borrow(), None, "{why}");
            assert_eq!(p.thread_id, "thr-new", "{why}");
        }
    })
}

// S7, S26: thread-warm-agent.test.ts:186
#[test]
fn a_refused_start_reaps_the_agent_it_claimed() {
    run_local(async {
        let fx = fx();
        hook!(fx.host, "SessionsCtx::claim_warm", dyn Fn(&str, &str) -> Option<String>, |_w: &str, _h: &str| Some("thr-warm".to_string()));
        let opts = StartOptions { harness: Some("claude".into()), notify: Some(strings(&["no-such-thread"])), ..go("one") };
        let e = fx.area.start_send("w1", opts, None).await.err().expect("the start is refused");
        assert_eq!(said(&e), said(&err("no thread no-such-thread to notify")));
        assert_eq!(order(&fx.host, &["claim_warm", "reap_kept", "sub_done"]), strings(&["claim_warm(w1, claude)", "reap_kept(thr-warm)"]));
        before_the_mint(&fx.host);
    })
}

// S26
#[test]
fn a_refused_start_that_claimed_nothing_reaps_nothing() {
    run_local(async {
        let fx = fx();
        let opts = StartOptions { notify: Some(strings(&["no-such-thread"])), ..go("one") };
        let e = fx.area.start_send("w1", opts, None).await.err().expect("the start is refused");
        assert_eq!(said(&e), said(&err("no thread no-such-thread to notify")));
        assert_eq!(order(&fx.host, &["reap_kept", "sub_done"]), Vec::<String>::new());
    })
}

// S26: sessions.ts:1494
#[test]
fn a_refused_start_on_a_sub_machine_arms_its_done() {
    run_local(async {
        let fx = fx();
        fx.host.add_entry(entry("w1", sub_json(true)));
        fx.host.add_record("sub-t", "w1", thread_record(json!({ "harness": "claude" })));
        let e = fx.area.start_send("w1", go("beside it"), None).await.err().expect("the start is refused");
        assert_eq!(said(&e), said(&sub_machine_taken_refusal("web", "sub-t")));
        assert_eq!(order(&fx.host, &["sub_done"]), strings(&["sub_done(w1)"]));

        let fx = fx_on(Agent::default());
        let e =
            fx.area.start_send("w1", StartOptions { notify: Some(strings(&["nobody"])), ..go("x") }, None).await.err().expect("refused");
        assert_eq!(said(&e), said(&err("no thread nobody to notify")));
        assert_eq!(order(&fx.host, &["sub_done"]), Vec::<String>::new(), "a workspace that is no sub-machine arms nothing");
    })
}

// S8
#[test]
fn the_thread_id_is_the_named_one_else_as_else_a_fresh_uuid() {
    run_local(async {
        let fx = fx();
        let p = fx
            .launches(StartOptions { r#as: Some(StartAs { thread_id: "as-t".into(), turn_id: "as-turn".into() }), ..go("hi") }, None)
            .await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("as-t", true));
        assert_eq!(fx.host.ids.uuids.borrow().clone(), strings(&["thr-new"]));

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("hi") }, None).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("t1", false));

        let fx = fx_on(Agent::default());
        let p = fx.launches(go("hi"), None).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("thr-new", true));
        assert!(fx.host.ids.uuids.borrow().is_empty());
    })
}

// S9
#[test]
fn a_thread_a_record_or_a_row_names_is_a_send_not_an_opening() {
    run_local(async {
        let as_t1 = || StartOptions { r#as: Some(StartAs { thread_id: "t1".into(), turn_id: "as-turn".into() }), ..go("hi") };

        let fx = fx();
        fx.host.add_record("t1", "w1", thread_record(json!({ "harness": "claude" })));
        let p = fx.launches(as_t1(), None).await;
        assert_eq!((p.opens, p.cleared), (false, true), "a record and no rows");
        assert_eq!(fx.host.hooks.called("spawn_guard"), strings(&["spawn_guard(send)"]));

        let fx = fx_on(Agent::default());
        fx.host
            .add_row("sess-t1", row(view("sess-t1", "w1", json!({ "threadId": "t1", "status": "completed", "startedAt": T0 })), "turn-t1"));
        let p = fx.launches(as_t1(), None).await;
        assert!(!p.opens, "a row and no record");
        assert_eq!(fx.host.hooks.called("spawn_guard"), strings(&["spawn_guard(send)"]));

        let fx = fx_on(Agent::default());
        let p = fx.launches(as_t1(), None).await;
        assert!(p.opens);
        assert_eq!(fx.host.hooks.called("spawn_guard"), strings(&["spawn_guard(thread_new)"]));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S10, S11

// S10: sub-spawn.test.ts:237
#[test]
fn a_sub_machine_runs_one_thread() {
    run_local(async {
        let fx = fx();
        fx.host.add_entry(entry("w1", sub_json(true)));
        fx.host.add_record("sub-t", "w1", thread_record(json!({ "harness": "claude" })));
        let e = fx.refused(go("beside it"), None).await;
        assert_eq!(said(&e), said(&sub_machine_taken_refusal("web", "sub-t")));
        assert_eq!(said(&e).2, Some(Kind::Usage));
        before_the_mint(&fx.host);
        assert_eq!(fx.host.hooks.called("entry_of"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", sub_json(true)));
        fx.host.add_record("sub-t", "w1", thread_record(json!({ "harness": "claude" })));
        fx.host.add_row(
            "sess-sub",
            row(view("sess-sub", "w1", json!({ "threadId": "sub-t", "status": "completed", "startedAt": T0 })), "turn-sub"),
        );
        let live = fx.host.live("w1").expect("the sub-machine");
        hook!(
            fx.host,
            "SessionsCtx::resume_sub",
            dyn Fn(&Entry, &str, &str, &str) -> LocalBoxFuture<'static, Result<Resumed, HostError>>,
            move |_e: &Entry, _t: &str, _h: &str, _p: &str| {
                let entry = live.clone();
                async move { Ok(Resumed { entry, preamble: None, notices: Vec::new() }) }.boxed_local()
            }
        );
        let p = fx.launches(StartOptions { thread: Some("sub-t".into()), ..go("pushed it") }, None).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("sub-t", false), "a send into its thread is not a second thread");
    })
}

// S11
#[test]
fn an_opening_the_computer_rule_keeps_out_is_refused_in_its_own_words() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w9", "lead-t"));
        let want = usage("a folder refusal", "its fix");
        let machine = usage("a machine refusal", "its fix");

        let fx = fx();
        let w = want.clone();
        hook!(
            fx.host,
            "SessionsCtx::opens_in",
            dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<bool, HostError>,
            |_r: &WorkspaceRecord, _o: Option<&Caller>| Ok(false)
        );
        hook!(
            fx.host,
            "SessionsCtx::of_threads_repository",
            dyn Fn(Option<&Caller>, &str) -> Result<bool, HostError>,
            |_o: Option<&Caller>, _p: &str| Ok(true)
        );
        hook!(
            fx.host,
            "SessionsCtx::elsewhere_refusal",
            dyn Fn(Option<&Caller>, &str, &str) -> Result<Option<HostError>, HostError>,
            move |_o: Option<&Caller>, _p: &str, _n: &str| Ok(Some(w.clone()))
        );
        let e = fx.refused(go("hi"), Some(child.clone())).await;
        assert_eq!(said(&e), said(&want), "a folder");
        assert_eq!(fx.host.hooks.called("elsewhere_refusal"), strings(&["elsewhere_refusal(p1, lab)"]));
        assert_eq!(fx.host.hooks.called("away_for"), Vec::<String>::new());
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", json!({ "name": "lab", "kind": "cloud" })));
        let m = machine.clone();
        hook!(
            fx.host,
            "SessionsCtx::opens_in",
            dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<bool, HostError>,
            |_r: &WorkspaceRecord, _o: Option<&Caller>| Ok(false)
        );
        hook!(
            fx.host,
            "SessionsCtx::of_threads_repository",
            dyn Fn(Option<&Caller>, &str) -> Result<bool, HostError>,
            |_o: Option<&Caller>, _p: &str| Ok(true)
        );
        hook!(
            fx.host,
            "SessionsCtx::away_for",
            dyn Fn(&WorkspaceRecord, Option<&Caller>, &str, &str) -> Result<Option<HostError>, HostError>,
            move |_r: &WorkspaceRecord, _o: Option<&Caller>, _a: &str, _w: &str| Ok(Some(m.clone()))
        );
        let e = fx.refused(go("hi"), Some(child.clone())).await;
        assert_eq!(said(&e), said(&machine), "a machine");
        assert_eq!(fx.host.hooks.called("away_for"), strings(&["away_for(start, w1)"]));
        assert_eq!(fx.host.hooks.called("elsewhere_refusal"), Vec::<String>::new());
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        hook!(
            fx.host,
            "SessionsCtx::opens_in",
            dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<bool, HostError>,
            |_r: &WorkspaceRecord, _o: Option<&Caller>| Ok(false)
        );
        let w = want.clone();
        hook!(
            fx.host,
            "SessionsCtx::elsewhere_refusal",
            dyn Fn(Option<&Caller>, &str, &str) -> Result<Option<HostError>, HostError>,
            move |_o: Option<&Caller>, _p: &str, _n: &str| Ok(Some(w.clone()))
        );
        fx.launches(go("hi"), Some(child.clone())).await;
        assert_eq!(fx.host.hooks.called("elsewhere_refusal"), Vec::<String>::new(), "a project of another repository reads as absent here");

        let fx = fx_on(Agent::default());
        hook!(
            fx.host,
            "SessionsCtx::of_threads_repository",
            dyn Fn(Option<&Caller>, &str) -> Result<bool, HostError>,
            |_o: Option<&Caller>, _p: &str| Ok(true)
        );
        let w = want;
        hook!(
            fx.host,
            "SessionsCtx::elsewhere_refusal",
            dyn Fn(Option<&Caller>, &str, &str) -> Result<Option<HostError>, HostError>,
            move |_o: Option<&Caller>, _p: &str, _n: &str| Ok(Some(w.clone()))
        );
        fx.launches(go("hi"), Some(child)).await;
        assert_eq!(fx.host.hooks.called("elsewhere_refusal"), Vec::<String>::new(), "a workspace the caller may open in is not asked");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S13

// S13: replaces.test.ts:96
#[test]
fn a_restart_only_opens() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let e = fx.refused(StartOptions { thread: Some("t1".into()), replaces: Some("a1".into()), ..go("and the parser") }, None).await;
        assert_eq!(said(&e), said(&usage(RESTART_OPENS_LINE, RESTART_OPENS_FIX)));
        assert_eq!(fx.host.hooks.called("thread_facts"), Vec::<String>::new(), "the rule is read before the replaced thread is");
        before_the_mint(&fx.host);
    })
}

// S13: replaces.test.ts:73
#[test]
fn a_restart_of_a_busy_thread_is_refused() {
    run_local(async {
        let fx = fx();
        hook!(fx.host, "SessionsCtx::thread_facts", dyn Fn(&str) -> Option<ThreadFacts>, |t: &str| if t == "a1" {
            Some(facts("a1", "running"))
        } else {
            None
        });
        let e = fx.refused(StartOptions { replaces: Some("a1".into()), ..go("build the parser") }, None).await;
        assert_eq!(said(&e), said(&usage(&replaces_working_line("a1"), &replaces_working_fix("a1"))));
        assert_eq!(order(&fx.host, &["entry_of", "thread_facts", "agent_off"]).first().map(String::as_str), Some("entry_of(w1)"));
        assert_eq!(fx.host.hooks.called("agent_off"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S14, S15

// S14: box-lead.test.ts:183, :232. The caller acts on its own box and not on w1; each project names its computer.
#[test]
fn a_send_across_computers_carries_no_files_and_no_follow() {
    run_local(async {
        let road = Some(AttachmentRoad::Inline);
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "wbox", "lead-t"));
        let across =
            |fx: &Fx| {
                fx.host.add_entry(entry("wbox", json!({ "name": "box", "kind": "place", "project": "pbox" })));
                fx.thread_on("t1", "w1", "claude");
                hook!(
                    fx.host,
                    "SessionsCtx::acts_on",
                    dyn Fn(&WorkspaceRecord, Option<&Caller>) -> bool,
                    |r: &WorkspaceRecord, _o: Option<&Caller>| r.id != "w1"
                );
                hook!(fx.host, "SessionsCtx::project_computer_name", dyn Fn(&str) -> Result<String, HostError>, |p: &str| Ok(
                    if p == "pbox" { "hetzner".to_string() } else { "default".to_string() }
                ));
            };
        let file = Attachment { media_type: "text/plain".into(), bytes: "aGk=".into(), name: Some("notes.sh".into()) };
        let sent = |o: StartOptions| StartOptions { thread: Some("t1".into()), ..o };

        let fx = fx_on(Agent { road, ..Default::default() });
        across(&fx);
        let e = fx.refused(sent(StartOptions { attachments: Some(vec![file.clone()]), ..go("see the file") }), Some(child.clone())).await;
        assert_eq!(said(&e), said(&usage(&send_files_across_line("hetzner", "default", "t1"), SEND_FILES_ACROSS_FIX)));
        before_the_mint(&fx.host);

        let e = fx.refused(sent(StartOptions { followed: Some(true), ..go("which branch?") }), Some(child.clone())).await;
        assert_eq!(said(&e), said(&usage(&wait_across_line("hetzner", "default", "t1"), WAIT_ACROSS_FIX)));
        before_the_mint(&fx.host);

        let p = fx.launches(sent(go("the box is done")), Some(child.clone())).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("t1", false), "its words alone go through");

        let fx = fx_on(Agent { road, ..Default::default() });
        across(&fx);
        let p = fx.launches(sent(StartOptions { attachments: Some(vec![file.clone()]), followed: Some(true), ..go("person") }), None).await;
        assert_eq!(p.thread_id, "t1", "the person's own send carries both");

        let fx = fx_on(Agent { road, ..Default::default() });
        fx.host.add_entry(entry("wbox", json!({ "name": "box", "kind": "place", "project": "pbox" })));
        fx.thread_on("t1", "w1", "claude");
        let p =
            fx.launches(sent(StartOptions { attachments: Some(vec![file]), followed: Some(true), ..go("full reach") }), Some(child)).await;
        assert_eq!(p.thread_id, "t1", "a caller that acts on the workspace carries both");
    })
}

// S15
#[test]
fn a_worktree_removed_by_hand_reads_gone() {
    run_local(async {
        let tree = |extra: Value| {
            let mut w = json!({ "path": "/wt/a", "made": true });
            for (k, v) in extra.as_object().expect("object") {
                w[k] = v.clone();
            }
            json!({ "name": "lab", "worktree": w })
        };

        let fx = fx();
        fx.host.add_entry(entry("w1", tree(json!({}))));
        hook!(fx.host, "LaunchCtx::folder_exists", dyn Fn(&str) -> bool, |_p: &str| false);
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.gone_now, Some("/wt/a".to_string()));
        assert_eq!(fx.host.hooks.called("worktree_gone"), strings(&["worktree_gone(removed, true)"]));
        assert_eq!(order(&fx.host, &["worktree_gone", "agent_off"]), strings(&["worktree_gone(removed, true)", "agent_off(claude)"]));

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", tree(json!({}))));
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.gone_now, None, "a folder that is there");
        assert_eq!(fx.host.hooks.called("worktree_gone"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", tree(json!({ "gone": true }))));
        hook!(fx.host, "LaunchCtx::folder_exists", dyn Fn(&str) -> bool, |_p: &str| false);
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.gone_now, None, "a worktree already read as gone");
        assert_eq!(fx.host.hooks.called("worktree_gone"), Vec::<String>::new());
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S16, S17

// S16: thread-keeps-agent.test.ts:89, :119
#[test]
fn a_thread_keeps_its_agent() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "codex");
        let e = fx.refused(StartOptions { thread: Some("t1".into()), harness: Some("claude".into()), ..go("two") }, None).await;
        assert_eq!(said(&e), said(&err(&thread_runs_on_line("codex", "claude"))));
        before_the_mint(&fx.host);
        assert_eq!(fx.host.hooks.called("agent_off"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "codex");
        hook!(fx.host, "LaunchCtx::default_agent_of", dyn Fn(&Preferences, &Entry) -> String, |_p: &Preferences, _e: &Entry| "claude"
            .to_string());
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("two") }, None).await;
        assert_eq!(p.opts.harness, Some("codex".to_string()), "a send naming none runs on the thread's agent");
        assert_eq!(fx.host.hooks.called("default_agent_of"), Vec::<String>::new());
        let p = fx.launches(StartOptions { thread: Some("t1".into()), harness: Some("codex".into()), ..go("two") }, None).await;
        assert_eq!(p.opts.harness, Some("codex".to_string()), "naming its own agent is no refusal");
    })
}

// S16: permission-relay.test.ts:251
#[test]
fn a_send_drops_its_access() {
    run_local(async {
        let picks = || StartOptions { permission_mode: Some("plan".into()), access: Some(AccessChoice::Full), ..go("two") };

        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..picks() }, None).await;
        assert_eq!(p.opts.permission_mode, None);
        assert!(p.opts.access.is_none());

        let fx = fx_on(Agent::default());
        let p = fx.launches(picks(), None).await;
        assert_eq!(p.opts.permission_mode, Some("plan".to_string()), "an opening keeps what it named");
        assert!(matches!(p.opts.access, Some(AccessChoice::Full)));
    })
}

// S17: thread-defaults.test.ts:185
#[test]
fn an_agent_turned_off_is_refused() {
    run_local(async {
        let fx = fx();
        hook!(fx.host, "LaunchCtx::agent_off", dyn Fn(&Entry, &str) -> bool, |_e: &Entry, h: &str| h == "claude");
        hook!(fx.host, "SessionsCtx::agent_label", dyn Fn(&str) -> String, |_h: &str| "Claude Code".to_string());
        hook!(fx.host, "SessionsCtx::computer_of", dyn Fn(&Entry) -> String, |_e: &Entry| "mac".to_string());
        let e = fx.refused(StartOptions { harness: Some("claude".into()), ..go("one") }, None).await;
        // agentOffLine (thread-defaults.ts:374) has no wsp-words function yet, so the sentence is spelled out.
        assert_eq!(said(&e), ("Claude Code is off on mac, so no thread there runs it".to_string(), None, Some(Kind::Usage)));
        assert_eq!(fx.host.hooks.called("confine_setup"), Vec::<String>::new());
        assert_eq!(fx.host.hooks.called("refuse_sub_send"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// S17
#[test]
fn the_agent_is_the_threads_else_the_outsides_else_the_asked_else_the_default() {
    run_local(async {
        let codex_default = |fx: &Fx| {
            hook!(fx.host, "LaunchCtx::default_agent_of", dyn Fn(&Preferences, &Entry) -> String, |_p: &Preferences, _e: &Entry| "codex"
                .to_string())
        };

        let fx = fx();
        codex_default(&fx);
        let p = fx.launches(go("one"), None).await;
        assert_eq!(p.opts.harness, Some("codex".to_string()), "none named: the default");
        assert_eq!(
            order(&fx.host, &["default_agent_of", "agent_off", "confine_setup"]),
            strings(&["default_agent_of", "agent_off(codex)", "confine_setup(codex)"])
        );
        assert_eq!(fx.asked.borrow()[0].0, "codex");

        let fx = fx_on(Agent::default());
        codex_default(&fx);
        let p = fx.launches(StartOptions { harness: Some("claude".into()), ..go("one") }, None).await;
        assert_eq!(p.opts.harness, Some("claude".to_string()), "named");
        assert_eq!(fx.host.hooks.called("default_agent_of"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        let p = fx
            .launches(StartOptions { harness: Some("claude".into()), outside: Some(outside("codex", "o-1", false)), ..go("one") }, None)
            .await;
        assert_eq!(p.opts.harness, Some("codex".to_string()), "the outside conversation's agent beats the asked one");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S18, S19

// S18: sub-queue.test.ts:308
#[test]
fn a_deleted_workspace_refuses_the_start() {
    run_local(async {
        let fx = fx();
        let ghost: Entry = entry("w1", json!({ "name": "Second" }));
        hook!(
            fx.host,
            "LaunchCtx::entry_of",
            dyn Fn(&str, Option<Caller>) -> LocalBoxFuture<'static, Result<Entry, HostError>>,
            move |_w: &str, _o: Option<Caller>| {
                let e = ghost.clone();
                async move { Ok(e) }.boxed_local()
            }
        );
        // The workspace is gone from `live` by the time the check runs (sessions.ts:351), though its entry was read.
        hook!(fx.host, "SessionsCtx::live_entry", dyn Fn(&str) -> Option<Entry>, |_w: &str| None);
        let e = fx.refused(go("hi"), None).await;
        assert_eq!(said(&e), said(&err(&wsp_words::place_state::deleted_before_start_line("Second"))));
        assert_eq!(fx.host.hooks.called("refuse_sub_send"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// S18: box-nap.test.ts:867
#[test]
fn a_napped_workspace_refuses_in_its_words() {
    run_local(async {
        let fx = fx();
        fx.host.add_entry(entry("w1", json!({ "name": "x", "phase": "napping" })));
        let e = fx.refused(go("go"), None).await;
        let state =
            workspace_state(&WorkspaceStateInput { phase: WorkspacePhase::Napping, machine_state: None, reach: None, wake_refused: None });
        let want = send_refusal(&SendRefusalKind::State(state), None, Some("x")).expect("a napping workspace refuses a send");
        assert_eq!(said(&e), said(&err(&want)));
        assert_eq!(fx.host.hooks.called("refuse_sub_send"), strings(&["refuse_sub_send"]));
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", json!({ "name": "x", "phase": "gone", "gone": "the provider lost it" })));
        let e = fx.refused(go("go"), None).await;
        let state =
            workspace_state(&WorkspaceStateInput { phase: WorkspacePhase::Gone, machine_state: None, reach: None, wake_refused: None });
        let want =
            send_refusal(&SendRefusalKind::State(state), Some("the provider lost it"), Some("x")).expect("a gone workspace refuses a send");
        assert_eq!(said(&e), said(&err(&want)));
        before_the_mint(&fx.host);
    })
}
