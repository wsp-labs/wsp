// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/{protocol,places,port-forwards,callback-relay,init-job,agents-report,preferences,broker-contract}.test.ts: events (EventUnion, EventsSubscribeReply, DaemonEvent).
//! The event payloads stand in as raw JSON until the views branch lands, so a case reads the event through
//! `parse_event`, names its variant, and reads the fields it is about off the whole event written back.
mod common;
use common::*;
use serde_json::{json, Value};
use wsp_wire::wire::daemon_events::{parse_daemon_event, DaemonEvent};
use wsp_wire::wire::events_union::{parse_event, EventUnion, EventsSubscribeReply};

fn written(e: &EventUnion) -> Value {
    serde_json::to_value(e).unwrap()
}

/// Where the TS case only asks that the event parse, not that it write back the same.
fn parsed(v: Value) -> EventUnion {
    parse_event(&v).unwrap_or_else(|e| panic!("refused {v}: {e:?}"))
}

fn with(v: &Value, key: &str, to: Value) -> Value {
    let mut out = v.clone();
    out[key] = to;
    out
}

fn without(v: &Value, key: &str) -> Value {
    let mut out = v.clone();
    out.as_object_mut().unwrap().remove(key);
    out
}

mod a_workspace_s_look {
    use super::*;

    // protocol.test.ts:401
    #[test]
    fn the_event_carries_both_facts_whole_so_a_cleared_one_reads_null_rather_than_going() {
        let theme = json!({ "dots": [{ "angle": 200, "radius": 0.5 }, { "angle": 20, "radius": 0.5 }], "harmony": "complementary", "grain": 0.25, "opacity": 0.6, "mode": "auto" });
        let e = json!({ "type": "workspace.look", "workspaceId": "ws_1", "theme": theme, "glyph": null });
        let r = reads_with(parse_event, e);
        assert!(matches!(r, EventUnion::WorkspaceLook(_)));
        let w = written(&r);
        assert_eq!(w["glyph"], Value::Null);
        assert!(w.as_object().unwrap().contains_key("glyph"));
        // A look event with no glyph key at all is refused: the event is whole.
        let err = refused_with(parse_event, json!({ "type": "workspace.look", "workspaceId": "ws_1", "theme": theme }));
        assert_eq!(err.path, path!["glyph"]);
        assert_eq!(err.code, "invalid_type");
    }
}

mod protocol_event_union {
    use super::*;

    // protocol.test.ts:409
    #[test]
    fn covers_workspace_session_mirroring_adapterevent_port_inbox() {
        let samples = vec![
            json!({ "type": "workspace.created", "workspace": { "id": "ws_1", "name": "x", "machineId": "m1", "phase": "running", "golden": "snap_g", "createdAt": "2026-09-01T00:00:00.000Z", "project": { "id": "pr_1a2b3c4d", "name": "task-1", "path": "/root/task-1", "computer": "here" } } }),
            json!({ "type": "workspace.napped", "workspaceId": "ws_1" }),
            json!({ "type": "workspace.woken", "workspaceId": "ws_1", "machineId": "m2" }),
            json!({ "type": "workspace.upgraded", "workspaceId": "ws_1", "machineId": "m3" }),
            json!({ "type": "workspace.deleted", "workspaceId": "ws_1" }),
            json!({ "type": "session.start", "workspaceId": "ws_1", "sessionId": "s1", "model": "claude-sonnet-4-5" }),
            json!({ "type": "session.delta", "workspaceId": "ws_1", "sessionId": "s1", "kind": "text", "text": "hi" }),
            json!({ "type": "session.delta", "workspaceId": "ws_1", "sessionId": "s1", "kind": "tool_use", "text": "{}", "toolName": "Bash", "toolUseId": "tu_1" }),
            json!({ "type": "session.done", "workspaceId": "ws_1", "sessionId": "s1", "result": { "status": "completed", "durationMs": 1200, "costUsd": 0.01, "text": "ok" } }),
            json!({ "type": "session.end", "workspaceId": "ws_1", "sessionId": "s1", "exitCode": 0, "sawResult": true }),
            json!({ "type": "port.open", "workspaceId": "ws_1", "port": 8080, "pid": 123 }),
            json!({ "type": "port.close", "workspaceId": "ws_1", "port": 8080 }),
            json!({ "type": "inbox.file", "workspaceId": "ws_1", "path": "/root/.wsp/inbox/a.png", "bytes": 168 }),
        ];
        let want = [
            "workspace.created",
            "workspace.napped",
            "workspace.woken",
            "workspace.upgraded",
            "workspace.deleted",
            "session.start",
            "session.delta",
            "session.delta",
            "session.done",
            "session.end",
            "port.open",
            "port.close",
            "inbox.file",
        ];
        for (s, ty) in samples.into_iter().zip(want) {
            let r = reads_with(parse_event, s);
            assert_eq!(written(&r)["type"], ty);
        }
        let err = refused_with(parse_event, json!({ "type": "workspace.exploded" }));
        assert_eq!(err.code, "invalid_union_discriminator");
        assert_eq!(err.path, path!["type"]);
        // session.start may carry the prompt so a replayed transcript shows the user's turn
        let started = json!({ "type": "session.start", "workspaceId": "ws_1", "sessionId": "s1", "prompt": "fix the flaky test" });
        let r = reads_with(parse_event, started);
        assert_eq!(written(&r)["prompt"], "fix the flaky test");
        // session.end with a null exit code (kill path) is valid
        let r = parsed(json!({ "type": "session.end", "workspaceId": "w", "sessionId": "s", "exitCode": null, "sawResult": false }));
        assert!(matches!(r, EventUnion::SessionEnd(_)));
        assert_eq!(written(&r)["exitCode"], Value::Null);
    }

