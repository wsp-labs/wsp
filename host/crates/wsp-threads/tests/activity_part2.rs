// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `activity.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("activity/world.rs");

// AC7, AC18: box-activity.test.ts:71
#[test]
fn a_box_with_no_list_is_refused_in_the_usage_class() {
    run_local(async {
        let lab = box_lab();
        lab.boxd.lists.set(false);
        let expected = usage_said(&box_behind_activity_line("hetzner"), BOX_BEHIND_ACTIVITY_FIX);
        assert_eq!(said_of(lab.read(ask_of(None, Some("hetzner")), None).await), expected);
        assert_eq!(said_of(lab.read(ask_of(Some(T1), None), None).await), expected);
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames()), chan(false, list_frames())]);
    });
}

// AC18: a box with no link or no channel is away
#[test]
fn a_box_with_no_link_or_no_channel_is_away() {
    run_local(async {
        let lab = box_lab();
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::has_link", Rc::new(|_p: &str| false));
        let away = ("hetzner is away".to_string(), None, None);
        assert_eq!(said_of(lab.read(ask_of(None, Some("hetzner")), None).await), away);
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::has_link", Rc::new(|_p: &str| true));
        lab.host.hooks.set::<dyn Fn(&str, DaemonEventListener) -> Option<LocalBoxFuture<'static, Result<Rc<dyn DaemonAsk>, HostError>>>>(
            "ActivityCtx::box_channel",
            Rc::new(|_p: &str, _s: DaemonEventListener| None),
        );
        assert_eq!(said_of(lab.read(ask_of(None, Some("hetzner")), None).await), away);
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}

// AC20, AC18
#[test]
fn a_boxs_threads_unnamed_come_newest_first() {
    run_local(async {
        let lab = box_lab();
        lab.box_thread(T2, "w2", "second", STARTED + 5_000);
        lab.local_thread(T3, "w3", "here", STARTED + 9_000, "completed");
        let mut rows = lab.boxd.procs.borrow().clone();
        rows.push(box_row(7300, 1, "node", "node /root/two/node_modules/.bin/vite", &thread_cgroup(T2)));
        lab.boxd.set_procs(rows);
        let view = view_of(lab.read(ask_of(None, Some("hetzner")), None).await);
        assert_eq!(ids_of(&view), vec![T2, T1]);
        assert_eq!(
            view["threads"][0],
            thread_json(
                T2,
                "second",
                "w2",
                50_000_000,
                1,
                vec![box_shown(7300, 1, 0, "node", "node /root/two/node_modules/.bin/vite", &[])]
            )
        );
    });
}

// AC26
#[test]
fn end_frees_a_kept_agent_and_signals_nothing() {
    run_local(async {
        let lab = Lab::main();
        lab.scopes(vec![json!({ "thread": T1, "agents": [4100] })]);
        lab.kept(T1, 4100);
        let answer = ended_of(lab.end(4100, STARTED, None).await);
        assert_eq!(answer, json!({ "pid": 4100, "ended": [agent()], "forced": [], "left": [], "freed": T1 }));
        assert_eq!(lab.host.hooks.called("reap_kept_now"), vec![format!("reap_kept_now({T1})")]);
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames())], "a fresh read and no kill channel");
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
    });
}

// AC11, AC26: the agent a running turn runs is not the kept one
#[test]
fn a_running_threads_agent_is_not_kept_and_its_end_is_refused() {
    run_local(async {
        let lab = Lab::main();
        lab.host.rows.borrow()[&format!("s-{T1}")].borrow_mut().view.status = serde_json::from_value(json!("running")).unwrap();
        lab.scopes(vec![json!({ "thread": T1, "agents": [4100] })]);
        lab.kept(T1, 4100);
        let said = said_of(lab.end(4100, STARTED, None).await);
        let scope = agent_scope(T1, "start the dev server");
        assert_eq!(said, (joined(&agent_end_line(4100.0, &scope), &agent_end_fix(&scope)), Some(agent_end_fix(&scope)), Some(Kind::Usage)));
        assert_eq!(lab.host.hooks.called("reap_kept_now"), Vec::<String>::new());
        assert_eq!(lab.here.chans().len(), 1);
    });
}

