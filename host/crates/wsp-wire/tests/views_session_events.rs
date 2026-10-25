// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/places.test.ts at a3b870a6e.

use serde_json::Value;
use wsp_wire::types::check::parse;
use wsp_wire::views::session_events::{wsp_bin_in, GUEST_DAEMON_DIR, GUEST_WSP_BIN};
use wsp_wire::views::session_events_more::SessionEvent;

// places.test.ts:240 at a3b870a6e
#[test]
fn names_the_wsp_command_in_a_bundle_by_one_rule_which_a_fork_and_a_joined_computer_bo() {
    assert_eq!(wsp_bin_in(GUEST_DAEMON_DIR), GUEST_WSP_BIN);
    assert_eq!(wsp_bin_in("/home/maya/.wsp/daemon"), "/home/maya/.wsp/daemon/wsp/dist/bin.js");
}

// transcript-reader.ts headShape: `{ ...e, text, cut }` appends `cut` after every key the stored row held, so a head writes it
// last; the schema lists it after `line`. The bytes of the event are the same either way.
#[test]
fn a_stored_event_s_head_writes_cut_last() {
    let text = r#"{"type":"session.delta","workspaceId":"w","sessionId":"s","kind":"tool_result","text":"abc","toolUseId":"t","line":2,"cut":9000,"bytes":9000,"exitCode":0}"#;
    let input: Value = serde_json::from_str(text).unwrap();
    let event = parse::<SessionEvent>(&input).unwrap();
    let written = serde_json::to_string(&event).unwrap();
    let want = r#"{"type":"session.delta","workspaceId":"w","sessionId":"s","kind":"tool_result","text":"abc","toolUseId":"t","line":2,"bytes":9000,"exitCode":0,"cut":9000}"#;
    assert_eq!(written, want);
    assert_eq!(written.len(), text.len());
}
