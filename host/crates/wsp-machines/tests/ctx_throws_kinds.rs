// SPDX-License-Identifier: AGPL-3.0-only
//! project_held may fail: the folder of a fork falls back to the project's path with no catch.
mod machines_b_support;

use serde_json::json;
use wsp_words::exit::HostError;

use machines_b_support::kinds_ctx::*;
use machines_b_support::*;

#[test]
fn an_unknown_project_fails_the_folder_of_a_fork_as_it_is() {
    // kinds.ts:557: the last answer of threadFolder is projectHeld(...).path, with no catch.
    run(async {
        let (host, area) = kinds();
        *host.project_held_failure.borrow_mut() = Some(HostError::new("no such project pr_1"));
        let fork = host.put_live(
            record_of("ws_fork", json!({ "project": "pr_1" }), &["place", "sub"]),
            FakeMachine::new("bx_f"),
        );
        let got = area.thread_folder(&fork.borrow(), None).await;
        assert_eq!(got, Err(HostError::new("no such project pr_1")));
    });
}