// AC26: activity.test.ts:106
#[test]
fn end_refuses_the_agent_and_sends_nothing() {
    run_local(async {
        let lab = Lab::main();
        let said = said_of(lab.end(4100, STARTED, None).await);
        let scope = agent_scope(T1, "start the dev server");
        assert_eq!(said, (joined(&agent_end_line(4100.0, &scope), &agent_end_fix(&scope)), Some(agent_end_fix(&scope)), Some(Kind::Usage)));
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames())]);
    });
}

// AC26: activity.test.ts:115
#[test]
fn end_refuses_a_pid_that_names_another_process_now() {
    run_local(async {
        let lab = Lab::main();
        let said = said_of(lab.end(4200, STARTED - 60_000, None).await);
        let now: wsp_frames::ProcEntry =
            serde_json::from_value(proc_row(4200, json!({ "comm": "node", "startedAt": STARTED + 1_000 }))).expect("a process row");
        assert_eq!(
            said,
            (joined(&end_reused_line(4200, &now, STARTED - 60_000), END_AGAIN_FIX), Some(END_AGAIN_FIX.to_string()), Some(Kind::NotFound))
        );
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames())]);
    });
}

// AC26: activity.test.ts:123
#[test]
fn end_refuses_a_process_wsp_did_not_start() {
    run_local(async {
        let lab = Lab::main();
        for started in [STARTED, 1] {
            let said = said_of(lab.end(5000, started, None).await);
            assert_eq!(said, (joined(&not_started_line(5000), NOT_STARTED_FIX), Some(NOT_STARTED_FIX.to_string()), Some(Kind::Usage)));
        }
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames()), chan(false, watch_frames())]);
    });
}

// AC27: activity.test.ts:133
#[test]
fn end_takes_the_process_and_its_tree_in_one_kill() {
    run_local(async {
        let lab = Lab::main();
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![vite(), esbuild()], &[], &[]));
        assert_eq!(lab.here.chans(), vec![chan(false, watch_frames()), chan(true, vec![kill_frame(4200, STARTED + 1_000, Some(VITE_ID))])]);
        let frame = lab.here.kills.borrow()[0].to_string();
        assert_eq!(
            frame,
            format!(
                r#"{{"op":"proc.kill","pid":4200,"signal":"TERM","mode":"tree","startedAt":{},"identity":"{VITE_ID}"}}"#,
                STARTED + 1_000
            ),
            "in this key order"
        );
        assert_eq!(lab.here.signals.borrow().clone(), vec![(4200, "TERM".to_string()), (4300, "TERM".to_string())]);
    });
}

// AC27: a process the daemon gave no identity is sent with none
#[test]
fn end_sends_no_identity_for_a_process_the_daemon_gave_none() {
    run_local(async {
        let lab = Lab::main();
        lab.here.procs.borrow_mut().push(proc_row(
            4600,
            json!({ "cmdline": "node serve.js", "kind": "dev", "owner": owned("fork", T1), "startedAt": STARTED + 4_000 }),
        ));
        let answer = ended_of(lab.end(4600, STARTED + 4_000, None).await);
        assert_eq!(
            answer,
            ended(4600, vec![shown(4600, 1, 0, STARTED + 4_000, "dev", "node", "node serve.js", 1_000_000, 0, &[])], &[], &[])
        );
        let frame = lab.here.kills.borrow()[0].to_string();
        assert_eq!(frame, format!(r#"{{"op":"proc.kill","pid":4600,"signal":"TERM","mode":"tree","startedAt":{}}}"#, STARTED + 4_000));
    });
}

// AC27: activity.test.ts:148
#[test]
fn end_signals_through_the_guard() {
    run_local(async {
        let lab = Lab::main();
        lab.end(4200, STARTED + 1_000, None).await.expect("the end answers");
        assert_eq!(
            lab.host.hooks.called("local_channel"),
            vec!["local_channel(false)", "local_channel(true)"],
            "the read is plain, the kill goes through the guard"
        );
        assert_eq!(lab.here.chans().iter().map(|c| c.guarded).collect::<Vec<_>>(), vec![false, true]);
    });
}

// AC27: activity.test.ts:159
#[test]
fn end_names_what_the_daemon_forced() {
    run_local(async {
        let lab = Lab::main();
        lab.here.stubborn.borrow_mut().push(4300);
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![vite(), esbuild()], &[4300], &[]));
        assert_eq!(lab.here.kills.borrow().len(), 1, "the grace and the KILL after it are the daemon's");
        assert_eq!(
            lab.here.signals.borrow().clone(),
            vec![(4200, "TERM".to_string()), (4300, "TERM".to_string()), (4300, "KILL".to_string())]
        );
    });
}

