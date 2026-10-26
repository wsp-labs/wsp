// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448), path 2: a line's try that a restart left behind is read off the transcript.
//! threads.ts:986 awaits `ctx.openThread(workspaceId, taken.threadId)` inside `takenTurn`, and sessions.ts:263 awaits
//! `ctx.takenTurn(...)` with no catch, so a transcript that cannot be read rejects the start: nothing is started and the
//! caller (a lead's end line, retried under the same request id) hears why. `SessionsCtx::taken_turn` is
//! `Result<Option<..>, HostError>` (sessions.rs:124) and start.rs:189 passes the error up, but `ThreadsArea::taken_turn`
//! (turn_end.rs:286) reads `.await.ok()?`, so the only thing the join can hand to `SessionsCtx` is `None`, which
//! start.rs:189-191 reads as "this request was never taken" and goes on to start a second turn with the same line.
#[path = "support_b/mod.rs"]
mod support;

use std::future::Future;
use std::rc::Rc;

use serde_json::json;
use support::threads_ctx::{entry, FakeThreadsCtx};
use support::Lend;
use support::{local, row, view};
use wsp_core::types::internal::{Taken, TakenOutcome};
use wsp_threads::threads::threads::ThreadsArea;

fn host() -> (Rc<FakeThreadsCtx>, Rc<ThreadsArea>) {
    let ctx = FakeThreadsCtx::new();
    let (e, _) = entry(json!({ "id": "ws1", "kind": "local", "name": "the-folder" }));
    ctx.add_entry(e);
    let area = ctx.area();
    (ctx, area)
}

fn output_of<F: Future>(_: &F) -> &'static str {
    std::any::type_name::<F::Output>()
}

/// The area has to be able to say that the thread could not be read. Its answer is an `Option`, which has no place for
/// the rejection, so a failed read and a request nobody took look the same to the start road.
#[test]
fn a_transcript_that_cannot_be_read_is_an_error_and_not_a_request_nobody_took() {
    local(async {
        let (ctx, area) = host();
        ctx.sessions.with(|s| {
            s.insert("cs-1".into(), row(view(json!({ "id": "cs-1", "threadId": "t1", "status": "completed" })), "u2"));
        });
        let taken = Taken { session_id: "cs-1".into(), thread_id: "t1".into(), turn_id: "u1".into(), outcome: TakenOutcome::Started };
        let call = area.taken_turn("ws1", &taken);
        let answer = output_of(&call);
        assert!(answer.contains("Result"), "takenTurn rejects when openThread does; the port answers {answer}");
    });
}
