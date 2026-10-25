// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/lifecycle.test.ts. The fake machine records every call in order (common/fake_machine.rs);
//! a hook that wants its own entry in the same order writes into the machine's `log`.
//!
//! The TS hooks reach back into the workspace (`ws.daemonReach()`, `ws.currentPhase` inside `wakeCheck`); a Rust hook
//! holds the `Rc<Workspace>` and does the same, since every method takes `&self`. The two cases that do are the last
//! of the verified-wake group, beside the cases that pin the machine's own order.
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use common::fake_machine::FakeMachine;
use futures::FutureExt;
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_engine::lifecycle::{
    MachineHook, MachineRef, MoveHook, ProviderMove, ResurrectHook, WakeCheckHook, WakeResult, Workspace, WorkspaceHooks, WorkspaceInitial,
    WorkspacePhase,
};

type Log = Arc<Mutex<Vec<String>>>;

fn log() -> Log {
    Arc::new(Mutex::new(Vec::new()))
}

fn lines(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// `{ goldenSnapshot: "snap_g", wakeAttempts }` and no hook.
fn base(wake_attempts: u32) -> WorkspaceHooks {
    WorkspaceHooks {
        golden_snapshot: "snap_g".into(),
        wake_attempts,
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

fn machine_ref(m: &Arc<FakeMachine>) -> MachineRef {
    m.clone()
}

fn running() -> WorkspaceInitial {
    WorkspaceInitial::default()
}

/// A resurrect hook that records `resurrect` into `order` and answers a fresh machine `m2`.
fn resurrect_into(order: &Log) -> ResurrectHook {
    let order = order.clone();
    Arc::new(move |_spec| {
        let order = order.clone();
        async move {
            order.lock().unwrap().push("resurrect".into());
            Ok(machine_ref(&FakeMachine::new("m2")))
        }
        .boxed_local()
    })
}

/// A resurrect hook of `stubMachine({ id: "m2" })` that nothing in the test is meant to reach.
fn resurrect_m2() -> ResurrectHook {
    Arc::new(|_spec| async { Ok(machine_ref(&FakeMachine::new("m2"))) }.boxed_local())
}

/// A wake check that answers `fault` for the first `failing` checks, then healthy; each check is one `check` entry.
fn check_failing(log: &Log, failing: u32, fault: &'static str) -> WakeCheckHook {
    let log = log.clone();
    let seen = Arc::new(AtomicU32::new(0));
    Arc::new(move |_m| {
        let log = log.clone();
        let n = seen.fetch_add(1, Ordering::SeqCst) + 1;
        async move {
            log.lock().unwrap().push("check".into());
            Ok((n <= failing).then(|| fault.to_string()))
        }
        .boxed_local()
    })
}

fn missing(message: &str) -> EngineError {
    EngineError::Provider(WspError {
        kind: ErrorKind::Missing,
        status: 404,
        code: None,
        message: message.into(),
        request_id: None,
        window: None,
        said: None,
    })
}

/// lifecycle.test.ts:30
#[tokio::test]
async fn checkpoint_hands_the_machine_the_life_it_tracked_true_before_a_wake_and_false_after() {
    let m = FakeMachine::new("m1");
    let ws = Workspace::new(machine_ref(&m), base(2), running());
    assert_eq!(ws.checkpoint("v1").await.unwrap(), "snap_x");
    ws.nap().await.unwrap();
    ws.wake(false).await.unwrap();
    assert_eq!(ws.checkpoint("v2").await.unwrap(), "snap_x");
    assert_eq!(m.calls(), strs(&["snapshot:v1:true", "pause", "resume", "snapshot:v2:false"]));
}

/// lifecycle.test.ts:39
#[tokio::test]
async fn a_backends_refusal_of_a_resumed_machine_comes_through_checkpoint_as_itself() {
    let m = FakeMachine::new("m1");
    m.refuse_resumed_snapshots();
    let ws = Workspace::new(machine_ref(&m), base(2), running());
    ws.nap().await.unwrap();
    ws.wake(false).await.unwrap();
    let e = ws.checkpoint("v2").await.unwrap_err();
    assert!(e.to_string().contains("snapshot v2 refused: resumed"), "{e}");
    assert_eq!(m.calls(), strs(&["pause", "resume", "snapshot:v2:false"]));
}

/// lifecycle.test.ts:45
#[tokio::test]
async fn a_wake_of_a_machine_that_vanished_while_paused_throws_the_providers_404_and_forks_nothing() {
    let dead = FakeMachine::new("m1");
    dead.fail_resume_with(missing("gone"));
    let forked = Arc::new(AtomicBool::new(false));
    let resurrect: ResurrectHook = {
        let forked = forked.clone();
        Arc::new(move |_spec| {
            forked.store(true, Ordering::SeqCst);
            async { Ok(machine_ref(&FakeMachine::new("m2"))) }.boxed_local()
        })
    };
    let ws = Workspace::new(machine_ref(&dead), WorkspaceHooks { resurrect: Some(resurrect), ..base(2) }, running());
    ws.nap().await.unwrap();
    let e = ws.wake(false).await.unwrap_err();
    assert_eq!(e.kind(), Some(ErrorKind::Missing), "{e:?}");
    assert!(!forked.load(Ordering::SeqCst));
    assert_eq!(ws.machine_id(), "m1");
    assert_eq!(ws.current_phase(), WorkspacePhase::Napping);
    assert_eq!(dead.calls(), strs(&["pause", "resume:failed"]));
}

/// lifecycle.test.ts:62. A check that does not dial; the workspace is asked for the route after each wake, and the
/// machine's own order shows whether it was minted again or handed from the cache, on both roads into the check.
#[tokio::test]
async fn the_wake_check_dials_a_route_minted_after_the_resume_not_the_one_cached_before_the_nap() {
    let order = log();
    let m = FakeMachine::with_log("m1", order.clone());
    let ws = Workspace::new(machine_ref(&m), WorkspaceHooks { wake_check: Some(check_failing(&order, 0, "")), ..base(2) }, running());
    let before = ws.daemon_reach().await.unwrap();
    ws.nap().await.unwrap();
    ws.wake(false).await.unwrap();
    assert_eq!(before.token, "t1");
    // The route cached before the nap was dropped when the machine resumed: the next ask mints again.
    assert_eq!(ws.daemon_reach().await.unwrap().token, "t2");
    // A resume that landed without its call takes the same road: the routes are dropped before the check.
    ws.nap().await.unwrap();
    ws.wake(true).await.unwrap();
    assert_eq!(ws.daemon_reach().await.unwrap().token, "t3");
    assert_eq!(
        lines(&order),
        strs(&["preview_url:7070", "pause", "resume", "check", "preview_url:7070", "pause", "check", "preview_url:7070"])
    );
}

/// lifecycle.test.ts:84, less the check's read of the phase, which the next case makes.
#[tokio::test]
async fn holds_the_phase_until_the_check_passes_then_stays_on_the_same_machine() {
    let order = log();
    let m = FakeMachine::with_log("m1", order.clone());
    let ws = Workspace::new(
        machine_ref(&m),
        WorkspaceHooks { resurrect: Some(resurrect_m2()), wake_check: Some(check_failing(&order, 0, "")), ..base(2) },
        running(),
    );
    ws.nap().await.unwrap();
    let result = ws.wake(false).await.unwrap();
    assert_eq!(lines(&order), strs(&["pause", "resume", "check"]));
    assert_eq!(ws.current_phase(), WorkspacePhase::Running);
    assert_eq!(ws.machine_id(), "m1");
    assert_eq!(result, WakeResult::default());
    assert_eq!(m.count("resume"), 1);
}

/// lifecycle.test.ts:62 as the TS writes it: the check dials `ws.daemonReach()` through the workspace it belongs to.
#[tokio::test]
async fn a_wake_check_that_dials_through_the_workspace_gets_a_route_minted_after_the_resume() {
    let m = FakeMachine::new("m1");
    let slot: Rc<RefCell<Option<Rc<Workspace>>>> = Rc::default();
    let handed: Rc<RefCell<Vec<String>>> = Rc::default();
    let check: WakeCheckHook = {
        let (slot, handed) = (slot.clone(), handed.clone());
        Arc::new(move |_m| {
            let ws = slot.borrow().clone().unwrap();
            let handed = handed.clone();
            async move {
                let reach = ws.daemon_reach().await.unwrap();
                handed.borrow_mut().push(reach.token);
                Ok(None)
            }
            .boxed_local()
        })
    };
    let ws = Rc::new(Workspace::new(machine_ref(&m), WorkspaceHooks { wake_check: Some(check), ..base(2) }, running()));
    *slot.borrow_mut() = Some(ws.clone());
    let before = ws.daemon_reach().await.unwrap();
    ws.nap().await.unwrap();
    ws.wake(false).await.unwrap();
    assert_eq!(before.token, "t1");
    // The check's own dial minted again; the route cached before the nap was not what it went through.
    assert_eq!(*handed.borrow(), vec!["t2".to_string()]);
    ws.nap().await.unwrap();
    ws.wake(true).await.unwrap();
    assert_eq!(*handed.borrow(), vec!["t2".to_string(), "t3".to_string()]);
    slot.borrow_mut().take();
}

/// lifecycle.test.ts:84 as the TS writes it: the check reads `ws.currentPhase` and sees `waking`.
#[tokio::test]
async fn a_wake_check_that_reads_the_workspace_sees_waking_and_the_wake_ends_running() {
    let m = FakeMachine::new("m1");
    let slot: Rc<RefCell<Option<Rc<Workspace>>>> = Rc::default();
    let seen: Rc<RefCell<Vec<WorkspacePhase>>> = Rc::default();
    let check: WakeCheckHook = {
        let (slot, seen) = (slot.clone(), seen.clone());
        Arc::new(move |_m| {
            let ws = slot.borrow().clone().unwrap();
            seen.borrow_mut().push(ws.current_phase());
            async { Ok(None) }.boxed_local()
        })
    };
    let ws = Rc::new(Workspace::new(
        machine_ref(&m),
        WorkspaceHooks { resurrect: Some(resurrect_m2()), wake_check: Some(check), ..base(2) },
        running(),
    ));
    *slot.borrow_mut() = Some(ws.clone());
    ws.nap().await.unwrap();
    let result = ws.wake(false).await.unwrap();
    assert_eq!(*seen.borrow(), vec![WorkspacePhase::Waking]);
    assert_eq!(ws.current_phase(), WorkspacePhase::Running);
    assert_eq!(ws.machine_id(), "m1");
    assert_eq!(result, WakeResult::default());
    assert_eq!(m.count("resume"), 1);
    slot.borrow_mut().take();
}

/// lifecycle.test.ts:102
#[tokio::test]
async fn a_check_that_fails_once_is_paused_and_resumed_again_and_a_passing_second_try_keeps_the_machine() {
    let m = FakeMachine::new("m1");
    let hooks =
        WorkspaceHooks { resurrect: Some(resurrect_m2()), wake_check: Some(check_failing(&log(), 1, "daemon did not answer")), ..base(2) };
    let ws = Workspace::new(machine_ref(&m), hooks, running());
    ws.nap().await.unwrap();
    let result = ws.wake(false).await.unwrap();
    assert_eq!(m.calls(), strs(&["pause", "resume", "pause", "resume"]));
    assert_eq!(m.count("kill"), 0);
    assert_eq!(ws.machine_id(), "m1");
    assert!(result.reason.expect("a reason").contains("daemon did not answer"));
}

/// lifecycle.test.ts:118
#[tokio::test]
async fn two_failed_checks_fail_the_wake_with_both_faults_and_the_machine_stays() {
    let m = FakeMachine::new("m1");
    let order = log();
    let restore_vault: MachineHook = {
        let order = order.clone();
        Arc::new(move |m| {
            let order = order.clone();
            async move {
                order.lock().unwrap().push(format!("restore:{}", m.id()));
                Ok(())
            }
            .boxed_local()
        })
    };
    let wake_check: WakeCheckHook = Arc::new(|m| async move { Ok(Some(format!("memMb 2048 != 4096 on {}", m.id()))) }.boxed_local());
    let hooks = WorkspaceHooks {
        resurrect: Some(resurrect_into(&order)),
        restore_vault: Some(restore_vault),
        wake_check: Some(wake_check),
        ..base(2)
    };
    let ws = Workspace::new(machine_ref(&m), hooks, running());
    ws.nap().await.unwrap();
    let e = ws.wake(false).await.unwrap_err();
    assert_eq!(
        e.to_string(),
        "the wake of m1 did not finish: attempt 1: memMb 2048 != 4096 on m1; attempt 2: memMb 2048 != 4096 on m1; the workspace keeps this machine and its disk"
    );
    assert_eq!(m.calls(), strs(&["pause", "resume", "pause", "resume"]));
    assert_eq!(m.count("kill"), 0);
    assert_eq!(lines(&order), Vec::<String>::new());
    assert_eq!(ws.machine_id(), "m1");
    assert_eq!(ws.current_phase(), WorkspacePhase::Napping);
}

/// lifecycle.test.ts:136
#[tokio::test]
async fn a_workspace_given_wake_attempts_1_never_re_pauses() {
    let m = FakeMachine::new("m1");
    let hooks =
        WorkspaceHooks { resurrect: Some(resurrect_m2()), wake_check: Some(check_failing(&log(), 99, "daemon did not answer")), ..base(1) };
    let ws = Workspace::new(machine_ref(&m), hooks, running());
    ws.nap().await.unwrap();
    let e = ws.wake(false).await.unwrap_err();
    assert!(e.to_string().contains("the wake of m1 did not finish: attempt 1: daemon did not answer"), "{e}");
    // A provider that bills every start declares one attempt: the re-pause and second resume never happen.
    assert_eq!(m.calls(), strs(&["pause", "resume"]));
    assert_eq!(m.count("kill"), 0);
    assert_eq!(ws.machine_id(), "m1");
}

/// lifecycle.test.ts:151
#[tokio::test]
async fn a_resume_that_ends_with_the_guest_unusable_fails_the_wake_with_the_providers_words() {
    let m = FakeMachine::new("m1");
    m.fail_resume_with(EngineError::GuestUnusable {
        machine_id: "m1".into(),
        provider: "Boat".into(),
        detail: "Error 24".into(),
        status: 200,
        // errors.ts:132 guestUnusableLine("Boat", "m1", "Error 24"), thread.ts:124.
        line: "Boat left m1 running but nothing on it can run: Error 24".into(),
    });
    let order = log();
    let wake_check: WakeCheckHook = {
        let order = order.clone();
        Arc::new(move |_m| {
            let order = order.clone();
            async move {
                order.lock().unwrap().push("check".into());
                Err(EngineError::Other { message: "a machine nothing can run on is never checked".into(), code: None })
            }
            .boxed_local()
        })
    };
    let hooks = WorkspaceHooks { resurrect: Some(resurrect_into(&order)), wake_check: Some(wake_check), ..base(2) };
    let ws = Workspace::new(machine_ref(&m), hooks, running());
    ws.nap().await.unwrap();
    let e = ws.wake(false).await.unwrap_err();
    assert!(e.to_string().contains("Boat left m1 running but nothing on it can run: Error 24"), "{e}");
    assert_eq!(ws.machine_id(), "m1");
    assert_eq!(ws.current_phase(), WorkspacePhase::Napping);
    assert_eq!(m.calls(), strs(&["pause", "resume:failed"]));
    assert_eq!(m.count("resume"), 0);
    assert_eq!(m.count("kill"), 0);
    assert_eq!(lines(&order), Vec::<String>::new());
}

/// lifecycle.test.ts:166
#[tokio::test]
async fn nap_stashes_the_vault_before_the_pause() {
    let order = log();
    let m = FakeMachine::with_log("m1", order.clone());
    let stash: MachineHook = {
        let order = order.clone();
        Arc::new(move |_m| {
            let order = order.clone();
            async move {
                order.lock().unwrap().push("stash".into());
                Ok(())
            }
            .boxed_local()
        })
    };
    let ws = Workspace::new(machine_ref(&m), WorkspaceHooks { stash_vault: Some(stash), ..base(2) }, running());
    ws.nap().await.unwrap();
    assert_eq!(lines(&order), strs(&["stash", "pause"]));
}

/// lifecycle.test.ts:174
#[tokio::test]
async fn note_running_puts_a_napping_phase_back_to_running_so_the_next_nap_pauses_the_machine_for_real() {
    let m = FakeMachine::new("m1");
    let ws = Workspace::new(machine_ref(&m), base(2), WorkspaceInitial { phase: Some(WorkspacePhase::Napping), first_life: None });
    ws.nap().await.unwrap();
    assert_eq!(m.count("pause"), 0);
    ws.note_running();
    assert_eq!(ws.current_phase(), WorkspacePhase::Running);
    ws.nap().await.unwrap();
    assert_eq!(m.count("pause"), 1);
    assert_eq!(ws.current_phase(), WorkspacePhase::Napping);
}

/// lifecycle.test.ts:186
#[tokio::test]
async fn a_failed_wake_with_no_resurrect_hook_throws_and_leaves_the_workspace_napping() {
    let m = FakeMachine::new("m1");
    let wake_check: WakeCheckHook = Arc::new(|_m| async { Ok(Some("no daemon".to_string())) }.boxed_local());
    let ws = Workspace::new(machine_ref(&m), WorkspaceHooks { wake_check: Some(wake_check), ..base(2) }, running());
    ws.nap().await.unwrap();
    let e = ws.wake(false).await.unwrap_err();
    assert!(e.to_string().contains("no daemon"), "{e}");
    assert_eq!(ws.current_phase(), WorkspacePhase::Napping);
}

fn move_word(m: ProviderMove) -> &'static str {
    match m {
        ProviderMove::Pause => "pause",
        ProviderMove::Resume => "resume",
    }
}

/// lifecycle.test.ts:196
#[tokio::test]
async fn nap_and_wake_run_their_pause_and_resume_through_the_move_hook_with_the_machine() {
    let m = FakeMachine::new("m1");
    let moves = log();
    let move_hook: MoveHook = {
        let moves = moves.clone();
        Arc::new(move |m, mv| {
            let moves = moves.clone();
            async move {
                moves.lock().unwrap().push(format!("{}@{}", move_word(mv), m.id()));
                match mv {
                    ProviderMove::Pause => m.pause(None).await,
                    ProviderMove::Resume => m.resume(None).await,
                }
            }
            .boxed_local()
        })
    };
    let ws = Workspace::new(machine_ref(&m), WorkspaceHooks { move_hook: Some(move_hook), ..base(2) }, running());
    ws.nap().await.unwrap();
    ws.wake(false).await.unwrap();
    assert_eq!(lines(&moves), strs(&["pause@m1", "resume@m1"]));
    assert_eq!(m.calls(), strs(&["pause", "resume"]));
}

/// lifecycle.test.ts:210
#[tokio::test]
async fn a_wakes_re_pause_goes_through_the_hook_too_and_a_move_the_hook_gives_up_on_leaves_the_phase_where_it_was() {
    let m = FakeMachine::new("m1");
    let moves = log();
    let move_hook: MoveHook = {
        let moves = moves.clone();
        Arc::new(move |m, mv| {
            let moves = moves.clone();
            async move {
                let first = {
                    let mut moves = moves.lock().unwrap();
                    moves.push(move_word(mv).to_string());
                    moves.len() == 1
                };
                if first {
                    return Err(EngineError::Other { message: "pause did not complete".into(), code: None });
                }
                match mv {
                    ProviderMove::Pause => m.pause(None).await,
                    ProviderMove::Resume => m.resume(None).await,
                }
            }
            .boxed_local()
        })
    };
    let hooks = WorkspaceHooks {
        resurrect: Some(resurrect_m2()),
        wake_check: Some(check_failing(&log(), 1, "no daemon")),
        move_hook: Some(move_hook),
        ..base(2)
    };
    let ws = Workspace::new(machine_ref(&m), hooks, running());
    let e = ws.nap().await.unwrap_err();
    assert!(e.to_string().contains("pause did not complete"), "{e}");
    assert_eq!(ws.current_phase(), WorkspacePhase::Running);
    ws.nap().await.unwrap();
    assert_eq!(ws.current_phase(), WorkspacePhase::Napping);
    ws.wake(false).await.unwrap();
    assert_eq!(lines(&moves), strs(&["pause", "pause", "resume", "pause", "resume"]));
    assert_eq!(m.calls(), strs(&["pause", "resume", "pause", "resume"]));
    assert_eq!(ws.machine_id(), "m1");
}

/// lifecycle.test.ts:236
#[tokio::test]
async fn rebuild_replaces_a_running_machine_with_a_golden_fork_restores_the_vault_kills_the_old_one() {
    let zombie = FakeMachine::new("m1");
    let order = log();
    let restore_vault: MachineHook = {
        let order = order.clone();
        Arc::new(move |m| {
            let order = order.clone();
            async move {
                order.lock().unwrap().push(format!("restore:{}", m.id()));
                Ok(())
            }
            .boxed_local()
        })
    };
    let hooks = WorkspaceHooks { resurrect: Some(resurrect_into(&order)), restore_vault: Some(restore_vault), ..base(2) };
    let ws = Workspace::new(machine_ref(&zombie), hooks, WorkspaceInitial { phase: None, first_life: Some(false) });
    ws.rebuild().await.unwrap();
    assert_eq!(lines(&order), strs(&["resurrect", "restore:m2"]));
    assert_eq!(zombie.calls(), strs(&["kill"]));
    assert_eq!(ws.machine_id(), "m2");
    assert_eq!(ws.current_phase(), WorkspacePhase::Running);
    assert!(ws.is_first_life());
}

/// lifecycle.test.ts:258
#[tokio::test]
async fn a_napping_workspace_rebuilds_too_and_comes_back_running() {
    let m = FakeMachine::new("m1");
    let ws = Workspace::new(machine_ref(&m), WorkspaceHooks { resurrect: Some(resurrect_m2()), ..base(2) }, running());
    ws.nap().await.unwrap();
    ws.rebuild().await.unwrap();
    assert_eq!(ws.machine_id(), "m2");
    assert_eq!(ws.current_phase(), WorkspacePhase::Running);
    assert_eq!(m.calls(), strs(&["pause", "kill"]));
}

/// lifecycle.test.ts:270
#[tokio::test]
async fn rebuild_refuses_without_a_resurrect_hook_and_keeps_the_machine() {
    let m = FakeMachine::new("m1");
    let ws = Workspace::new(machine_ref(&m), base(2), running());
    let e = ws.rebuild().await.unwrap_err();
    assert!(e.to_string().contains("resurrect"), "{e}");
    assert_eq!(ws.machine_id(), "m1");
    assert_eq!(m.calls(), Vec::<String>::new());
}
