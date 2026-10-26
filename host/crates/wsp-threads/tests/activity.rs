// SPDX-License-Identifier: AGPL-3.0-only
//! spec/activity.md. TS cases: host/test/activity.test.ts (24), runtime/test/box-activity.test.ts (7).
//!
//! The TS drives the command line over a fake daemon; here `Activity::new(host)` runs over the `FakeHost`'s
//! `ActivityCtx` and a fake daemon of this file (a port of host/test/fake-procs.ts) answers each channel the host
//! opens, logging every frame it got in order and whether the channel was closed.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("activity/world.rs");

// ===============================================================================================================
// One read of this computer
// ===============================================================================================================

// AC10-AC14, AC25: activity.test.ts:48
#[test]
fn lists_a_threads_tree_then_wsps_own_and_leaves_out_what_nobody_started() {
    run_local(async {
        let lab = Lab::main();
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(view, view_json(THIS_COMPUTER, T0, vec![main_thread(T1, "start the dev server")], vec![daemon_row()], vec![]));
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames())]);
        assert_eq!(lab.host.hooks.called("local_channel"), vec!["local_channel(false)"]);
        let one = view_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(
            one,
            view_json(THIS_COMPUTER, T0, vec![main_thread(T1, "start the dev server")], vec![], vec![]),
            "a thread asked alone has no wsp or unowned rows"
        );
    });
}

// AC14: activity.test.ts:81 (the host's read whole, with its host pid and this computer's name)
#[test]
fn a_read_names_this_computer_and_stamps_the_time_of_the_snapshot() {
    run_local(async {
        let lab = Lab::main();
        lab.host.clock.advance(7_000);
        lab.here.set_procs(vec![daemon_proc()]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(view, view_json(THIS_COMPUTER, T0 + 7_000, vec![], vec![daemon_row()], vec![]));
    });
}

// AC10-AC13: activity.test.ts:93
#[test]
fn lists_the_newest_thread_first() {
    run_local(async {
        let lab = Lab::new();
        lab.local_thread(T1, "w1", "one", STARTED, "completed");
        lab.local_thread(T2, "w2", "two", STARTED + 1_000, "completed");
        lab.here
            .set_procs(vec![proc_row(4100, json!({ "owner": owned("fork", T1) })), proc_row(4101, json!({ "owner": owned("fork", T2) }))]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        let one = |id: &str, title: &str, ws: &str, pid: u32| {
            thread_json(id, title, ws, 1_000_000, 0, vec![shown(pid, 1, 0, STARTED, "other", "node", "node", 1_000_000, 0, &[])])
        };
        assert_eq!(view["threads"], json!([one(T2, "two", "w2", 4101), one(T1, "one", "w1", 4100)]));
    });
}

// AC13: the rule alone (no TS case)
#[test]
fn running_threads_come_first_and_equal_starts_keep_their_order() {
    run_local(async {
        let lab = Lab::new();
        lab.local_thread(T1, "w1", "a", STARTED + 2_000, "completed");
        lab.local_thread(T2, "w2", "b", STARTED, "running");
        lab.local_thread(T3, "w3", "c", STARTED + 2_000, "completed");
        lab.here.set_procs(vec![
            proc_row(4100, json!({ "owner": owned("fork", T1) })),
            proc_row(4101, json!({ "owner": owned("fork", T2) })),
            proc_row(4102, json!({ "owner": owned("fork", T3) })),
        ]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(ids_of(&view), vec![T2, T1, T3]);
    });
}

// AC11: the scope's own workspace beats the latest row's, a warm scope names no thread
#[test]
fn a_scope_names_its_workspace_and_a_warm_one_is_left_out() {
    run_local(async {
        let lab = Lab::main();
        lab.scopes(vec![
            json!({ "thread": T1, "workspace": "w-from-scope" }),
            json!({ "thread": "warm-agent-0000", "warm": true, "agents": [4100] }),
        ]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(
            view["threads"],
            json!([thread_json(T1, "start the dev server", "w-from-scope", 470_000_000, 13, vec![vite(), esbuild(), agent(), tool()])])
        );
        assert!(
            !lab.host.hooks.called("thread_facts").contains(&"thread_facts(warm-agent-0000)".to_string()),
            "a warm agent no send claimed has no title read"
        );
    });
}

// AC11: the title is the thread's word where the facts hold none
#[test]
fn a_thread_with_no_facts_is_titled_by_its_word() {
    run_local(async {
        let lab = Lab::main();
        lab.facts.borrow_mut().clear();
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(view["threads"][0]["title"], json!(thread_word(T1)));
    });
}

// AC5
#[test]
fn ports_are_grouped_by_pid_and_sorted() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.ports.borrow_mut() = json!([
            { "port": 5174, "pid": 4200, "uid": 0, "loopback": false },
            { "port": 5173, "pid": 4200, "uid": 0, "loopback": false },
            { "port": 9000, "pid": "4300", "uid": 0, "loopback": false },
            { "pid": 4300, "uid": 0, "loopback": false },
            { "port": 5175, "pid": 4100, "uid": 0, "loopback": false },
        ]);
        let view = view_of(lab.read(ask_of(Some(T1), None), None).await);
        let rows = &view["threads"][0]["processes"];
        let ports: Vec<Value> = rows.as_array().unwrap().iter().map(|p| json!([p["pid"], p["ports"]])).collect();
        assert_eq!(ports, vec![json!([4200, [5173, 5174]]), json!([4300, []]), json!([4100, [5175]]), json!([4150, []])]);
    });
}

// AC5
#[test]
fn a_ports_answer_that_is_not_a_list_is_no_ports() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.ports.borrow_mut() = json!("nothing");
        let view = view_of(lab.read(ask_of(Some(T1), None), None).await);
        let all: Vec<Value> = view["threads"][0]["processes"].as_array().unwrap().iter().map(|p| p["ports"].clone()).collect();
        assert_eq!(all, vec![json!([]), json!([]), json!([]), json!([])]);
    });
}