// AC27: activity.test.ts:173 and :186
#[test]
fn end_signals_nothing_when_the_pid_holds_another_process() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.after_read.borrow_mut() = Some(Box::new(|d: &Daemon| {
            for p in d.procs.borrow_mut().iter_mut() {
                if p["pid"] == 4200 {
                    p["comm"] = json!("sleep");
                    p["cmdline"] = json!("sleep 600");
                    p["id"] = json!("99999");
                }
            }
        }));
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![], &[], &[]));
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert!(lab.here.procs.borrow().iter().any(|p| p["pid"] == 4200));
        assert_eq!(lab.here.kills.borrow().len(), 1, "the daemon was asked and refused it");
    });
}

// AC27: any other refusal is the End's
#[test]
fn a_refusal_of_the_kill_other_than_not_found_is_the_ends() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.kill_reply.borrow_mut() = Some(Err(refused(Some("denied"), "operation not permitted")));
        let said = said_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(said, ("operation not permitted".to_string(), None, None));
        assert_eq!(
            lab.here.chans(),
            vec![chan(false, watch_frames()), chan(true, vec![kill_frame(4200, STARTED + 1_000, Some(VITE_ID))])],
            "the kill channel is closed after the failure"
        );
    });
}

// AC27: forced and left are named once, and ended is the plan's rows the daemon signalled
#[test]
fn forced_and_left_are_named_once_and_ended_is_what_was_signalled() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.kill_reply.borrow_mut() =
            Some(Ok(reply(json!({ "signalled": [4300, 4200, 4300], "forced": [4300, 4300], "left": [4300, 4300] }))));
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![vite(), esbuild()], &[4300], &[4300]), "ended follows the plan's order, each row once");
    });
}

// AC27: a daemon that signalled only the head
#[test]
fn ended_holds_only_the_rows_the_daemon_signalled() {
    run_local(async {
        let lab = Lab::main();
        *lab.here.kill_reply.borrow_mut() = Some(Ok(reply(json!({ "signalled": [4200], "forced": [], "left": [] }))));
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![vite()], &[], &[]));
    });
}

// AC27: activity.test.ts:215
#[test]
fn end_on_a_daemon_before_kill_modes_names_the_pid_alone() {
    run_local(async {
        let lab = Lab::main();
        lab.here.modes.set(false);
        let answer = ended_of(lab.end(4200, STARTED + 1_000, None).await);
        assert_eq!(answer, ended(4200, vec![vite()], &[], &[]));
        assert_eq!(lab.here.signals.borrow().clone(), vec![(4200, "TERM".to_string())]);
    });
}

// AC28-AC32: activity.test.ts:225
#[test]
fn settle_stop_ends_every_tree_in_one_go() {
    run_local(async {
        let lab = Lab::main();
        lab.here.procs.borrow_mut().push(vitest_proc(T1));
        lab.here.hold_kills.set(2);
        let answer = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, None).await);
        assert_eq!(answer, json!([ended(4200, vec![vite(), esbuild()], &[], &[]), ended(4400, vec![vitest()], &[], &[])]));
        assert_eq!(
            lab.here.chans(),
            vec![
                chan(false, list_frames()),
                chan(true, vec![kill_frame(4200, STARTED + 1_000, Some(VITE_ID)), kill_frame(4400, STARTED + 3_000, Some("88002"))])
            ]
        );
        assert_eq!(lab.here.first_answer_saw.get(), Some(2), "both kills were sent before the first was answered");
        assert_eq!(signalled(&lab), vec![4200, 4300, 4400]);
        assert_eq!(table_pids(&lab.here), vec![1, 4100, 4150, 5000]);
    });
}

