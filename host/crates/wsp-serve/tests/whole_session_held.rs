// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/session-held.test.ts at 65f6b89c7, case for case. A thread is in the listing from the moment its
//! start holds its row, and every subscriber hears so then, before the harness has announced its session: a window draws a
//! thread the command line started without waiting on its launch.
#[macro_use]
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::*;
use serde_json::{json, Value};
use wsp_core::types::api::StartOptions;
use wsp_core::types::events::ANY_EVENT;

// "a held thread" > "is said to every subscriber with its row already listed, before the harness announces anything"
#[test]
fn is_said_to_every_subscriber_with_its_row_already_listed_before_the_harness_announces_anything() {
    drive(|| async {
        let release = Gate::new();
        let slow = Scripted::new().cwd("/root/app").when(When::Gate(release.clone()));
        let fx = runtime(&StubBackend::new(None)).adapter("claude", slow.factory()).build();
        let rt = fx.rt.clone();
        let ws = create_on(&rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let said: Rc<RefCell<Vec<Value>>> = Rc::default();
        let listed: Rc<RefCell<Vec<Value>>> = Rc::default();
        let (heard, rows, listing, w) = (said.clone(), listed.clone(), rt.clone(), ws.clone());
        let _ = rt.events().on(
            ANY_EVENT,
            Rc::new(move |e| {
                let e = serde_json::to_value(e).expect("an event");
                if e["type"] != "session.held" && e["type"] != "session.start" {
                    return Ok(());
                }
                if e["type"] == "session.held" {
                    let (rows, listing, w) = (rows.clone(), listing.clone(), w.clone());
                    tokio::task::spawn_local(async move {
                        let found = listing.sessions().list(Some(w), None).await.expect("a list");
                        rows.borrow_mut().extend(found.iter().map(|r| j(r)["threadId"].clone()));
                    });
                }
                heard.borrow_mut().push(e);
                Ok(())
            }),
        );
        let started = rt
            .sessions()
            .start(&ws, StartOptions { prompt: "fix it".into(), request_id: Some("req_1".into()), ..Default::default() }, None)
            .await
            .expect("a start");
        sleep_ms(20).await;
        let types = |said: &Rc<RefCell<Vec<Value>>>| {
            said.borrow().iter().map(|e| e["type"].as_str().unwrap_or_default().to_string()).collect::<Vec<_>>()
        };
        assert_eq!(types(&said), ["session.held"]);
        let held = said.borrow()[0].clone();
        assert_eq!(held["workspaceId"], json!(ws));
        // The send it answers, so the client that minted the id drops its own tile for it and no other.
        assert_eq!(held["requestId"], "req_1");
        assert!(listed.borrow().contains(&held["threadId"]), "{:?} lacks {}", listed.borrow(), held["threadId"]);
        release.open();
        started.finished().await;
        assert_eq!(types(&said), ["session.held", "session.start"]);
        rt.close().await.expect("a close");
    });
}