// AC5: ports are asked first, before any process frame
#[test]
fn the_listeners_are_asked_before_the_processes() {
    run_local(async {
        let lab = Lab::main();
        lab.read(ActivityAsk::default(), None).await.expect("a read");
        assert_eq!(lab.here.seq.borrow().clone(), vec!["c0:ports.watch", "c0:proc.watch", "c0:close"]);
    });
}

// AC6, AC8: activity.test.ts:363
#[test]
fn a_daemon_with_no_list_is_watched_instead() {
    run_local(async {
        let lab = Lab::main();
        lab.here.lists.set(false);
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await.expect_err("the settle is refused");
        let left = left_thread(T1, "start the dev server", "w1", 470_000_000, 13, vec![vite(), esbuild()], None);
        assert_eq!(refusal, processes_left_refusal(LeftVerb::Settle, &[left], Some(&IndexSet::new())));
        assert_eq!(
            lab.here.chans(),
            vec![chan(false, vec![json!({ "op": "ports.watch" }), json!({ "op": "proc.list" }), json!({ "op": "proc.watch" })])]
        );
    });
}

// AC6
#[test]
fn any_other_failure_of_the_list_is_the_reads() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.list_error.borrow_mut() = Some(refused(None, "the sampler is down"));
        let said = said_of(lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await);
        assert_eq!(said, ("the sampler is down".to_string(), None, None));
        assert_eq!(lab.here.chans(), vec![chan(false, list_frames())], "no watch after a list that failed otherwise");
    });
}

// AC7: a one-shot read of a box that has no list is refused, never watched
#[test]
fn a_read_that_may_not_watch_fails_with_the_unknown_op_line() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.box_thread(T1, "w1", "go", STARTED);
        lab.boxd.lists.set(false);
        let said = said_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(said, usage_said(&box_behind_activity_line("hetzner"), BOX_BEHIND_ACTIVITY_FIX));
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames())]);
    });
}

// AC8
#[test]
fn no_snapshot_in_ten_seconds_fails_the_read() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.watch_events.borrow_mut() = Some(vec![]);
        let door = lab.door.clone();
        let task = tokio::task::spawn_local(async move { door.read(ActivityAsk::default(), None).await });
        settle().await;
        lab.host.clock.advance(9_999);
        settle().await;
        assert!(!task.is_finished(), "the read waits out ten seconds");
        lab.host.clock.advance(1);
        settle().await;
        assert!(task.is_finished(), "the read gives up on the clock, not on a real timer");
        let said = said_of(task.await.expect("the task ran"));
        assert_eq!(said, (format!("{THIS_COMPUTER}'s daemon sent no processes in 10 s"), None, None));
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames())]);
        assert_eq!(lab.host.clock.pending(), 0, "the wait's timer is cleared");
    });
}

