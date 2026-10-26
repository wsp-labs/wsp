// SPDX-License-Identifier: AGPL-3.0-only
//! project_held and checkout_of may fail in the sub-spawn checks: the project read has no catch, the checkout maps to the branch-unread refusal.
mod machines_b_support;

use machines_b_support::spawn_ctx::*;
use machines_b_support::*;
use wsp_machines::machines::sub_spawn::*;
use wsp_words::exit::HostError;
use wsp_words::words::computer::branch_unread_refusal;

fn asked() -> SpawnAsk {
    SpawnAsk {
        name: NAME.to_string(),
        project: "pr_1".to_string(),
        on: "boat".to_string(),
        ..SpawnAsk::default()
    }
}

#[test]
fn an_unknown_project_refuses_the_spawn_as_it_is() {
    // sub-spawn.ts:166: projectHeld of the lead's project, with no catch.
    run(async {
        let f = lead_on_this_computer();
        *f.host.project_held_failure.borrow_mut() = Some(HostError::new("no such project pr_1"));
        let got = f
            .area
            .spawn_sub(asked(), Some(&lead_caller()))
            .await
            .map(|_| ());
        assert_eq!(got, Err(HostError::new("no such project pr_1")));
    });
}

#[test]
fn an_unreadable_lead_checkout_refuses_the_spawn_with_the_branch_unread_words() {
    // sub-spawn.ts:176: checkoutOf in the withDaemon callback; its .catch rethrows the branch-unread refusal.
    run(async {
        let f = lead_on_this_computer();
        *f.host.checkout_failure.borrow_mut() = Some(HostError::new("no such project pr_1"));
        let got = f
            .area
            .spawn_sub(asked(), Some(&lead_caller()))
            .await
            .map(|_| ());
        assert_eq!(
            got,
            Err(HostError::new(branch_unread_refusal(
                "web",
                "no such project pr_1"
            )))
        );
    });
}
