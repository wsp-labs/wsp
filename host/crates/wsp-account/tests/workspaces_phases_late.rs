// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `workspaces_phases.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support/base.rs"]
mod base;
#[path = "support/phases.rs"]
mod phases_support;
include!("workspaces_phases/world.rs");

// disk-watch.test.ts:234 "naps the machine and wakes it at the size asked, the record and its rate following"
#[test]
fn resize_naps_with_the_long_wait_then_wakes_at_the_new_size() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running", "sub": { "on": "box", "repo": "acme/x", "base": { "branch": "main", "sha": "abc" }, "branch": "wsp/x", "folder": "/work/x", "rateUsdPerHour": 0.05 } }));
        h.f.shapes.set(vec![Some(shape(4.0, 8192.0))]);
        assert_eq!(RESIZE_NAP_MS, 30 * 60_000);
        let view = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.unwrap();
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("nap_with"), vec![json!({ "id": "ws_a", "napWaitMs": 1_800_000 })]);
        assert!(entry.borrow().nap_wait_ms.is_none() && entry.borrow().resize_to.is_none());
        let wake_call = &h.args("ws_wake")[0];
        assert_eq!(
            (&wake_call["resizeTo"], &wake_call["size"], &wake_call["shape"]),
            (&json!({ "cpu": 4, "memMb": 8192 }), &json!({ "cpu": 4, "memMb": 8192 }), &json!(null))
        );
        let last = h.persisted().pop().unwrap();
        assert_eq!(last["size"], json!({ "cpu": 4, "memMb": 8192 }));
        assert_eq!(last["shape"], json!({ "cpu": 4, "memMb": 8192 }));
        assert_eq!(last["sub"]["rateUsdPerHour"], json!(0.1));
        assert_eq!(rec_json(&entry), last);
        assert_eq!(h.calls(&["nap_with", "ws_wake", "shape_of", "status_now"]), ["nap_with", "ws_wake", "shape_of", "status_now"]);
    });
}

// disk-watch.test.ts:259 "says so when the machine came back at another size than asked"
#[test]
fn resize_that_landed_elsewhere_follows_the_machine_and_refuses() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.shapes.set(vec![Some(shape(2.0, 4096.0))]);
        let err = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.unwrap_err();
        assert_eq!(err, kinded("a came back at 2x4, not the 4x8 asked for. Run wsp resize again.", Kind::Conflict));
        let last = h.persisted().pop().unwrap();
        assert_eq!(last["size"], json!({ "cpu": 2, "memMb": 4096 }));
        assert_eq!(last["shape"], json!({ "cpu": 2, "memMb": 4096 }));
        assert_eq!(rec_json(&entry), last);
        assert_eq!(h.args("status_now").len(), 1);
    });
}

// disk-watch.test.ts:273 "keeps the size the provider moved it to when the wake fails after the resume landed"
#[test]
fn resize_whose_wake_fails_follows_the_old_size() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.wakes.set(vec![WakePlan::Done(Err(other("still restoring")))]);
        h.f.shapes.set(vec![Some(shape(8.0, 16384.0))]);
        let err = resize(&h.ctx, "ws_a", size(8.0, 16384.0), None).await;
        // 8x16 is not offered by the default capabilities: the provider's own two sizes are the ones asked of.
        assert_eq!(
            err.unwrap_err().message,
            "8x16 is not a size this provider offers; the sizes are 2x4 ($0.05/hr), 4x8 ($0.10/hr). Ask for one of those instead."
        );
        // An offered size whose wake fails after the resume landed keeps the size the shape says.
        *h.f.caps.borrow_mut() = caps_with_sizes();
        h.f.shapes.set(vec![Some(shape(4.0, 8192.0))]);
        let err = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.unwrap_err();
        assert_eq!(err.message, "still restoring");
        assert_eq!(rec_json(&entry)["size"], json!({ "cpu": 4, "memMb": 8192 }));
        assert_eq!(rec_json(&entry)["shape"], json!({ "cpu": 4, "memMb": 8192 }));
        assert!(entry.borrow().resize_to.is_none());
        // A shape nobody read leaves the size as it was asked before, and no shape on the record.
        *entry.borrow().record.borrow_mut() =
            record("ws_a", "cloud", "pr_cloud", json!({ "name": "a", "machineId": "m_a", "phase": "running" }));
        h.f.shapes.set(vec![None]);
        let err = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.unwrap_err();
        assert_eq!(err.message, "still restoring");
        assert_eq!(rec_json(&entry)["size"], json!({ "cpu": 2, "memMb": 4096 }));
        assert!(rec_json(&entry).get("shape").is_none());
        assert!(entry.borrow().resize_to.is_none());
    });
}

