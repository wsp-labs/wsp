// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `statusNow` throws through the pull requests area (pull-requests.ts:179, 217). The throw reaches the
//! caller of the act that pushed the status.
#![allow(dead_code, unused_imports)]
mod common;
include!("pull_requests/world.rs");

// pull-requests.ts:179 (dropPullRequest): statusNow is the last thing the drop does, after persist.
#[test]
fn ctx_throws_status_now_passes_through_drop_pull_request() {
    run(async {
        let (w, area, e) = setup();
        keep(&e, json!({}));
        e.borrow_mut().pr = Some(seen_of(fact(json!({}), T0)));
        *w.status_error.borrow_mut() = Some("the status could not be pushed".into());
        let got = area.drop_pull_request(&e).await.expect_err("statusNow throws");
        assert_eq!(got.message(), "the status could not be pushed");
        assert_eq!(w.log.borrow().clone(), vec!["persist".to_string(), "status".to_string()]);
    });
}