// AC29, AC31: activity.test.ts:240
#[test]
fn settle_ask_refuses_naming_what_outlives_the_agent() {
    run_local(async {
        let lab = Lab::main();
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await.expect_err("the settle is refused");
        assert_eq!(refusal, refusal_of(LeftVerb::Settle, &[t1_left(vec![vite(), esbuild()], 470_000_000)], &[]));
        assert_eq!(lab.here.chans(), vec![chan(false, list_frames())], "one list, answered at once, never a watch waiting on the sampler");
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
    });
}

// AC28, AC31: activity.test.ts:264
#[test]
fn delete_ask_refuses_and_keep_reads_nothing() {
    run_local(async {
        let lab = Lab::main();
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Delete, None).await.expect_err("the delete is refused");
        assert_eq!(refusal, refusal_of(LeftVerb::Delete, &[t1_left(vec![vite(), esbuild()], 470_000_000)], &[]));
        let calls = lab.host.hooks.calls();
        let chans = lab.here.chans();
        let kept = all_ended(lab.meet(&[T1], ProcessesAsk::Keep, MeetVerb::Delete, None).await);
        assert_eq!(kept, json!([]));
        assert_eq!(lab.host.hooks.calls(), calls, "keep asks the host nothing");
        assert_eq!(lab.here.chans(), chans);
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert!(table_pids(&lab.here).contains(&4200));
    });
}

// AC28
#[test]
fn keep_asks_nothing_of_the_host_and_opens_no_channel() {
    run_local(async {
        let lab = Lab::main();
        lab.box_thread(T2, "w2", "boxed", STARTED + 10);
        let kept = all_ended(lab.meet(&[T1, T2], ProcessesAsk::Keep, MeetVerb::Settle, None).await);
        assert_eq!(kept, json!([]));
        assert_eq!(lab.host.hooks.calls(), Vec::<String>::new());
        assert_eq!(lab.here.chans(), vec![]);
    });
}

// AC29, AC30, AC32: activity.test.ts:287
#[test]
fn a_shell_the_threads_pane_opened_is_asked_about_and_stopped() {
    run_local(async {
        let lab = with_shell();
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await.expect_err("the settle is refused");
        assert_eq!(refusal, refusal_of(LeftVerb::Settle, &[shell_left()], &[]));
        let me = Some(scoped_by(T1, "w1"));
        let answer = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, me).await);
        assert_eq!(answer, json!([ended(4200, vec![vite(), esbuild()], &[], &[]), ended(5100, vec![shell(), shell_child()], &[], &[])]));
        assert_eq!(lab.here.chans().len(), 3);
        assert_eq!(
            lab.here.chans()[2].frames,
            vec![kill_frame(4200, STARTED + 1_000, Some(VITE_ID)), kill_frame(5100, STARTED + 5_000, Some("88100"))]
        );
        assert_eq!(table_pids(&lab.here), vec![1, 4100, 4150, 5000]);
        assert_eq!(lab.here.direct.borrow().clone(), Vec::<Value>::new(), "a shell here is ended by its tree, no pty.kill");
    });
}

// AC28, AC31: activity.test.ts:301
#[test]
fn delete_asks_about_a_shell_the_pane_opened_and_keep_leaves_it_running() {
    run_local(async {
        let lab = with_shell();
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Delete, None).await.expect_err("the delete is refused");
        assert_eq!(refusal, refusal_of(LeftVerb::Delete, &[shell_left()], &[]));
        let kept = all_ended(lab.meet(&[T1], ProcessesAsk::Keep, MeetVerb::Delete, None).await);
        assert_eq!(kept, json!([]));
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.direct.borrow().clone(), Vec::<Value>::new());
        assert!(table_pids(&lab.here).contains(&5100) && table_pids(&lab.here).contains(&5110));
    });
}