    // protocol.test.ts:462
    #[test]
    fn session_steer_is_a_session_event_the_message_the_person_sent_into_a_running_turn() {
        let steered = json!({ "type": "session.steer", "workspaceId": "ws_1", "sessionId": "s1", "turnId": "turn_0001", "threadId": "thread_0001", "at": 1756687889412u64, "prompt": "and say pineapple", "requestId": "req_7" });
        let r = reads_with(parse_event, steered.clone());
        assert!(matches!(r, EventUnion::SessionSteer(_)));
        assert_eq!(written(&r)["requestId"], "req_7");
        let stamped = with(&steered, "seq", json!(9));
        let r = reads_with(parse_event, stamped);
        assert_eq!(written(&r)["seq"], 9);
    }
}

mod a_create_s_stage_lines_on_the_wire {
    use super::*;

    // protocol.test.ts:477
    #[test]
    fn a_step_s_own_detail_rides_the_event_beside_its_message_and_is_not_a_notice() {
        let stage = json!({ "type": "workspace.creating", "workspaceId": "ws_1", "name": "clone-test", "stage": "hostname-set", "elapsedMs": 4100 });
        let mut said = stage.clone();
        said["message"] = json!("hostname not set; the workspace keeps the machine's own name");
        said["detail"] = json!("hostname clone-test on fk_0b77 failed: hostname: sethostname: Operation not permitted");
        let r = reads_with(parse_event, said);
        assert!(matches!(r, EventUnion::WorkspaceCreating(_)));
        assert_eq!(written(&r)["detail"], "hostname clone-test on fk_0b77 failed: hostname: sethostname: Operation not permitted");
        reads_with(parse_event, with(&stage, "message", json!("ready")));
        let mut bad = with(&stage, "message", json!("ready"));
        bad["detail"] = json!(7);
        let err = refused_with(parse_event, bad);
        assert_eq!(err.path, path!["detail"]);
        assert_eq!(err.code, "invalid_type");
    }
}

mod a_relayed_permission_prompt_on_the_wire {
    use super::*;

    // protocol.test.ts:513
    #[test]
    fn both_halves_are_session_events_so_a_transcript_replays_a_prompt_and_how_it_close() {
        let ask = json!({
            "type": "session.permission", "workspaceId": "ws_1", "sessionId": "s1", "turnId": "turn_0001", "threadId": "thread_0001", "at": 1757320000000u64,
            "askId": "d9aa99d3-be4e-4a2b-8766-1b9494cde4f6", "toolName": "Write", "toolUseId": "toolu_1",
            "input": "{\"file_path\":\"/root/out.txt\",\"content\":\"hi\"}", "detail": "out.txt",
            "options": [
                { "id": "allow", "label": "Allow", "effect": "allow" },
                { "id": "deny", "label": "Deny", "effect": "deny" },
                { "id": "mode:acceptEdits", "label": "Allow, then Accept edits", "effect": "mode", "mode": "acceptEdits" },
            ],
        });
        let closed = json!({ "type": "session.permission.closed", "workspaceId": "ws_1", "sessionId": "s1", "turnId": "turn_0001", "threadId": "thread_0001", "at": 1757320005000u64, "askId": ask["askId"], "outcome": "allowed", "optionId": "allow" });
        let r = reads_with(parse_event, ask.clone());
        assert!(matches!(r, EventUnion::SessionPermission(_)));
        let r = reads_with(parse_event, closed.clone());
        assert!(matches!(r, EventUnion::SessionPermissionClosed(_)));
        for e in [ask, closed] {
            let r = reads_with(parse_event, with(&e, "seq", json!(12)));
            assert_eq!(written(&r)["seq"], 12);
        }
    }
}

mod session_queued {
    use super::*;

