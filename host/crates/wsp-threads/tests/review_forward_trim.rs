// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448), path 1: the held end's added words are cut with JavaScript's `trim`.
//! turns.ts:199 reads `text.slice(told.said.length).trim()`. `String.prototype.trim` strips U+FEFF and keeps U+0085
//! (node: `"﻿".trim() === ""`, `"\^E".trim().length === 1`); forward.rs:411 uses Rust's `str::trim`, which does the
//! opposite. `wsp_words::js::js_trim` is the one road for it.
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("turns_run/world.rs");

/// A turn that told its reply over finished work (running 0, so the line promised nothing more), and whose held end
/// then carries the same words and a byte order mark. TS adds nothing, finds the status unchanged and sends nothing.
#[test]
fn a_held_end_that_adds_only_a_byte_order_mark_tells_the_lead_nothing() {
    run_local(async {
        let rig = rig_with(FakeSession::new(SESS), |t| t.notify = Some(vec!["lead".into()]));
        rig.send(json!({ "type": "turn.tasks", "sessionId": SESS, "running": 0, "replied": { "status": "completed", "text": "Done." } }));
        assert_eq!(rig.host.hooks.called("notify_end").len(), 1, "the reply itself is told once");
        rig.send(json!({ "type": "turn.done", "sessionId": SESS, "held": true, "result": { "status": "completed", "text": "Done.\u{feff}" } }));
        assert_eq!(rig.host.hooks.called("notify_end").len(), 1, "a line already told, plus a BOM, adds no second line");
    });
}

/// The other way round: U+0085 is not whitespace to `trim`, so the added word is a line TS sends.
#[test]
fn a_held_end_that_adds_a_next_line_character_is_told_as_words() {
    run_local(async {
        let rig = rig_with(FakeSession::new(SESS), |t| t.notify = Some(vec!["lead".into()]));
        rig.send(json!({ "type": "turn.tasks", "sessionId": SESS, "running": 0, "replied": { "status": "completed", "text": "Done." } }));
        rig.send(json!({ "type": "turn.done", "sessionId": SESS, "held": true, "result": { "status": "completed", "text": "Done.\u{85}" } }));
        assert_eq!(rig.host.hooks.called("notify_end").len(), 2, "U+0085 is a word to JavaScript's trim");
    });
}

/// turns.ts:281 reads `o.reason.trim() !== ""`: a reason that is only a byte order mark is no reason, so the deny goes
/// with the plain denied line and no reason, as a blank reason does.
#[test]
fn a_denial_whose_reason_is_a_byte_order_mark_carries_no_reason() {
    run_local(async {
        let r = rig_with(FakeSession::with(SESS, |s| s.answers = true), |_| {});
        r.send(ask_event("ask_1"));
        let got = r.handle.answer("ask_1".into(), "deny".into(), Some("\u{feff}".into())).expect("an answer road").await;
        assert_eq!(got.ok(), Some(answered(SessionAnswerOutcome::Answered)));
        assert_eq!(r.session.calls(), vec![format!("answer(ask_1, deny, Denied, {}, \"\")", denied_line(None))]);
    });
}
