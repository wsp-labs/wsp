// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `backendFor` throws through the phases area's resize, where the record's kind has no backend
//! (workspaces.ts:419). The throw is the answer, and no size is offered or napped after it.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/phases.rs"]
mod phases_support;
include!("workspaces_phases/world.rs");

// workspaces.ts:419 (resize): backendFor is read before the size is looked up; its throw ends the resize.
#[test]
fn ctx_throws_backend_capabilities_passes_through_resize() {
    run(async {
        let h = H::new();
        h.cloud("ws_a", json!({ "phase": "running" }));
        *h.f.caps_fail.borrow_mut() = Some(plain("no backend answers for this kind"));
        let got = resize(&h.ctx, "ws_a", size(4.0, 8192.0), None).await.expect_err("backendFor throws");
        assert_eq!(got.message, "no backend answers for this kind");
        assert_eq!(h.calls(&["backend_capabilities"]), vec!["backend_capabilities".to_string()]);
        assert!(h.args("nap_with").is_empty(), "nothing napped after the throw");
        assert!(h.persisted().is_empty() && h.events().is_empty(), "nothing written after the throw");
    });
}
