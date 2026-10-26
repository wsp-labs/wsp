// SPDX-License-Identifier: AGPL-3.0-only
//! A workspace's machine moves: nap, wake (asking the provider again on its cadence, a deleted disk replaced or a
//! sub-thread released, a stop the person pulls), resize, upgrade, rebuild, the record's own changes, the daemon,
//! delete and forget. spec/workspaces_phases.md; workspaces.ts:22-65 and :289-645 at 65f6b89c7. One test per row of the
//! spec's "Rules against tests" table, then the strong cases of the TS tests the test map lists (wake-asks-again,
//! disk-gone, pause, box-nap, disk-watch, forget, reconcile, runtime, folder-threads) that those names do not cover,
//! each naming the file:line it ports. Every sentence is the TS protocol function's own text, written out, since
//! wsp-words' bodies are not written yet. The wake's clock is the `TestClock`: `sleeps` waits on it.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/phases.rs"]
mod phases_support;
include!("workspaces_phases/world.rs");

// ------------------------------------------------------------------------------------------------------ rule 2

// disk-gone.test.ts:99 and the sub-thread's release (workspaces.ts:36-46)
#[test]
fn disk_gone_releases_a_sub_thread() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "sub": sub_json() }));
        let warned = wsp_core::console::capture();
        let line = format!("{BOAT_LINE}; the next send starts a new machine from the image");
        let err = disk_gone(&h.ctx, &entry, disk_gone_error("m_a", BOAT_LINE, SAID)).await.unwrap_err();
        assert_eq!(err, kinded(&line, Kind::Conflict));
        assert_eq!(rec(&entry).disk_gone.as_deref(), Some(line.as_str()));
        assert_eq!(h.args("unfork"), vec![json!({ "id": "ws_a" })]);
        assert_eq!(h.args("sub_lost"), vec![json!({ "id": "ws_a", "line": line })]);
        assert_eq!(h.args("tell_threads"), vec![json!({ "id": "ws_a", "line": line })]);
        assert_eq!(h.calls(&["unfork", "sub_lost", "tell_threads", "ws_rebuild"]), ["unfork", "sub_lost", "tell_threads"]);
        assert_eq!(warned.lines(), [format!("wake of ws_a: m_a cannot resume: {SAID}")]);
        assert!(h.events().is_empty());
        // A delete the provider refuses is warned and the release goes on.
        h.f.unforks.set(vec![Err(other("the provider said no"))]);
        let err = disk_gone(&h.ctx, &entry, disk_gone_error("m_a", BOAT_LINE, SAID)).await.unwrap_err();
        assert_eq!(err, kinded(&line, Kind::Conflict));
        assert_eq!(warned.lines()[2], "wake of ws_a: m_a was not deleted: the provider said no");
        assert_eq!(h.args("sub_lost").len(), 2);
    });
}

// disk-gone.test.ts:45 "starts a new machine from the image in the same wake, retires the old one and says the disk is gone"
#[test]
fn disk_gone_rebuilds_from_the_image_and_says_replaced() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "wakeRefused": "an old refusal" }));
        let warned = wsp_core::console::capture();
        let view = disk_gone(&h.ctx, &entry, disk_gone_error("m_a", BOAT_LINE, SAID)).await.unwrap();
        let line = replaced_line();
        let record = expected(json!({ "phase": "running", "machineId": "m_new", "diskGone": line }));
        assert_eq!(rec_json(&entry), record);
        assert_eq!(h.persisted(), vec![record]);
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.upgraded", "workspaceId": "ws_a", "machineId": "m_new" })]);
        assert_eq!(h.args("tell_threads"), vec![json!({ "id": "ws_a", "line": line })]);
        assert_eq!(
            h.calls(&["ws_rebuild", "follow_machine", "persist", "emit", "tell_threads", "reach_of", "emit_status"]),
            ["ws_rebuild", "follow_machine", "persist", "emit", "tell_threads", "reach_of", "emit_status"]
        );
        let status = &h.args("emit_status")[0];
        assert_eq!((&status["reach"], &status["words"]), (&json!("reachable"), &json!(line)));
        assert_eq!(warned.lines(), [format!("wake of ws_a: m_a cannot resume: {SAID}"), "wake of ws_a: m_a replaced by m_new".to_owned()]);
    });
}