// AC32: activity.test.ts:315
#[test]
fn delete_with_stop_ends_a_shell_the_pane_opened_with_the_rest() {
    run_local(async {
        let lab = with_shell();
        let answer = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Delete, None).await);
        assert_eq!(answer, json!([ended(4200, vec![vite(), esbuild()], &[], &[]), ended(5100, vec![shell(), shell_child()], &[], &[])]));
        assert!(!table_pids(&lab.here).contains(&5100));
    });
}

// AC10, AC29: activity.test.ts:324
#[test]
fn a_thread_beside_it_keeps_its_own_processes() {
    run_local(async {
        let lab = Lab::main();
        lab.local_thread(T2, "w1", "start the docs server", STARTED + 10, "completed");
        let theirs = owned("fork", T2);
        lab.here.procs.borrow_mut().extend([
            proc_row(4600, json!({ "cmdline": "node /lab/node_modules/.bin/vite --port 5174", "rss": 90_000_000, "kind": "dev", "owner": theirs, "startedAt": STARTED + 3_000 })),
            proc_row(4650, json!({ "ppid": 4600, "comm": "esbuild", "cmdline": "/lab/node_modules/esbuild/bin/esbuild --service", "rss": 20_000_000, "owner": theirs, "startedAt": STARTED + 4_000 })),
        ]);
        lab.here.set_ports(&[(5173, 4200), (5174, 4600)]);
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await.expect_err("the settle is refused");
        assert_eq!(
            refusal,
            refusal_of(LeftVerb::Settle, &[t1_left(vec![vite(), esbuild()], 470_000_000)], &[]),
            "the refusal carries this thread's tree alone"
        );
        let stopped = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, None).await);
        assert_eq!(stopped, json!([ended(4200, vec![vite(), esbuild()], &[], &[])]));
        assert_eq!(signalled(&lab), vec![4200, 4300]);
        assert!(table_pids(&lab.here).contains(&4600) && table_pids(&lab.here).contains(&4650));
        let kept = all_ended(lab.meet(&[T2], ProcessesAsk::Keep, MeetVerb::Settle, None).await);
        assert_eq!(kept, json!([]));
        assert_eq!(signalled(&lab), vec![4200, 4300]);
    });
}

// AC30, AC31: activity.test.ts:197
#[test]
fn a_thread_settling_itself_hears_which_processes_only_the_person_ends() {
    run_local(async {
        let lab = Lab::main();
        lab.here.procs.borrow_mut().push(proc_row(
            4800,
            json!({ "comm": "python3", "cmdline": "python3 -m http.server", "owner": owned("env", T1), "startedAt": STARTED + 8_000 }),
        ));
        let me = || Some(scoped_by(T1, "w1"));
        let refusal = lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, me()).await.expect_err("the ask is refused");
        assert_eq!(refusal, refusal_of(LeftVerb::Settle, &[t1_left(vec![vite(), esbuild(), python()], 471_000_000)], &[4800]));
        let said = said_of(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, me()).await);
        assert_eq!(said, (joined(&not_threads_line(4800), NOT_THREADS_FIX), Some(NOT_THREADS_FIX.to_string()), Some(Kind::Usage)));
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
        assert_eq!(lab.here.chans(), vec![chan(false, list_frames()), chan(false, list_frames())], "no kill channel before the refusal");
    });
}

// AC30
#[test]
fn nothing_left_answers_nothing_and_signals_nothing() {
    run_local(async {
        let lab = Lab::main();
        lab.here.procs.borrow_mut().retain(|p| p["pid"] != 4200 && p["pid"] != 4300);
        for processes in [ProcessesAsk::Ask, ProcessesAsk::Stop] {
            let answer = all_ended(lab.meet(&[T1], processes, MeetVerb::Settle, None).await);
            assert_eq!(answer, json!([]));
        }
        assert_eq!(lab.here.chans(), vec![chan(false, list_frames()), chan(false, list_frames())]);
        assert_eq!(lab.here.signals.borrow().clone(), vec![]);
    });
}