// AC8: the first snapshot ends the wait and clears the timer
#[test]
fn a_snapshot_inside_the_wait_is_the_read_and_clears_the_timer() {
    run_local(async {
        let lab = Lab::main();
        lab.read(ActivityAsk::default(), None).await.expect("a read");
        assert_eq!(lab.host.clock.pending(), 0);
    });
}

// AC8
#[test]
fn events_of_other_types_and_snapshots_that_do_not_parse_are_ignored() {
    run_local(async {
        let lab = Lab::main();
        let mut good = lab.here.snapshot();
        good["type"] = json!("proc.snapshot");
        *lab.here.watch_events.borrow_mut() = Some(vec![
            json!({ "type": "proc.changes", "at": 1, "daemon": 1, "total": 0, "procs": [] }),
            json!({ "type": "proc.snapshot", "procs": "not rows" }),
            good,
        ]);
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(view, view_json(THIS_COMPUTER, T0, vec![main_thread(T1, "start the dev server")], vec![daemon_row()], vec![]));
    });
}

// AC9
#[test]
fn the_channel_is_closed_after_a_failed_read() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.ports_error.borrow_mut() = Some(refused(None, "boom"));
        let said = said_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(said, ("boom".to_string(), None, None));
        assert_eq!(lab.here.chans(), vec![chan(false, vec![json!({ "op": "ports.watch" })])]);
    });
}

// AC4
#[test]
fn a_host_with_no_daemon_here_reads_nothing() {
    run_local(async {
        let lab = Lab::main();
        lab.host.hooks.set::<dyn Fn() -> bool>("ActivityCtx::has_local_daemon", Rc::new(|| false));
        let none = (wsp_threads::activity::NO_DAEMON_HERE.to_string(), None, None);
        assert_eq!(said_of(lab.read(ActivityAsk::default(), None).await), none);
        assert_eq!(said_of(lab.end(4200, STARTED + 1_000, None).await), none);
        assert_eq!(said_of(lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await), none);
        assert_eq!(lab.host.hooks.called("local_channel"), Vec::<String>::new());
        assert_eq!(lab.here.chans(), vec![]);
    });
}

// AC22: the read waits for the host
#[test]
fn a_read_and_an_end_wait_for_the_host_before_anything_else() {
    run_local(async {
        let lab = Lab::main();
        lab.host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "ActivityCtx::ready",
            Rc::new(|| async { Err(HostError::new("the host is not ready")) }.boxed_local()),
        );
        let not_ready = ("the host is not ready".to_string(), None, None);
        assert_eq!(said_of(lab.read(ActivityAsk::default(), None).await), not_ready);
        assert_eq!(said_of(lab.end(4200, STARTED + 1_000, None).await), not_ready);
        assert_eq!(lab.host.hooks.calls(), vec!["ready", "ready"]);
        assert_eq!(lab.here.chans(), vec![]);
    });
}

// ===============================================================================================================
// Which computer
// ===============================================================================================================

// AC1
#[test]
fn this_computers_name_in_any_case_is_here() {
    run_local(async {
        let lab = Lab::main();
        for computer in [None, Some("here"), Some("this computer"), Some("THIS COMPUTER")] {
            let view = view_of(lab.read(ask_of(None, computer), None).await);
            assert_eq!(view["computer"], json!(THIS_COMPUTER), "{computer:?}");
        }
        let named = Lab::main();
        named.host.hooks.set::<dyn Fn() -> bool>("ActivityCtx::has_door", Rc::new(|| true));
        named.host.hooks.set::<dyn Fn(&str) -> Option<String>>(
            "ActivityCtx::place_name",
            Rc::new(|p: &str| (p == "here").then(|| "Mac mini".to_string())),
        );
        for computer in [None, Some("here"), Some("MAC MINI"), Some("mac mini"), Some("this computer")] {
            let view = view_of(named.read(ask_of(None, computer), None).await);
            assert_eq!(view["computer"], json!("Mac mini"), "{computer:?}");
        }
        assert_eq!(named.here.chans().len(), 5);
    });
}