#[test]
fn resize_clears_the_long_wait_when_the_nap_fails() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.base.answer_with("nap_with", vec![err_answer("the provider refused the pause", Some("conflict"))]);
        let err = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.unwrap_err();
        assert_eq!(err, kinded("the provider refused the pause", Kind::Conflict));
        assert!(entry.borrow().nap_wait_ms.is_none());
        assert!(entry.borrow().resize_to.is_none());
        assert_eq!(rec_json(&entry)["size"], json!({ "cpu": 2, "memMb": 4096 }));
        assert!(h.args("ws_wake").is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------- rules 6 and 7

// wake-asks-again.test.ts:373 "a stop asked of a workspace with no wake in flight answers with the record as it stands"
#[test]
fn stop_wake_pulls_the_stop_and_waits() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        let view = stop_wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(view.phase, WorkspacePhase::Running);
        // With a wake held on the provider, the stop ends it and the caller waits for it to end.
        *entry.borrow().record.borrow_mut() =
            record("ws_a", "cloud", "pr_cloud", json!({ "name": "a", "machineId": "m_a", "phase": "napping" }));
        h.f.wakes.set(vec![WakePlan::Hold]);
        let _held = h.f.hold_wake();
        let task = h.wake_task("ws_a");
        settle().await;
        let token = entry.borrow().wake_stop.clone().expect("a wake in flight holds its stop");
        let left = stop_wake(&h.ctx, "ws_a", None).await.unwrap();
        assert!(token.is_cancelled());
        assert!(task.is_finished(), "the stop returned after the wake ended");
        assert_eq!(left.phase, WorkspacePhase::Napping);
        assert_eq!(task.await.unwrap().unwrap_err(), plain(WAKE_STOPPED));
    });
}

// runtime-builders.test.ts:608 and machine-gates.test.ts:60
#[test]
fn upgrade_follows_the_new_machine_and_says_upgraded() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        let view = upgrade(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.machine_id, "m_new");
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(h.args("refuse_cannot"), vec![json!({ "id": "ws_a", "cap": "replacesMachine", "act": "have its machine replaced" })]);
        assert_eq!(h.persisted(), vec![expected(json!({ "phase": "running", "machineId": "m_new" }))]);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.upgraded", "workspaceId": "ws_a", "machineId": "m_new" })]);
        assert_eq!(
            h.calls(&["entry_of", "refuse_cannot", "ws_upgrade", "follow_machine", "persist", "emit"]),
            ["entry_of", "refuse_cannot", "ws_upgrade", "follow_machine", "persist", "emit"]
        );
        // A provider that stands no machine in refuses before the machine is touched.
        h.f.base.answer_with(
            "refuse_cannot",
            vec![err_answer("a cannot have its machine replaced on the provider it runs on", Some("conflict"))],
        );
        let err = upgrade(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, kinded("a cannot have its machine replaced on the provider it runs on", Kind::Conflict));
        assert_eq!(h.args("ws_upgrade").len(), 1);
        // An upgrade that fails persists and says nothing.
        h.f.base.script.borrow_mut().shift_remove("refuse_cannot");
        h.f.upgrades.set(vec![Err(other("no machine"))]);
        let err = upgrade(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err.message, "no machine");
        assert_eq!(h.persisted().len(), 1);
        assert_eq!(h.events().len(), 1);
    });
}

// ------------------------------------------------------------------------------------------------------- rule 8

// box-nap.test.ts:533 "a rebuild of a napped workspace wakes it and takes the vault off the woken machine"
#[test]
fn rebuild_of_a_napped_keeping_backend_wakes_first() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.base.answer_with("keeps_images", vec![json!(true)]);
        h.f.base.answer_with("pause_keeps_disk", vec![json!(true)]);
        let warned = wsp_core::console::capture();
        let view = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.machine_id, "m_new");
        assert_eq!(h.args("refuse_cannot")[0], json!({ "id": "ws_a", "cap": "replacesMachine", "act": "be rebuilt" }));
        // It woke first, then took the vault off the running machine: an upgrade, not a rebuild.
        assert_eq!(
            h.calls(&["recover_gone", "ws_wake", "ws_upgrade", "ws_rebuild", "follow_machine", "emit"]),
            ["recover_gone", "ws_wake", "follow_machine", "emit", "ws_upgrade", "follow_machine", "emit"]
        );
        assert_eq!(h.events().last().unwrap(), &json!({ "type": "workspace.upgraded", "workspaceId": "ws_a", "machineId": "m_new" }));
        let reason = "rebuilt: m_a replaced by m_new";
        assert_eq!(h.args("emit_status").pop().unwrap()["words"], json!(reason));
        assert_eq!(warned.lines(), [format!("rebuild of ws_a: {reason}")]);
        assert_eq!(rec(&entry).phase, WorkspacePhase::Running);
    });
}

