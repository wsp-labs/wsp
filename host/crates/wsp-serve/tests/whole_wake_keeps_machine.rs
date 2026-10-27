// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/wake-keeps-machine.test.ts at 65f6b89c7, case for case.
mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::*;
use serde_json::json;
use wsp_core::console::capture;
use wsp_engine::box_backend::plan::box_budgets;
use wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH;
use wsp_wire::wire::capabilities::PauseMode;
use wsp_words::workspace_state::gone_refusal;

/// A stub that declares what a box does: one wake attempt, a pause that keeps the disk and nothing in memory, and
/// forks off the seller's named snapshots. The guest holds a daemon token, so the wake dials the daemon rather than
/// settling for an exec, and the daemon's wait is shrunk so a silent one is read in a test's time.
fn boat() -> Arc<StubBackend> {
    let backend = StubBackend::new(None);
    let bare = backend.exec_impl_fn();
    backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
        if cmd.contains(DAEMON_TOKEN_PATH) {
            let answer = token_guest_answer(&cmd);
            Box::pin(async move { Ok(answer) })
        } else {
            bare(m, cmd, stdin)
        }
    }));
    backend.set_capabilities(|c| {
        c.live_clone_forks = false;
        c.pause_mode = Some(PauseMode::Disk);
    });
    backend.lifecycle().set_budgets(box_budgets());
    backend.lifecycle().set_daemon_answers_ms(300.0);
    backend
}

// "a wake on a box keeps the workspace's machine" > "a daemon that does not answer the wake fails it, and the workspace still names its own machine, which is neither replaced nor killed"
#[test]
fn a_daemon_that_does_not_answer_the_wake_fails_it_and_the_workspace_still_names_its_own_machine_which_is_neither_replaced_nor_killed() {
    drive(|| async {
        let backend = boat();
        let port = dropping_port().await;
        let _warn = capture();
        let fx = runtime(&backend).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("boat"), None).await.expect("a workspace");
        let m1 = backend.machine(0);
        m1.set_preview_url(&format!("ws://127.0.0.1:{}", port.port));
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("daemon on m1 did not answer within 300 ms"), "{}", refused.message);
        let record = j(&fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace"));
        assert_eq!(record["machineId"], "m1");
        assert_eq!(backend.machines().iter().map(|m| m.id().to_string()).collect::<Vec<_>>(), ["m1"]);
        assert!(!m1.killed.get());
        assert!(!(events.has("workspace.woken") || events.has("workspace.upgraded")));
        fx.rt.close().await.expect("a close");
    });
}

// "a machine the provider says vanished while paused settles the workspace gone: no fresh fork and the refusal says what was lost"
#[test]
fn a_machine_the_provider_says_vanished_while_paused_settles_the_workspace_gone_no_fresh_fork_and_the_refusal_says_what_was_lost() {
    drive(|| async {
        let backend = boat();
        let _warn = capture();
        let fx = runtime(&backend).with(|o| o.gone_confirm_ms = Some(0)).build();
        let ws = create_on(&fx.rt, CreateOn::golden("boat"), None).await.expect("a workspace");
        let m1 = backend.machine(0);
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        m1.killed.set(true);
        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.err().map(|e| e.message);
        let record = j(&fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace"));
        assert_eq!(record["phase"], "gone");
        assert_eq!(record["machineId"], "m1");
        assert_eq!(backend.machines().iter().map(|m| m.id().to_string()).collect::<Vec<_>>(), ["m1"]);
        let words = record["gone"].as_str().expect("words").to_string();
        // ^machine m1 is gone at the provider: the wake found it gone at \S+Z
        let at = words.strip_prefix("machine m1 is gone at the provider: the wake found it gone at ").expect(&words);
        let stamp = at.split_whitespace().next().unwrap_or_default();
        assert!(stamp.ends_with('Z'), "{words}");
        let refused = refused.expect("a refusal");
        assert_eq!(refused, gone_refusal(Some("boat"), "wake", Some(&words)));
        assert!(
            refused.starts_with("boat's machine is gone with its disk, so work that was not pushed is lost; rebuild it to wake, which brings back its home folder from the last saved nap"),
            "{refused}"
        );
        fx.rt.close().await.expect("a close");
    });
}

fn confirming_read_says_the_machine_is_paused(_backend: &Arc<StubBackend>) {}

fn confirming_read_fails(backend: &Arc<StubBackend>) {
    backend.set_get(Rc::new(|_| Box::pin(async { Err(fetch_failed()) })));
}

async fn a_resume_the_provider_answers_404_for(confirming: fn(&Arc<StubBackend>)) {
    let backend = boat();
    let _warn = capture();
    let fx = runtime(&backend).with(|o| o.gone_confirm_ms = Some(0)).build();
    let events = fx.events();
    let ws = create_on(&fx.rt, CreateOn::golden("boat"), None).await.expect("a workspace");
    let m1 = backend.machine(0);
    fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
    m1.set_resume(Rc::new(|| Box::pin(async { Err(missing("Box not found")) })));
    confirming(&backend);
    let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
    assert_eq!(refused.kind.as_ref().map(|k| k.as_str()), Some("missing"));
    let record = j(&fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace"));
    assert_matches(&record, json!({ "phase": "napping", "machineId": "m1" }));
    assert!(record.get("gone").is_none());
    assert_eq!(backend.machines().iter().map(|m| m.id().to_string()).collect::<Vec<_>>(), ["m1"]);
    assert!(!m1.killed.get());
    assert!(!(events.has("workspace.gone") || events.has("workspace.woken") || events.has("workspace.upgraded")));
    fx.rt.close().await.expect("a close");
}

// A gateway copy that never held the machine answers 404 while the provider still holds it: the resume's 404 alone
// settles nothing, and only confirming reads that say gone do.
// "a resume the provider answers 404 for, whose confirming reads say the machine is paused, leaves the workspace napping on its machine"
#[test]
fn a_resume_the_provider_answers_404_for_whose_confirming_reads_say_the_machine_is_paused_leaves_the_workspace_napping_on_its_machine() {
    drive(|| a_resume_the_provider_answers_404_for(confirming_read_says_the_machine_is_paused));
}

// "a resume the provider answers 404 for, whose confirming reads fail, leaves the workspace napping on its machine"
#[test]
fn a_resume_the_provider_answers_404_for_whose_confirming_reads_fail_leaves_the_workspace_napping_on_its_machine() {
    drive(|| a_resume_the_provider_answers_404_for(confirming_read_fails));
}
