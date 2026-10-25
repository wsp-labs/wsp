// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of lifecycle (spec/lifecycle.md): the phase a reader sees while a wake runs, the order of every move,
//! and what a failed move leaves.
mod common;

use std::sync::{Arc, Mutex};

use futures::FutureExt;
use wsp_engine::errors::EngineError;
use wsp_engine::lifecycle::*;

fn hooks(attempts: u32) -> WorkspaceHooks {
    WorkspaceHooks {
        golden_snapshot: "wsp-mac-default-v1".into(),
        wake_attempts: attempts,
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

#[tokio::test]
async fn a_reader_holding_the_phase_sees_waking_while_the_check_runs_and_running_after() {
    let machine = common::fake_machine::FakeMachine::new("m1");
    let mut h = hooks(1);
    let seen: Arc<Mutex<Vec<WorkspacePhase>>> = Arc::default();
    let cell: Arc<Mutex<Option<Arc<Mutex<WorkspacePhase>>>>> = Arc::default();
    let (seen2, cell2) = (seen.clone(), cell.clone());
    h.wake_check = Some(Arc::new(move |_m| {
        let phase = *cell2.lock().unwrap().as_ref().unwrap().lock().unwrap();
        seen2.lock().unwrap().push(phase);
        async { Ok(None) }.boxed()
    }));
    let mut ws = Workspace::new(machine.clone(), h, WorkspaceInitial { phase: Some(WorkspacePhase::Napping), first_life: None });
    *cell.lock().unwrap() = Some(ws.phase_cell());
    assert_eq!(ws.wake(false).await.unwrap(), WakeResult { reason: None });
    assert_eq!(*seen.lock().unwrap(), vec![WorkspacePhase::Waking]);
    assert_eq!(ws.current_phase(), WorkspacePhase::Running);
    assert!(!ws.is_first_life());
}

#[tokio::test]
async fn a_wake_of_a_running_workspace_does_nothing_and_a_nap_of_a_napping_one_does_nothing() {
    let machine = common::fake_machine::FakeMachine::new("m1");
    let mut ws = Workspace::new(machine.clone(), hooks(2), WorkspaceInitial::default());
    assert_eq!(ws.wake(false).await.unwrap(), WakeResult::default());
    assert_eq!(machine.calls(), Vec::<String>::new());
    assert!(ws.is_first_life());
    let mut napping =
        Workspace::new(machine.clone(), hooks(2), WorkspaceInitial { phase: Some(WorkspacePhase::Napping), first_life: Some(false) });
    napping.nap().await.unwrap();
    assert_eq!(machine.calls(), Vec::<String>::new());
    assert!(!napping.is_first_life());
}

#[tokio::test]
async fn a_landed_wake_sends_no_resume_on_its_first_attempt_but_does_on_its_second() {
    let machine = common::fake_machine::FakeMachine::new("m1");
    let mut h = hooks(2);
    let checks = Arc::new(Mutex::new(0));
    let c = checks.clone();
    h.wake_check = Some(Arc::new(move |_m| {
        let mut n = c.lock().unwrap();
        *n += 1;
        let fault = (*n == 1).then(|| "daemon silent".to_string());
        async move { Ok(fault) }.boxed()
    }));
    let mut ws = Workspace::new(machine.clone(), h, WorkspaceInitial { phase: Some(WorkspacePhase::Napping), first_life: None });
    let got = ws.wake(true).await.unwrap();
    assert_eq!(got.reason.as_deref(), Some("attempt 1: daemon silent"));
    assert_eq!(machine.calls(), vec!["pause".to_string(), "resume".to_string()]);
    assert_eq!(ws.current_phase(), WorkspacePhase::Running);
}

#[tokio::test]
async fn a_failed_re_pause_is_one_more_fault_and_the_wake_goes_on() {
    let machine = common::fake_machine::FakeMachine::new("m1");
    let mut h = hooks(2);
    let n = Arc::new(Mutex::new(0));
    let n2 = n.clone();
    h.wake_check = Some(Arc::new(move |_m| {
        *n2.lock().unwrap() += 1;
        async { Ok(Some("silent".to_string())) }.boxed()
    }));
    h.move_hook = Some(Arc::new(|m, mv| {
        async move {
            match mv {
                ProviderMove::Pause => Err(EngineError::Other { message: "stop refused".into(), code: None }),
                ProviderMove::Resume => m.resume(None).await,
            }
        }
        .boxed()
    }));
    let mut ws = Workspace::new(machine.clone(), h, WorkspaceInitial { phase: Some(WorkspacePhase::Napping), first_life: None });
    let e = ws.wake(false).await.unwrap_err();
    let line = wsp_words::words::computer::wake_failed_line("m1", "attempt 1: silent; re-pause failed: stop refused; attempt 2: silent");
    assert_eq!(e.to_string(), line);
    assert_eq!(*n.lock().unwrap(), 2);
    assert_eq!(ws.current_phase(), WorkspacePhase::Napping);
    assert_eq!(machine.calls(), vec!["resume".to_string(), "resume".to_string()]);
}

#[tokio::test]
async fn a_wake_with_zero_attempts_fails_at_once_with_no_faults_and_no_move() {
    let machine = common::fake_machine::FakeMachine::new("m1");
    let mut ws = Workspace::new(machine.clone(), hooks(0), WorkspaceInitial { phase: Some(WorkspacePhase::Napping), first_life: None });
    let e = ws.wake(false).await.unwrap_err();
    assert_eq!(e.to_string(), wsp_words::words::computer::wake_failed_line("m1", ""));
    assert_eq!(machine.calls(), Vec::<String>::new());
    assert_eq!(ws.current_phase(), WorkspacePhase::Napping);
    assert!(ws.is_first_life());
}

#[tokio::test]
async fn an_upgrade_exports_retires_resurrects_imports_in_that_order_and_lands_running_first_life() {
    let log: Arc<Mutex<Vec<String>>> = Arc::default();
    let old = common::fake_machine::FakeMachine::new("old");
    let fresh = common::fake_machine::FakeMachine::new("new");
    let mut h = hooks(1);
    let (l1, l2, l3) = (log.clone(), log.clone(), log.clone());
    h.vault_export = Some(Arc::new(move |m| {
        l1.lock().unwrap().push(format!("export {}", m.id()));
        async { Ok(Some(b"vault".to_vec())) }.boxed()
    }));
    let f = fresh.clone();
    h.resurrect = Some(Arc::new(move |spec| {
        l2.lock().unwrap().push(format!("resurrect {}", spec.is_some()));
        let m: MachineRef = f.clone();
        async move { Ok(m) }.boxed()
    }));
    h.vault_import = Some(Arc::new(move |m, bytes| {
        l3.lock().unwrap().push(format!("import {} {}", m.id(), String::from_utf8(bytes).unwrap()));
        async { Ok(()) }.boxed()
    }));
    let mut ws = Workspace::new(old.clone(), h, WorkspaceInitial { phase: Some(WorkspacePhase::Napping), first_life: Some(false) });
    ws.upgrade(None).await.unwrap();
    assert_eq!(*log.lock().unwrap(), vec!["export old".to_string(), "resurrect false".to_string(), "import new vault".to_string()]);
    assert_eq!(old.calls(), vec!["kill".to_string()]);
    assert_eq!(ws.machine_id(), "new");
    assert_eq!(ws.current_phase(), WorkspacePhase::Running);
    assert!(ws.is_first_life());
}

#[tokio::test]
async fn an_upgrade_over_the_vault_cap_goes_on_with_no_import() {
    let old = common::fake_machine::FakeMachine::new("old");
    let fresh = common::fake_machine::FakeMachine::new("new");
    let imported = Arc::new(Mutex::new(false));
    let mut h = hooks(1);
    h.vault_export = Some(Arc::new(|_m| async { Ok(None) }.boxed()));
    let f = fresh.clone();
    h.resurrect = Some(Arc::new(move |_s| {
        let m: MachineRef = f.clone();
        async move { Ok(m) }.boxed()
    }));
    let i = imported.clone();
    h.vault_import = Some(Arc::new(move |_m, _b| {
        *i.lock().unwrap() = true;
        async { Ok(()) }.boxed()
    }));
    let mut ws = Workspace::new(old.clone(), h, WorkspaceInitial::default());
    ws.upgrade(None).await.unwrap();
    assert!(!*imported.lock().unwrap());
    assert_eq!(ws.machine_id(), "new");
}

#[tokio::test]
async fn a_rebuild_whose_old_machine_is_already_gone_still_lands_and_any_other_retire_failure_is_thrown_after_the_swap() {
    let old = common::fake_machine::FakeMachine::new("old");
    let fresh = common::fake_machine::FakeMachine::new("new");
    let mut h = hooks(1);
    let f = fresh.clone();
    h.resurrect = Some(Arc::new(move |_s| {
        let m: MachineRef = f.clone();
        async move { Ok(m) }.boxed()
    }));
    h.retire = Some(Arc::new(|_m| {
        async { Err(EngineError::Provider(wsp_engine::errors::classify(404, Some("not_found"), Some("gone"), None))) }.boxed()
    }));
    let mut ws = Workspace::new(old.clone(), h.clone(), WorkspaceInitial { phase: Some(WorkspacePhase::Napping), first_life: Some(false) });
    ws.rebuild().await.unwrap();
    assert_eq!((ws.machine_id(), ws.current_phase(), ws.is_first_life()), ("new".to_string(), WorkspacePhase::Running, true));

    h.retire = Some(Arc::new(|_m| async { Err(EngineError::Other { message: "delete refused".into(), code: None }) }.boxed()));
    let mut ws = Workspace::new(old.clone(), h, WorkspaceInitial { phase: Some(WorkspacePhase::Napping), first_life: Some(false) });
    let e = ws.rebuild().await.unwrap_err();
    assert_eq!(e.to_string(), "delete refused");
    // The swap stood before the retire failed; the phase did not move.
    assert_eq!(ws.machine_id(), "new");
    assert!(ws.is_first_life());
    assert_eq!(ws.current_phase(), WorkspacePhase::Napping);
}

#[tokio::test]
async fn a_port_route_is_kept_per_port_and_a_remint_drops_only_that_port() {
    let machine = common::fake_machine::FakeMachine::new("m1");
    let mut ws = Workspace::new(machine.clone(), hooks(1), WorkspaceInitial::default());
    let a = ws.port_reach(3000).await.unwrap();
    let b = ws.daemon_reach().await.unwrap();
    let a2 = ws.port_reach(3000).await.unwrap();
    assert_eq!(a, a2, "a fresh cached route is reused");
    let a3 = ws.remint_port_reach(3000).await.unwrap();
    assert_ne!(a3.token, a.token);
    let b2 = ws.daemon_reach().await.unwrap();
    assert_eq!(b2, b, "the daemon's route was not dropped");
    assert_eq!(machine.calls(), vec!["preview_url:3000".to_string(), "preview_url:7070".to_string(), "preview_url:3000".to_string()]);
}