// box-nap.test.ts:552 "a rebuild refuses in one line when a napped workspace cannot be woken, and keeps the machine"
#[test]
fn a_rebuild_whose_wake_fails_refuses_and_keeps_the_machine() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.base.answer_with("keeps_images", vec![json!(true)]);
        h.f.base.answer_with("pause_keeps_disk", vec![json!(true)]);
        h.f.wakes.set(vec![WakePlan::Done(Err(other("still restoring")))]);
        let err = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a could not be woken to back up before the rebuild: still restoring", Kind::Conflict));
        assert!(h.args("ws_upgrade").is_empty() && h.args("ws_rebuild").is_empty());
        assert_eq!(rec(&entry).machine_id, "m_a");
        assert!(h.events().is_empty());
    });
}

// disk-gone.test.ts:174 "a rebuild of a paused workspace whose disk is gone makes one new machine, not two"
#[test]
fn a_rebuild_whose_wake_met_a_deleted_disk_makes_one_machine_not_two() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.f.base.answer_with("keeps_images", vec![json!(true)]);
        h.f.base.answer_with("pause_keeps_disk", vec![json!(true)]);
        h.f.wakes.set(vec![WakePlan::Done(Err(disk_gone_error("m_a", BOAT_LINE, SAID)))]);
        let view = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.machine_id, "m_new");
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert_eq!(h.args("ws_rebuild").len(), 1);
        assert!(h.args("ws_upgrade").is_empty());
    });
}

// runtime-create.test.ts:548 and box-nap.test.ts:488
#[test]
fn rebuild_from_running_takes_a_vault_then_falls_back() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running", "wakeRefused": "an old refusal" }));
        h.f.base.answer_with("keeps_images", vec![json!(true)]);
        h.f.base.answer_with("pause_keeps_disk", vec![json!(true)]);
        let view = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(view.machine_id, "m_new");
        assert_eq!(h.calls(&["ws_wake", "ws_upgrade", "ws_rebuild"]), ["ws_upgrade"]);
        assert_eq!(h.persisted(), vec![expected(json!({ "phase": "running", "machineId": "m_new" }))]);
        // The vault over its cap: warned, and the machine is replaced with none.
        *entry.borrow().record.borrow_mut() =
            record("ws_a", "cloud", "pr_cloud", json!({ "name": "a", "machineId": "m_a", "phase": "running" }));
        *h.f.engine_machine.borrow_mut() = None;
        h.f.upgrades.set(vec![Err(other("vault over cap"))]);
        let warned = wsp_core::console::capture();
        rebuild(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.calls(&["ws_upgrade", "ws_rebuild"])[1..], ["ws_upgrade", "ws_rebuild"]);
        assert_eq!(
            warned.lines(),
            [
                "rebuild of ws_a: the running machine gave no vault (vault over cap); replacing with none".to_owned(),
                "rebuild of ws_a: rebuilt: m_a replaced by m_new".to_owned()
            ]
        );
        assert_eq!(h.args("emit_status").pop().unwrap()["words"], json!("rebuilt: m_a replaced by m_new"));
    });
}

// runtime-create.test.ts:548 and :581 "rebuild of a workspace that never napped imports nothing and says so"
#[test]
fn rebuild_reports_whether_a_nap_vault_was_there() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        let imported = h.cloud("ws_b", json!({ "name": "b", "machineId": "m_b" }));
        h.f.base.store.blobs.borrow_mut().entry("vaults".to_owned()).or_default().insert("ws_b".to_owned(), vec![1, 2, 3]);
        let none = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(none.machine_id, "m_new");
        assert_eq!(h.args("emit_status").pop().unwrap()["words"], json!("rebuilt: m_a replaced by m_new, no vault to import"));
        assert_eq!(h.calls(&["ws_wake", "ws_upgrade", "ws_rebuild"]), ["ws_rebuild"]);
        *h.f.engine_machine.borrow_mut() = None;
        rebuild(&h.ctx, "ws_b", None).await.unwrap();
        assert_eq!(h.args("emit_status").pop().unwrap()["words"], json!("rebuilt: m_b replaced by m_new, nap-time vault imported"));
        assert_eq!(h.events().len(), 2);
        assert_eq!(rec(&entry).phase, WorkspacePhase::Running);
        assert_eq!(rec(&imported).machine_id, "m_new");
    });
}

// gone.test.ts:955 and runtime-gone.test.ts:56: a record recovered out of gone is answered, not rebuilt
#[test]
fn a_rebuild_of_a_gone_record_that_recovers_answers_the_view() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "gone", "gone": "machine m_a is gone at the provider" }));
        h.f.base.answer_with("recover_gone", vec![json!("running")]);
        let view = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(view.phase, WorkspacePhase::Running);
        assert!(h.args("ws_rebuild").is_empty() && h.args("ws_upgrade").is_empty());
        assert!(h.events().is_empty());
    });
}

