// SPDX-License-Identifier: AGPL-3.0-only
//! kind_has_daemon may fail (status push has no catch); checkout_of's reading keeps the TS catch.
mod machines_b_support;

use machines_b_support::reach_ctx::*;
use machines_b_support::*;
use wsp_machines::machines::area_error::AreaError;
use wsp_words::exit::HostError;

#[test]
fn an_unwired_kind_fails_the_status_push_as_it_is() {
    // reach.ts:170: moduleOf(kind).hasDaemon inside reachOf, which pushStatus (reach.ts:376) calls with no catch.
    run(async {
        let host = reach_host();
        let entry = host.put_live(
            plain_record("ws_1", "web", "running", "bx_1"),
            FakeMachine::new("bx_1"),
        );
        let area = reach(&host);
        *knobs().kind_has_daemon_failure.borrow_mut() =
            Some(HostError::new("no module is wired for the kind cloud"));
        let got = area.push_status(&entry).await.map_err(AreaError::into_host);
        assert_eq!(
            got,
            Err(HostError::new("no module is wired for the kind cloud"))
        );
        assert!(host.events.borrow().is_empty(), "no status is pushed");
    });
}

#[test]
fn an_unreadable_checkout_keeps_the_last_fact_and_asks_for_no_git_read() {
    // reach.ts:393 and 412: checkoutOf sits in the withDaemon callback, and the catch keeps the held fact.
    run(async {
        let host = reach_host();
        let entry = host.put_live(
            plain_record("ws_1", "web", "running", "bx_1"),
            FakeMachine::new("bx_1"),
        );
        let area = reach(&host);
        *knobs().checkout_failure.borrow_mut() = Some(HostError::new("no such project pr_1"));
        let got = area.read_checkout(&entry, true).await;
        assert!(
            got.is_none(),
            "the reading keeps the fact it held, which is none"
        );
        let asked = host.calls_of("with_daemon");
        assert!(
            !asked
                .iter()
                .any(|args| args.iter().any(|a| a.contains("git.status"))),
            "no git.status frame is sent: {asked:?}"
        );
    });
}