// AC29
#[test]
fn a_box_thread_the_caller_does_not_reach_is_not_found() {
    run_local(async {
        let lab = box_lab();
        lab.host
            .hooks
            .set::<dyn Fn(&str, Option<&Caller>) -> bool>("ActivityCtx::settles_thread", Rc::new(|_t: &str, _c: Option<&Caller>| false));
        let said = said_of(lab.meet(&[T1], ProcessesAsk::Ask, MeetVerb::Settle, None).await);
        assert_eq!(said, nf(&format!("no thread {}", thread_word(T1))));
        assert_eq!(lab.boxd.chans(), vec![]);
        assert_eq!(lab.here.chans(), vec![]);
    });
}

// AC29: this computer once, each box once for all its threads, a box's threads named by the box
#[test]
fn meet_reads_this_computer_once_and_each_box_once_and_names_the_box() {
    run_local(async {
        let lab = Lab::main();
        lab.join_box();
        lab.box_thread(T2, "w2", "second", STARTED + 10);
        lab.box_thread(T3, "w3", "third", STARTED + 20);
        lab.boxd.set_procs(vec![
            box_row(7300, 1, "node", "node /root/two/node_modules/.bin/vite", &thread_cgroup(T2)),
            box_row(7400, 1, "node", "node /root/three/node_modules/.bin/vite", &thread_cgroup(T3)),
        ]);
        let refusal = lab.meet(&[T1, T2, T3], ProcessesAsk::Ask, MeetVerb::Settle, None).await.expect_err("the settle is refused");
        let on_box = |id: &str, title: &str, ws: &str, pid: u32, path: &str| {
            left_thread(
                id,
                title,
                ws,
                50_000_000,
                1,
                vec![box_shown(pid, 1, 0, "node", &format!("node /root/{path}/node_modules/.bin/vite"), &[])],
                Some("hetzner"),
            )
        };
        let left = [
            t1_left(vec![vite(), esbuild()], 470_000_000),
            on_box(T2, "second", "w2", 7300, "two"),
            on_box(T3, "third", "w3", 7400, "three"),
        ];
        assert_eq!(refusal, refusal_of(LeftVerb::Settle, &left, &[]));
        assert_eq!(lab.here.chans(), vec![chan(false, list_frames())]);
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames())]);
        assert_eq!(lab.host.hooks.called("box_channel"), vec!["box_channel(box)"]);
    });
}

// AC21
#[test]
fn a_box_end_kills_each_listed_shell_and_waits_out_the_grace() {
    run_local(async {
        let lab = box_end_lab();
        arm_end_thread(&lab, || {});
        *lab.boxd.pty_kill.borrow_mut() =
            Rc::new(|_d: &Daemon, pty: &str| if pty == "pty_1" { Err(refused(None, "no such shell")) } else { Ok(reply(json!({}))) });
        let warned = wsp_core::console::capture();
        let door = lab.door.clone();
        let task = tokio::task::spawn_local(async move { door.meet(&ids(&[T1]), ProcessesAsk::Stop, MeetVerb::Settle, None).await });
        for i in 1..=10 {
            settle().await;
            assert!(!task.is_finished(), "still inside the grace at poll {i}");
            lab.host.clock.advance(200);
        }
        settle().await;
        assert!(task.is_finished(), "the grace is {END_GRACE_MS} ms of the clock");
        let answer = all_ended(task.await.expect("the task ran"));
        assert_eq!(answer, json!([ended(7100, box_ended_rows(), &[], &[7100, 7300, 7400])]));
        assert_eq!(lab.boxd.direct.borrow().clone(), pty_frames(), "each listed shell, one at a time, never the other cgroup's");
        assert_eq!(warned.lines(), vec![format!("shell pty_1 of thread {} was not ended: no such shell", thread_word(T1))]);
        assert_eq!(
            end_calls(&lab),
            vec![format!("end_thread({T1})"), "with_daemon".to_string(), "with_daemon".to_string(), format!("close_servers({T1})")]
        );
        let chans = lab.boxd.chans();
        assert_eq!(chans.len(), 12, "the meet's read, one look after the end, one after each 200 ms");
        assert!(chans.iter().all(|c| *c == chan(false, list_frames())));
        let seq = lab.boxd.seq.borrow().clone();
        let at = seq.iter().position(|s| s == "direct:pty.kill").expect("a pty.kill");
        assert_eq!(&seq[at..at + 2], ["direct:pty.kill", "direct:pty.kill"]);
        assert_eq!(seq[..at].last().map(String::as_str), Some("c0:close"), "the shells are ended after the meet's own read");
        assert_eq!(lab.host.clock.pending(), 0);
    });
}

