// SPDX-License-Identifier: AGPL-3.0-only
//! The pipe pool's pure rules the TS tests reach only through a daemon: the refusal count, its words, and the auth
//! frame and answer. Read off packages/runtime/src/pipe.ts at 65f6b89c7. Spec: spec/pipe_pool.md rules 2, 3, 7.
use wsp_core::pipe::pool::{auth_answer, auth_frame, wait_words};
use wsp_core::pipe::{token_refusals, token_refused_line, PIPE_BEHIND_AGAIN_MS, TOKEN_REFUSED_MAX_MS};

#[test]
fn token_refusals_doubles_after_the_host_s_own_write_up_to_30_min() {
    let mut r = token_refusals(PIPE_BEHIND_AGAIN_MS);
    assert_eq!(r.refused(0), (1, 60_000));
    // Each refusal right after the host's own write counts on and waits twice as long.
    r.wrote(10);
    assert_eq!(r.refused(20), (2, 120_000));
    r.wrote(30);
    assert_eq!(r.refused(40), (3, 240_000));
    // A refusal with no write since the last starts again at one.
    assert_eq!(r.refused(50), (1, 60_000));
    // A write longer ago than twice the longest wait does not count as the one this refusal follows.
    r.wrote(0);
    assert_eq!(r.refused(2 * TOKEN_REFUSED_MAX_MS as i64), (1, 60_000));
    r.wrote(0);
    assert_eq!(r.refused(2 * TOKEN_REFUSED_MAX_MS as i64 - 1), (2, 120_000));
    // The wait stops at 30 minutes.
    let mut long = token_refusals(PIPE_BEHIND_AGAIN_MS);
    let mut waits = Vec::new();
    for i in 0..7 {
        waits.push(long.refused(i * 10).1);
        long.wrote(i * 10 + 5);
    }
    assert_eq!(waits, vec![60_000, 120_000, 240_000, 480_000, 960_000, 1_800_000, 1_800_000]);
    // The pool passes its own first wait.
    assert_eq!(token_refusals(150).refused(0), (1, 150));
}

#[test]
fn token_refused_line_words() {
    assert_eq!(
        token_refused_line("ws-a", "a pipe was dialled with", 1, 60_000).as_deref(),
        Some("ws-a: its daemon refused the token a pipe was dialled with, so the host writes its own there again and dials in 60 s")
    );
    assert_eq!(
        token_refused_line("ws-a", "a pipe was dialled with", 2, 120_000).as_deref(),
        Some("ws-a: its daemon refused the host's token again right after the host wrote it, so something else writes that file too; the host writes its own there again and dials in 2 min, waiting twice as long each time up to 30 min")
    );
    assert_eq!(token_refused_line("ws-a", "a pipe was dialled with", 3, 240_000), None);
    assert_eq!(token_refused_line("ws-a", "a pipe was dialled with", 0, 60_000), None);
}

#[test]
fn wait_words_round_as_math_round_does() {
    assert_eq!(wait_words(500), "1 s");
    assert_eq!(wait_words(1_500), "2 s");
    assert_eq!(wait_words(90_000), "90 s");
    assert_eq!(wait_words(119_999), "120 s");
    assert_eq!(wait_words(120_000), "2 min");
    assert_eq!(wait_words(150_000), "3 min");
    assert_eq!(wait_words(1_800_000), "30 min");
}

#[test]
fn the_auth_answer_is_a_daemon_response_with_id_0() {
    assert_eq!(auth_answer(r#"{"id":0,"ok":true}"#), Some(true));
    assert_eq!(auth_answer(r#"{"id":0,"ok":true,"extra":1}"#), Some(true));
    assert_eq!(auth_answer(r#"{"id":0,"ok":false,"error":"unauthorized"}"#), Some(false));
    // An error answer with no words is no DaemonResponse at all.
    assert_eq!(auth_answer(r#"{"id":0,"ok":false}"#), None);
    assert_eq!(auth_answer(r#"{"id":1,"ok":true}"#), None);
    assert_eq!(auth_answer(r#"{"id":null,"ok":true}"#), None);
    assert_eq!(auth_answer(r#"{"type":"daemon.hello","root":"/root","version":157}"#), None);
    assert_eq!(auth_answer(r#"{"op":"pipe.open","door":"http","heldMs":0}"#), None);
    assert_eq!(auth_answer("not json"), None);
}

#[test]
fn the_auth_frame_is_the_ts_text() {
    assert_eq!(auth_frame("pipe-token"), r#"{"id":0,"op":"auth","token":"pipe-token","pipe":true}"#);
    assert_eq!(auth_frame("a\"b"), r#"{"id":0,"op":"auth","token":"a\"b","pipe":true}"#);
}