// disk-gone.test.ts:76 "a new machine that does not start leaves the record napping on the old one and says both"
#[test]
fn disk_gone_with_no_new_machine_keeps_the_phase_and_says_why() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "waking" }));
        h.f.rebuilds.set(vec![Err(other("no machine"))]);
        let err = disk_gone(&h.ctx, &entry, disk_gone_error("m_a", BOAT_LINE, SAID)).await.unwrap_err();
        let words = format!("{BOAT_LINE}, and no new machine started: no machine");
        assert_eq!(err, plain(&words));
        assert_eq!(h.persisted(), vec![expected(json!({}))]);
        assert_eq!(rec(&entry).machine_id, "m_a");
        let status = &h.args("emit_status")[0];
        assert_eq!((&status["reach"], &status["words"]), (&json!("napping"), &json!(words)));
        assert_eq!(h.calls(&["follow_machine", "emit", "tell_threads"]), Vec::<String>::new());
        assert!(h.events().is_empty());
    });
}

// ------------------------------------------------------------------------------------------------------ rule 3

// pause.test.ts:61 "nap and wake are refused with the provider's own sentence on a backend without a pause mode"
#[test]
fn nap_refuses_a_pauseless_kind_and_naps() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.base.answer_with(
            "refuse_pauseless",
            vec![err_answer("a is this computer, and it cannot be paused on the provider it runs on", Some("conflict"))],
        );
        let err = nap(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a is this computer, and it cannot be paused on the provider it runs on", Kind::Conflict));
        assert_eq!(h.args("refuse_pauseless"), vec![json!({ "id": "ws_a", "act": "be paused" })]);
        assert!(h.args("nap_with").is_empty());
        h.f.base.script.borrow_mut().shift_remove("refuse_pauseless");
        let view = nap(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Napping);
        assert_eq!(h.args("nap_with"), vec![json!({ "id": "ws_a", "napWaitMs": null })]);
        assert_eq!(
            h.calls(&["spawn_guard", "entry_of", "refuse_pauseless", "nap_with"])[3..],
            ["spawn_guard", "entry_of", "refuse_pauseless", "nap_with"]
        );
        assert_eq!(h.args("spawn_guard")[0], json!({ "act": "pause", "scoped": false }));
        // A guard that refuses ends it before the entry is read.
        h.f.base.answer_with("spawn_guard", vec![err_answer("the cap", Some("conflict"))]);
        let err = nap(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, kinded("the cap", Kind::Conflict));
        assert_eq!(h.args("nap_with").len(), 1);
    });
}

// ---------------------------------------------------------------------------------------------------- rules 4.1-4.7

#[test]
fn wake_of_a_tree_workspace_answers_talked_fields() {
    run(async {
        let h = H::new();
        h.f.base.hold(record(
            "ws_far",
            "place",
            "pr_cloud",
            json!({ "name": "far", "phase": "running", "folder": "/srv/far", "home": "/home/far" }),
        ));
        h.f.base.answer_with("talks_to_tree_on", vec![json!(true)]);
        let found = wake(&h.ctx, "ws_far", Some(asker("t_lead1234567", "ws_lead", "t_root"))).await.unwrap();
        let mut full = json_of(&view_of(&h.f.base, &rec(&h.f.base.live.borrow()["ws_far"])));
        let keep = ["id", "name", "machineId", "phase", "kind", "golden", "createdAt", "project", "gone"];
        full.as_object_mut().unwrap().retain(|k, _| keep.contains(&k.as_str()));
        assert_eq!(json_of(&found), full);
        assert!(!json_of(&found).as_object().unwrap().contains_key("folder"));
        // The person's own wake of it is the one that ran, with no scope.
        assert_eq!(h.args("entry_of"), vec![json!({ "id": "ws_far", "thread": null, "act": "wake", "scoped": false })]);
        assert_eq!(h.args("talks_to_tree_on").len(), 2);
    });
}

// pause.test.ts:123 "a wake asked during the pause waits for it and then wakes; a second nap joins the first"
#[test]
fn wake_joins_a_wake_in_flight() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Hold]);
        let release = h.f.hold_wake();
        let first = h.wake_task("ws_a");
        settle().await;
        assert!(entry.borrow().waking.is_some(), "the wake is stored for later callers to join");
        assert!(entry.borrow().wake_stop.is_some(), "the stop token is stored");
        let second = h.wake_task("ws_a");
        settle().await;
        assert_eq!(h.args("ws_wake").len(), 1, "one resume for two callers");
        release.send(done(None)).unwrap();
        let (a, b) = (first.await.unwrap().unwrap(), second.await.unwrap().unwrap());
        assert_eq!(a, b);
        assert_eq!(a.phase, WorkspacePhase::Running);
        assert_eq!(h.args("ws_wake").len(), 1);
        assert_eq!(h.events().len(), 1);
        fields_clear(&entry);
    });
}