#[test]
fn rebuild_waits_for_a_wake_in_flight_and_drops_its_failure() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        let (tx, rx) = oneshot::channel();
        entry.borrow_mut().waking = Some(joined_error(rx));
        let ctx = h.ctx.clone();
        let task = tokio::task::spawn_local(async move { rebuild(&ctx, "ws_a", None).await });
        settle().await;
        assert!(h.args("ws_rebuild").is_empty());
        tx.send(Err(plain("the wake failed"))).unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(h.args("ws_rebuild").len(), 1);
        assert_eq!(h.calls(&["refuse_cannot", "recover_gone", "ws_rebuild"]), ["refuse_cannot", "recover_gone", "ws_rebuild"]);
    });
}

// ----------------------------------------------------------------------------------------------------- rules 9, 10

// reconcile.test.ts:374 "names the record, keeps its id and machine, says so once, and the next listing carries it"
#[test]
fn rename_writes_the_record_and_the_name_row_and_emits() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        let view = rename(&h.ctx, "ws_a", " fresh ", Some(the_person())).await.unwrap();
        assert_eq!(view.name, "fresh");
        assert_eq!(h.persisted(), vec![expected(json!({ "phase": "running", "name": "fresh" }))]);
        assert_eq!(rec_json(&entry), h.persisted()[0]);
        assert_eq!(
            h.f.base.store.row("workspace-names", "ws_a"),
            Some(json!({ "workspaceId": "ws_a", "name": "fresh", "project": "pr_cloud" }))
        );
        assert_eq!(h.writes(), ["put workspace-names/ws_a"]);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.renamed", "workspaceId": "ws_a", "name": "fresh" })]);
        assert_eq!(h.args("name_refusal"), vec![json!({ "name": "fresh" })]);
        // reconcile.test.ts:434: the same name after nameGiven writes nothing and emits nothing.
        let again = rename(&h.ctx, "ws_a", "  fresh", None).await.unwrap();
        assert_eq!(json_of(&again), view_json(&h, &entry));
        assert_eq!(h.persisted().len(), 1);
        assert_eq!(h.events().len(), 1);
        assert_eq!(h.args("name_refusal").len(), 1);
    });
}

// reconcile.test.ts:395 "takes the name the fork's own rule takes: one another workspace holds"
#[test]
fn rename_refuses_a_name_taken() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        let taken = "other is already a workspace; pick another name, or delete it first";
        h.f.base.answer_with("name_refusal", vec![json!(taken)]);
        let err = rename(&h.ctx, "ws_a", "other", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded(taken, Kind::Conflict));
        assert_eq!(rec(&entry).name, "a");
        assert!(h.persisted().is_empty() && h.events().is_empty() && h.writes().is_empty());
    });
}

// reconcile.test.ts:331 and :357 "holds the theme and the glyph beside the name" and "null clears one fact back to none"
#[test]
fn look_sets_theme_and_glyph_and_emits_nulls() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        let theme: Value = serde_json::from_str(THEME).unwrap();
        let both =
            WorkspaceLook { theme: Some(Some(serde_json::from_value(theme.clone()).unwrap())), glyph: Some(Some(WorkspaceGlyph::Bug)) };
        let view = look(&h.ctx, "ws_a", both, Some(the_person())).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, &entry));
        assert_eq!(rec_json(&entry)["theme"], theme);
        assert_eq!(rec_json(&entry)["glyph"], json!("bug"));
        assert_eq!(h.args("put_look").iter().map(|a| a["key"].clone()).collect::<Vec<_>>(), [json!("theme"), json!("glyph")]);
        assert_eq!(h.persisted().len(), 1);
        assert_eq!(h.events(), vec![json!({ "type": "workspace.look", "workspaceId": "ws_a", "theme": theme, "glyph": "bug" })]);
        // null clears one fact and leaves the other; the record loses the key rather than holding a null.
        let clear = WorkspaceLook { theme: Some(None), glyph: None };
        look(&h.ctx, "ws_a", clear, None).await.unwrap();
        assert!(rec_json(&entry).get("theme").is_none());
        assert_eq!(rec_json(&entry)["glyph"], json!("bug"));
        assert_eq!(h.events().pop().unwrap(), json!({ "type": "workspace.look", "workspaceId": "ws_a", "theme": null, "glyph": "bug" }));
        let clear_both = WorkspaceLook { theme: None, glyph: Some(None) };
        look(&h.ctx, "ws_a", clear_both, None).await.unwrap();
        assert_eq!(h.events().pop().unwrap(), json!({ "type": "workspace.look", "workspaceId": "ws_a", "theme": null, "glyph": null }));
        assert_eq!(h.persisted().len(), 3);
    });
}

// ---------------------------------------------------------------------------------------------------- rules 11, 15

