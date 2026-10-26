// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `refusalFor` throws through the review area's origin refusal, where a record's project is not held
//! (workspaces.ts:1206-1209). The throw is the answer the caller gets.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/review.rs"]
mod review;

use std::rc::Rc;

use base::{person, plain, project, record, run};
use review::FakeReview;
use serde_json::json;
use wsp_account::projects::workspaces::review as door;
use wsp_account::projects::workspaces::review::ReviewCtx;

// workspaces.ts:1206-1209 (originRefusal): `ctx.refusalFor(live.get(id)?.record, origin)` is the answer, throw and all.
#[test]
fn ctx_throws_refusal_for_passes_through_origin_refusal() {
    run(async {
        let fake = FakeReview::new();
        let ctx: Rc<dyn ReviewCtx> = fake.clone();
        fake.base.hold_project(project(
            "pr_1",
            "spoo",
            "here",
            json!({ "remote": "https://github.com/dev/spoo.git", "path": "/home/me/spoo" }),
        ));
        fake.base.hold(record("ws_1", "local", "pr_1", json!({})));
        *fake.refusal_fail.borrow_mut() = Some(plain("the record's project is not held"));
        let got = door::origin_refusal(&ctx, "ws_1", Some(person())).await.expect_err("refusalFor throws");
        assert_eq!(got.message, "the record's project is not held");
        assert_eq!(fake.base.names(), vec!["ready", "refusal_for"]);
    });
}
