// SPDX-License-Identifier: AGPL-3.0-only
//! project_path, checkout_of, module_of and backend_for may fail on a roots write or a daemon sync, which have no catch.
mod common;
#[path = "support/daemon.rs"]
mod support;

use serde_json::{json, Value};
use support::{err, World};

fn one_cloud_entry() -> World {
    World::new(&json!({"entries": [
        {"record": {"id": "a", "kind": "cloud", "machineId": "m1", "phase": "running", "project": "p"}},
    ]}))
}

fn roots_logged(log: &[Value]) -> bool {
    log.iter().any(|e| e[0] == "roots")
}

#[test]
fn an_unknown_project_fails_the_roots_write_as_it_is() {
    // daemon.ts:102: projectHeld(...).path in the roots write, with no catch.
    common::run_local(async {
        let w = one_cloud_entry();
        *w.rt.project_path_failure.borrow_mut() = Some(err("no such project p"));
        let got = w.area.write_daemon_roots(&w.entry("a"), false).await;
        assert_eq!(got, Err(err("no such project p")));
        assert!(!roots_logged(&w.log.take()), "no roots file is written");
    });
}

#[test]
fn a_checkout_the_host_cannot_name_fails_the_roots_write_as_it_is() {
    // daemon.ts:102: checkoutOf(...) in the roots write, with no catch.
    common::run_local(async {
        let w = one_cloud_entry();
        *w.rt.checkout_failure.borrow_mut() = Some(err("no such project p"));
        let got = w.area.write_daemon_roots(&w.entry("a"), false).await;
        assert_eq!(got, Err(err("no such project p")));
        assert!(!roots_logged(&w.log.take()), "no roots file is written");
    });
}

#[test]
fn an_unwired_kind_fails_the_daemon_sync_before_the_daemon_is_asked() {
    // daemon.ts:195: moduleOf(kind) opens the sync's work, with no catch.
    common::run_local(async {
        let w = one_cloud_entry();
        *w.rt.module_of_failure.borrow_mut() = Some(err("no module is wired for the kind cloud"));
        let got = w.area.sync_daemon(&w.entry("a")).await;
        assert_eq!(got, Err(err("no module is wired for the kind cloud")));
        let log = w.log.take();
        assert!(!roots_logged(&log), "no roots file is written: {log:?}");
    });
}

#[test]
fn an_unwired_backend_fails_the_daemon_sync_before_the_version_is_read() {
    // daemon.ts:141 and 213: canDeployDaemon reads backendFor, with no catch.
    common::run_local(async {
        let w = one_cloud_entry();
        *w.rt.backend_for_failure.borrow_mut() =
            Some(err("no backend is wired for the kind cloud"));
        let got = w.area.sync_daemon(&w.entry("a")).await;
        assert_eq!(got, Err(err("no backend is wired for the kind cloud")));
        let log = w.log.take();
        assert!(
            !log.iter().any(|e| e[0] == "daemonVersion"),
            "the version is not asked: {log:?}"
        );
    });
}