// AC2: activity.test.ts:395
#[test]
fn a_computer_this_host_does_not_hold_is_refused() {
    run_local(async {
        let lab = Lab::main();
        let said = said_of(lab.read(ask_of(None, Some("nowhere")), None).await);
        assert_eq!(
            said,
            usage_said(&no_such_place_refusal("nowhere", &[]), "Name one wsp computers lists."),
            "a host with no door holds no rows"
        );
        lab.join_box();
        let said = said_of(lab.read(ask_of(None, Some("nowhere")), None).await);
        assert_eq!(said, usage_said(&no_such_place_refusal("nowhere", &["hetzner".to_string()]), "Name one wsp computers lists."));
        assert_eq!(lab.here.chans(), vec![]);
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC2
#[test]
fn a_computer_matches_by_id_or_by_name_in_any_case() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.box_thread(T1, "w1", "go", STARTED);
        for computer in ["box", "hetzner", "HETZNER"] {
            let view = view_of(lab.read(ask_of(None, Some(computer)), None).await);
            assert_eq!(view["computer"], json!("hetzner"), "{computer}");
        }
        assert_eq!(lab.boxd.chans().len(), 3);
    });
}

// AC2
#[test]
fn a_box_this_host_has_not_joined_is_not_read() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::joined", Rc::new(|_p: &str| false));
        let said = said_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(said, usage_said(&not_read_line("hetzner"), NOT_READ_FIX));
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC3: box-activity.test.ts:64
#[test]
fn an_end_on_a_box_signals_nothing() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.box_thread(T1, "w1", "go", STARTED);
        lab.boxd.set_procs(vec![box_row(7100, 1, "node", "node vite", &thread_cgroup(T1))]);
        let mut ask = end_of(7100, STARTED + 7_100);
        ask.computer = Some("hetzner".into());
        let said = said_of(lab.door.end(ask, None).await);
        assert_eq!(said, usage_said(&box_end_line("hetzner"), BOX_END_FIX));
        assert_eq!(lab.boxd.chans(), vec![]);
        assert_eq!(lab.boxd.kills.borrow().clone(), Vec::<Value>::new());
        assert_eq!(lab.host.hooks.called("local_channel"), Vec::<String>::new());
        assert_eq!(lab.boxd.procs.borrow().len(), 1);
    });
}

// ===============================================================================================================
// Who reaches what
// ===============================================================================================================

// AC15, AC25: activity.test.ts:374
#[test]
fn a_relayed_caller_reads_nothing_outside_a_thread_and_ends_none() {
    run_local(async {
        let lab = Lab::main();
        lab.here
            .procs
            .borrow_mut()
            .push(proc_row(4700, json!({ "cmdline": "node watch.js", "owner": owned("env", "t-gone"), "startedAt": STARTED + 7_000 })));
        let unowned = shown(4700, 1, 0, STARTED + 7_000, "other", "node", "node watch.js", 1_000_000, 0, &[]);
        let person = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(person["unowned"], json!([unowned]));
        lab.drives(|_k: WorkspaceKind, c: Option<&Caller>| !is_relayed(c));
        lab.host.hooks.set::<dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<Option<HostError>, HostError>>(
            "ActivityCtx::refusal_for",
            Rc::new(|_r: &WorkspaceRecord, c: Option<&Caller>| {
                Ok(is_relayed(c).then(|| HostError::with_kind("a relayed road drives no local workspace", Kind::Usage)))
            }),
        );
        let seen = view_of(lab.read(ActivityAsk::default(), Some(relayed())).await);
        assert_eq!(seen, view_json(THIS_COMPUTER, T0, vec![], vec![], vec![]));
        let said = said_of(lab.end(4700, STARTED + 7_000, Some(relayed())).await);
        assert_eq!(said, (joined(&not_threads_line(4700), NOT_THREADS_FIX), Some(NOT_THREADS_FIX.to_string()), Some(Kind::Usage)));
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.chans().len(), 3, "two reads and the end's read; no kill channel");
    });
}