// runtime-reach.test.ts:116 and :763
#[test]
fn update_daemon_refuses_in_the_ts_order() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({}));
        h.f.base.answer_with("served_by_its_computer", vec![json!("here")]);
        h.f.base.answer_with("has_deploy_daemon", vec![json!(false)]);
        let err = update_daemon(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, plain("a has no daemon of its own: Mac answers its files and git through this host"));
        h.f.base.script.borrow_mut().shift_remove("served_by_its_computer");
        let err = update_daemon(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, plain("this runtime cannot deploy a daemon; the host wires the bundle"));
        h.f.base.script.borrow_mut().shift_remove("has_deploy_daemon");
        let err = update_daemon(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, plain("wake a before updating its daemon"));
        assert!(h.args("deploy_daemon_on").is_empty());
        entry.borrow().record.borrow_mut().phase = WorkspacePhase::Running;
        update_daemon(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.args("deploy_daemon_on"), vec![json!({ "id": "ws_a" })]);
        h.f.base.answer_with("deploy_daemon_on", vec![err_answer("the deploy failed", None)]);
        assert_eq!(update_daemon(&h.ctx, "ws_a", None).await.unwrap_err(), plain("the deploy failed"));
    });
}

// local-workspace.test.ts:1387 and :1405 "says a host that wires no such road cannot start one"
#[test]
fn restart_daemon_drops_the_polled_reach_and_pushes_status() {
    run(async {
        let h = H::new();
        h.local("ws_a", json!({}));
        h.cloud("ws_b", json!({ "name": "b", "machineId": "m_b", "phase": "running" }));
        h.f.base.answer_with("has_restart_daemon", vec![json!(false)]);
        let err = restart_daemon(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, plain("a's daemon runs on this computer, which this host does not hold the process of"));
        let err = restart_daemon(&h.ctx, "ws_b", None).await.unwrap_err();
        assert_eq!(err, plain("b's daemon runs on a machine wsp forks, which this host does not hold the process of"));
        assert!(h.args("restart_daemon_of").is_empty());
        h.f.base.script.borrow_mut().shift_remove("has_restart_daemon");
        restart_daemon(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(
            h.calls(&["restart_daemon_of", "shared_daemon", "shared_daemon_replaced", "forget_polled_reach", "push_status"]),
            ["restart_daemon_of", "shared_daemon", "forget_polled_reach", "push_status"]
        );
        // local-workspace.test.ts:484 a shared daemon is replaced, even where the restart fails, and the reach stays.
        h.f.base.answer_with("shared_daemon", vec![json!(true)]);
        restart_daemon(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.args("shared_daemon_replaced").len(), 1);
        h.f.base.answer_with("restart_daemon_of", vec![err_answer("the daemon would not start", None)]);
        let err = restart_daemon(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, plain("the daemon would not start"));
        assert_eq!(h.args("shared_daemon_replaced").len(), 2);
        assert_eq!(h.args("forget_polled_reach").len(), 2);
        assert_eq!(h.args("push_status").len(), 2);
    });
}

// ------------------------------------------------------------------------------------------------------- rule 12

// runtime-index.test.ts / runtime.test.ts:63 a failed create is cleared with delete
#[test]
fn delete_of_a_failed_create_emits_deleted() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("forget_failed_create", vec![json!(true)]);
        delete(&h.ctx, "ws_failed", Some(the_person())).await.unwrap();
        assert_eq!(h.events(), vec![json!({ "type": "workspace.deleted", "workspaceId": "ws_failed" })]);
        assert_eq!(h.args("spawn_guard"), vec![json!({ "act": "delete", "scoped": false })]);
        assert_eq!(h.calls(&["spawn_guard", "forget_failed_create", "entry_of", "cut_off"]), ["spawn_guard", "forget_failed_create"]);
        // A live id is no failed create, whoever asks: the failed list is not asked.
        h.cloud("ws_a", json!({ "phase": "released" }));
        delete(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.args("forget_failed_create").len(), 1);
        assert_eq!(h.events().len(), 1);
    });
}

// the spec's edge case: a thread's delete of a failed create goes through entryOf and so refuses
#[test]
fn delete_of_a_failed_create_by_a_thread_goes_through_entry_of() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("forget_failed_create", vec![json!(true)]);
        let err = delete(&h.ctx, "ws_failed", Some(asker("t_lead1234567", "ws_lead", "t_root"))).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_failed", Kind::NotFound));
        assert!(h.args("forget_failed_create").is_empty());
        assert!(h.events().is_empty());
        assert_eq!(h.args("spawn_guard"), vec![json!({ "act": "delete", "scoped": true })]);
    });
}

