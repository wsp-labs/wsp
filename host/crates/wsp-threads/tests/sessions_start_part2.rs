// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `sessions_start.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_start/world.rs");

// S18: sessions.ts:358
#[test]
fn an_unchecked_running_machine_is_proven_before_its_state_is_read() {
    run_local(async {
        let fx = fx();
        let unchecked = entry("w1", json!({ "name": "lab" }));
        unchecked.borrow_mut().unchecked = true;
        fx.host.add_entry(unchecked);
        fx.launches(go("hi"), None).await;
        assert_eq!(
            order(&fx.host, &["confine_setup", "proven", "refuse_sub_send", "copy_blocked"]),
            strings(&["confine_setup(claude)", "proven", "refuse_sub_send", "copy_blocked"])
        );

        let fx = fx_on(Agent::default());
        let napping = entry("w1", json!({ "name": "lab", "phase": "napping" }));
        napping.borrow_mut().unchecked = true;
        fx.host.add_entry(napping);
        fx.refused(go("hi"), None).await;
        assert_eq!(fx.host.hooks.called("proven"), Vec::<String>::new(), "only a running machine is proven");

        let fx = fx_on(Agent::default());
        fx.launches(go("hi"), None).await;
        assert_eq!(fx.host.hooks.called("proven"), Vec::<String>::new(), "a checked machine is not");
    })
}