// AC15, AC25: a thread reads its own threads and no command line outside them
#[test]
fn a_thread_reads_its_own_threads_and_not_what_runs_outside_them() {
    run_local(async {
        let lab = Lab::main();
        let view = view_of(lab.read(ActivityAsk::default(), Some(scoped_by(T1, "w1"))).await);
        assert_eq!(view, view_json(THIS_COMPUTER, T0, vec![main_thread(T1, "start the dev server")], vec![], vec![]));
    });
}

// AC15: drives is asked with the kind of the computer read
#[test]
fn what_runs_outside_a_thread_is_asked_of_what_the_caller_drives_on_that_kind() {
    run_local(async {
        let lab = Lab::main();
        lab.box_thread(T2, "w2", "boxed", STARTED + 10);
        lab.join_box();
        lab.boxd.set_procs(vec![daemon_proc()]);
        let kinds: Rc<RefCell<Vec<WorkspaceKind>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = kinds.clone();
        lab.drives(move |k: WorkspaceKind, _c: Option<&Caller>| {
            seen.borrow_mut().push(k);
            k != WorkspaceKind::Place
        });
        let here = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(here["wsp"], json!([daemon_row()]));
        let there = view_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(there, view_json("hetzner", T0, vec![], vec![], vec![]), "a box the caller does not drive shows no wsp row");
        assert!(kinds.borrow().contains(&WorkspaceKind::Local) && kinds.borrow().contains(&WorkspaceKind::Place));
    });
}

// AC16, AC22: a thread the caller may not settle is not found
#[test]
fn a_thread_asked_that_the_caller_does_not_reach_is_not_found() {
    run_local(async {
        let lab = Lab::main();
        lab.host
            .hooks
            .set::<dyn Fn(&str, Option<&Caller>) -> bool>("ActivityCtx::settles_thread", Rc::new(|_t: &str, _c: Option<&Caller>| false));
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, nf(&format!("no thread {}", thread_word(T1))));
        assert_eq!(lab.here.chans(), vec![], "nothing is read for a thread that is not reached");
    });
}

// AC16: the workspace rule's refusal makes a thread not reached
#[test]
fn a_workspace_the_rule_refuses_the_caller_makes_its_thread_not_found() {
    run_local(async {
        let lab = Lab::main();
        lab.host.hooks.set::<dyn Fn(&WorkspaceRecord, Option<&Caller>) -> Result<Option<HostError>, HostError>>(
            "ActivityCtx::refusal_for",
            Rc::new(|_r: &WorkspaceRecord, _c: Option<&Caller>| Ok(Some(HostError::new("not yours")))),
        );
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, nf(&format!("no thread {}", thread_word(T1))));
    });
}

// AC22: no record and no facts
#[test]
fn a_thread_nobody_holds_is_not_found() {
    run_local(async {
        let lab = Lab::main();
        let ask = ActivityAsk { thread_ids: Some(vec![T1.to_string(), T3.to_string()]), ..ActivityAsk::default() };
        let said = said_of(lab.read(ask, None).await);
        assert_eq!(said, nf(&format!("no thread {}", thread_word(T3))));
    });
}

// AC16: no workspace record at all reads what the caller drives, by the computer's kind
#[test]
fn a_thread_with_no_live_workspace_is_reached_by_what_the_caller_drives() {
    run_local(async {
        let lab = Lab::main();
        lab.host.entries.borrow_mut().clear();
        let kinds: Rc<RefCell<Vec<WorkspaceKind>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = kinds.clone();
        lab.drives(move |k: WorkspaceKind, _c: Option<&Caller>| {
            seen.borrow_mut().push(k);
            false
        });
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, nf(&format!("no thread {}", thread_word(T1))));
        assert_eq!(kinds.borrow().first(), Some(&WorkspaceKind::Local));
    });
}