#[test]
fn a_joined_wake_hands_its_failure_to_every_caller() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Hold]);
        let release = h.f.hold_wake();
        let first = h.wake_task("ws_a");
        settle().await;
        let second = h.wake_task("ws_a");
        settle().await;
        release.send(Err(other("boom"))).unwrap();
        let (a, b) = (first.await.unwrap().unwrap_err(), second.await.unwrap().unwrap_err());
        assert_eq!(a, b);
        assert_eq!(a.message, "boom");
        fields_clear(&entry);
    });
}

#[test]
fn wake_of_gone_refuses_unless_recovered() {
    run(async {
        let h = H::new();
        let gone = "machine m_a is gone at the provider";
        let entry = h.cloud("ws_a", json!({ "phase": "gone", "gone": gone }));
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, plain(&gone_refusal("a", "wake", gone)));
        assert!(h.args("ws_wake").is_empty());
        // Recovered to running: the view, and nothing woken.
        h.f.base.answer_with("recover_gone", vec![json!("running")]);
        let view = wake(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert!(h.args("ws_wake").is_empty());
        // Recovered to napping: the wake goes on and resumes it.
        *entry.borrow().record.borrow_mut() =
            record("ws_a", "cloud", "pr_cloud", json!({ "name": "a", "machineId": "m_a", "phase": "gone", "gone": gone }));
        h.f.base.script.borrow_mut().shift_remove("recover_gone");
        h.f.base.answer_with("recover_gone", vec![json!("napping")]);
        let view = wake(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("ws_wake").len(), 1);
    });
}

// gone.test.ts:696, runtime-gone.test.ts:56 a wake of a running record reads the machine and the record follows the fact
#[test]
fn wake_of_running_reads_the_machine_and_settles_gone() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.states.set(vec![Ok(MachineRead::Gone)]);
        h.f.base.answer_with("settle_gone", vec![json!(true)]);
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        let at = format_seconds(1_000_000);
        assert_eq!(at, "1970-01-01T00:16:40Z");
        assert_eq!(err, plain(&gone_refusal("a", "wake", &format!("machine m_a is gone at the provider: the wake found it gone at {at}"))));
        assert_eq!(h.args("settle_gone"), vec![json!({ "id": "ws_a", "by": "wake", "at": 1_000_000, "answer": null })]);
        // A 404 on the read says what the provider said.
        h.f.base.live.borrow()["ws_a"].borrow().record.borrow_mut().phase = WorkspacePhase::Running;
        h.f.states.set(vec![Err(missing())]);
        let err = wake(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(
            err,
            plain(&gone_refusal(
                "a",
                "wake",
                &format!("machine m_a is gone at the provider: the wake found it gone at {at} (the sandbox is gone)")
            ))
        );
        assert_eq!(h.args("settle_gone")[1]["answer"], json!("the sandbox is gone"));
        assert!(h.args("ws_wake").is_empty());
    });
}

#[test]
fn a_read_that_fails_or_a_gone_that_is_not_settled_leaves_a_running_record_running() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        // Any failure that is not a 404 reads running.
        h.f.states.set(vec![Err(other("socket hang up"))]);
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert!(h.args("settle_gone").is_empty());
        // A gone reading the sweep does not confirm leaves it as it was.
        h.f.states.set(vec![Ok(MachineRead::Gone)]);
        let view = wake(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("settle_gone").len(), 1);
        assert!(h.args("ws_wake").is_empty());
        assert!(h.persisted().is_empty());
    });
}

// pause.test.ts:488 "a wake asked while the record still says running asks the provider once and resumes a machine it paused"
#[test]
fn wake_of_running_adopts_a_provider_pause() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.states.set(vec![Ok(MachineRead::Paused)]);
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(
            h.calls(&["machine_state", "adopt_pause", "refuse_pauseless", "ws_wake"]),
            ["machine_state", "adopt_pause", "refuse_pauseless", "ws_wake"]
        );
        assert_eq!(h.args("refuse_pauseless"), vec![json!({ "id": "ws_a", "act": "be woken" })]);
    });
}