// reconcile.test.ts:291 "a second delete of the same workspace joins the first: one kill, one drop"
#[test]
fn delete_joins_a_delete_in_flight() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        let (tx, rx) = oneshot::channel();
        *h.f.cut_gate.borrow_mut() = Some(rx);
        let c1 = h.ctx.clone();
        let first = tokio::task::spawn_local(async move { delete(&c1, "ws_a", None).await });
        settle().await;
        assert!(entry.borrow().deleting.is_some(), "the delete is stored for later callers to join");
        let c2 = h.ctx.clone();
        let second = tokio::task::spawn_local(async move { delete(&c2, "ws_a", None).await });
        settle().await;
        tx.send(()).unwrap();
        first.await.unwrap().unwrap();
        second.await.unwrap().unwrap();
        assert_eq!(h.args("cut_off").len(), 1);
        assert_eq!(h.args("unfork").len(), 1);
        assert_eq!(h.args("drop_workspace").len(), 1);
        assert!(entry.borrow().deleting.is_none());
        // And a failure is every caller's.
        let entry = h.cloud("ws_b", json!({ "name": "b", "machineId": "m_b", "phase": "running" }));
        let (tx, rx) = oneshot::channel();
        *h.f.cut_gate.borrow_mut() = Some(rx);
        h.f.base.answer_with("drop_workspace", vec![err_answer("the store refused", None)]);
        let (c1, c2) = (h.ctx.clone(), h.ctx.clone());
        let first = tokio::task::spawn_local(async move { delete(&c1, "ws_b", None).await });
        settle().await;
        let second = tokio::task::spawn_local(async move { delete(&c2, "ws_b", None).await });
        settle().await;
        tx.send(()).unwrap();
        assert_eq!(first.await.unwrap().unwrap_err(), plain("the store refused"));
        assert_eq!(second.await.unwrap().unwrap_err(), plain("the store refused"));
        assert!(entry.borrow().deleting.is_none());
    });
}

// sub-queue.test.ts:300 "a held spawn's record deleted goes with its start" and sub-queue.test.ts:519
#[test]
fn delete_of_a_queued_spawn_stops_it() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "queued" }));
        h.f.base.answer_with("delete_queued", vec![json!("stopped")]);
        delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(h.args("delete_queued"), vec![json!({ "id": "ws_a" })]);
        assert_eq!(h.calls(&["cut_off", "unfork", "machine_kill", "drop_workspace"]), Vec::<String>::new());
        assert!(entry.borrow().deleting.is_none());
        // A failed stop is the answer.
        h.f.base.script.borrow_mut().shift_remove("delete_queued");
        h.f.base.answer_with("delete_queued", vec![err_answer("the start already landed", Some("conflict"))]);
        let err = delete(&h.ctx, "ws_a", None).await.unwrap_err();
        assert_eq!(err, kinded("the start already landed", Kind::Conflict));
        // A queued record no spawn holds is deleted as any, asking nothing of a machine it has none of.
        h.f.base.script.borrow_mut().shift_remove("delete_queued");
        delete(&h.ctx, "ws_a", None).await.unwrap();
        assert_eq!(h.calls(&["cut_off", "unfork", "machine_kill", "drop_workspace"]), ["cut_off", "drop_workspace"]);
    });
}

// folder-threads.test.ts:800 "refuses a worktree wsp made while it holds a file no commit has, and removes it once clean"
#[test]
fn delete_refuses_a_worktree_with_changes_before_anything() {
    run(async {
        let h = H::new();
        let entry = h.local("ws_a", json!({ "worktree": { "path": "/wt/a", "branch": "wsp/a", "made": true } }));
        h.f.paths.borrow_mut().push("/wt/a".to_owned());
        h.f.base.answer_with("refuse_changed", vec![err_answer("a has a file no commit holds", Some("conflict"))]);
        let err = delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a has a file no commit holds", Kind::Conflict));
        assert_eq!(h.args("refuse_changed"), vec![json!({ "path": "/wt/a" })]);
        assert_eq!(h.calls(&["cut_off", "end_sessions", "drop_workspace"]), Vec::<String>::new());
        assert!(entry.borrow().deleting.is_none());
        // The rule reads only a worktree wsp made, that stands, on disk.
        h.f.base.script.borrow_mut().shift_remove("refuse_changed");
        let variants = [
            json!({ "path": "/wt/a", "made": false }),
            json!({ "path": "/wt/a", "made": true, "gone": true }),
            json!({ "path": "/wt/elsewhere", "made": true }),
        ];
        for (i, tree) in variants.iter().enumerate() {
            let id = format!("ws_v{i}");
            h.local(&id, json!({ "name": id, "worktree": tree }));
            delete(&h.ctx, &id, None).await.unwrap();
        }
        assert_eq!(h.args("refuse_changed").len(), 1);
        assert!(h.args("remove_worktree_ending").is_empty());
    });
}

