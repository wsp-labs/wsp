// SPDX-License-Identifier: AGPL-3.0-only
//! The Workspace through moves that fail half-way, held to what the TS left (tests/fixtures/scenes/lifecycle.json from
//! oracle/lifecycle_scenes.oracle.ts; spec/lifecycle.md rules W-1..W-10): every machine call and hook in order, then
//! the machine, phase and first life the workspace holds, and the failure's words.
mod common;

use std::sync::{Arc, Mutex};

use common::fake_machine::FakeMachine;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_engine::errors::EngineError;
use wsp_engine::lifecycle::*;

type Log = Arc<Mutex<Vec<String>>>;

fn want(name: &str) -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/lifecycle.json")).unwrap();
    serde_json::from_str::<Vec<Value>>(&text).unwrap().into_iter().find(|s| s["name"] == name).unwrap_or_else(|| panic!("no scene {name}"))
}

fn hooks() -> WorkspaceHooks {
    WorkspaceHooks {
        golden_snapshot: "g".into(),
        wake_attempts: 2,
        resurrect: None,
        vault_export: None,
        vault_import: None,
        stash_vault: None,
        restore_vault: None,
        wake_check: None,
        move_hook: None,
        retire: None,
    }
}

fn initial(w: &Value) -> WorkspaceInitial {
    WorkspaceInitial {
        phase: match w["initial"]["phase"].as_str() {
            Some("napping") => Some(WorkspacePhase::Napping),
            Some("running") => Some(WorkspacePhase::Running),
            _ => None,
        },
        first_life: w["initial"]["firstLife"].as_bool(),
    }
}

fn phase_word(p: WorkspacePhase) -> &'static str {
    match p {
        WorkspacePhase::Running => "running",
        WorkspacePhase::Napping => "napping",
        WorkspacePhase::Waking => "waking",
    }
}

fn resurrect(log: &Log) -> ResurrectHook {
    let log = log.clone();
    Arc::new(move |_spec| {
        log.lock().unwrap().push("resurrect".into());
        let m: MachineRef = FakeMachine::with_log("m2", log.clone());
        async move { Ok(m) }.boxed()
    })
}

fn failing(log: &Log, what: &'static str) -> MachineHook {
    let log = log.clone();
    Arc::new(move |_m| {
        log.lock().unwrap().push(what.into());
        async move { Err(EngineError::Other { message: format!("{what} broke"), code: None }) }.boxed()
    })
}

fn exporting(log: &Log, payload: &'static [u8]) -> VaultExportHook {
    let log = log.clone();
    Arc::new(move |_m| {
        log.lock().unwrap().push("export".into());
        async move { Ok(Some(payload.to_vec())) }.boxed()
    })
}

/// Holds the scene's log, answer, failure and the workspace's state after against the TS's.
fn same(w: &Value, log: &Log, ws: &Workspace, answer: Value, err: Option<EngineError>) {
    assert_eq!(json!(*log.lock().unwrap()), w["log"], "every call and hook in order");
    assert_eq!(answer, w["answer"], "the answer");
    assert_eq!(json!(err.as_ref().map(|e| e.to_string())), w["err"], "the failure's words");
    assert_eq!(ws.machine_id(), w["machine"].as_str().unwrap(), "the machine it holds");
    assert_eq!(phase_word(ws.current_phase()), w["phase"].as_str().unwrap(), "the phase");
    assert_eq!(ws.is_first_life(), w["firstLife"].as_bool().unwrap(), "first life");
}

fn wake_answer(r: &WakeResult) -> Value {
    match &r.reason {
        Some(reason) => json!({ "reason": reason }),
        None => json!({}),
    }
}

