// SPDX-License-Identifier: AGPL-3.0-only
//! project_held, module_of and daemon_answers_ms may fail; the preview route's lifecycle read is caught by the TS, the rest are not.
mod common;
#[path = "support/create.rs"]
mod support;

use serde_json::json;
use support::{err, World};

#[test]
fn an_unknown_project_ends_the_create_before_any_fork() {
    // create.ts:41: projectHeld in road.workspaceBinds, outside any try.
    common::run_local(async {
        let fixture = common::fixture("create");
        let sc = common::scene(&fixture, "a provider fork lands and is ready");
        let w = World::new(&sc["scene"]);
        *w.rt.project_held_failure.borrow_mut() = Some(err("no such project p_1"));
        let got = w.create().await.map(|_| ());
        assert_eq!(got, Err(err("no such project p_1")));
        let log = w.log.take();
        assert!(
            !log.iter().any(|e| e[0] == json!("fork")),
            "no machine is forked: {log:?}"
        );
    });
}

#[test]
fn a_preview_route_with_no_lifecycle_is_its_own_notice_and_the_create_goes_on() {
    // create.ts:169: the lifecycle read is inside the try that reports the preview route as not minted.
    common::run_local(async {
        let fixture = common::fixture("create");
        let sc = common::scene(
            &fixture,
            "a route and a daemon: both asked, the sync started",
        );
        let w = World::new(&sc["scene"]);
        *w.rt.daemon_answers_failure.borrow_mut() =
            Some(err("cloud machines declare no lifecycle"));
        let created = w.create().await;
        assert!(created.is_ok(), "the create lands");
        let log = w.log.take();
        assert!(
            log.contains(&json!([
                "report",
                "preview-route",
                "No preview route to the daemon.",
                { "notice": "preview route for m-new not minted (cloud machines declare no lifecycle)" }
            ])),
            "the route is reported as not minted: {log:?}"
        );
        assert!(
            log.contains(&json!(["pingDaemon", "ws_1"])),
            "the daemon is still asked: {log:?}"
        );
    });
}

#[test]
fn an_unwired_kind_ends_the_create_before_the_record_is_persisted() {
    // create.ts:193: moduleOf(kind).landProject, outside any try.
    common::run_local(async {
        let fixture = common::fixture("create");
        let sc = common::scene(&fixture, "a provider fork lands and is ready");
        let w = World::new(&sc["scene"]);
        *w.rt.module_of_failure.borrow_mut() = Some(err("no module is wired for the kind cloud"));
        let got = w.create().await.map(|_| ());
        assert_eq!(got, Err(err("no module is wired for the kind cloud")));
        let log = w.log.take();
        assert!(
            !log.iter().any(|e| e[0] == json!("persist")),
            "the record is never persisted: {log:?}"
        );
    });
}
