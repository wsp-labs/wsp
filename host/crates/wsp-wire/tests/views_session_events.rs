// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/places.test.ts at a3b870a6e.

use wsp_wire::views::session_events::{wsp_bin_in, GUEST_DAEMON_DIR, GUEST_WSP_BIN};

// places.test.ts:240 at a3b870a6e
#[test]
fn names_the_wsp_command_in_a_bundle_by_one_rule_which_a_fork_and_a_joined_computer_bo() {
    assert_eq!(wsp_bin_in(GUEST_DAEMON_DIR), GUEST_WSP_BIN);
    assert_eq!(wsp_bin_in("/home/maya/.wsp/daemon"), "/home/maya/.wsp/daemon/wsp/dist/bin.js");
}