// AC25: of the threads asked only those on this computer are kept, and wsp and unowned are emptied
#[test]
fn threads_asked_that_run_elsewhere_are_left_out_of_a_read_here() {
    run_local(async {
        let lab = Lab::main();
        lab.join_box();
        lab.box_thread(T2, "w2", "boxed", STARTED + 10);
        let ask = ActivityAsk { thread_ids: Some(vec![T1.to_string(), T2.to_string()]), ..ActivityAsk::default() };
        let view = view_of(lab.read(ask, None).await);
        assert_eq!(view, view_json(THIS_COMPUTER, T0, vec![main_thread(T1, "start the dev server")], vec![], vec![]));
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC25: of every thread only the reached ones are listed
#[test]
fn threads_the_caller_does_not_reach_are_left_out_of_a_read_of_everything() {
    run_local(async {
        let lab = Lab::main();
        lab.local_thread(T2, "w2", "other", STARTED + 10, "completed");
        let mut rows = main_table(T1);
        rows.push(proc_row(4600, json!({ "owner": owned("fork", T2), "startedAt": STARTED + 3_000 })));
        lab.here.set_procs(rows);
        lab.host
            .hooks
            .set::<dyn Fn(&str, Option<&Caller>) -> bool>("ActivityCtx::settles_thread", Rc::new(|t: &str, _c: Option<&Caller>| t == T1));
        let view = view_of(lab.read(ActivityAsk::default(), None).await);
        assert_eq!(ids_of(&view), vec![T1]);
        assert_eq!(view["wsp"], json!([daemon_row()]));
    });
}

// AC23: box-activity.test.ts:85
#[test]
fn names_the_unjoined_box_a_thread_runs_on() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.box_thread(T1, "w1", "go", STARTED);
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::joined", Rc::new(|_p: &str| false));
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, usage_said(&not_joined_activity_line(T1, "hetzner"), &not_joined_activity_fix("hetzner")));
        assert!(!said.0.contains("cloud"));
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC23: box-activity.test.ts:96
#[test]
fn says_cloud_machine_for_a_provider_this_host_does_not_fork_on() {
    run_local(async {
        let lab = Lab::new();
        lab.join_box();
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::joined", Rc::new(|_p: &str| false));
        lab.thread_in(T1, "w1", json!({ "kind": "cloud", "place": "box" }), "Fix it", STARTED, "completed");
        let said = said_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(said, usage_said(&cloud_activity_line(T1), CLOUD_ACTIVITY_FIX));
    });
}

// AC24
#[test]
fn a_computer_named_that_is_not_the_threads_is_not_found() {
    run_local(async {
        let lab = Lab::main();
        lab.join_box();
        lab.box_thread(T2, "w2", "boxed", STARTED + 10);
        let said = said_of(lab.read(ask_of(Some(T1), Some("hetzner")), None).await);
        assert_eq!(said, nf(&format!("no thread {} on hetzner", thread_word(T1))));
        let said = said_of(lab.read(ask_of(Some(T2), Some("here")), None).await);
        assert_eq!(said, nf(&format!("no thread {} on {THIS_COMPUTER}", thread_word(T2))));
        assert_eq!(lab.here.chans(), vec![]);
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC18, AC9: box-activity.test.ts:35
#[test]
fn lists_a_boxs_threads_by_their_cgroups() {
    run_local(async {
        let lab = box_lab();
        lab.host.clock.advance(9_000);
        let view = view_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(view, view_json("hetzner", T0 + 9_000, vec![box_thread_json()], vec![], vec![]));
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames())], "once, over the link, never a watch");
        assert_eq!(lab.host.hooks.called("box_channel"), vec!["box_channel(box)"]);
        assert_eq!(lab.host.hooks.called("local_channel"), Vec::<String>::new());
    });
}

// AC18: box-activity.test.ts:47
#[test]
fn totals_a_box_thread_by_its_cgroup() {
    run_local(async {
        let lab = box_lab();
        *lab.boxd.cgroups.borrow_mut() = Some(json!([{ "cgroup": thread_cgroup(T1), "memory": 30_000_000 }]));
        let mut thread = box_thread_json();
        thread["memory"] = json!(30_000_000);
        let by_computer = view_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(by_computer["threads"], json!([thread.clone()]));
        let by_thread = view_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(by_thread["threads"], json!([thread]));
    });
}

// AC18, AC24, AC25: box-activity.test.ts:56
#[test]
fn reads_a_thread_alone_on_its_box() {
    run_local(async {
        let lab = box_lab();
        let view = view_of(lab.read(ask_of(Some(T1), None), None).await);
        assert_eq!(view, view_json("hetzner", T0, vec![box_thread_json()], vec![], vec![]));
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames())]);
    });
}