/// W-1: a check that answers `Some("")` names a fault: `attempt 1: ` is pushed and the machine paused and resumed again.
#[tokio::test]
async fn an_empty_fault_is_still_a_fault() {
    let w = want("an_empty_fault_is_still_a_fault");
    let log: Log = Arc::default();
    let mut h = hooks();
    let n = Arc::new(Mutex::new(0));
    h.wake_check = Some(Arc::new(move |_m| {
        let mut k = n.lock().unwrap();
        *k += 1;
        let fault = (*k == 1).then(String::new);
        async move { Ok(fault) }.boxed()
    }));
    let mut ws = Workspace::new(FakeMachine::with_log("m1", log.clone()), h, initial(&w));
    let r = ws.wake(false).await.unwrap();
    same(&w, &log, &ws, wake_answer(&r), None);
}

/// W-2: a rebuild whose restore fails has already swapped: the new machine and first life stand, the old one is never
/// retired, and the phase is not moved.
#[tokio::test]
async fn a_rebuild_whose_restore_fails_keeps_the_new_machine_and_leaves_the_old_alive() {
    let w = want("a_rebuild_whose_restore_fails_keeps_the_new_machine_and_leaves_the_old_alive");
    let log: Log = Arc::default();
    let mut h = hooks();
    h.resurrect = Some(resurrect(&log));
    h.restore_vault = Some(failing(&log, "restore"));
    let mut ws = Workspace::new(FakeMachine::with_log("m1", log.clone()), h, initial(&w));
    let e = ws.rebuild().await.err();
    same(&w, &log, &ws, Value::Null, e);
}

/// W-3: an upgrade whose export fails changes nothing and retires nothing.
#[tokio::test]
async fn an_upgrade_whose_export_fails_changes_nothing() {
    let w = want("an_upgrade_whose_export_fails_changes_nothing");
    let log: Log = Arc::default();
    let mut h = hooks();
    h.resurrect = Some(resurrect(&log));
    let l = log.clone();
    h.vault_export = Some(Arc::new(move |_m| {
        l.lock().unwrap().push("export".into());
        async { Err(EngineError::Other { message: "export broke".into(), code: None }) }.boxed()
    }));
    let mut ws = Workspace::new(FakeMachine::with_log("m1", log.clone()), h, initial(&w));
    let e = ws.upgrade(None).await.err();
    same(&w, &log, &ws, Value::Null, e);
}

/// W-4: an upgrade whose resurrect fails after the retire still holds the machine it killed, phase and life unmoved.
#[tokio::test]
async fn an_upgrade_whose_resurrect_fails_after_the_retire_keeps_the_killed_machine() {
    let w = want("an_upgrade_whose_resurrect_fails_after_the_retire_keeps_the_killed_machine");
    let log: Log = Arc::default();
    let mut h = hooks();
    let l = log.clone();
    h.resurrect = Some(Arc::new(move |_spec| {
        l.lock().unwrap().push("resurrect".into());
        async { Err(EngineError::Other { message: "resurrect broke".into(), code: None }) }.boxed()
    }));
    h.vault_export = Some(exporting(&log, b"v"));
    let mut ws = Workspace::new(FakeMachine::with_log("m1", log.clone()), h, initial(&w));
    let e = ws.upgrade(None).await.err();
    same(&w, &log, &ws, Value::Null, e);
}

/// W-5: an upgrade whose import fails holds the new machine, but neither the phase nor the first life moves.
#[tokio::test]
async fn an_upgrade_whose_import_fails_keeps_the_new_machine_and_the_old_phase() {
    let w = want("an_upgrade_whose_import_fails_keeps_the_new_machine_and_the_old_phase");
    let log: Log = Arc::default();
    let mut h = hooks();
    h.resurrect = Some(resurrect(&log));
    h.vault_export = Some(exporting(&log, b"v"));
    let l = log.clone();
    h.vault_import = Some(Arc::new(move |_m, _p| {
        l.lock().unwrap().push("import".into());
        async { Err(EngineError::Other { message: "import broke".into(), code: None }) }.boxed()
    }));
    let mut ws = Workspace::new(FakeMachine::with_log("m1", log.clone()), h, initial(&w));
    let e = ws.upgrade(None).await.err();
    same(&w, &log, &ws, Value::Null, e);
}