// pause.test.ts:341 "pause on a record that says napping over a machine the provider holds running"
#[test]
fn wake_of_a_napping_record_whose_machine_runs_adopts_running() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.f.base.answer_with("runs_under_napping", vec![json!(true)]);
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(
            h.calls(&["machine_state", "runs_under_napping", "adopt_running", "ws_wake", "refuse_pauseless"]),
            ["runs_under_napping", "adopt_running"]
        );
    });
}

#[test]
fn wake_of_running_proves_an_unchecked_machine() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        entry.borrow_mut().unchecked = true;
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(h.calls(&["machine_state", "proven", "ws_wake"]), ["machine_state", "proven"]);
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert!(!entry.borrow().unchecked);
        wake(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.args("proven").len(), 1, "a checked machine is not proven again");
    });
}

// pause.test.ts:123 a wake asked during the pause waits for it (spec 4.4), and a failed nap is no reason to refuse
#[test]
fn wake_waits_for_a_nap_in_flight_and_drops_its_failure() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        let (tx, rx) = oneshot::channel();
        entry.borrow_mut().napping = Some(joined_error(rx));
        let task = h.wake_task("ws_a");
        settle().await;
        assert!(h.args("ws_wake").is_empty(), "the wake waits for the nap");
        tx.send(Err(plain("the provider refused the pause"))).unwrap();
        let view = task.await.unwrap().unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("ws_wake").len(), 1);
    });
}

#[test]
fn wake_stops_at_a_sub_thread_or_a_blocked_copy() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.f.base.answer_with("refuse_sub_wake", vec![err_answer("a sub-thread wakes by a send", Some("usage"))]);
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a sub-thread wakes by a send", Kind::Usage));
        h.f.base.script.borrow_mut().shift_remove("refuse_sub_wake");
        h.f.base.answer_with("copy_blocked", vec![err_answer("this computer blocks copies", Some("conflict"))]);
        let err = wake(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, kinded("this computer blocks copies", Kind::Conflict));
        assert_eq!(
            h.calls(&["entry_of", "refuse_sub_wake", "copy_blocked", "persist", "ws_wake"]),
            ["entry_of", "refuse_sub_wake", "entry_of", "refuse_sub_wake", "copy_blocked"]
        );
        assert!(h.persisted().is_empty());
    });
}

// ------------------------------------------------------------------------------------------------------ rule 4.8

// pause.test.ts:357 "wake takes the provider's running as the fact: the record becomes running with the line in"
#[test]
fn wake_resumes_persists_and_says_woken() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "wakeRefused": "an old refusal" }));
        entry.borrow_mut().delete_said =
            Some(DeleteSaid { phase: WorkspacePhase::Running, line: "a's machine m_a is still running".to_owned() });
        *h.f.engine_machine.borrow_mut() = Some("m_a2".to_owned());
        h.f.wakes.set(vec![WakePlan::Done(done(Some("the guest answered late")))]);
        let warned = wsp_core::console::capture();
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(view.machine_id, "m_a2");
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(
            h.persisted(),
            vec![
                expected(json!({ "phase": "waking", "wakeRefused": "an old refusal" })),
                expected(json!({ "phase": "running", "machineId": "m_a2" }))
            ]
        );
        assert_eq!(h.events(), vec![json!({ "type": "workspace.woken", "workspaceId": "ws_a", "machineId": "m_a2" })]);
        assert_eq!(h.args("ws_wake")[0]["landed"], json!(false));
        let status = h.args("emit_status");
        assert_eq!((&status[0]["reach"], &status[0]["words"], &status[0]["phase"]), (&json!("napping"), &json!(null), &json!("waking")));
        assert_eq!(
            (&status[1]["reach"], &status[1]["words"], &status[1]["phase"]),
            (&json!("reachable"), &json!("the guest answered late"), &json!("running"))
        );
        assert_eq!(
            h.calls(&["refuse_pauseless", "persist", "emit_status", "ws_wake", "follow_machine", "emit"]),
            ["refuse_pauseless", "persist", "emit_status", "ws_wake", "follow_machine", "persist", "emit", "emit_status"]
        );
        assert_eq!(warned.lines(), ["wake of ws_a: the guest answered late"]);
        assert!(entry.borrow().delete_said.is_none(), "deleteSaid is dropped");
        fields_clear(&entry);
    });
}

