// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the phases half (workspaces.ts:289-645): a failure the engine throws reaches the caller as the same error,
//! with the kind it was born with. The TS rethrows `e` itself in the wake (workspaces.ts:401), the upgrade (:476), the
//! rebuild (:515) and the delete (:599 `throw e`), and exit.ts:37 reads the class off `e.kind`: a provider's 401 is
//! `kind "auth"`, which classes `auth` (exit 2); `classify` and `exitClassOf` printed by
//! oracle/zz-review-protocol.oracle.ts: `CLASS 401 auth auth`.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/phases.rs"]
mod phases_support;
include!("workspaces_phases/world.rs");

use wsp_engine::errors::{EngineError, ErrorKind, WspError};

/// The provider's 401 for a key it no longer takes, as `classify(401, ...)` builds it (errors.ts:19).
fn refused_key() -> EngineError {
    EngineError::Provider(WspError {
        kind: ErrorKind::Auth,
        status: 401,
        code: None,
        message: "the key is not valid".to_owned(),
        request_id: None,
        window: None,
        said: None,
    })
}

// R-PRJ-P1. A delete the provider refuses with an account error is that error in the TS (workspaces.ts:599), so the
// command exits 2. The Rust turns it into an error with no kind (phases.rs:191 `engine`), which exits 1.
#[test]
fn a_delete_the_provider_refuses_as_auth_keeps_its_kind() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.unforks.set(vec![Err(refused_key())]);
        let err = delete(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err.message, "the key is not valid");
        assert_eq!(err.kind, Some(Kind::Auth), "exit code {}", err.exit_code());
    });
}

// R-PRJ-P2. The same for an upgrade (workspaces.ts:476) and a rebuild (:515).
#[test]
fn an_upgrade_and_a_rebuild_the_provider_refuses_as_auth_keep_their_kind() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        h.f.upgrades.set(vec![Err(refused_key())]);
        let err = upgrade(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err.kind, Some(Kind::Auth), "upgrade: exit code {}", err.exit_code());

        h.f.rebuilds.set(vec![Err(refused_key())]);
        let err = rebuild(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err.kind, Some(Kind::Auth), "rebuild: exit code {}", err.exit_code());
    });
}

// R-PRJ-P3. A wake the provider refuses as auth is thrown as `e` (workspaces.ts:401).
#[test]
fn a_wake_the_provider_refuses_as_auth_keeps_its_kind() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({}));
        h.f.wakes.set(vec![WakePlan::Done(Err(refused_key()))]);
        let err = wake(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err.message, "the key is not valid");
        assert_eq!(err.kind, Some(Kind::Auth), "exit code {}", err.exit_code());
    });
}