    // protocol.test.ts:569
    #[test]
    fn is_a_pushed_event_not_a_session_event_a_start_waiting_behind_the_thread_s_runnin() {
        let queued = json!({ "type": "session.queued", "workspaceId": "ws_1", "threadId": "thread_0001", "harness": "codex", "prompt": "and then this", "requestId": "req_8" });
        let r = reads_with(parse_event, queued.clone());
        assert!(matches!(r, EventUnion::SessionQueued(_)));
        assert_eq!(written(&r)["requestId"], "req_8");
        let r = reads_with(parse_event, with(&queued, "seq", json!(4)));
        assert_eq!(written(&r)["seq"], 4);
        reads_with(parse_event, without(&queued, "requestId"));
        for key in ["prompt", "threadId", "harness"] {
            let err = refused_with(parse_event, without(&queued, key));
            assert_eq!(err.path, path![key], "missing {key}");
            assert_eq!(err.code, "invalid_type");
        }
    }
}

mod session_wire_fields_the_face_reads {
    use super::*;

    // protocol.test.ts:583
    #[test]
    fn every_session_event_may_carry_at_ms_epoch_and_turnid_both_survive_a_json_round_t() {
        let scope = |mut v: Value| {
            v["workspaceId"] = json!("ws_1");
            v["sessionId"] = json!("s1");
            v["at"] = json!(1756687889412u64);
            v["turnId"] = json!("turn_0001");
            v
        };
        let events = [
            scope(json!({ "type": "session.start", "prompt": "hello" })),
            scope(json!({ "type": "session.delta", "kind": "text", "text": "hi" })),
            scope(json!({ "type": "session.done", "result": { "status": "completed" } })),
            scope(json!({ "type": "session.end", "exitCode": 0, "sawResult": true })),
        ];
        for e in events {
            let r = reads_with(parse_event, e.clone());
            let w = written(&r);
            assert_eq!(w["at"], 1756687889412u64);
            assert_eq!(w["turnId"], "turn_0001");
        }
    }

    // protocol.test.ts:599
    #[test]
    fn every_session_event_may_carry_a_threadid_a_transcript_without_one_still_parses() {
        let scope = |mut v: Value| {
            v["workspaceId"] = json!("ws_1");
            v["sessionId"] = json!("s1");
            v["turnId"] = json!("turn_0001");
            v["threadId"] = json!("thread_0001");
            v
        };
        let events = [
            scope(json!({ "type": "session.start", "prompt": "hello" })),
            scope(json!({ "type": "session.delta", "kind": "text", "text": "hi" })),
            scope(json!({ "type": "session.done", "result": { "status": "completed" } })),
            scope(json!({ "type": "session.end", "exitCode": 0, "sawResult": true })),
        ];
        for e in events {
            let r = reads_with(parse_event, e.clone());
            assert_eq!(written(&r)["threadId"], "thread_0001");
            let before = without(&e, "threadId");
            let r = reads_with(parse_event, before);
            assert!(!written(&r).as_object().unwrap().contains_key("threadId"));
        }
    }

    // protocol.test.ts:650
    #[test]
    fn port_open_names_the_listening_process_on_both_the_daemon_and_runtime_wires() {
        let daemon_side = json!({ "type": "port.open", "port": 8080, "pid": 123, "process": "node" });
        let r = reads_with(parse_daemon_event, daemon_side.clone());
        assert!(matches!(r, DaemonEvent::PortOpen { .. }));
        let runtime_side = with(&daemon_side, "workspaceId", json!("ws_1"));
        let r = reads_with(parse_event, runtime_side);
        assert!(matches!(r, EventUnion::PortOpen(_)));
        assert_eq!(written(&r)["process"], "node");
        let err = refused_with(parse_daemon_event, with(&daemon_side, "process", json!(1)));
        assert_eq!(err.path, path!["process"]);
        assert_eq!(err.code, "invalid_type");
    }

    // protocol.test.ts:658
    #[test]
    fn port_close_may_say_who_held_the_port_whether_it_exited_and_when_on_both_wires() {
        let daemon_side = json!({ "type": "port.close", "port": 8412, "pid": 53479, "process": "python3", "command": "python3 -m http.server 8412", "exited": true, "at": "2026-09-05T12:04:00.000Z" });
        let r = reads_with(parse_daemon_event, daemon_side.clone());
        match r {
            DaemonEvent::PortClose { exited, command, .. } => {
                assert_eq!(exited, Some(true));
                assert_eq!(command.as_deref(), Some("python3 -m http.server 8412"));
            }
            other => panic!("{other:?}"),
        }
        let runtime_side = with(&daemon_side, "workspaceId", json!("ws_1"));
        let r = reads_with(parse_event, runtime_side);
        assert!(matches!(r, EventUnion::PortClose(_)));
        let bare = reads_with(parse_daemon_event, json!({ "type": "port.close", "port": 8412 }));
        match bare {
            DaemonEvent::PortClose { exited, pid, process, command, left, at, watch, .. } => {
                assert_eq!((exited, pid, process, command, left, at, watch), (None, None, None, None, None, None, None));
            }
            other => panic!("{other:?}"),
        }
        let err = refused_with(parse_daemon_event, with(&daemon_side, "exited", json!("yes")));
        assert_eq!(err.path, path!["exited"]);
        assert_eq!(err.code, "invalid_type");
    }
}