// wake-asks-again.test.ts:116 "asks again on its own once a cadence, counting the asks on the row"
#[test]
fn wake_asks_again_on_the_cadence_from_its_start() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.asks(4 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered())), WakePlan::Done(Err(unanswered())), WakePlan::Done(done(None))]);
        let began = h.now();
        let task = h.wake_task("ws_a");
        h.pump(&task).await;
        let view = task.await.unwrap().unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("sleeps").iter().map(|a| a["ms"].clone()).collect::<Vec<_>>(), [json!(EVERY), json!(EVERY)]);
        assert_eq!(began, 1_000_000);
        let asked: Vec<Value> = h.args("emit_status").iter().map(|a| a["wakeAsk"].clone()).collect();
        assert_eq!(asked[..3], [json!(null), json!({ "ask": 1, "of": 4 }), json!({ "ask": 2, "of": 4 })]);
        assert!(h.args("emit_status")[1]["wakeSaid"].is_null(), "wakeSaid is dropped between asks");
        assert_eq!(h.args("ws_wake").iter().map(|a| a["landed"].clone()).collect::<Vec<_>>(), [json!(false), json!(false), json!(false)]);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.woken", "workspaceId": "ws_a", "machineId": "m_a" })]);
        fields_clear(&entry);
    });
}

// wake-asks-again.test.ts:141 "counts the cadence from when the ask began, so a cap that eats half of it does not push the next ask out"
#[test]
fn wake_counts_the_cadence_from_when_the_ask_began() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.asks(3 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Hold, WakePlan::Done(Err(unanswered())), WakePlan::Done(done(None))]);
        let first = h.f.hold_wake();
        let began = h.now();
        let task = h.wake_task("ws_a");
        settle().await;
        // The provider sits on the call for half the cadence and the cap cuts it off there.
        h.clock().advance(EVERY / 2);
        first.send(Err(unanswered())).unwrap();
        settle().await;
        assert_eq!(h.args("sleeps"), vec![json!({ "ms": EVERY / 2, "now": began + EVERY / 2 })]);
        base::assert_due(h.clock(), &[began + EVERY]);
        h.clock().advance(EVERY / 2);
        h.pump(&task).await;
        task.await.unwrap().unwrap();
        // Ask 2 failed at once, a cadence in: what is left of its cadence is a whole one.
        assert_eq!(h.args("sleeps")[1]["ms"], json!(EVERY));
        assert_eq!(h.args("ws_wake").len(), 3);
    });
}

// wake-asks-again.test.ts:141 the other way round: a call that ran past the cadence is asked again at once
#[test]
fn wake_asks_again_at_once_when_the_ask_outran_the_cadence() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.asks(3 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Hold, WakePlan::Done(done(None))]);
        let first = h.f.hold_wake();
        let task = h.wake_task("ws_a");
        settle().await;
        h.clock().advance(EVERY + EVERY / 2);
        first.send(Err(unanswered())).unwrap();
        settle().await;
        assert_eq!(h.args("sleeps")[0]["ms"], json!(0));
        h.clock().advance(1);
        settle().await;
        assert!(task.is_finished());
        task.await.unwrap().unwrap();
        assert_eq!(h.args("ws_wake").len(), 2);
    });
}

// wake-asks-again.test.ts:207 "reads before it asks again, sends no second resume at one that came up"
#[test]
fn wake_reads_the_machine_before_asking_again() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.asks(30 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered())), WakePlan::Done(done(None))]);
        h.f.base.answer_with("took_the_resume", vec![json!(true)]);
        let task = h.wake_task("ws_a");
        h.pump(&task).await;
        task.await.unwrap().unwrap();
        assert_eq!(h.args("ws_wake").iter().map(|a| a["landed"].clone()).collect::<Vec<_>>(), [json!(false), json!(true)]);
        assert_eq!(h.calls(&["ws_wake", "sleeps", "took_the_resume"]), ["ws_wake", "sleeps", "took_the_resume", "ws_wake"]);
    });
}