// S18
#[test]
fn a_sub_machines_own_refusal_stands_and_ends_the_checks() {
    run_local(async {
        let fx = fx();
        let own = usage("wait for it", "a fix");
        let o = own.clone();
        hook!(fx.host, "SessionsCtx::refuse_sub_send", dyn Fn(&Entry) -> Result<(), HostError>, move |_e: &Entry| Err(o.clone()));
        let e = fx.refused(go("hi"), None).await;
        assert_eq!(said(&e), said(&own));
        assert_eq!(fx.host.hooks.called("copy_blocked"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// S19
#[test]
fn a_thread_not_running_is_checked_for_a_blocked_copy_its_mounts_and_a_kept_agent() {
    run_local(async {
        let fx = fx();
        let p = fx.launches(go("hi"), None).await;
        assert!(p.cleared);
        assert_eq!(
            order(&fx.host, &["copy_blocked", "worktree_mounted", "kept_check", "spawn_guard"]),
            strings(&["copy_blocked", "worktree_mounted", "kept_check(thr-new)", "spawn_guard(thread_new)"])
        );

        let fx = fx_on(Agent::default());
        hook!(fx.host, "SessionsCtx::copy_blocked", dyn Fn(&Entry) -> LocalBoxFuture<'static, Result<(), HostError>>, |_e: &Entry| async {
            Err(HostError::new("this computer is blocked"))
        }
        .boxed_local());
        let e = fx.refused(go("hi"), None).await;
        assert_eq!(said(&e), said(&err("this computer is blocked")));
        assert_eq!(order(&fx.host, &["worktree_mounted", "kept_check", "spawn_guard"]), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// S19
#[test]
fn a_running_thread_is_not_checked_that_way_and_is_not_cleared() {
    run_local(async {
        let fx = fx();
        thread_in(&fx.host, "t1", "w1", "claude", "running");
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("more") }, None).await;
        assert!(!p.cleared);
        assert_eq!(order(&fx.host, &["copy_blocked", "worktree_mounted", "kept_check"]), Vec::<String>::new());
        assert_eq!(fx.host.hooks.called("spawn_guard"), strings(&["spawn_guard(send)"]));
    })
}

// S19: session-title.test.ts:684
#[test]
fn a_blank_title_is_refused() {
    run_local(async {
        let fx = fx();
        let e = fx.refused(StartOptions { title: Some("  \n ".into()), ..go("go") }, None).await;
        assert_eq!(said(&e), said(&err(EMPTY_TITLE_LINE)));
        assert_eq!(fx.host.hooks.called("spawn_guard"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// S19
#[test]
fn a_title_is_the_first_line_trimmed_and_the_guard_names_the_act() {
    run_local(async {
        let fx = fx();
        let raw = "  Review  1671\nsecond line";
        let p = fx.launches(StartOptions { title: Some(raw.into()), ..go("go") }, None).await;
        assert_eq!(p.title, Some(title_line(raw)));

        let fx = fx_on(Agent::default());
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        let denial = usage("a thread may not start here", "ask the lead");
        let d = denial.clone();
        hook!(
            fx.host,
            "SessionsCtx::spawn_guard",
            dyn Fn(&str, Option<&Caller>) -> Result<(), HostError>,
            move |_a: &str, _o: Option<&Caller>| Err(d.clone())
        );
        let e = fx.refused(go("hi"), Some(child)).await;
        assert_eq!(said(&e), said(&denial));
        assert_eq!(fx.host.hooks.called("spawn_guard"), strings(&["spawn_guard(thread_new)"]));
        before_the_mint(&fx.host);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S20, S21, invariant 3

// S20: replaces.test.ts:136
#[test]
fn a_persons_restart_of_a_child_stands_where_the_replaced_thread_stood() {
    run_local(async {
        let restart = || StartOptions { replaces: Some("a1".into()), ..go("build the parser") };
        let finished = |fx: &Fx| {
            hook!(fx.host, "SessionsCtx::thread_facts", dyn Fn(&str) -> Option<ThreadFacts>, |t: &str| if t == "a1" {
                Some(facts("a1", "completed"))
            } else {
                None
            })
        };

        let fx = fx();
        finished(&fx);
        hook!(fx.host, "SessionsCtx::parent_of", dyn Fn(&str) -> Option<String>, |t: &str| if t == "a1" {
            Some("lead-t".to_string())
        } else {
            None
        });
        hook!(fx.host, "SessionsCtx::root_of", dyn Fn(&str) -> String, |t: &str| if t == "a1" {
            "lead-t".to_string()
        } else {
            t.to_string()
        });
        let p = fx.launches(restart(), None).await;
        assert_eq!(p.tree, TreeOf { parent_thread_id: Some("lead-t".into()), root_thread_id: Some("lead-t".into()) });
        assert_eq!(p.replaces, Some("a1".to_string()));
        assert_eq!(fx.host.hooks.called("tree_of"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        finished(&fx);
        let stands = TreeOf { parent_thread_id: Some("top".into()), root_thread_id: Some("top".into()) };
        let s = stands.clone();
        hook!(fx.host, "SessionsCtx::tree_of", dyn Fn(Option<&ThreadScope>) -> TreeOf, move |_s: Option<&ThreadScope>| s.clone());
        let p = fx.launches(restart(), None).await;
        assert_eq!(p.tree, stands, "a replaced thread with no parent is the person's own: the caller's tree");
        assert_eq!(fx.host.hooks.called("tree_of"), strings(&["tree_of"]));
    })
}

// S20
#[test]
fn an_opening_hangs_under_its_callers_root_and_a_send_reads_its_threads_own_tree() {
    run_local(async {
        let fx = fx();
        let kid = scope("kid", "w1", "lead-t");
        let seen: Rc<RefCell<Vec<Option<ThreadScope>>>> = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        hook!(fx.host, "SessionsCtx::tree_of", dyn Fn(Option<&ThreadScope>) -> TreeOf, move |s: Option<&ThreadScope>| {
            log.borrow_mut().push(s.cloned());
            TreeOf { parent_thread_id: Some("kid".into()), root_thread_id: Some("lead-t".into()) }
        });
        let p = fx.launches(go("child"), Some(by(WorkspaceOrigin::Relayed, kid.clone()))).await;
        assert_eq!(p.tree, TreeOf { parent_thread_id: Some("kid".into()), root_thread_id: Some("lead-t".into()) });
        assert_eq!(*seen.borrow(), vec![Some(kid)]);

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        hook!(fx.host, "SessionsCtx::parent_of", dyn Fn(&str) -> Option<String>, |t: &str| if t == "t1" {
            Some("kid".to_string())
        } else {
            None
        });
        hook!(fx.host, "SessionsCtx::root_of", dyn Fn(&str) -> String, |t: &str| if t == "t1" {
            "lead-t".to_string()
        } else {
            t.to_string()
        });
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("more") }, None).await;
        assert_eq!(p.tree, TreeOf { parent_thread_id: Some("kid".into()), root_thread_id: Some("lead-t".into()) });
        assert_eq!(fx.host.hooks.called("tree_of"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("more") }, None).await;
        assert_eq!(p.tree, TreeOf::default(), "a thread that is its own root with no parent carries no tree keys");
    })
}

// S20, S21, invariant 3: spawn-guard.test.ts:1309, runtime-notify.test.ts:474
#[test]
fn invariant_3_the_tree_and_notify_by_come_from_the_callers_token() {
    run_local(async {
        let lead = scope("T", "w1", "R");
        let caller = by(WorkspaceOrigin::Relayed, lead.clone());
        let tokened = || StartOptions {
            notify: Some(strings(&["me"])),
            turn_token: Some("tok-T".into()),
            started_by: Some(SessionOrigin::Agent),
            ..go("build it")
        };
        let wire = |fx: &Fx, seen: &Rc<RefCell<Vec<Option<ThreadScope>>>>| {
            fx.thread_on("T", "w1", "claude");
            hook!(fx.host, "SessionsCtx::thread_of_token", dyn Fn(&str) -> Result<String, HostError>, |t: &str| Ok(if t == "tok-T" {
                "T".to_string()
            } else {
                "someone-else".to_string()
            }));
            let log = seen.clone();
            hook!(fx.host, "SessionsCtx::tree_of", dyn Fn(Option<&ThreadScope>) -> TreeOf, move |s: Option<&ThreadScope>| {
                log.borrow_mut().push(s.cloned());
                match s {
                    Some(s) => TreeOf { parent_thread_id: Some(s.thread_id.clone()), root_thread_id: Some(s.root_thread_id.clone()) },
                    None => TreeOf::default(),
                }
            });
        };

        // An opening out of a thread: the tree and the notify scope are the caller's, and me is that thread.
        let seen = Rc::new(RefCell::new(Vec::new()));
        let fx = fx();
        wire(&fx, &seen);
        let p = fx.launches(tokened(), Some(caller.clone())).await;
        assert_eq!(*seen.borrow(), vec![Some(lead.clone())]);
        assert_eq!(p.tree, TreeOf { parent_thread_id: Some("T".into()), root_thread_id: Some("R".into()) });
        assert_eq!(p.asked, Some(strings(&["T"])));
        assert_eq!(p.notify, Some(strings(&["T"])));
        assert_eq!(p.notify_by, Some(lead.clone()));
        assert_eq!(p.notify_road, Some(WorkspaceOrigin::Relayed));
        assert_eq!(fx.host.hooks.called("thread_of_token"), strings(&["thread_of_token(tok-T)"]));

        // The same message from the person: no scope, no road, and me stays me, since nothing names a thread.
        let seen = Rc::new(RefCell::new(Vec::new()));
        let fx = fx_on(Agent::default());
        wire(&fx, &seen);
        let p = fx.launches(StartOptions { turn_token: None, ..tokened() }, None).await;
        assert_eq!(*seen.borrow(), vec![None]);
        assert_eq!(p.tree, TreeOf::default());
        assert_eq!(p.asked, Some(strings(&["me"])));
        assert_eq!((p.notify_by.clone(), p.notify_road), (None, None));
        assert_eq!(fx.host.hooks.called("thread_of_token"), Vec::<String>::new());

        // The same message by a road with no thread behind it: a road and no scope.
        let seen = Rc::new(RefCell::new(Vec::new()));
        let fx = fx_on(Agent::default());
        wire(&fx, &seen);
        let p = fx.launches(StartOptions { turn_token: None, ..tokened() }, Some(Caller::Road(WorkspaceOrigin::Paired))).await;
        assert_eq!(*seen.borrow(), vec![None]);
        assert_eq!((p.notify_by, p.notify_road), (None, Some(WorkspaceOrigin::Paired)));

        // A send into a thread that names no targets takes what the thread's opener registered, whoever sends it.
        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        let registered = NotifyOn { notify: strings(&["boss"]), by: Some(scope("boss", "w1", "boss")), road: Some(WorkspaceOrigin::Here) };
        let r = registered.clone();
        hook!(fx.host, "SessionsCtx::notify_on", dyn Fn(&str) -> Option<NotifyOn>, move |t: &str| if t == "t1" {
            Some(r.clone())
        } else {
            None
        });
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("more") }, Some(caller.clone())).await;
        assert_eq!(
            (p.asked, p.notify, p.notify_by, p.notify_road),
            (None, Some(strings(&["boss"])), registered.by.clone(), Some(WorkspaceOrigin::Here))
        );

        // A send that names targets stamps the caller's scope and road over the registered ones.
        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        fx.thread_on("T", "w1", "claude");
        let r = registered.clone();
        hook!(fx.host, "SessionsCtx::notify_on", dyn Fn(&str) -> Option<NotifyOn>, move |t: &str| if t == "t1" {
            Some(r.clone())
        } else {
            None
        });
        let p = fx.launches(StartOptions { thread: Some("t1".into()), notify: Some(strings(&["T"])), ..go("more") }, Some(caller)).await;
        assert_eq!((p.notify, p.notify_by, p.notify_road), (Some(strings(&["T"])), Some(lead), Some(WorkspaceOrigin::Relayed)));
    })
}

// S21: runtime-notify.test.ts:474, :580
#[test]
fn notify_me_is_the_callers_thread() {
    run_local(async {
        let fx = fx();
        for t in ["T1", "T2"] {
            fx.thread_on(t, "w1", "claude");
        }
        hook!(fx.host, "SessionsCtx::thread_of_token", dyn Fn(&str) -> Result<String, HostError>, |t: &str| match t {
            "tok-1" => Ok("T1".to_string()),
            "tok-2" => Ok("T2".to_string()),
            other => Ok(other.to_string()),
        });
        let run = |notify: &[&str], token: Option<&str>| StartOptions {
            notify: Some(strings(notify)),
            turn_token: token.map(String::from),
            ..go("build")
        };

        assert_eq!(fx.launches(run(&["me"], Some("tok-1")), None).await.notify, Some(strings(&["T1"])));
        assert_eq!(
            fx.launches(run(&["me"], Some("tok-2")), None).await.notify,
            Some(strings(&["T2"])),
            "each token resolves to its own thread"
        );
        assert_eq!(fx.launches(run(&["T1", "T1"], None), None).await.notify, Some(strings(&["T1"])), "named twice is one target");
        assert_eq!(
            fx.launches(run(&["me", "T1"], Some("tok-1")), None).await.notify,
            Some(strings(&["T1"])),
            "me and its own thread are one target"
        );
        assert_eq!(
            fx.launches(run(&["T2", "me", "T2"], Some("tok-1")), None).await.notify,
            Some(strings(&["T2", "T1"])),
            "the first of a target is kept, in order"
        );
        let before = fx.host.hooks.called("thread_of_token").len();
        assert_eq!(fx.launches(run(&["me"], None), None).await.notify, Some(strings(&["me"])), "no token: me is the person");
        assert_eq!(fx.host.hooks.called("thread_of_token").len(), before);
    })
}

// S21: runtime-notify.test.ts:283, :296, spawn-guard.test.ts:1309, :1351
#[test]
fn notify_refuses_itself_a_stranger_and_a_cycle() {
    run_local(async {
        // a stranger: not held, then held but not reached
        let fx = fx();
        let e = fx.refused(StartOptions { notify: Some(strings(&["other-thread"])), ..go("out") }, None).await;
        assert_eq!(said(&e), said(&err("no thread other-thread to notify")));
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        fx.thread_on("mine", "w1", "claude");
        hook!(fx.host, "SessionsCtx::reaches_row", dyn Fn(&SessionView, Option<&Caller>, Option<TreeTalk>) -> bool, |_v: &SessionView,
                                                                                                                     _o: Option<
            &Caller,
        >,
                                                                                                                     _k: Option<
            TreeTalk,
        >| false);
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        let e = fx.refused(StartOptions { notify: Some(strings(&["mine"])), ..go("out") }, Some(child)).await;
        assert_eq!(said(&e), said(&err("no thread mine to notify")));
        before_the_mint(&fx.host);

        // itself, by id and through me
        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        hook!(fx.host, "SessionsCtx::thread_of_token", dyn Fn(&str) -> Result<String, HostError>, |_t: &str| Ok("t1".to_string()));
        for opts in [
            StartOptions { thread: Some("t1".into()), notify: Some(strings(&["t1"])), ..go("again") },
            StartOptions { thread: Some("t1".into()), notify: Some(strings(&["me"])), turn_token: Some("tok".into()), ..go("again") },
        ] {
            let e = fx.refused(opts, None).await;
            assert_eq!(said(&e), said(&err("a thread cannot notify itself")));
        }
        before_the_mint(&fx.host);

        // a cycle: the target's chain reaches this thread
        let target = "bbbbbbbb-1234-5678";
        let fx = fx_on(Agent::default());
        fx.thread_on(target, "w1", "claude");
        hook!(fx.host, "SessionsCtx::notify_on", dyn Fn(&str) -> Option<NotifyOn>, move |t: &str| if t == target {
            Some(NotifyOn { notify: strings(&["c-thread"]), by: None, road: None })
        } else {
            None
        });
        let reached: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let log = reached.clone();
        hook!(fx.host, "SessionsCtx::notify_reach", dyn Fn(&[String]) -> IndexSet<String>, move |n: &[String]| {
            log.borrow_mut().push(n.to_vec());
            ["c-thread", "thr-new"].iter().map(|s| s.to_string()).collect()
        });
        let e = fx.refused(StartOptions { notify: Some(strings(&[target])), ..go("loop") }, None).await;
        assert_eq!(said(&e), said(&err("thread bbbbbbbb already notifies this thread; a cycle would run forever")));
        assert_eq!(*reached.borrow(), vec![strings(&["c-thread"])]);
        before_the_mint(&fx.host);

        // a chain that does not come back passes
        hook!(fx.host, "SessionsCtx::notify_reach", dyn Fn(&[String]) -> IndexSet<String>, |_n: &[String]| ["c-thread"]
            .iter()
            .map(|s| s.to_string())
            .collect());
        fx.host.ids.uuids.borrow_mut().push("thr-new".into());
        let p = fx.launches(StartOptions { notify: Some(strings(&[target])), ..go("no loop") }, None).await;
        assert_eq!(p.notify, Some(strings(&[target])));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S22, S23, S24

// S22, S18
#[test]
fn a_released_sub_thread_is_resumed_on_its_new_machine() {
    run_local(async {
        let fx = fx();
        let mut doc = sub_json(false);
        doc["phase"] = json!("released");
        fx.host.add_entry(entry("w1", doc));
        let moved = fx.host.add_entry(entry("w2", json!({ "name": "web", "kind": "cloud" })));
        let to = moved.clone();
        hook!(
            fx.host,
            "SessionsCtx::resume_sub",
            dyn Fn(&Entry, &str, &str, &str) -> LocalBoxFuture<'static, Result<Resumed, HostError>>,
            move |_e: &Entry, _t: &str, _h: &str, _p: &str| {
                let entry = to.clone();
                async move { Ok(Resumed { entry, preamble: Some("You run on a machine.".into()), notices: strings(&["moved", "again"]) }) }
                    .boxed_local()
            }
        );
        let minted_for: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let log = minted_for.clone();
        hook!(
            fx.host,
            "LaunchCtx::thread_launch",
            dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>,
            move |e: &Entry, _t: &str, _r: &str, _a: bool| {
                log.borrow_mut().push(id_of(e));
                async { Ok(ThreadLaunch::default()) }.boxed_local()
            }
        );
        let p = fx.launches(go("pushed it"), None).await;
        assert_eq!(id_of(&p.entry), "w2");
        assert_eq!(p.opts.prompt, sub_opening("You run on a machine.", "pushed it"));
        assert_eq!(p.notice, Some("moved\nagain".to_string()));
        assert_eq!(p.clouds, None, "a sub-machine is told no clouds");
        assert_eq!(*minted_for.borrow(), strings(&["w2"]), "the launch is minted for the machine the turn runs on");
        assert_eq!(
            order(&fx.host, &["refuse_sub_send", "resume_sub", "thread_launch"]),
            strings(&["refuse_sub_send", "resume_sub(thr-new, claude, pushed it)", "thread_launch(thr-new, thr-new, false)"])
        );
    })
}

// S23
#[test]
fn clouds_are_told_for_a_folder_workspace_that_is_no_sub_machine() {
    run_local(async {
        let fx = fx();
        assert_eq!(fx.launches(go("hi"), None).await.clouds, Some(Vec::new()), "a folder, with no clouds held");
        assert_eq!(fx.host.hooks.called("place_rows"), strings(&["place_rows"]));

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", json!({ "name": "lab", "kind": "cloud" })));
        assert_eq!(fx.launches(go("hi"), None).await.clouds, None, "a machine");
        assert_eq!(fx.host.hooks.called("place_rows"), Vec::<String>::new());
    })
}

// S24: sub-queue.test.ts:175
#[test]
fn a_queued_workspace_holds_the_start() {
    run_local(async {
        let fx = fx();
        fx.host.add_entry(entry(
            "w1",
            json!({ "name": "Second", "phase": "queued", "queue": { "since": "2026-10-01T00:00:00.000Z", "why": "slot" } }),
        ));
        let kid = scope("kid", "w1", "lead-t");
        let caller = by(WorkspaceOrigin::Relayed, kid);
        let tree = TreeOf { parent_thread_id: Some("lead-t".into()), root_thread_id: Some("lead-t".into()) };
        let t = tree.clone();
        hook!(fx.host, "SessionsCtx::tree_of", dyn Fn(Option<&ThreadScope>) -> TreeOf, move |_s: Option<&ThreadScope>| t.clone());
        hook!(fx.host, "SessionsCtx::notify_on", dyn Fn(&str) -> Option<NotifyOn>, |_t: &str| Some(NotifyOn {
            notify: strings(&["boss"]),
            by: None,
            road: None
        }));
        let held: Rc<RefCell<Vec<(String, String, TreeOf, String, Option<String>, Option<String>, Option<Vec<String>>, Option<Caller>)>>> =
            Rc::new(RefCell::new(Vec::new()));
        let log = held.clone();
        hook!(
            fx.host,
            "SessionsCtx::hold_queued",
            dyn Fn(
                &str,
                &str,
                TreeOf,
                StartOptions,
                Option<Vec<String>>,
                Option<&Caller>,
            ) -> LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>>,
            move |w: &str, t: &str, tree: TreeOf, o: StartOptions, n: Option<Vec<String>>, c: Option<&Caller>| {
                log.borrow_mut().push((
                    w.to_string(),
                    t.to_string(),
                    tree,
                    o.prompt.clone(),
                    o.title.clone(),
                    o.harness.clone(),
                    n,
                    c.cloned(),
                ));
                async { Ok(Handle::new("held-row", "held-turn")) }.boxed_local()
            }
        );
        let opts = StartOptions { title: Some("  Second  \nmore".into()), ..go("build it") };
        match fx.prepare(opts, Some(caller.clone())).await {
            Ok(Checked::Answered(h)) => assert_eq!((h.id(), h.turn_id()), ("held-row".to_string(), "held-turn".to_string())),
            Ok(Checked::Launch(_)) => panic!("a queued workspace launches nothing"),
            Err(e) => panic!("refused: {}", e.message),
        }
        let want_title = title_line("  Second  \nmore");
        assert_eq!(
            *held.borrow(),
            vec![(
                "w1".to_string(),
                "thr-new".to_string(),
                tree,
                "build it".to_string(),
                Some(want_title),
                Some("claude".to_string()),
                None,
                Some(caller)
            )],
            "the targets passed are the ones the start named, none here, never the registered ones"
        );
        assert_eq!(fx.host.hooks.called("adapter_for"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// every check passed: what the launch is handed
#[test]
fn a_plain_opening_prepares_everything_the_launch_needs() {
    run_local(async {
        let fx = fx_on(Agent { takes_servers: true, ..Default::default() });
        minted(&fx);
        let p = fx.launches(go("hello"), None).await;
        assert_eq!(p.workspace_id, "w1");
        assert_eq!(p.origin, None);
        assert_eq!(p.opts.prompt, "hello");
        assert_eq!(p.opts.harness, Some("claude".to_string()));
        assert_eq!(p.prefs, default_preferences());
        same_plan(&p.plan, &None);
        assert_eq!((p.thread_id.as_str(), p.opens, p.cleared), ("thr-new", true, true));
        assert_eq!(id_of(&p.entry), "w1");
        assert_eq!((p.replaces.clone(), p.resume.clone(), p.gone_now.clone(), p.title.clone()), (None, None, None, None));
        assert_eq!(p.tree, TreeOf::default());
        assert_eq!(
            (p.asked.clone(), p.notify.clone(), p.notify_by.clone(), p.notify_road, p.notice.clone()),
            (None, None, None, None, None)
        );
        assert_eq!(p.clouds, Some(Vec::new()));
        assert_eq!(p.turn_token, "01".repeat(16), "randomBytes(16) as hex, drawn after the thread id");
        assert_eq!(p.launch.scoped_device_id, Some("dev-1".to_string()));
        assert_eq!(p.built.harness, "claude");
        assert_eq!(p.fork_turn, None);
        assert!(p.records.is_empty());
        assert_eq!(p.mcp_servers.as_ref().map(|m| m.keys().cloned().collect::<Vec<_>>()), Some(strings(&["wsp"])));
        assert!(!p.waiting.get());

        {
            let asked = fx.asked.borrow();
            assert_eq!(asked.len(), 1);
            let (harness, ask) = &asked[0];
            assert_eq!(harness, "claude");
            let env: Vec<(&str, &str)> = ask.turn_env.iter().flatten().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            assert_eq!(env, vec![(TURN_TOKEN_ENV, p.turn_token.as_str()), (THREAD_ENV, "thr-new"), ("WSP_HOST", "h-tok")]);
            assert_eq!(ask.thread, Some("thr-new".to_string()));
            assert!(!ask.asks_until_stopped);
            let reads = ask.waiting.as_ref().expect("the waiting flag");
            assert!(!reads(), "the adapter reads the flag the row writes");
            p.waiting.set(true);
            assert!(reads(), "the adapter reads the flag the row writes");
            p.waiting.set(false);
        }
        assert_eq!(
            order(
                &fx.host,
                &[
                    "ready",
                    "preferences_held",
                    "preferences",
                    "fork_opened",
                    "entry_of",
                    "default_agent_of",
                    "agent_off",
                    "confine_setup",
                    "refuse_sub_send",
                    "copy_blocked",
                    "worktree_mounted",
                    "kept_check",
                    "spawn_guard",
                    "tree_of",
                    "notify_on",
                    "place_rows",
                    "thread_launch",
                    "adapter_for"
                ]
            ),
            strings(&[
                "ready",
                "preferences_held",
                "preferences",
                "fork_opened(w1)",
                "entry_of(w1)",
                "default_agent_of",
                "agent_off(claude)",
                "confine_setup(claude)",
                "refuse_sub_send",
                "copy_blocked",
                "worktree_mounted",
                "kept_check(thr-new)",
                "spawn_guard(thread_new)",
                "tree_of",
                "notify_on(thr-new)",
                "place_rows",
                "thread_launch(thr-new, thr-new, false)",
                "adapter_for(claude)"
            ])
        );

        let fx = fx_on(Agent::default());
        fx.launches(StartOptions { asks_until_stopped: true, ..go("hello") }, None).await;
        assert!(fx.asked.borrow()[0].1.asks_until_stopped);
    })
}

// S25: spawn-guard.test.ts:1233
#[test]
fn a_refusal_after_the_mint_revokes_the_device() {
    run_local(async {
        let revoked = |fx: &Fx| order(&fx.host, &["revoke_device"]);

        // a harness no adapter is built for
        let fx = fx();
        minted(&fx);
        hook!(
            fx.host,
            "LaunchCtx::adapter_for",
            dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>,
            |_e: &Entry, a: AdapterAsk| Err(HostError::new(format!("no adapter for {}", a.named.unwrap_or_default())))
        );
        let e = fx.refused(StartOptions { harness: Some("nope".into()), ..go("hi") }, None).await;
        settle().await;
        assert_eq!(said(&e), said(&err("no adapter for nope")));
        assert_eq!(revoked(&fx), strings(&["revoke_device(dev-1)"]));
        nothing_held(&fx.host);

        // a fork the agent cannot make
        let fx = fx_on(Agent::default());
        minted(&fx);
        forking(&fx, plan_with(true, Some("src-sess")));
        let e = fx.refused(go("hi"), None).await;
        settle().await;
        assert_eq!(said(&e), said(&usage(&forks_not_line("claude"), FORKS_NOT_FIX)));
        assert_eq!(revoked(&fx), strings(&["revoke_device(dev-1)"]));
        nothing_held(&fx.host);

        // a fork of a thread whose agent has said nothing yet
        let fx = fx_on(Agent { forks: true, ..Default::default() });
        minted(&fx);
        forking(&fx, plan_with(true, None));
        let e = fx.refused(go("hi"), None).await;
        settle().await;
        assert_eq!(said(&e), said(&refusal(&fork_refused_line("claude", ASIDE_NO_SESSION_LINE), FORK_REFUSED_FIX, Some(Kind::Conflict))));
        assert_eq!(revoked(&fx), strings(&["revoke_device(dev-1)"]));
        nothing_held(&fx.host);

        // files the harness refuses
        let fx = fx_on(Agent::default());
        minted(&fx);
        let file = Attachment { media_type: "image/png".into(), bytes: "aGk=".into(), name: Some("shot.png".into()) };
        let records = vec![attachment_record(&file)];
        let e = fx.refused(StartOptions { attachments: Some(vec![file]), ..go("hi") }, None).await;
        settle().await;
        let line = files_blocked(&records, None, "claude").expect("a harness with no road refuses an image (attachments.ts:149)");
        assert_eq!(said(&e), said(&err(&line)));
        assert_eq!(revoked(&fx), strings(&["revoke_device(dev-1)"]));
        nothing_held(&fx.host);

        // servers a harness takes none of, spawn-guard.test.ts:767
        let fx = fx_on(Agent::default());
        minted(&fx);
        let mut servers = IndexMap::new();
        servers.insert("mine".to_string(), McpServerSpec { command: "x".into(), args: Vec::new(), no_slate: None });
        let line = mcp_servers_blocked(Some(&servers), false, "claude").expect("a harness that takes none refuses servers");
        let e = fx.refused(StartOptions { mcp_servers: Some(servers), ..go("hi") }, None).await;
        settle().await;
        assert_eq!(said(&e), said(&err(&line)));
        assert_eq!(revoked(&fx), strings(&["revoke_device(dev-1)"]));
        nothing_held(&fx.host);
    })
}

// S25
#[test]
fn a_launch_with_no_scoped_device_revokes_nothing_and_a_failed_revoke_is_only_warned() {
    run_local(async {
        let fx = fx();
        hook!(
            fx.host,
            "LaunchCtx::adapter_for",
            dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>,
            |_e: &Entry, _a: AdapterAsk| Err(HostError::new("no adapter"))
        );
        let e = fx.refused(go("hi"), None).await;
        settle().await;
        assert_eq!(said(&e), said(&err("no adapter")));
        assert_eq!(fx.host.hooks.called("revoke_device"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        minted(&fx);
        hook!(
            fx.host,
            "LaunchCtx::adapter_for",
            dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>,
            |_e: &Entry, _a: AdapterAsk| Err(HostError::new("no adapter"))
        );
        hook!(fx.host, "LaunchCtx::revoke_device", dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>, |_d: &str| async {
            Err(HostError::new("the door is down"))
        }
        .boxed_local());
        let lines = console::capture();
        let e = fx.refused(go("hi"), None).await;
        settle().await;
        assert_eq!(said(&e), said(&err("no adapter")), "the refusal stands, not the revoke's failure");
        assert_eq!(lines.lines(), vec![format!("the token of thread {} was not taken away: the door is down", thread_word("thr-new"))]);
    })
}

// S25: the fork the agent can make carries its turn, by count where the agent forks by count
#[test]
fn a_fork_the_agent_can_make_carries_its_turn() {
    run_local(async {
        let fx = fx_on(Agent { forks: true, by_count: true, ..Default::default() });
        forking(&fx, plan_with(true, Some("src-sess")));
        hook!(
            fx.host,
            "SessionsCtx::fork_turn_of",
            dyn Fn(&ForkPlan, bool) -> Result<Option<ForkTurn>, HostError>,
            |_p: &ForkPlan, by_count: bool| Ok(Some(if by_count {
                ForkTurn::After(Vec::new())
            } else {
                ForkTurn::Anchor("turn-src".into())
            }))
        );
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.fork_turn, Some(ForkTurn::After(Vec::new())));
        same_plan(&p.plan, &Some(plan_with(true, Some("src-sess"))));
    })
}

// spawn-guard.test.ts:753, :767: the wsp tools ride a launch for a harness that takes servers
#[test]
fn the_wsp_tools_ride_the_launch_for_a_harness_that_takes_servers() {
    run_local(async {
        let wsp = McpServerSpec { command: "wsp".into(), args: strings(&["mcp"]), no_slate: None };
        let mine = McpServerSpec { command: "x".into(), args: Vec::new(), no_slate: None };

        let fx = fx_on(Agent { takes_servers: true, ..Default::default() });
        minted(&fx);
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.mcp_servers.map(|m| m.into_iter().collect::<Vec<_>>()), Some(vec![("wsp".to_string(), wsp.clone())]));

        let fx = fx_on(Agent { takes_servers: true, ..Default::default() });
        minted(&fx);
        let mut named = IndexMap::new();
        named.insert("mine".to_string(), mine.clone());
        let p = fx.launches(StartOptions { mcp_servers: Some(named), ..go("hi") }, None).await;
        assert_eq!(
            p.mcp_servers.map(|m| m.into_iter().collect::<Vec<_>>()),
            Some(vec![("wsp".to_string(), wsp.clone()), ("mine".to_string(), mine.clone())]),
            "the wsp server first, the caller's after"
        );

        let fx = fx_on(Agent::default());
        minted(&fx);
        let p = fx.launches(go("hi"), None).await;
        assert!(p.mcp_servers.is_none(), "a harness that takes none gets none and is not refused");

        let fx = fx_on(Agent { takes_servers: true, ..Default::default() });
        minted(&fx);
        hook!(fx.host, "SessionsCtx::tree_of", dyn Fn(Option<&ThreadScope>) -> TreeOf, |_s: Option<&ThreadScope>| TreeOf {
            parent_thread_id: Some("lead-t".into()),
            root_thread_id: Some("lead-t".into())
        });
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        let p = fx.launches(go("hi"), Some(child)).await;
        let served = wsp_threads::threads::launch::served_to(Some(&wsp), "lead-t", "thr-new").expect("a server");
        assert_eq!(
            p.mcp_servers.map(|m| m.into_iter().collect::<Vec<_>>()),
            Some(vec![("wsp".to_string(), served)]),
            "a thread under another serves its tools with no slate"
        );
        assert_eq!(
            fx.host.hooks.called("thread_launch"),
            strings(&["thread_launch(thr-new, lead-t, false)"]),
            "the launch is minted for the tree's root"
        );
    })
}
