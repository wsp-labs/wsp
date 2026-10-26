// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `checkoutOf` throws through the git area where the record's project is not held (ctx.checkoutOf,
//! context.ts:349, account/channels.ts:114). The area must answer that same error and act on nothing after it.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/git.rs"]
mod git;
include!("workspaces_git/world.rs");

use base::plain;

// workspaces.ts:698 (bringBack): checkoutOf is read after spawnGuard, entryOf and copyBlocked, before any daemon ask.
#[test]
fn ctx_throws_checkout_of_passes_through_bring_back() {
    run(async {
        let w = world(json!({}));
        w.base.checkout_fail.replace(Some(plain("cart is not a project this host holds")));
        let got = area::bring_back(&w.ctx, "ws1", None, None, origin()).await;
        refusal(got, "cart is not a project this host holds", None);
        assert_eq!(w.base.names(), vec!["entry_of", "copy_blocked"]);
        assert!(w.base.asks().is_empty(), "no daemon op after the throw");
    });
}
