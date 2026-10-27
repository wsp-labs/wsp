// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/serve-replay.test.ts at 65f6b89c7, case for case. Event replay over the socket: a client that
//! lost its socket resubscribes with the last sequence it saw and gets what it missed, in order, once. The runtime runs
//! on a fake clock and the waits below are driven by the socket's own frames, so nothing here reads the wall clock.
#[macro_use]
mod common;

use std::rc::Rc;

use common::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_serve::serve::{serve_runtime, ServeOptions};

const TOKEN: &str = "secret";

async fn serve(rt: &Fx, port: Option<u16>, fc: &FakeClock) -> Rc<dyn wsp_serve::serve::RuntimeServer> {
    let clock = fc.clone();
    serve_runtime(
        rt.rt.clone(),
        ServeOptions {
            port: Some(port.unwrap_or(0)),
            auth_token: TOKEN.into(),
            now: Some(Rc::new(move || clock.now())),
            ..Default::default()
        },
    )
    .await
    .expect("the runtime serves")
}

struct Boot {
    h: Rc<Driven>,
    fx: Fx,
    workspace_id: String,
    c: Rc<WsClient>,
    port: u16,
    srv: Rc<dyn wsp_serve::serve::RuntimeServer>,
}

/// A runtime with one project, one workspace of it and one running session: project.added is seq 1, the create's stages
/// are seq 2 to 5 (the project's clone is one of them), workspace.created is seq 6, the cost tick the create lands (the
/// meter's first point, at the machine's rate) is seq 7, session.held is seq 8, session.start is seq 9, the thread.head
/// it pushes is seq 10, and the preferences record moving to name that workspace as the last target is seq 11.
async fn boot() -> Boot {
    let h = Driven::replay();
    let fc = FakeClock::at_now();
    let fx = runtime(&StubBackend::new(None)).adapter("claude", h.factory()).clock(&fc).build();
    let srv = serve(&fx, None, &fc).await;
    let port = srv.port();
    let c = WsClient::connect(port, TOKEN).await;
    let created = create_over_wire(&c, "x", json!({ "golden": "snap_g" })).await;
    let workspace_id = created["workspace"]["id"].as_str().expect("a workspace").to_string();
    c.request("sessions.start", json!({ "workspaceId": workspace_id, "prompt": "go" })).await;
    Boot { h, fx, workspace_id, c, port, srv }
}

fn seqs(events: &[Value]) -> Vec<i64> {
    events.iter().map(|e| e["seq"].as_i64().expect("a seq")).collect()
}

fn texts(events: &[Value]) -> Vec<String> {
    events.iter().map(|e| e["text"].as_str().expect("a text").to_string()).collect()
}

/// `expect(sub).toEqual({ id: sub.id, ok: true, seq, [gap: true,] stream: expect.any(String) })`: no other key.
fn assert_sub(sub: &Value, seq: i64, gap: bool) {
    let mut keys: Vec<&str> = sub.as_object().expect("a reply").keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut want = vec!["id", "ok", "seq", "stream"];
    if gap {
        want.push("gap");
    }
    want.sort_unstable();
    assert_eq!(keys, want, "{sub}");
    assert_eq!(sub["ok"], json!(true));
    assert_eq!(sub["seq"], json!(seq), "{sub}");
    assert!(sub["stream"].is_string(), "{sub}");
    if gap {
        assert_eq!(sub["gap"], json!(true));
    }
}

// "events.subscribe replay" > "a fresh subscribe without a cursor behaves as today: nothing replayed, live events flow, each stamped once per runtime"
#[test]
fn a_fresh_subscribe_without_a_cursor_behaves_as_today_nothing_replayed_live_events_flow_each_stamped_once_per_runtime() {
    drive(|| async {
        let b = boot().await;
        b.h.last().delta("before");
        let sub = b.c.request("events.subscribe", json!({})).await;
        assert_sub(&sub, 12, false);

        let other = WsClient::connect(b.port, TOKEN).await;
        assert_eq!(other.request("events.subscribe", json!({})).await["seq"], 12);

        b.h.last().delta("live");
        let mine = b.c.received(1).await;
        let theirs = other.received(1).await;
        assert_eq!(mine.len(), 1);
        assert_matches(&mine[0], json!({ "type": "session.delta", "text": "live", "seq": 13 }));
        assert_eq!(theirs, mine);
        b.c.close().await;
        other.close().await;
        b.srv.close().await.expect("a close");
    });
}

