// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-machines at 2c4f6cb46: the clone road with `HOME` unset. Alone in its binary, since it clears `HOME`
//! for the process.
#![allow(clippy::await_holding_refcell_ref)]
mod machines_b_support;

use machines_b_support::kinds_ctx::*;
use machines_b_support::*;
use serde_json::json;
use wsp_wire::views::workspace::ProjectSource;

/// kinds.ts:124-141 reads `~` through node's `os.homedir()`, which falls back to the passwd entry when `HOME` is unset
/// (`env -u HOME node -e 'console.log(require("os").homedir())'` prints `/root` here), so `~/` names root's home and
/// the clone goes to `/root/etc`. The Rust `here_home()` (kinds/roads.rs:94) reads `HOME` alone, answers `""`, and
/// `~/` resolves to `/`, so a repo named `etc` is refused as taken (`/etc`, shortened against the empty home to `~/etc already holds files`)
/// before any clone. No local machine is
/// wired, so neither outcome touches the disk: the TS road fails at the machine instead.
#[test]
fn a_tilde_folder_with_home_unset_is_the_passwd_home_as_node_reads_it() {
    std::env::remove_var("HOME");
    run(async {
        let (_host, area) = kinds();
        let source: ProjectSource =
            serde_json::from_value(json!({ "kind": "github", "repo": "acme/etc" })).unwrap();
        let said = area.clone_here("acme/etc", &source, "~/").await;
        let message = said.err().map(|e| e.message).unwrap_or_default();
        assert!(
            !message.contains("already holds files"),
            "`~/` with HOME unset resolved to `/`, not the passwd home: {message}"
        );
    })
}
