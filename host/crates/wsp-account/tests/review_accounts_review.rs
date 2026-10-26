// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the review draft road: what a person ticks is what posts as them. Expected values are the TS's: the line
//! named, or printed by node.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/review.rs"]
mod review;

use std::rc::Rc;

use base::{person, project, rec_json, record, run, FakeBase};
use review::{draft_json, FakeReview, HEAD};
use serde_json::{json, Value};
use wsp_account::projects::workspaces::review as door;
use wsp_account::projects::workspaces::review::ReviewCtx;
use wsp_account::projects::workspaces::seam::Entry;
use wsp_core::types::api::ReviewDraftAsk;
use wsp_words::exit::HostError;

const REMOTE: &str = "https://github.com/dev/spoo.git";

struct H {
    fake: Rc<FakeReview>,
    ctx: Rc<dyn ReviewCtx>,
}

impl H {
    fn new() -> H {
        let fake = FakeReview::new();
        let ctx: Rc<dyn ReviewCtx> = fake.clone();
        fake.base.hold_project(project("pr_1", "spoo", "here", json!({ "remote": REMOTE, "path": "/home/me/spoo" })));
        H { fake, ctx }
    }
}

fn ok<T>(r: Result<T, HostError>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("expected success, got: {}", e.message),
    }
}

fn under_review(h: &H, over: Value) -> Entry {
    let mut v = json!({ "name": "Review #7 Rename the status word", "from": { "kind": "review", "repo": "dev/spoo", "number": 7,
        "url": "https://github.com/dev/spoo/pull/7", "title": "Rename the status word", "base": "main", "head": { "branch": "lab/review-me", "oid": HEAD } } });
    base::merge(&mut v, over);
    h.fake.base.hold(record("ws_rev", "local", "pr_1", v))
}

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-6: workspaces.ts:1108 `new Map((on ?? []).map(t => [t.id, t.on]))`: a Map built from pairs keeps the LAST value
// for an id named twice. review.rs:291 takes the first. The comment c1 starts ticked; the ask names it on, then off.
// The TS leaves it off. The Rust leaves it on, and a ticked comment is what review_post sends as the person.

#[test]
fn an_id_ticked_twice_takes_its_last_tick_as_a_map_does() {
    run(async {
        let h = H::new();
        let entry = under_review(&h, json!({ "review": draft_json(json!({})) }));
        let ask = ReviewDraftAsk {
            workspace_id: "ws_rev".to_owned(),
            on: Some(vec![("c1".to_owned(), true), ("c1".to_owned(), false)]),
            ..Default::default()
        };
        ok(door::review_draft(&h.ctx, ask, Some(person())).await);
        assert_eq!(
            rec_json(&entry)["review"]["comments"][0]["on"],
            json!(false),
            "c1 was named on then off: the TS Map keeps off, so the comment is not posted as the person"
        );
    })
}