mod event_replay_wire_fields {
    use super::*;

    // protocol.test.ts:669
    #[test]
    fn every_event_may_carry_seq_a_positive_integer_that_survives_a_json_round_trip_his() {
        let events = vec![
            json!({ "type": "workspace.napped", "workspaceId": "ws_1", "seq": 1 }),
            json!({ "type": "workspace.status", "status": { "id": "ws_1", "name": "x", "machineId": "m1", "phase": "running", "golden": "g", "createdAt": "t", "project": { "id": "pr_1a2b3c4d", "name": "x", "path": "/root/x", "computer": "here" }, "machineState": "running", "reach": { "state": "unsupported" }, "size": { "cpu": 2, "memMb": 4096 }, "rateUsdPerHour": 0.1 }, "seq": 2 }),
            json!({ "type": "session.delta", "workspaceId": "ws_1", "sessionId": "s1", "kind": "text", "text": "hi", "seq": 3 }),
            json!({ "type": "port.open", "workspaceId": "ws_1", "port": 8080, "seq": 4 }),
            json!({ "type": "golden.stage", "name": "default", "stage": "ready", "seq": 5 }),
        ];
        for (n, e) in events.iter().enumerate() {
            let r = reads_with(parse_event, e.clone());
            assert_eq!(written(&r)["seq"], n as u64 + 1);
        }
        let unstamped = without(&events[0], "seq");
        let r = reads_with(parse_event, unstamped);
        assert!(!written(&r).as_object().unwrap().contains_key("seq"));
        let zero = refused_with(parse_event, with(&events[0], "seq", json!(0)));
        assert_eq!(zero.path, path!["seq"]);
        assert_eq!(zero.code, "too_small");
        let fraction = refused_with(parse_event, with(&events[0], "seq", json!(1.5)));
        assert_eq!(fraction.path, path!["seq"]);
        assert_eq!(fraction.code, "invalid_type");
        let text = refused_with(parse_event, with(&events[0], "seq", json!("1")));
        assert_eq!(text.path, path!["seq"]);
        assert_eq!(text.code, "invalid_type");
    }

    // protocol.test.ts:700
    #[test]
    fn the_subscribe_reply_names_the_runtime_s_head_and_stream_and_when_the_cursor_is_g() {
        let r: EventsSubscribeReply = reads(json!({ "seq": 0 }));
        assert_eq!((r.seq, r.stream, r.gap), (0.into(), None, None));
        let r: EventsSubscribeReply = reads(json!({ "seq": 5002, "gap": true, "stream": "0b6a9c1e-0000-4000-8000-000000000000" }));
        assert_eq!(r.seq, 5002.into());
        assert_eq!(r.stream.as_deref(), Some("0b6a9c1e-0000-4000-8000-000000000000"));
        assert!(r.gap.is_some());
        let e = refused::<EventsSubscribeReply>(json!({ "seq": 1, "stream": 7 }));
        assert_eq!(e.path, path!["stream"]);
        assert_eq!(e.code, "invalid_type");
        let e = refused::<EventsSubscribeReply>(json!({ "seq": 5002, "gap": false }));
        assert_eq!(e.path, path!["gap"]);
        assert_eq!(e.code, "invalid_literal");
        let e = refused::<EventsSubscribeReply>(json!({ "gap": true }));
        assert_eq!(e.path, path!["seq"]);
        assert_eq!(e.code, "invalid_type");
    }
}

mod golden_wire_schemas {
    use super::*;

    // protocol.test.ts:1202
    #[test]
    fn golden_stage_rides_the_event_union_with_a_closed_stage_enum_and_an_optional_deta() {
        let e = json!({ "type": "golden.stage", "name": "default", "stage": "smoke-forking", "detail": "claude --version" });
        let r = reads_with(parse_event, e);
        assert!(matches!(r, EventUnion::GoldenStage(_)));
        assert_eq!(written(&r)["stage"], "smoke-forking");
        assert_eq!(written(&r)["detail"], "claude --version");
    }

