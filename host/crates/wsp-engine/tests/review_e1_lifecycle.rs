// SPDX-License-Identifier: AGPL-3.0-only
//! Review of packet e1: lifecycle.rs against the TS.
mod common;

use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use wsp_engine::errors::Result;
use wsp_engine::lifecycle::*;

fn hooks(attempts: u32) -> WorkspaceHooks {
    WorkspaceHooks {
        golden_snapshot: "g".into(),
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

fn napping() -> WorkspaceInitial {
    WorkspaceInitial { phase: Some(WorkspacePhase::Napping), first_life: None }
}

/// lifecycle.ts:129-158: `wake` sets `waking` and its catch sets `napping` on any end, so a caller that races the wake
/// against a timer (the TS cannot cancel it) still finds the phase settled once the wake ends. In Rust the caller's
/// way to give up is to drop the future, and nothing settles the phase then: it reads waking for good.
#[tokio::test]
async fn a_wake_the_caller_stops_waiting_for_does_not_leave_the_phase_waking() {
    let machine = common::fake_machine::FakeMachine::new("m1");
    let mut h = hooks(1);
    h.wake_check = Some(Arc::new(|_m| futures::future::pending::<Result<Option<String>>>().boxed_local()));
    let ws = Workspace::new(machine, h, napping());
    let cell = ws.phase_cell();
    let gave_up = tokio::time::timeout(Duration::from_millis(20), ws.wake(false)).await.is_err();
    assert!(gave_up, "the check never answers, so the timer wins");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_ne!(*cell.lock().unwrap(), WorkspacePhase::Waking, "TS: napping once the abandoned wake ends");
    assert_ne!(ws.current_phase(), WorkspacePhase::Waking);
}

/// lifecycle.ts:140-142 with lifecycle.test.ts:62-80 (`minted` is 3 over three dials): the wake check dials through
/// `ws.daemonReach()`, so the route it mints is the one the workspace keeps and the first reach after the wake mints
/// nothing. A Rust hook is handed only the machine, so the best it can do is `machine.preview_url(7070)` itself, and the
/// workspace then mints a second route on its first reach. tests/lifecycle.rs:152 says so in a comment; spec/lifecycle.md
/// does not.
#[tokio::test]
async fn the_route_the_wake_check_dials_is_the_one_the_workspace_keeps() {
    let machine = common::fake_machine::FakeMachine::new("m1");
    let mut h = hooks(1);
    h.wake_check = Some(Arc::new(|m| {
        async move {
            m.preview_url(7070).await?;
            Ok(None)
        }
        .boxed_local()
    }));
    let ws = Workspace::new(machine.clone(), h, napping());
    ws.wake(false).await.unwrap();
    ws.daemon_reach().await.unwrap();
    assert_eq!(machine.count("preview_url:7070"), 1, "TS: one mint, the check's, kept by the workspace; calls: {:?}", machine.calls());
}
