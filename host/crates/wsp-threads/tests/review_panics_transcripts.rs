// SPDX-License-Identifier: AGPL-3.0-only
//! Review: panics a stored transcript index or a stored row can reach in the transcripts area. Each test is red now.
#![allow(dead_code, unused_imports)]
#[path = "support_b/mod.rs"]
mod support;

use serde_json::{json, Value};
use support::transcripts_ctx::{FakeTranscriptsCtx, WithState};
use support::{event, local};
use wsp_core::types::internal::{index_bytes, TranscriptIndex};

const WS: &str = "ws1";

fn start() -> Value {
    json!({ "type": "session.start", "workspaceId": WS, "sessionId": "s1", "turnId": "u1", "threadId": "t1", "prompt": "go" })
}

/// A transcript index document on disk whose `pos` is the largest u64 (`index_read` takes `pos` as a u64,
/// internal_index.rs:684). transcripts.ts:677 adds one to a float.
fn index_at_the_top() -> String {
    let kept = TranscriptIndex { pos: u64::MAX, ..TranscriptIndex::default() };
    String::from_utf8(index_bytes(&kept, None, false)).expect("utf-8")
}

#[test]
fn record_survives_a_stored_index_whose_pos_is_u64_max() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.rows.indexes.borrow_mut().insert(WS.into(), index_at_the_top());
        let area = ctx.area();
        area.load_index(WS);
        assert_eq!(ctx.index_of(WS).expect("the kept index is held").pos, u64::MAX);
        area.record(event(start()));
    });
}

#[test]
fn record_copies_survives_a_stored_index_whose_pos_is_u64_max() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.rows.indexes.borrow_mut().insert(WS.into(), index_at_the_top());
        let area = ctx.area();
        area.load_index(WS);
        let mut copy = start();
        copy["copiedFrom"] = json!("t0");
        area.record_copies(&[event(copy)]);
    });
}

/// A stored index whose words of a thread whose turn runs say `size` is the largest usize (internal_index.rs:694
/// takes any u64). `ranked_shares` gives a running thread all of its own size and adds it to what the threads before
/// it hold (rows.rs:685); the TS adds floats (transcripts.ts:466).
#[test]
fn load_index_survives_a_stored_words_size_at_the_top_of_usize_for_a_running_thread() {
    use wsp_core::types::internal::SessionEntry;
    use wsp_core::types::internal::ThreadWords;
    use wsp_wire::types::js_number::JsNumber;
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let words = |size: usize, last: f64| ThreadWords {
            lines: vec!["a line".to_string()],
            open: None,
            last: JsNumber(last),
            size,
            first: Some("a line".to_string()),
            opened: None,
            cold: None,
            room: None,
        };
        let mut kept = TranscriptIndex::default();
        kept.words.insert("quiet".into(), words(10, 2000.0));
        kept.words.insert("run".into(), words(usize::MAX, 1000.0));
        ctx.rows.indexes.borrow_mut().insert(WS.into(), String::from_utf8(index_bytes(&kept, None, true)).expect("utf-8"));
        let running = support::row(support::view(json!({ "id": "r1", "threadId": "run", "status": "running" })), "u1");
        ctx.sessions.borrow_mut().insert("r1".to_string(), running);
        let area = ctx.area();
        area.load_index(WS);
    });
}

/// `on_kept` (transcripts/kept.rs:207) is `tokio::task::spawn_local`: outside a `LocalSet` it panics.
#[test]
fn keep_sent_images_runs_on_a_runtime_with_no_local_set() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
    rt.block_on(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let png = wsp_wire::types::attachments::Attachment { media_type: "image/png".into(), bytes: "AAAA".into(), name: None };
        drop(area.keep_sent_images(WS, "t1", Some("req_a"), &[png]));
    });
}