// AC21: nothing left at the first look ends the wait
#[test]
fn a_box_end_that_took_hold_before_the_first_look_does_not_wait() {
    run_local(async {
        let lab = box_end_lab();
        let d = lab.boxd.clone();
        let mine = thread_cgroup(T1);
        arm_end_thread(&lab, move || d.procs.borrow_mut().retain(|p| p["cgroup"] != json!(mine)));
        let answer = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, None).await);
        assert_eq!(answer, json!([ended(7100, box_ended_rows(), &[], &[])]));
        assert_eq!(lab.boxd.direct.borrow().clone(), pty_frames(), "the shells are the first read's, not the next one's");
        assert_eq!(lab.boxd.chans().len(), 2);
        assert_eq!(lab.host.clock.pending(), 0);
    });
}

// AC19, AC21: a look that fails counts every process standing
#[test]
fn a_failed_look_during_the_grace_counts_every_process_standing() {
    run_local(async {
        let lab = box_end_lab();
        let d = lab.boxd.clone();
        arm_end_thread(&lab, move || *d.list_error.borrow_mut() = Some(refused(None, "disk full")));
        let warned = wsp_core::console::capture();
        let door = lab.door.clone();
        let task = tokio::task::spawn_local(async move { door.meet(&ids(&[T1]), ProcessesAsk::Stop, MeetVerb::Settle, None).await });
        for _ in 1..=10 {
            settle().await;
            lab.host.clock.advance(200);
        }
        settle().await;
        assert!(task.is_finished());
        let answer = all_ended(task.await.expect("the task ran"));
        assert_eq!(answer, json!([ended(7100, box_ended_rows(), &[], &[7100, 7300, 7400])]));
        let line = "hetzner's processes were not read for a settle or a delete: disk full";
        assert_eq!(warned.lines(), vec![line; 11], "each failed look is warned");
    });
}

// AC19
#[test]
fn a_quiet_box_read_that_fails_is_warned_and_holds_nothing() {
    run_local(async {
        let lab = box_end_lab();
        *lab.boxd.list_error.borrow_mut() = Some(refused(None, "disk full"));
        let warned = wsp_core::console::capture();
        for processes in [ProcessesAsk::Ask, ProcessesAsk::Stop] {
            let answer = all_ended(lab.meet(&[T1], processes, MeetVerb::Settle, None).await);
            assert_eq!(answer, json!([]));
        }
        let line = "hetzner's processes were not read for a settle or a delete: disk full";
        assert_eq!(warned.lines(), vec![line; 2]);
        assert_eq!(lab.boxd.chans(), vec![chan(false, list_frames()), chan(false, list_frames())]);
        assert_eq!(lab.boxd.direct.borrow().clone(), Vec::<Value>::new());
        assert_eq!(lab.host.hooks.called("end_thread"), Vec::<String>::new());
    });
}

// AC19
#[test]
fn a_box_that_is_away_holds_nothing_for_a_settle() {
    run_local(async {
        let lab = box_end_lab();
        lab.host.hooks.set::<dyn Fn(&str) -> bool>("ActivityCtx::has_link", Rc::new(|_p: &str| false));
        let warned = wsp_core::console::capture();
        let answer = all_ended(lab.meet(&[T1], ProcessesAsk::Stop, MeetVerb::Settle, None).await);
        assert_eq!(answer, json!([]));
        assert_eq!(warned.lines(), vec!["hetzner's processes were not read for a settle or a delete: hetzner is away"]);
        assert_eq!(lab.boxd.chans(), vec![]);
    });
}