    // protocol.test.ts:1212
    #[test]
    fn a_golden_stage_frame_may_name_the_install_step_it_belongs_to_with_the_command_a() {
        let step = json!({ "label": "mongosh", "command": "npm install -g mongosh" });
        let e =
            json!({ "type": "golden.stage", "name": "default", "stage": "installing-tools", "detail": "mongosh (24/30)", "step": step });
        let r = reads_with(parse_event, e);
        assert!(matches!(r, EventUnion::GoldenStage(_)));
        assert_eq!(written(&r)["step"], step);
    }
}

mod thread_provenance {
    use super::*;

    // protocol.test.ts:1500
    #[test]
    fn sessions_start_may_name_who_its_thread_s_ends_are_told_one_target_or_several_eac() {
        // Only the event half: the request half is the RuntimeRequest union's.
        let told = json!({ "type": "session.notify", "workspaceId": "ws_1", "sessionId": "s1", "turnId": "turn_0002", "threadId": "thread_child_0001", "at": 1756687889412u64, "notify": "thread_parent_0001", "text": "thread thread_c finished (completed, 8m 12s, $1.94): all green" });
        let r = reads_with(parse_event, told.clone());
        assert!(matches!(r, EventUnion::SessionNotify(_)));
        assert_eq!(written(&r)["notify"], "thread_parent_0001");
        let r = reads_with(parse_event, with(&told, "seq", json!(12)));
        assert_eq!(written(&r)["seq"], 12);
    }

    // protocol.test.ts:1519
    #[test]
    fn sessions_start_may_carry_the_client_s_request_id_and_session_start_carries_it_ba() {
        // Only the event half: the request half is the RuntimeRequest union's.
        let started = json!({ "type": "session.start", "workspaceId": "ws_1", "sessionId": "s1", "prompt": "go", "requestId": "req_1" });
        let r = reads_with(parse_event, started.clone());
        assert!(matches!(r, EventUnion::SessionStart(_)));
        assert_eq!(written(&r)["requestId"], "req_1");
        let r = reads_with(parse_event, without(&started, "requestId"));
        assert!(!written(&r).as_object().unwrap().contains_key("requestId"));
    }
}

mod the_project_export {
    use super::*;

    // protocol.test.ts:1964
    #[test]
    fn its_events_ride_the_union_with_the_stages_in_order_and_the_result_counts_session() {
        for stage in ["packing", "downloading", "landing", "done", "failed"] {
            let e = json!({ "type": "project.export", "workspaceId": "ws_1", "source": "/root/work/proj", "dest": "/Users/dev/proj", "stage": stage, "message": "x", "elapsedMs": 3, "seq": 1 });
            let r = reads_with(parse_event, e);
            assert!(matches!(r, EventUnion::ProjectExport(_)));
            assert_eq!(written(&r)["stage"], stage);
        }
    }
}

mod the_newest_release_as_the_host_read_it {
    use super::*;

    // protocol.test.ts:2046
    #[test]
    fn is_asked_for_and_checked_with_no_arguments_and_rides_one_event_every_socket_fold() {
        let release = json!({
            "state": "read",
            "latest": { "version": "0.3.0", "tag": "v0.3.0", "url": "https://github.com/wsp-labs/wsp/releases/tag/v0.3.0", "publishedAt": "2026-09-24T10:00:00Z" },
            "checkedAt": "2026-09-24T11:00:00.000Z",
            "triedAt": "2026-09-24T11:00:00.000Z",
            "shape": "service",
        });
        let e = json!({ "type": "release.changed", "release": release, "seq": 3 });
        let r = reads_with(parse_event, e);
        assert!(matches!(r, EventUnion::ReleaseChanged(_)));
        let w = written(&r);
        assert_eq!(w["release"], release);
        assert_eq!(w["seq"], 3);
    }
}

mod the_doctor_s_computer_road_on_the_wire {
    use super::*;

    // protocol.test.ts:2240
    #[test]
    fn one_line_of_a_road_carries_which_run_it_is_the_words_and_which_stream_said_them() {
        // A host source's event and no member of the runtime's own union: nothing retains it and nothing replays it.
        let err = refused_with(parse_event, json!({ "type": "doctor.line", "doctorId": "d_abc", "line": "spoo answers", "stream": "out" }));
        assert_eq!(err.code, "invalid_union_discriminator");
        assert_eq!(err.path, path!["type"]);
    }
}

mod the_four_events_a_computer_you_own_rides_the_runtime_s_own_stream_on {
    use super::*;