// wake-asks-again.test.ts:171 and :245 gives up, and leaves the rebuild road on the record
#[test]
fn wake_gives_up_after_every_ask_and_writes_wake_refused() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.asks(3 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered()))]);
        let warned = wsp_core::console::capture();
        let task = h.wake_task("ws_a");
        h.pump(&task).await;
        let err = task.await.unwrap().unwrap_err();
        let line = gave_up("3 resume requests", "2m");
        assert_eq!(err, plain(&line));
        assert_eq!(h.args("ws_wake").len(), 3);
        assert_eq!(h.args("sleeps").len(), 2);
        let last = h.persisted().pop().unwrap();
        assert_eq!(last, expected(json!({ "phase": "napping", "wakeRefused": line })));
        assert_eq!(rec(&entry).wake_refused.as_deref(), Some(line.as_str()));
        let status = h.args("emit_status").pop().unwrap();
        assert_eq!((&status["reach"], &status["words"], &status["wakeAsk"]), (&json!("napping"), &json!(line), &json!(null)));
        assert_eq!(warned.lines(), [format!("wake of ws_a failed: {line}")]);
        assert_eq!(h.args("arm_late_read").len(), 1);
        assert!(h.events().is_empty());
        fields_clear(&entry);
        // A wake that lands takes the road away with the fault that made it.
        h.f.wakes.set(vec![WakePlan::Done(done(None))]);
        wake(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(rec(&entry).wake_refused, None);
    });
}

// wake-asks-again.test.ts:379 "a backend that declares no asking is asked once"
#[test]
fn wake_with_no_resume_asks_asks_once() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered()))]);
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert!(err.message.starts_with("the provider answered none of 1 resume request over "), "{}", err.message);
        assert!(err
            .message
            .ends_with("; the work on this machine's disk stays with the provider, and a rebuild starts a new machine from the image"));
        assert_eq!(h.args("ws_wake").len(), 1);
        assert!(h.args("sleeps").is_empty());
        assert!(h.args("emit_status").iter().all(|a| a["wakeAsk"].is_null()));
        assert_eq!(rec(&entry).phase, WorkspacePhase::Napping);
    });
}

// wake-asks-again.test.ts:280 "stops when the person stops it from the row, while the host waits to ask again"
#[test]
fn wake_stopped_between_asks_answers_wake_stopped_and_notes_paused() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.asks(30 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered()))]);
        let warned = wsp_core::console::capture();
        let task = h.wake_task("ws_a");
        settle().await;
        assert_eq!(h.clock().pending(), 1, "the host is waiting to ask again");
        let ctx = h.ctx.clone();
        let stop = tokio::task::spawn_local(async move { stop_wake(&ctx, "ws_a", Some(the_person())).await });
        settle().await;
        let left = stop.await.unwrap().unwrap();
        assert_eq!(left.phase, WorkspacePhase::Napping);
        assert_eq!(task.await.unwrap().unwrap_err(), plain(WAKE_STOPPED));
        assert_eq!(h.persisted().pop().unwrap(), expected(json!({ "phase": "napping" })));
        let status = h.args("emit_status").pop().unwrap();
        assert_eq!((&status["reach"], &status["words"]), (&json!("napping"), &json!(WAKE_STOPPED)));
        assert_eq!(h.args("ws_note_paused").len(), 1);
        assert_eq!(h.args("arm_late_read").len(), 1);
        assert!(warned.lines().is_empty(), "a stop is not warned");
        // Nothing asks again after the stop: the cadence's timer is spent with no call behind it.
        assert_eq!(h.clock().pending(), 0);
        h.clock().advance(EVERY * 5);
        settle().await;
        assert_eq!(h.args("ws_wake").len(), 1);
        fields_clear(&entry);
    });
}

// wake-asks-again.test.ts:306 "stops while the host is on a call by ending it"
#[test]
fn a_stop_while_the_host_is_on_a_call_ends_it() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.asks(30 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Hold]);
        let _never_answered = h.f.hold_wake();
        let task = h.wake_task("ws_a");
        settle().await;
        let left = stop_wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(left.phase, WorkspacePhase::Napping);
        assert_eq!(task.await.unwrap().unwrap_err(), plain(WAKE_STOPPED));
        assert_eq!(h.args("ws_wake").len(), 1);
        assert!(h.args("sleeps").is_empty(), "the stop ended the call before any wait");
        assert_eq!(h.args("emit_status").pop().unwrap()["words"], json!(WAKE_STOPPED));
        fields_clear(&entry);
    });
}

// box-nap.test.ts:552 "a rebuild refuses in one line when a napped workspace cannot be woken" shares the engine's words: noCommandsYetLine
#[test]
fn wake_says_no_commands_yet_for_a_restore_unfinished() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(restore_unfinished("m_a", "still restoring")))]);
        let warned = wsp_core::console::capture();
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        let line = "a is up and takes no commands yet (still restoring); wake it again in a minute or two";
        assert_eq!(err, plain(line));
        assert_eq!(warned.lines(), [format!("wake of ws_a failed: {line}")]);
        assert_eq!(h.persisted().pop().unwrap(), expected(json!({ "phase": "napping" })));
        assert_eq!(rec(&entry).wake_refused, None);
        let status = h.args("emit_status").pop().unwrap();
        assert_eq!((&status["reach"], &status["words"]), (&json!("napping"), &json!(line)));
    });
}

