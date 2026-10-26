// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448): the blank checks read JavaScript's `trim`. sessions.ts:1077, :1191 and :1212 call
//! `String.prototype.trim`, which strips U+FEFF and keeps U+0085; Rust's `str::trim` does the opposite
//! (node: `"﻿".trim() === ""`, `"\^E".trim().length === 1`). wsp-words has `js::js_trim` for this.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("sessions_acts/world.rs");

#[test]
fn a_rename_to_a_byte_order_mark_alone_is_refused_as_blank() {
    run_local(async {
        let (host, area) = renaming_host();
        let _wrote = writes(&host, SessionRenameWrite::Written);
        let err = area.rename_row("no-such-session", "\u{FEFF}", None).await.expect_err("sessions.ts:1077-1078 refuses a blank name");
        assert_eq!(said(&err), (EMPTY_TITLE_LINE.to_string(), None, None));
    });
}

#[test]
fn a_rename_keeps_a_trailing_next_line_as_javascript_does() {
    run_local(async {
        let (host, area) = renaming_host();
        let _wrote = writes(&host, SessionRenameWrite::Written);
        let err = area.rename_row("no-such-session", "\u{0085}", None).await;
        assert!(err.is_ok(), "U+0085 is not blank to JavaScript's trim, so the rename goes on to its lookup");
    });
}
