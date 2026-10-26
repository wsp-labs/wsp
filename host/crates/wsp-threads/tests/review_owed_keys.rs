// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448), path 2: the key order of what a line writes to the store and records. The TS builds
//! each line by spreading (threads.ts:680 `{ ...line, since, requestId }`, :888 `{ id, from, notify, text, ...as,
//! toPerson, ...images }`, :921 `{ id, from, notify, text, ...named, toPerson, ...sub }`, :606 `{ type, ...row,
//! threadId, notify }`), so keys land in the order the code writes them (node prints each order in the review). The
//! port serializes `Owed` and the wire rows through their structs, whose order is fixed (owed.rs:59-84).
#[path = "support_b/mod.rs"]
mod support;

use std::rc::Rc;

use serde_json::{json, Value};
use support::threads_ctx::{entry, FakeThreadsCtx};
use support::Lend;
use support::{local, row, settle, view};
use wsp_threads::threads::threads::{Owed, PersonRow, ThreadsArea};

const PARENT: &str = "parent0001";

fn host() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "phase": "running" }));
    ctx.add_entry(e);
    ctx.sessions.with(|s| {
        s.insert(
            "r-p".into(),
            row(view(json!({ "id": "r-p", "threadId": PARENT, "claudeSessionId": "cs-p", "status": "completed", "startedAt": 1 })), "u-p"),
        );
    });
    let area = ctx.area();
    (ctx, area)
}

fn keys(value: &Value) -> Vec<String> {
    value.as_object().expect("an object").keys().cloned().collect()
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn bare(id: &str) -> Owed {
    Owed {
        id: id.into(),
        from: "child0001".into(),
        notify: PARENT.into(),
        text: "t".into(),
        by: Some(
            serde_json::from_value(json!({ "kind": "thread", "threadId": "lead", "workspaceId": "ws1", "rootThreadId": "lead" }))
                .expect("a scope"),
        ),
        road: Some(serde_json::from_value(json!("here")).expect("an origin")),
        started_by: None,
        to_person: Some(PersonRow { workspace_id: "ws-c".into(), session_id: "cs-c".into(), turn_id: "u-c".into(), text: "told".into() }),
        since: Some(1),
        request_id: Some("req".into()),
        tries: Some(1),
        next: Some(2),
        images: None,
        awaits_result: None,
    }
}

/// A child's end line for a sub-machine is built as `{ id, from, notify, text, ...named, toPerson, awaitsResult }` and
/// gets `since` and `requestId` after (threads.ts:921, :680): `awaitsResult` sits before them, and `tries` and `next`
/// come last (:731-733). The port's `Owed` writes `awaitsResult` as its last field.
#[test]
fn a_line_awaiting_a_sub_threads_result_keeps_awaits_result_before_since_and_request_id() {
    let line = Owed { awaits_result: Some("ws-c".into()), ..bare("u-c:0:parent0001") };
    assert_eq!(
        keys(&serde_json::to_value(&line).expect("a line serializes")),
        words(&["id", "from", "notify", "text", "by", "road", "toPerson", "awaitsResult", "since", "requestId", "tries", "next"])
    );
}

/// An unread steer's line is built as `{ id, from, notify, text, ...as, toPerson, ...images }` with `as` holding
/// `startedBy`, `by`, `road` in that order (threads.ts:881-888, keepOnTurn :1000), so `startedBy` comes before `by` and
/// `images` before `since`.
#[test]
fn an_unread_steers_line_keeps_started_by_before_by_and_images_before_since() {
    let images =
        serde_json::from_value(json!({ "under": "req-s", "records": [{ "mediaType": "image/png", "bytes": 3 }] })).expect("images");
    let line = Owed {
        started_by: Some(serde_json::from_value(json!("person")).expect("an origin")),
        images: Some(images),
        ..bare("u-t:unread:st1")
    };
    assert_eq!(
        keys(&serde_json::to_value(&line).expect("a line serializes")),
        words(&["id", "from", "notify", "text", "startedBy", "by", "road", "toPerson", "images", "since", "requestId", "tries", "next"])
    );
}

/// The person's row for a fallen line: threads.ts:606 spreads the row (workspaceId, sessionId, turnId, text) before
/// `threadId` and `notify`.
#[test]
fn the_person_row_of_a_fallen_line_has_its_text_before_its_thread() {
    local(async {
        let (ctx, area) = host();
        ctx.sessions.with(|s| s.get("r-p").expect("row").borrow_mut().view.claude_session_id = None);
        let line = Owed {
            id: "L1".into(),
            from: "child0001".into(),
            notify: PARENT.into(),
            text: "t".into(),
            by: None,
            road: None,
            started_by: None,
            to_person: Some(PersonRow {
                workspace_id: "ws-c".into(),
                session_id: "cs-c".into(),
                turn_id: "u-c".into(),
                text: "told".into(),
            }),
            since: None,
            request_id: None,
            tries: None,
            next: None,
            images: None,
            awaits_result: None,
        };
        area.deliver(line, false);
        settle().await;
        let told = ctx.recorded_json().into_iter().find(|e| e["notify"] == "me").expect("the person is told");
        assert_eq!(keys(&told), words(&["type", "workspaceId", "sessionId", "turnId", "text", "threadId", "notify"]));
    });
}