// "events.subscribe replay" > "events emitted between a close and a re-subscribe arrive once and in order, then live continues"
#[test]
fn events_emitted_between_a_close_and_a_re_subscribe_arrive_once_and_in_order_then_live_continues() {
    drive(|| async {
        let b = boot().await;
        b.c.request("events.subscribe", json!({})).await;
        b.h.last().delta("a");
        b.h.last().delta("b");
        let seen = b.c.received(2).await;
        assert_eq!(seqs(&seen), [12, 13]);
        let cursor = seen.last().expect("an event")["seq"].as_i64().expect("a seq");
        b.c.close().await;
        b.c.closed().await;

        b.h.last().delta("dark 1");
        b.h.last().delta("dark 2");
        b.h.last().delta("dark 3");

        let back = WsClient::connect(b.port, TOKEN).await;
        let sub = back.request("events.subscribe", json!({ "after": cursor })).await;
        assert_sub(&sub, 16, false);
        b.h.last().delta("live");
        let got = back.received(4).await;
        assert_eq!(texts(&got), ["dark 1", "dark 2", "dark 3", "live"]);
        assert_eq!(seqs(&got), [14, 15, 16, 17]);
        back.close().await;
        b.srv.close().await.expect("a close");
    });
}

// "events.subscribe replay" > "an in-ring cursor replays exactly the events after it, whether or not that socket ever saw them"
#[test]
fn an_in_ring_cursor_replays_exactly_the_events_after_it_whether_or_not_that_socket_ever_saw_them() {
    drive(|| async {
        let b = boot().await;
        for t in ["1", "2", "3", "4", "5", "6"] {
            b.h.last().delta(t);
        }
        let late = WsClient::connect(b.port, TOKEN).await;
        let sub = late.request("events.subscribe", json!({ "after": 14 })).await;
        assert_sub(&sub, 17, false);
        b.h.last().delta("live");
        let got = late.received(4).await;
        assert_eq!(seqs(&got), [15, 16, 17, 18]);
        assert_eq!(texts(&got), ["4", "5", "6", "live"]);

        let caught_up = WsClient::connect(b.port, TOKEN).await;
        let sub = caught_up.request("events.subscribe", json!({ "after": 18 })).await;
        assert_sub(&sub, 18, false);
        assert!(sub["id"].is_number());
        b.h.last().delta("only this");
        assert_eq!(texts(&caught_up.received(1).await), ["only this"]);
        late.close().await;
        caught_up.close().await;
        b.srv.close().await.expect("a close");
    });
}

// "events.subscribe replay" > "a cursor older than the ring gets gap and no replay; sessions.history is the refetch and holds the turn"
#[test]
fn a_cursor_older_than_the_ring_gets_gap_and_no_replay_sessions_history_is_the_refetch_and_holds_the_turn() {
    drive(|| async {
        let b = boot().await;
        // 5000 deltas after project.added (1), the create's stages and workspace.created (2 to 6), the create's cost
        // tick (7), session.held (8), session.start (9), its thread.head (10) and the record's target (11): the ring keeps
        // 12..5011 and drops the first eleven.
        for i in 1..=5000 {
            b.h.last().delta(&format!("d{i}"));
        }

        let stale = WsClient::connect(b.port, TOKEN).await;
        let sub = stale.request("events.subscribe", json!({ "after": 1 })).await;
        assert_sub(&sub, 5011, true);
        b.h.last().delta("live");
        let got = stale.received(1).await;
        assert_eq!(got.len(), 1);
        assert_matches(&got[0], json!({ "text": "live", "seq": 5012 }));

        // The transcript is capped at the same 5000, so it too starts past the first events; its tail is the truth the chat needs.
        let history = stale.request("sessions.history", json!({ "workspaceId": b.workspace_id })).await;
        let history = history["events"].as_array().expect("events").clone();
        assert_eq!(history.len(), 5000);
        assert_matches(history.last().expect("an event"), json!({ "type": "session.delta", "text": "live" }));
        stale.close().await;

        // "live" made 5012 the head, so the ring now holds 13..5012: cursor 12 is the oldest that replays, 11 is a gap.
        let edge = WsClient::connect(b.port, TOKEN).await;
        assert!(edge.request("events.subscribe", json!({ "after": 12 })).await.get("gap").is_none());
        let all = edge.received(5000).await;
        assert_eq!(seqs(&all)[0], 13);
        assert_eq!(seqs(&all)[4999], 5012);
        edge.close().await;
        let out = WsClient::connect(b.port, TOKEN).await;
        assert_matches(&out.request("events.subscribe", json!({ "after": 11 })).await, json!({ "ok": true, "seq": 5012, "gap": true }));
        out.close().await;
        b.srv.close().await.expect("a close");
    });
}