    // places.test.ts:299
    #[test]
    fn parses_each_with_the_sequence_every_other_event_carries() {
        let place = json!({ "id": "p_1", "kind": "computer", "name": "old-macbook", "default": true });
        let joined = parsed(json!({ "type": "place.joined", "place": place, "from": "192.168.1.34", "seq": 3 }));
        assert!(matches!(joined, EventUnion::PlaceJoined(_)));
        assert_eq!(written(&joined)["seq"], 3);
        let present = parsed(json!({ "type": "place.present", "placeId": "p_1", "from": "192.168.1.34", "seq": 4 }));
        assert!(matches!(present, EventUnion::PlacePresent(_)));
        assert_eq!(written(&present)["seq"], 4);
        let absent = parsed(json!({ "type": "place.absent", "placeId": "p_1", "seq": 5 }));
        assert!(matches!(absent, EventUnion::PlaceAbsent(_)));
        assert_eq!(written(&absent)["seq"], 5);
        let removed = parsed(json!({ "type": "place.removed", "placeId": "p_1", "seq": 6 }));
        assert!(matches!(removed, EventUnion::PlaceRemoved(_)));
        assert_eq!(written(&removed)["seq"], 6);
    }

    // places.test.ts:306
    #[test]
    fn refuses_a_join_with_no_address_it_came_from_since_the_sheet_says_where_it_connec() {
        let place = json!({ "id": "p_1", "kind": "computer", "name": "old-macbook", "default": true });
        let err = refused_with(parse_event, json!({ "type": "place.joined", "place": place, "seq": 3 }));
        assert_eq!(err.path, path!["from"]);
        assert_eq!(err.code, "invalid_type");
    }
}

mod localhost_forward_wire_shapes {
    use super::*;

    // port-forwards.test.ts:6
    #[test]
    fn localhost_url_carries_a_port_the_laptop_can_bind_and_nothing_else() {
        for port in [1024, 8123, 65535] {
            let r = reads_with(parse_daemon_event, json!({ "type": "localhost.url", "port": port }));
            assert!(matches!(r, DaemonEvent::LocalhostUrl { .. }));
            assert_eq!(serde_json::to_value(&r).unwrap()["port"], port);
        }
        let refusals = [
            (json!(80), "too_small"),
            (json!(1023), "too_small"),
            (json!(65536), "too_big"),
            (json!(70000), "too_big"),
            (json!(1.5), "invalid_type"),
            (json!(-1), "too_small"),
            (json!("8123"), "invalid_type"),
        ];
        for (port, code) in refusals {
            let err = refused_with(parse_daemon_event, json!({ "type": "localhost.url", "port": port }));
            assert_eq!(err.path, path!["port"], "port {port}");
            assert_eq!(err.code, code, "port {port}");
        }
        let err = refused_with(parse_daemon_event, json!({ "type": "localhost.url" }));
        assert_eq!(err.path, path!["port"]);
        assert_eq!(err.code, "invalid_type");
    }

    // port-forwards.test.ts:24
    #[test]
    fn forward_open_and_forward_close_ride_the_runtime_event_stream() {
        let f = json!({ "workspaceId": "ws_1", "port": 8123, "startedAt": "2026-09-04T10:00:00.000Z", "name": "api", "kind": "url" });
        let r = reads_with(parse_event, json!({ "type": "forward.open", "forward": f }));
        assert!(matches!(r, EventUnion::ForwardOpen(_)));
        assert_eq!(written(&r)["forward"], f);
        let r = reads_with(parse_event, json!({ "type": "forward.close", "workspaceId": "ws_1", "port": 8123 }));
        assert!(matches!(r, EventUnion::ForwardClose(_)));
        let err = refused_with(parse_event, json!({ "type": "forward.open" }));
        assert_eq!(err.path, path!["forward"]);
        assert_eq!(err.code, "invalid_type");
        let err = refused_with(parse_event, json!({ "type": "forward.close", "workspaceId": "ws_1", "port": 80 }));
        assert_eq!(err.path, path!["port"]);
        assert_eq!(err.code, "too_small");
    }
}

mod callback_relay_wire_shapes {
    use super::*;

    // callback-relay.test.ts:55
    #[test]
    fn browser_open_carries_the_url_and_when_the_redirect_uri_named_one_the_port() {
        let bare = json!({ "type": "browser.open", "url": "https://github.com/login/device" });
        let r = reads_with(parse_daemon_event, bare);
        match r {
            DaemonEvent::BrowserOpen { url, port } => {
                assert_eq!(url, "https://github.com/login/device");
                assert!(port.is_none());
            }
            other => panic!("{other:?}"),
        }
        let with_port = json!({ "type": "browser.open", "url": "https://dash.example.com/auth?redirect_uri=http%3A%2F%2Flocalhost%3A8976%2Fcb", "port": 8976 });
        let r = reads_with(parse_daemon_event, with_port);
        match r {
            DaemonEvent::BrowserOpen { port, .. } => assert_eq!(serde_json::to_value(port).unwrap(), 8976),
            other => panic!("{other:?}"),
        }
        let err = refused_with(parse_daemon_event, json!({ "type": "browser.open" }));
        assert_eq!(err.path, path!["url"]);
        assert_eq!(err.code, "invalid_type");
        for url in ["file:///etc/passwd", "javascript:alert(1)", "smb://host/share", "\\\\host\\share", "ftp://x/y"] {
            let err = refused_with(parse_daemon_event, json!({ "type": "browser.open", "url": url }));
            assert_eq!(err.path, path!["url"], "{url}");
            assert_eq!(err.code, "custom", "{url}");
        }
    }