// folder-threads.test.ts:766 and :787, reconcile.test.ts:175, pause.test.ts:237
#[test]
fn delete_takes_everything_away_in_the_ts_order() {
    run(async {
        let h = H::new();
        let entry = h.local("ws_a", json!({ "worktree": { "path": "/wt/a", "branch": "wsp/a", "made": true } }));
        entry.borrow_mut().delete_said = Some(DeleteSaid { phase: WorkspacePhase::Running, line: "an old line".to_owned() });
        h.f.paths.borrow_mut().push("/wt/a".to_owned());
        h.f.base.threads.borrow_mut().extend([
            ("t_1".to_owned(), base::thread_on("ws_a")),
            ("t_x".to_owned(), base::thread_on("ws_other")),
            ("t_2".to_owned(), base::thread_on("ws_a")),
        ]);
        for (id, thread, ws) in [("s1", "t_1", "ws_a"), ("s3", "t_3", "ws_a"), ("s4", "t_4", "ws_other"), ("s5", "t_5", "ws_a")] {
            h.f.base.sessions.borrow_mut().push(
                serde_json::from_value(
                    json!({ "id": id, "workspaceId": ws, "harness": "claude", "status": "completed", "threadId": thread }),
                )
                .unwrap(),
            );
        }
        delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        let order = [
            "spawn_guard",
            "entry_of",
            "refuse_changed",
            "cut_off",
            "end_sessions",
            "drop_checkpoints",
            "remove_worktree_ending",
            "drop_thread_files",
            "machine_kill",
            "unfork",
            "drop_workspace",
        ];
        assert_eq!(
            h.calls(&order),
            [
                "spawn_guard",
                "entry_of",
                "refuse_changed",
                "cut_off",
                "end_sessions",
                "drop_checkpoints",
                "drop_checkpoints",
                "drop_checkpoints",
                "drop_checkpoints",
                "remove_worktree_ending",
                "drop_thread_files",
                "machine_kill",
                "drop_workspace"
            ]
        );
        assert_eq!(h.args("end_sessions"), vec![json!({ "id": "ws_a", "reason": DELETED_REASON })]);
        // The thread records of the workspace, then the session rows' threads the records do not name.
        assert_eq!(
            h.args("drop_checkpoints").iter().map(|a| a["thread"].clone()).collect::<Vec<_>>(),
            [json!("t_1"), json!("t_2"), json!("t_3"), json!("t_5")]
        );
        assert_eq!(h.args("drop_thread_files"), vec![json!({ "id": "ws_a", "threads": ["t_1", "t_2"] })]);
        assert!(h.args("unfork").is_empty(), "a computer wsp does not run is killed, not unforked");
        assert!(entry.borrow().deleting.is_none() && entry.borrow().delete_said.is_none());
        assert!(!h.f.base.live.borrow().contains_key("ws_a"));
    });
}

// reconcile.test.ts:548 "keeps the record, listed and deletable, when the machine will not die" and runtime.test.ts:542
#[test]
fn delete_of_a_fork_unforks_its_machine_and_drops_no_checkpoints() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.base.threads.borrow_mut().push(("t_1".to_owned(), base::thread_on("ws_a")));
        delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(
            h.calls(&[
                "cut_off",
                "end_sessions",
                "drop_checkpoints",
                "remove_worktree_ending",
                "drop_thread_files",
                "machine_kill",
                "unfork",
                "drop_workspace"
            ]),
            ["cut_off", "end_sessions", "drop_thread_files", "unfork", "drop_workspace"]
        );
        // A machine the provider cannot find is gone already; another failure is the delete's, and the record stays.
        h.local("ws_b", json!({ "name": "b", "machineId": "m_b" }));
        h.f.kills.set(vec![Err(missing())]);
        delete(&h.ctx, "ws_b", None).await.unwrap();
        assert_eq!(h.args("drop_workspace").len(), 2);
        let entry = h.local("ws_c", json!({ "name": "c", "machineId": "m_c" }));
        h.f.kills.set(vec![Err(other("the kill was refused"))]);
        let err = delete(&h.ctx, "ws_c", None).await.unwrap_err();
        assert_eq!(err.message, "the kill was refused");
        assert_eq!(h.args("drop_workspace").len(), 2);
        assert!(h.f.base.live.borrow().contains_key("ws_c"));
        assert!(entry.borrow().deleting.is_none());
    });
}

#[test]
fn delete_asks_nothing_of_a_machine_that_is_not_standing() {
    run(async {
        let h = H::new();
        let released = h.cloud("ws_r", json!({ "phase": "released", "machineId": "" }));
        let lost = h.cloud("ws_l", json!({ "phase": "running", "sub": { "on": "box", "repo": "acme/x", "base": { "branch": "main", "sha": "abc" }, "branch": "wsp/x", "folder": "/work/x", "cloudGone": true } }));
        delete(&h.ctx, "ws_r", Some(the_person())).await.unwrap();
        delete(&h.ctx, "ws_l", None).await.unwrap();
        assert!(h.args("unfork").is_empty() && h.args("machine_kill").is_empty());
        assert_eq!(h.args("drop_workspace"), vec![json!({ "id": "ws_r" }), json!({ "id": "ws_l" })]);
        assert!(released.borrow().deleting.is_none() && lost.borrow().deleting.is_none());
    });
}

