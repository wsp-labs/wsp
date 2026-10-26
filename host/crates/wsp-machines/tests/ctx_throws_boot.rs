// SPDX-License-Identifier: AGPL-3.0-only
//! backend_for, module_of and talks_to_its_tree may fail; the TS catches the first two, not the third.
mod common;
#[path = "support/boot.rs"]
mod support;

use serde_json::json;
use support::{held_for_test, thread_caller, ws, Opts, Sight, World};
use wsp_core::types::internal::TreeTalk;
use wsp_machines::machines::boot::entries::RowAt;
use wsp_words::exit::HostError;
use wsp_words::words::thread::GoneSeenBy;

fn load_world(o: Opts) -> World {
    World::load(Opts {
        sight_word: Some("sightMachine"),
        sight_first: true,
        ..o
    })
}

#[test]
fn a_record_whose_backend_is_unwired_is_not_reread_and_stays_held() {
    // boot.ts:207: backendFor sits in a .then with a .catch that answers undefined, so nothing is marked gone.
    common::run_local(async {
        let w = load_world(Opts {
            sights: vec![Sight::Running],
            ..Opts::default()
        });
        held_for_test(&w, "503 busy").await;
        *w.rt.backend_for_failure.borrow_mut() =
            Some(HostError::new("no backend is wired for the kind cloud"));
        w.log.clear();
        let got = w.area.reread_held("a", GoneSeenBy::Sweep).await;
        assert_eq!(got, Ok(()));
        let log = w.log.take();
        assert!(
            !log.iter().any(|e| e[0] == "markGone"),
            "nothing is marked gone: {log:?}"
        );
        assert!(w.area.is_held_away("a"), "the record stays held away");
        assert!(
            !w.rt.rereading.borrow().contains("a"),
            "the reread lets go of the record"
        );
    });
}

#[test]
fn a_record_whose_kind_has_no_module_is_left_as_it_was_with_the_reason() {
    // boot.ts:243: moduleOf(kind) sits in a try whose catch warns "left as it was".
    common::run_local(async {
        let w = World::load(Opts::default());
        *w.rt.module_of_failure.borrow_mut() =
            Some(HostError::new("no module is wired for the kind cloud"));
        w.log.clear();
        let got = w
            .area
            .hydrate_workspace(ws("a", json!({})), None, None, None)
            .await;
        assert_eq!(got.map(|e| e.is_none()), Ok(true), "no entry is made");
        let log = w.log.take();
        assert!(
            log.contains(&json!([
                "warn",
                "workspace a is left as it was: no module is wired for the kind cloud"
            ])),
            "the record is left with its reason: {log:?}"
        );
    });
}

#[test]
fn a_talk_by_a_caller_whose_project_is_unknown_fails_the_row_it_reaches() {
    // rules.ts:164 and 110: boxOf reads projectHeld of the caller's workspace; boot.ts:659 has no catch.
    common::run_local(async {
        let w = World::rows(Opts {
            live: json!({"a": {}}),
            records: json!({}),
            ..Opts::default()
        });
        w.started().await;
        let thread = thread_caller();
        *w.rt.talks_to_tree_failure.borrow_mut() = Some(HostError::new("no such project p_1"));
        let row = RowAt {
            thread_id: Some("t-a-own".to_string()),
            workspace_id: "a".to_string(),
        };
        let got = w
            .area
            .entry_of_row(&row, Some(&thread), Some(TreeTalk::List), false)
            .await
            .map(|e| e.is_some());
        assert_eq!(got, Err(HostError::new("no such project p_1")));
    });
}