/// W-6: an empty payload is a payload: it is imported (a port testing `is_empty()` is wrong).
#[tokio::test]
async fn an_empty_payload_is_still_imported() {
    let w = want("an_empty_payload_is_still_imported");
    let log: Log = Arc::default();
    let mut h = hooks();
    h.resurrect = Some(resurrect(&log));
    h.vault_export = Some(exporting(&log, b""));
    let l = log.clone();
    h.vault_import = Some(Arc::new(move |_m, p| {
        l.lock().unwrap().push(format!("import:{}", p.len()));
        async { Ok(()) }.boxed()
    }));
    let mut ws = Workspace::new(FakeMachine::with_log("m1", log.clone()), h, initial(&w));
    let e = ws.upgrade(None).await.err();
    same(&w, &log, &ws, Value::Null, e);
}

/// W-7: a resume that fails on the first attempt is answered as itself, before first life ends: it stays true and the
/// phase goes back to napping.
#[tokio::test]
async fn a_failed_resume_keeps_the_first_life_and_naps() {
    let w = want("a_failed_resume_keeps_the_first_life_and_naps");
    let log: Log = Arc::default();
    let m = FakeMachine::with_log("m1", log.clone());
    m.fail_resume_with(EngineError::Other { message: "resume broke".into(), code: None });
    let mut ws = Workspace::new(m, hooks(), initial(&w));
    let e = ws.wake(false).await.err();
    same(&w, &log, &ws, Value::Null, e);
}

/// W-8: a wake's re-pause is the pause move alone; only `nap` stashes the vault.
#[tokio::test]
async fn a_wakes_re_pause_stashes_no_vault() {
    let w = want("a_wakes_re_pause_stashes_no_vault");
    let log: Log = Arc::default();
    let mut h = hooks();
    let l = log.clone();
    h.stash_vault = Some(Arc::new(move |_m| {
        l.lock().unwrap().push("stash".into());
        async { Ok(()) }.boxed()
    }));
    let n = Arc::new(Mutex::new(0));
    h.wake_check = Some(Arc::new(move |_m| {
        let mut k = n.lock().unwrap();
        *k += 1;
        let fault = (*k == 1).then(|| "silent".to_string());
        async move { Ok(fault) }.boxed()
    }));
    let mut ws = Workspace::new(FakeMachine::with_log("m1", log.clone()), h, initial(&w));
    let r = ws.wake(false).await.unwrap();
    same(&w, &log, &ws, wake_answer(&r), None);
}

/// W-9: a checkpoint reads no phase: a napping workspace snapshots with its life and stays napping.
#[tokio::test]
async fn a_checkpoint_of_a_napping_workspace_snapshots_and_moves_nothing() {
    let w = want("a_checkpoint_of_a_napping_workspace_snapshots_and_moves_nothing");
    let log: Log = Arc::default();
    let mut ws = Workspace::new(FakeMachine::with_log("m1", log.clone()), hooks(), initial(&w));
    let got = ws.checkpoint("cp").await.unwrap();
    same(&w, &log, &ws, json!(got), None);
}

/// W-10: a wake that runs out of attempts fails with a plain error (`Other`, no code) carrying `wake_failed_line`.
#[tokio::test]
async fn a_wake_that_runs_out_fails_with_plain_words() {
    let w = want("a_wake_that_runs_out_fails_with_plain_words");
    let log: Log = Arc::default();
    let mut h = hooks();
    h.wake_attempts = 1;
    h.wake_check = Some(Arc::new(|_m| async { Ok(Some("silent".to_string())) }.boxed()));
    let mut ws = Workspace::new(FakeMachine::with_log("m1", log.clone()), h, initial(&w));
    let e = ws.wake(false).await.unwrap_err();
    assert!(matches!(e, EngineError::Other { code: None, .. }), "{e:?}");
    same(&w, &log, &ws, Value::Null, Some(e));
}