// runtime.test.ts:460 "a delete the provider answers and never acts on keeps the record, fails naming the machine" and :483
#[test]
fn delete_refused_by_a_machine_alive_says_why_on_the_record() {
    run(async {
        let h = H::new();
        let entry = h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.unforks.set(vec![Err(alive("m_a", "running"))]);
        let warned = wsp_core::console::capture();
        let err = delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        let line = "a's machine m_a is still running after three asks; run wsp delete again or delete it at the provider";
        assert_eq!(err, kinded(line, Kind::MachineAlive));
        let said = entry.borrow().delete_said.clone().expect("the line stands on the record");
        assert_eq!((said.phase, said.line.as_str()), (WorkspacePhase::Running, line));
        assert_eq!(warned.lines(), [line]);
        assert!(h.args("drop_workspace").is_empty());
        assert!(h.f.base.live.borrow().contains_key("ws_a"));
        assert!(entry.borrow().deleting.is_none());
        // The next delete starts by dropping the line, and one the provider takes drops the record.
        h.f.unforks.set(vec![Ok(())]);
        delete(&h.ctx, "ws_a", None).await.unwrap();
        assert!(entry.borrow().delete_said.is_none());
        assert_eq!(h.args("drop_workspace").len(), 1);
        // Any other failure of the delete propagates and says nothing on the record.
        let other_entry = h.cloud("ws_b", json!({ "name": "b", "machineId": "m_b", "phase": "running" }));
        h.f.unforks.set(vec![Err(other("the provider is down"))]);
        let err = delete(&h.ctx, "ws_b", None).await.unwrap_err();
        assert_eq!(err.message, "the provider is down");
        assert!(other_entry.borrow().delete_said.is_none());
        assert!(h.f.base.live.borrow().contains_key("ws_b"));
    });
}

// ---------------------------------------------------------------------------------------------------- rules 13, 14

// forget.test.ts:20, :41, :51 and :69
#[test]
fn forget_refuses_an_undriven_kind_and_a_machine_still_standing() {
    run(async {
        let h = H::new();
        h.local("ws_a", json!({}));
        let err = forget(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a runs on this computer, which is not at a provider; run wsp delete a", Kind::Conflict));
        assert!(h.args("read_gone").is_empty());
        h.f.base.hold(record("ws_p", "place", "pr_cloud", json!({ "name": "p", "machineId": "m_p" })));
        let err = forget(&h.ctx, "ws_p", None).await.unwrap_err();
        assert_eq!(err, kinded("p runs on a computer you joined, which is not at a provider; run wsp delete p", Kind::Conflict));
        // A machine any read still finds is kept whole, with the reason.
        h.cloud("ws_b", json!({ "name": "b", "machineId": "m1", "phase": "running" }));
        for state in ["running", "paused"] {
            h.f.base.script.borrow_mut().shift_remove("read_gone");
            h.f.base.answer_with("read_gone", vec![json!(state)]);
            let err = forget(&h.ctx, "ws_b", None).await.unwrap_err();
            assert_eq!(
                err,
                kinded(&format!("b's machine m1 is still {state}; pause it or delete it at the provider first"), Kind::Conflict)
            );
        }
        assert!(h.args("end_sessions").is_empty() && h.args("drop_workspace").is_empty());
        assert_eq!(h.args("read_gone").iter().map(|a| a["machine"].clone()).collect::<Vec<_>>(), [json!("m1"), json!("m1")]);
        // A gone one is dropped, its sessions ended with the delete's reason first.
        h.f.base.script.borrow_mut().shift_remove("read_gone");
        forget(&h.ctx, "ws_b", None).await.unwrap();
        assert_eq!(h.calls(&["read_gone", "end_sessions", "drop_workspace"]).split_off(2), ["read_gone", "end_sessions", "drop_workspace"]);
        assert_eq!(h.args("end_sessions"), vec![json!({ "id": "ws_b", "reason": DELETED_REASON })]);
        assert_eq!(h.args("drop_workspace"), vec![json!({ "id": "ws_b" })]);
    });
}

// idle.test.ts:375 "workspaces.touch over the wire starts the window over"
#[test]
fn touch_touches_the_idle_window() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        touch(&h.ctx, "ws_a", Some(the_person())).await.unwrap();
        assert_eq!(h.calls(&["entry_of", "idle_touch"]), ["entry_of", "idle_touch"]);
        assert_eq!(h.args("idle_touch"), vec![json!({ "id": "ws_a" })]);
        let err = touch(&h.ctx, "ws_none", None).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_none", Kind::NotFound));
        assert_eq!(h.args("idle_touch").len(), 1);
    });
}