    // callback-relay.test.ts:66
    #[test]
    fn callback_port_names_a_loopback_listener_port_open_may_say_it_is_loopback_bound() {
        let r = reads_with(parse_daemon_event, json!({ "type": "callback.port", "port": 45543 }));
        assert!(matches!(r, DaemonEvent::CallbackPort { .. }));
        assert_eq!(serde_json::to_value(&r).unwrap()["port"], 45543);
        let err = refused_with(parse_daemon_event, json!({ "type": "callback.port" }));
        assert_eq!(err.path, path!["port"]);
        assert_eq!(err.code, "invalid_type");
        let open = reads_with(parse_daemon_event, json!({ "type": "port.open", "port": 8976, "loopback": true }));
        match open {
            DaemonEvent::PortOpen { loopback, .. } => assert_eq!(loopback, Some(true)),
            other => panic!("{other:?}"),
        }
        let plain = reads_with(parse_daemon_event, json!({ "type": "port.open", "port": 3000 }));
        match plain {
            DaemonEvent::PortOpen { loopback, pid, process, watch, .. } => {
                assert_eq!((loopback, pid, process, watch), (None, None, None, None))
            }
            other => panic!("{other:?}"),
        }
    }
}

mod the_init_job_view {
    use super::*;

    // init-job.test.ts:163
    #[test]
    fn the_event_rides_the_events_channel_beside_the_runtime_s_own_and_the_ops_parse_as() {
        // Only the event half: the ops are the RuntimeRequest union's, and the event's own shape is the views'.
        let row = |id: &str, kind: &str, label: &str, state: &str| json!({ "id": id, "kind": kind, "label": label, "state": state });
        let job = json!({
            "id": "init_1", "road": "manual", "phase": "building", "keys": { "solari": true }, "step": 0, "stoppable": true, "screens": [],
            "rows": [row("stage/creating", "stage", "Creating the machine", "done"), row("stage/deploying-daemon", "stage", "Installing the base tools", "running"), row("stage/ready", "stage", "Waiting for the machine", "waiting")],
            "progress": { "done": 1, "total": 3 }, "log": ["Recipe saved to /tmp/recipe.json"],
        });
        let event = json!({ "type": "init.job", "job": job, "seq": 3 });
        let r = reads_with(parse_event, event);
        assert!(matches!(r, EventUnion::InitJob(_)));
        let w = written(&r);
        assert_eq!(w["seq"], 3);
        assert_eq!(w["job"], job);
    }

    // init-job.test.ts:234
    #[test]
    fn the_need_s_arrival_is_its_own_event_on_the_same_channel_one_per_need_and_the_wir() {
        let event = json!({ "type": "job.needs-you", "jobId": "init_1", "needsYou": { "what": "sign in to GitHub CLI login", "since": 1760000000000u64 } });
        let r = reads_with(parse_event, with(&event, "seq", json!(4)));
        assert!(matches!(r, EventUnion::JobNeedsYou(_)));
        assert_eq!(written(&r)["seq"], 4);
        // A need without its clock is refused.
        let err = refused_with(parse_event, json!({ "type": "job.needs-you", "jobId": "init_1", "needsYou": { "what": "sign in" } }));
        assert_eq!(err.path, path!["needsYou", "since"]);
        assert_eq!(err.code, "invalid_type");
    }
}

mod the_agents_report_on_the_wire {
    use super::*;

    // agents-report.test.ts:82
    #[test]
    fn a_sign_in_s_progress_carries_the_page_the_code_it_printed_and_whether_a_code_goe() {
        // Only the change event: the sign-in progress and the sign-in line are the views'.
        let r = parsed(json!({ "type": "agents.changed", "seq": 3 }));
        assert!(matches!(r, EventUnion::AgentsChanged(_)));
        assert_eq!(written(&r)["type"], "agents.changed");
        assert_eq!(written(&r)["seq"], 3);
    }
}

mod the_preferences_record {
    use super::*;