// "events.subscribe replay" > "a cursor past head is a gap too: that is what a cursor from a runtime that restarted looks like"
#[test]
fn a_cursor_past_head_is_a_gap_too_that_is_what_a_cursor_from_a_runtime_that_restarted_looks_like() {
    drive(|| async {
        let b = boot().await;
        let c = WsClient::connect(b.port, TOKEN).await;
        let sub = c.request("events.subscribe", json!({ "after": 90 })).await;
        assert_sub(&sub, 11, true);
        b.h.last().delta("live");
        assert_eq!(seqs(&c.received(1).await), [12]);
        c.close().await;
        b.srv.close().await.expect("a close");
    });
}

// "events.subscribe replay" > "runtime restarts between the drop and the re-subscribe: a cursor from the old stream is a gap, nothing of the new stream is replayed"
#[test]
fn runtime_restarts_between_the_drop_and_the_re_subscribe_a_cursor_from_the_old_stream_is_a_gap_nothing_of_the_new_stream_is_replayed() {
    drive(|| async {
        let state = sqlite_state();
        let first = Driven::replay();
        let fc = FakeClock::at_now();
        let rt_a = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", first.factory()).clock(&fc).build();
        let srv = serve(&rt_a, None, &fc).await;
        let port = srv.port();
        let c = WsClient::connect(port, TOKEN).await;
        let created = create_over_wire(&c, "x", json!({ "golden": "snap_g" })).await;
        let workspace_id = created["workspace"]["id"].as_str().expect("a workspace").to_string();
        c.request("sessions.start", json!({ "workspaceId": workspace_id, "prompt": "go" })).await;
        let sub_a = c.request("events.subscribe", json!({})).await;
        let stream_a = sub_a["stream"].as_str().expect("a stream").to_string();
        first.last().delta("old 1");
        let seen = c.received(1).await;
        let cursor = seen[0]["seq"].as_i64().expect("a seq");
        assert!(sub_a["stream"].is_string());
        assert_eq!(cursor, 12);
        srv.close().await.expect("a close");
        c.closed().await;

        // The new process over the same store issues sequences from 1 again (among its first are the end it writes for the
        // turn the restart cut and that thread's head) and soon passes the old cursor.
        let second = Driven::replay();
        let rt_b = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", second.factory()).clock(&fc).build();
        // The new backend never heard of the first machine: the host confirms it gone once it serves, and the gone
        // event, its row and its cost tick take three sequences before anything else.
        let rt = rt_b.rt.clone();
        let ws = workspace_id.clone();
        wait!([rt, ws] j(&rt.workspaces().get(&ws, None, None).await.expect("a workspace"))["phase"] == "gone");
        let srv = serve(&rt_b, Some(port), &fc).await;
        let back = WsClient::connect(port, TOKEN).await;
        let again = create_over_wire(&back, "y", json!({ "golden": "snap_g" })).await;
        back.request("sessions.start", json!({ "workspaceId": again["workspace"]["id"], "prompt": "go" })).await;
        for t in ["new 1", "new 2", "new 3"] {
            second.last().delta(t);
        }

        let sub_b = back.request("events.subscribe", json!({ "after": cursor, "stream": stream_a })).await;
        assert_sub(&sub_b, 19, true);
        assert_ne!(sub_b["stream"], json!(stream_a));
        second.last().delta("live");
        let got = back.received(1).await;
        assert_eq!(texts(&got), ["live"]);
        assert_eq!(seqs(&got), [20]);
        // Without the stream the same cursor would have replayed 13 to 20 of a stream this client never saw.
        let naive = WsClient::connect(port, TOKEN).await;
        let sub = naive.request("events.subscribe", json!({ "after": cursor })).await;
        assert_sub(&sub, 20, false);
        assert!(sub["id"].is_number());
        assert_eq!(sub["stream"], sub_b["stream"]);
        let replayed = naive.received(8).await;
        assert_eq!(texts(&replayed[4..]), ["new 1", "new 2", "new 3", "live"]);
        back.close().await;
        naive.close().await;
        srv.close().await.expect("a close");
    });
}

// "events.subscribe replay" > "the transcript on disk never carries a sequence: it belongs to the process, not the record"
#[test]
fn the_transcript_on_disk_never_carries_a_sequence_it_belongs_to_the_process_not_the_record() {
    drive(|| async {
        let b = boot().await;
        b.h.last().delta("x");
        b.h.last().finish(json!({ "status": "completed" }));
        b.c.close().await;
        let history = b.fx.rt.sessions().history(&b.workspace_id, None).await.expect("a history");
        assert_eq!(history.len(), 4);
        for e in &history {
            assert!(j(e).get("seq").is_none(), "{}", j(e));
        }
        b.srv.close().await.expect("a close");
    });
}