// pause.test.ts:465 "a wake the provider refuses leaves the record paused, not waking, and the next wake tries again"
#[test]
fn a_wake_the_provider_refuses_leaves_the_record_paused_with_the_error_on_the_row() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(other("the provider refused the resume"))), WakePlan::Done(done(None))]);
        let warned = wsp_core::console::capture();
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err.message, "the provider refused the resume");
        assert_eq!(warned.lines(), ["wake of ws_a failed: the provider refused the resume"]);
        assert_eq!(rec(&entry).phase, WorkspacePhase::Napping);
        assert_eq!(h.persisted().pop().unwrap(), expected(json!({ "phase": "napping" })));
        let status = h.args("emit_status").pop().unwrap();
        assert_eq!((&status["reach"], &status["words"]), (&json!("napping"), &json!("the provider refused the resume")));
        assert_eq!(h.args("arm_late_read").len(), 1);
        assert!(h.events().is_empty());
        fields_clear(&entry);
        assert_eq!(wake(&h.ctx, "ws_a", None).await.unwrap().phase, WorkspacePhase::Running);
    });
}

// gone.test.ts:196 a resume the provider answers 404 for is a sighting like any other
#[test]
fn a_wake_the_provider_answers_404_for_settles_the_record_gone_where_the_reads_agree() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(missing()))]);
        h.f.base.answer_with("settle_gone", vec![json!(true)]);
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        let at = format_seconds(1_000_000);
        assert_eq!(
            err,
            plain(&gone_refusal(
                "a",
                "wake",
                &format!("machine m_a is gone at the provider: the wake found it gone at {at} (the sandbox is gone)")
            ))
        );
        assert_eq!(h.args("settle_gone"), vec![json!({ "id": "ws_a", "by": "wake", "at": 1_000_000, "answer": "the sandbox is gone" })]);
        fields_clear(&entry);
        // Not settled: the provider's own error is the answer and the record falls back to what the engine says.
        *entry.borrow().record.borrow_mut() =
            record("ws_a", "cloud", "pr_cloud", json!({ "name": "a", "machineId": "m_a", "phase": "napping" }));
        h.f.base.script.borrow_mut().shift_remove("settle_gone");
        let err = wake(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err.message, "machine not found");
        assert_eq!(rec(&entry).phase, WorkspacePhase::Napping);
    });
}

// disk-gone.test.ts:45 a wake met by a deleted disk goes through disk_gone, in the same wake
#[test]
fn a_wake_met_by_a_deleted_disk_replaces_the_machine_in_the_same_wake() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(disk_gone_error("m_a", BOAT_LINE, SAID)))]);
        let view = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.machine_id, "m_new");
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(view.disk_gone.as_deref(), Some(replaced_line().as_str()));
        assert_eq!(h.args("ws_wake").len(), 1);
        assert_eq!(h.args("ws_rebuild").len(), 1);
        assert_eq!(rec(&entry).machine_id, "m_new");
        fields_clear(&entry);
    });
}

#[test]
fn wake_clears_its_join_fields_however_it_ends() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        entry.borrow_mut().wake_said = Some("said".to_owned());
        entry.borrow_mut().wake_ask = Some(wsp_core::types::wiring::WakeAsk { ask: 2, of: 3 });
        h.f.wakes.set(vec![WakePlan::Done(done(None))]);
        wake(&h.ctx, "ws_a", None).await.unwrap();
        fields_clear(&entry);
        h.cloud("ws_b", json!({ "name": "b", "machineId": "m_b" }));
        let entry_b = h.f.base.live.borrow()["ws_b"].clone();
        entry_b.borrow_mut().wake_said = Some("said".to_owned());
        h.f.wakes.set(vec![WakePlan::Done(Err(other("boom")))]);
        wake(&h.ctx, "ws_b", None).await.unwrap_err();
        fields_clear(&entry_b);
        h.cloud("ws_c", json!({ "name": "c", "machineId": "m_c" }));
        let entry_c = h.f.base.live.borrow()["ws_c"].clone();
        h.asks(30 * EVERY, EVERY);
        h.f.wakes.set(vec![WakePlan::Done(Err(unanswered()))]);
        let ctx = h.ctx.clone();
        let task = tokio::task::spawn_local(async move { wake(&ctx, "ws_c", None).await });
        settle().await;
        assert!(entry_c.borrow().wake_ask.is_some());
        stop_wake(&h.ctx, "ws_c", None).await.unwrap();
        task.await.unwrap().unwrap_err();
        fields_clear(&entry_c);
    });
}