    // preferences.test.ts:193
    #[test]
    fn the_runtime_takes_preferences_get_and_preferences_set_with_a_patch_and_the_chang() {
        // Only the event half: the changed event carries the whole record, here its defaults.
        let defaults = json!({ "theme": "system", "lightTheme": "paper", "darkTheme": "graphite", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {}, "projectLook": {}, "projectIcon": {}, "computerLook": {}, "serverIcons": true, "agentVersions": true, "usageLogs": true, "productUsage": true, "keepAwake": true, "transparency": true, "projectOrder": [], "keybindings": {}, "appFont": "", "codeFont": "", "agentDefaults": {}, "projectDefaults": {}, "sendWith": "enter", "midTurn": "queue", "notifyNeeds": "notify-sound", "notifyDone": "notify", "planAlerts": true, "settleAfter": "2h", "askDelete": true, "onQuit": "ask", "newThreadIn": "current", "labs": false });
        let r = parsed(json!({ "type": "preferences.changed", "preferences": defaults, "seq": 4 }));
        assert!(matches!(r, EventUnion::PreferencesChanged(_)));
        let w = written(&r);
        assert_eq!(w["preferences"], defaults);
        assert_eq!(w["seq"], 4);
    }
}

mod the_credential_contracts {
    use super::*;

    // broker-contract.test.ts:96
    #[test]
    fn thread_waited_is_a_stored_event_and_a_pushed_one() {
        let waited = json!({ "type": "thread.waited", "workspaceId": "ws_1", "sessionId": "s_1", "threadId": "t_1", "ms": 840000, "said": "your computer was away" });
        let r = reads_with(parse_event, with(&waited, "seq", json!(4)));
        assert!(matches!(r, EventUnion::ThreadWaited(_)));
        let w = written(&r);
        assert_eq!(w["type"], "thread.waited");
        assert_eq!(w["ms"], 840000);
    }
}

// Edges no TS test pins, each printed by the TS module at a3b870a6e.
mod edges_the_ts_module_printed {
    use super::*;
    use wsp_wire::wire::events_union::{parse_event, EventsSubscribeReply, EVENT_TYPES};

    // events-union.ts:18-96 with helpers.ts:81: seq is a positive integer after every other key; 0 and a fraction
    // refuse at ["seq"]; the type is written first.
    #[test]
    fn seq_is_a_positive_integer_written_after_the_event() {
        let read = reads_with(parse_event, json!({ "type": "forward.close", "workspaceId": "w", "port": 8123, "seq": 3 }));
        assert_eq!(serde_json::to_string(&read).unwrap(), r#"{"type":"forward.close","workspaceId":"w","port":8123,"seq":3}"#);
        let e = refused_with(parse_event, json!({ "type": "forward.close", "workspaceId": "w", "port": 8123, "seq": 0 }));
        assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("too_small", path!["seq"], "Number must be greater than 0"));
        let e = refused_with(parse_event, json!({ "type": "forward.close", "workspaceId": "w", "port": 8123, "seq": 1.5 }));
        assert_eq!((e.code.as_str(), e.path.clone()), ("invalid_type", path!["seq"]));
        let bare = reads_with(parse_event, json!({ "type": "forward.close", "workspaceId": "w", "port": 8123 }));
        assert!(serde_json::to_value(&bare).unwrap().get("seq").is_none());
    }

    #[test]
    fn an_unknown_type_is_one_discriminator_issue_naming_all_77_types_in_order() {
        let e = refused_with(parse_event, json!({ "type": "nope" }));
        assert_eq!((e.code.as_str(), e.path.clone()), ("invalid_union_discriminator", path!["type"]));
        let options: Vec<String> = EVENT_TYPES.iter().map(|s| s.to_string()).collect();
        assert_eq!(e.detail["options"], json!(options));
        assert_eq!((EVENT_TYPES.len(), EVENT_TYPES[0], EVENT_TYPES[76]), (77, "workspace.creating", "aside.text"));
    }

    // events-union.ts:105-109: seq a non-negative integer (0 before the first event), gap only the literal true.
    #[test]
    fn the_subscribe_reply_takes_seq_zero_and_gap_true_alone() {
        let first: EventsSubscribeReply = reads_exact(json!({ "seq": 0 }));
        assert_eq!((first.seq, first.stream.is_none(), first.gap.is_none()), (0.into(), true, true));
        let gap: EventsSubscribeReply = reads_exact(json!({ "seq": 5, "stream": "s", "gap": true }));
        assert_eq!((gap.seq, gap.stream.as_deref(), gap.gap.is_some()), (5.into(), Some("s"), true));
        let e = refused::<EventsSubscribeReply>(json!({ "seq": -1 }));
        assert_eq!((e.code.as_str(), e.path.clone()), ("too_small", path!["seq"]));
        let e = refused::<EventsSubscribeReply>(json!({ "seq": 5, "stream": "s", "gap": false }));
        assert_eq!((e.code.as_str(), e.path.clone()), ("invalid_literal", path!["gap"]));
    }
}