// ------------------------------------------------------------------------------------------------------- rule 5

// disk-watch.test.ts:293 "is refused for a size not sold, on a machine that cannot move, and while a turn runs there"
#[test]
fn resize_refuses_in_the_ts_order() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        let ask = size(4.0, 8192.0);
        let order = ["entry_of", "spawn_guard", "refuse_pauseless", "refuse_sub_wake", "can_resume_at", "backend_capabilities", "nap_with"];
        // The guard first, after the entry is read.
        h.f.base.answer_with("spawn_guard", vec![err_answer("the cap", Some("conflict"))]);
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded("the cap", Kind::Conflict));
        h.f.base.script.borrow_mut().shift_remove("spawn_guard");
        assert_eq!(h.args("entry_of")[0], json!({ "id": "ws_a", "thread": null, "act": "wake", "scoped": false }));
        assert_eq!(h.args("spawn_guard")[0], json!({ "act": "resize", "scoped": false }));
        h.f.base.answer_with("refuse_pauseless", vec![err_answer("a cannot be moved to another size", Some("conflict"))]);
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded("a cannot be moved to another size", Kind::Conflict));
        assert_eq!(h.args("refuse_pauseless")[0]["act"], json!("be moved to another size"));
        h.f.base.script.borrow_mut().shift_remove("refuse_pauseless");
        h.f.base.answer_with("refuse_sub_wake", vec![err_answer("a sub-thread is moved by its lead", Some("usage"))]);
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded("a sub-thread is moved by its lead", Kind::Usage));
        h.f.base.script.borrow_mut().shift_remove("refuse_sub_wake");
        // A machine with no resumeAt.
        h.f.base.answer_with("can_resume_at", vec![json!(false)]);
        let no_move = refusal_line("Boat changes no machine's size once it is made", "Start a new thread at the size you want instead.");
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded(&no_move, Kind::Invalid));
        h.f.base.answer_with("provider_of", vec![json!(null)]);
        let none_named =
            refusal_line("the provider changes no machine's size once it is made", "Start a new thread at the size you want instead.");
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded(&none_named, Kind::Invalid));
        h.f.base.script.borrow_mut().shift_remove("can_resume_at");
        h.f.base.script.borrow_mut().shift_remove("provider_of");
        // A size the provider does not sell.
        let not_sold = refusal_line(
            "32x64 is not a size this provider offers; the sizes are 2x4 ($0.05/hr), 4x8 ($0.10/hr)",
            "Ask for one of those instead.",
        );
        assert_eq!(resize(&h.ctx, "ws_a", size(32.0, 65536.0), None).await.unwrap_err(), kinded(&not_sold, Kind::Invalid));
        // A turn running on a thread of the workspace.
        h.f.base.sessions.borrow_mut().push(
            serde_json::from_value(
                json!({ "id": "s1", "workspaceId": "ws_a", "harness": "claude", "status": "running", "threadId": "t_1" }),
            )
            .unwrap(),
        );
        h.f.running_threads.borrow_mut().push("t_1".to_owned());
        let busy = refusal_line(
            "a has a turn running, and the stop a move to another size takes would end it",
            "Stop the turn or wait for it to end, then move it.",
        );
        assert_eq!(resize(&h.ctx, "ws_a", ask, None).await.unwrap_err(), kinded(&busy, Kind::Conflict));
        assert_eq!(h.args("thread_runs"), vec![json!({ "thread": "t_1" })]);
        assert!(h.args("nap_with").is_empty());
        assert!(h.persisted().is_empty() && h.events().is_empty());
        let seen: Vec<String> = h.calls(&order).into_iter().take(6).collect();
        assert_eq!(seen, ["entry_of", "spawn_guard", "entry_of", "spawn_guard", "refuse_pauseless", "entry_of"]);
    });
}

#[test]
fn resize_to_the_same_size_does_nothing() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        let view = resize(&h.ctx, "ws_a", size(2.0, 4096.0), None).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert!(h.args("nap_with").is_empty() && h.args("ws_wake").is_empty());
        assert!(h.persisted().is_empty() && h.events().is_empty());
    });
}
