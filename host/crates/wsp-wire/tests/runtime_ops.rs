// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/protocol.test.ts, port-forwards.test.ts, agents-report.test.ts,
//! preferences.test.ts, thread-defaults.test.ts, init-job.test.ts and broker-contract.test.ts: runtime_ops, the
//! RuntimeRequest union read through parse_runtime_request, and the op lists beside it.
mod common;
use common::*;
use serde_json::json;
use serde_json::Value;
use wsp_wire::types::{JsNumber, SchemaError};
use wsp_wire::views::workspace::WorkspaceOrigin;
use wsp_wire::wire::runtime::{
    parse_runtime_request, request_secrets, RuntimeOp, RuntimeRequest, DEVICE_OPS, RUNTIME_OPS, SECRET_REQUEST_FIELDS, THREAD_OPS,
};

fn read(v: Value) -> RuntimeRequest {
    reads_with(parse_runtime_request, v)
}

fn refuse(v: Value) -> SchemaError {
    refused_with(parse_runtime_request, v)
}

/// A required field left out is zod's invalid_type at that field.
fn lacks(v: Value, field: &str) {
    let e = refuse(v);
    assert_eq!(e.code, "invalid_type");
    assert_eq!(e.path, path![field]);
}

/// An op word the union does not list is zod's invalid_union_discriminator on ["op"].
fn unknown_op(v: Value) {
    let e = refuse(v);
    assert_eq!(e.code, "invalid_union_discriminator");
    assert_eq!(e.path, path!["op"]);
}

fn with(mut v: Value, key: &str, value: Value) -> Value {
    v[key] = value;
    v
}

fn without(mut v: Value, key: &str) -> Value {
    v.as_object_mut().unwrap().remove(key);
    v
}

macro_rules! op {
    ($r:expr, $pat:pat => $body:expr) => {
        match $r.op {
            $pat => $body,
            other => panic!("{other:?}"),
        }
    };
}

mod a_workspaces_look {
    use super::*;

    // protocol.test.ts:388
    #[test]
    fn the_op_takes_one_fact_at_a_time_null_to_clear_it_and_refuses_a_theme_outside_the() {
        let theme = json!({ "dots": [{ "angle": 200, "radius": 0.5 }, { "angle": 20, "radius": 0.5 }], "harmony": "complementary", "grain": 0.25, "opacity": 0.6, "mode": "auto" });
        let both = read(json!({ "id": 1, "op": "workspaces.look", "workspaceId": "ws_1", "theme": theme }));
        op!(both, RuntimeOp::WorkspacesLook { theme: t, glyph, .. } => {
            assert_eq!(t.clone().flatten().map(|v| serde_json::to_value(v).unwrap()), Some(theme.clone()));
            assert_eq!(glyph, None);
        });
        let cleared = read(json!({ "id": 2, "op": "workspaces.look", "workspaceId": "ws_1", "glyph": null }));
        op!(cleared, RuntimeOp::WorkspacesLook { theme: t, glyph, .. } => {
            assert_eq!(t, None);
            assert_eq!(glyph, Some(None));
        });
        let mixed = read(json!({ "id": 3, "op": "workspaces.look", "workspaceId": "ws_1", "theme": null, "glyph": "rocket" }));
        op!(mixed, RuntimeOp::WorkspacesLook { theme: t, glyph, .. } => {
            assert_eq!(t, Some(None));
            assert_eq!(glyph.clone().flatten().map(|v| serde_json::to_value(v).unwrap()), Some(json!("rocket")));
        });
        let bare = read(json!({ "id": 4, "op": "workspaces.look", "workspaceId": "ws_1" }));
        op!(bare, RuntimeOp::WorkspacesLook { theme: t, glyph, .. } => {
            assert_eq!(t, None);
            assert_eq!(glyph, None);
        });
        let e = refuse(json!({ "id": 5, "op": "workspaces.look", "workspaceId": "ws_1", "theme": "red" }));
        assert_eq!(e.path, path!["theme"]);
        lacks(json!({ "id": 6, "op": "workspaces.look", "theme": theme }), "workspaceId");
    }
}

mod a_relayed_permission_prompt_on_the_wire {
    use super::*;

    // protocol.test.ts:545
    #[test]
    fn the_answer_op_and_its_outcomes_are_on_the_wire_so_a_client_can_name_an_option_an() {
        let req = json!({ "id": 31, "op": "sessions.answer", "sessionId": "s1", "askId": "ask_1", "optionId": "allow" });
        let r = read(req.clone());
        op!(r, RuntimeOp::SessionsAnswer { session_id, ask_id, option_id, reason, .. } => {
            assert_eq!(session_id, "s1");
            assert_eq!(ask_id, "ask_1");
            assert_eq!(option_id, "allow");
            assert_eq!(reason, None);
        });
        lacks(without(req.clone(), "askId"), "askId");
        lacks(without(req, "optionId"), "optionId");
    }

    // protocol.test.ts:556
    #[test]
    fn the_access_op_and_its_outcomes_are_on_the_wire_so_a_pick_made_mid_turn_can_be_se() {
        let req = json!({ "id": 32, "op": "sessions.access", "sessionId": "s1", "permissionMode": "bypassPermissions" });
        let r = read(req.clone());
        op!(r, RuntimeOp::SessionsAccess { session_id, permission_mode, .. } => {
            assert_eq!(session_id, "s1");
            assert_eq!(permission_mode, "bypassPermissions");
        });
        lacks(without(req.clone(), "permissionMode"), "permissionMode");
        lacks(without(req, "sessionId"), "sessionId");
    }
}

mod event_replay_wire_fields {
    use super::*;

    // protocol.test.ts:688
    #[test]
    fn events_subscribe_takes_an_optional_after_cursor_a_non_negative_integer() {
        let bare = json!({ "id": 1, "op": "events.subscribe" });
        let cursor = json!({ "id": 2, "op": "events.subscribe", "after": 41 });
        let start = json!({ "id": 3, "op": "events.subscribe", "after": 0 });
        let resumed = json!({ "id": 4, "op": "events.subscribe", "after": 41, "stream": "0b6a9c1e-0000-4000-8000-000000000000" });
        let r = read(bare.clone());
        op!(r, RuntimeOp::EventsSubscribe { after, stream, .. } => {
            assert_eq!(after, None);
            assert_eq!(stream, None);
        });
        let r = read(cursor);
        op!(r, RuntimeOp::EventsSubscribe { after, .. } => assert_eq!(after, Some(41)));
        let r = read(start);
        op!(r, RuntimeOp::EventsSubscribe { after, .. } => assert_eq!(after, Some(0)));
        let r = read(resumed);
        op!(r, RuntimeOp::EventsSubscribe { after, stream, .. } => {
            assert_eq!(after, Some(41));
            assert_eq!(stream.as_deref(), Some("0b6a9c1e-0000-4000-8000-000000000000"));
        });
        let e = refuse(with(bare.clone(), "stream", json!(7)));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["stream"]));
        let e = refuse(with(bare.clone(), "after", json!(-1)));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["after"]));
        let e = refuse(with(bare.clone(), "after", json!(2.5)));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["after"]));
        let e = refuse(with(bare, "after", json!("41")));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["after"]));
    }
}

mod runtime_wire_types {
    use super::*;

    // protocol.test.ts:815
    #[test]
    fn parses_the_client_ops_serveruntime_dispatches() {
        let reqs = vec![
            json!({ "id": 1, "op": "auth", "token": "t" }),
            json!({ "id": 2, "op": "ticket.issue", "purpose": "connect" }),
            json!({ "id": 3, "op": "events.subscribe" }),
            json!({ "id": 4, "op": "workspaces.create", "project": "spoo-landing", "golden": "snap_g", "name": "x", "cpu": 2 }),
            json!({ "id": 4.5, "op": "projects.add", "source": "/Users/dev/wsp" }),
            json!({ "id": 5, "op": "workspaces.list" }),
            json!({ "id": 6, "op": "workspaces.get", "workspaceId": "ws_1" }),
            json!({ "id": 7, "op": "workspaces.nap", "workspaceId": "ws_1" }),
            json!({ "id": 8, "op": "workspaces.wake", "workspaceId": "ws_1" }),
            json!({ "id": 9, "op": "workspaces.upgrade", "workspaceId": "ws_1" }),
            json!({ "id": 10, "op": "workspaces.delete", "workspaceId": "ws_1" }),
            json!({ "id": 11, "op": "sessions.start", "workspaceId": "ws_1", "prompt": "do the thing" }),
            json!({ "id": 12, "op": "sessions.list" }),
            json!({ "id": 13, "op": "golden.get", "name": "default" }),
            json!({ "id": 14, "op": "capabilities.get" }),
            json!({ "id": 15, "op": "sessions.history", "workspaceId": "ws_1" }),
            json!({ "id": 16, "op": "daemon.open", "workspaceId": "ws_1" }),
            json!({ "id": 17, "op": "golden.prepare", "name": "default" }),
            json!({ "id": 18, "op": "golden.prepare", "name": "default", "kind": "desktop" }),
            json!({ "id": 19, "op": "golden.seal", "builderId": "m1" }),
            json!({ "id": 20, "op": "daemon.send", "channel": "ch_1", "frame": { "op": "pty.write", "ptyId": "p1", "data": "ls\r" } }),
            json!({ "id": 21, "op": "sessions.interrupt", "sessionId": "s1" }),
            json!({ "id": 22, "op": "workspaces.forget", "workspaceId": "ws_1" }),
            json!({ "id": 23, "op": "workspaces.stopWake", "workspaceId": "ws_1" }),
            json!({ "id": 24, "op": "daemon.close", "channel": "ch_1" }),
        ];
        for r in reqs {
            read(r);
        }
        // sessionId required
        lacks(json!({ "id": 21, "op": "sessions.interrupt" }), "sessionId");
        // project+name required
        let e = refuse(json!({ "id": 1, "op": "workspaces.create" }));
        assert_eq!(e.code, "invalid_type");
        // a workspace is a project's
        lacks(json!({ "id": 1, "op": "workspaces.create", "name": "x" }), "project");
        // The image is the project's computer's own head unless a project image is named outright.
        let r = read(json!({ "id": 1, "op": "workspaces.create", "project": "spoo-landing", "name": "x" }));
        op!(r, RuntimeOp::WorkspacesCreate { golden, .. } => assert_eq!(golden, None));
        // The two roads that recorded a workspace of their own leave with the projects recut.
        unknown_op(json!({ "id": 1, "op": "workspaces.createLocal" }));
        unknown_op(json!({ "id": 1, "op": "workspaces.createSsh", "address": "dev@box" }));
        let e = refuse(json!({ "id": 1, "op": "golden.prepare", "name": "d", "kind": "browser" }));
        assert_eq!(e.path, path!["kind"]);
        // builderId required
        lacks(json!({ "id": 1, "op": "golden.seal" }), "builderId");
        // The two roads that handed a daemon token out leave the wire with the relay.
        unknown_op(json!({ "id": 1, "op": "workspaces.daemonReach", "workspaceId": "ws_1" }));
        unknown_op(json!({ "id": 1, "op": "golden.builderReach", "builderId": "m1" }));
        assert!(!RUNTIME_OPS.contains(&"workspaces.daemonReach"));
        assert!(!RUNTIME_OPS.contains(&"golden.builderReach"));
    }

    // protocol.test.ts:864
    #[test]
    fn every_request_carries_where_it_reached_the_host_from_on_the_envelope_and_not_per() {
        let relayed = [
            (json!({ "id": 1, "op": "workspaces.list", "origin": "relayed" }), WorkspaceOrigin::Relayed),
            (json!({ "id": 2, "op": "workspaces.nap", "workspaceId": "ws_1", "origin": "relayed" }), WorkspaceOrigin::Relayed),
            (
                json!({ "id": 3, "op": "sessions.start", "workspaceId": "ws_1", "prompt": "go", "origin": "relayed" }),
                WorkspaceOrigin::Relayed,
            ),
            (json!({ "id": 4, "op": "workspaces.exec", "workspaceId": "ws_1", "argv": ["ls"], "origin": "here" }), WorkspaceOrigin::Here),
        ];
        for (frame, origin) in relayed {
            assert_eq!(read(frame).origin, Some(origin));
        }
        // A client on this computer names none, and nothing is added to what it sent.
        assert_eq!(read(json!({ "id": 5, "op": "workspaces.list" })).origin, None);
        let e = refuse(json!({ "id": 6, "op": "workspaces.list", "origin": "machine" }));
        assert_eq!((e.code.as_str(), e.path), ("invalid_enum_value", path!["origin"]));
    }

    // protocol.test.ts:877
    #[test]
    fn the_daemon_channel_ops_carry_a_frame_the_host_never_authenticates_for_the_page_a() {
        // The host sent the auth frame when it opened the channel; a page that could send one would pick the socket's identity.
        let e = refuse(json!({ "id": 1, "op": "daemon.send", "channel": "ch_1", "frame": { "op": "auth", "token": "t" } }));
        assert_eq!((e.code.as_str(), e.path), ("custom", path!["frame", "op"]));
        // A channel is one daemon's: a workspace of this host's, or a computer the person owns.
        let r = read(json!({ "id": 1, "op": "daemon.open", "workspaceId": "w_1" }));
        op!(r, RuntimeOp::DaemonOpen { workspace_id, .. } => assert_eq!(workspace_id.as_deref(), Some("w_1")));
        let r = read(json!({ "id": 1, "op": "daemon.open", "placeId": "p_1" }));
        op!(r, RuntimeOp::DaemonOpen { place_id, .. } => assert_eq!(place_id.as_deref(), Some("p_1")));
        lacks(json!({ "id": 1, "op": "daemon.close" }), "channel");
        // The panes are the person's: an agent inside a machine drives workspaces through the exec and session ops.
        for op in ["daemon.open", "daemon.send", "daemon.close", "workspaces.daemonReach"] {
            assert!(!THREAD_OPS.contains(&op), "{op}");
        }
    }

    // protocol.test.ts:1001
    #[test]
    fn sessions_start_carries_the_composers_model_effort_and_permission_mode_as_the_har() {
        let picked = json!({ "id": 22, "op": "sessions.start", "workspaceId": "ws_1", "prompt": "go", "model": "claude-opus-5", "effort": "high", "permissionMode": "acceptEdits", "contextWindow": "1m" });
        let r = read(picked.clone());
        op!(r, RuntimeOp::SessionsStart { model, effort, permission_mode, context_window, .. } => {
            assert_eq!(model.as_deref(), Some("claude-opus-5"));
            assert_eq!(effort.as_deref(), Some("high"));
            assert_eq!(permission_mode.as_deref(), Some("acceptEdits"));
            assert_eq!(context_window.as_deref(), Some("1m"));
        });
        let mut bare = picked.clone();
        for k in ["model", "effort", "permissionMode", "contextWindow"] {
            bare = without(bare, k);
        }
        let r = read(bare);
        op!(r, RuntimeOp::SessionsStart { model, effort, permission_mode, context_window, .. } => {
            assert_eq!((model, effort, permission_mode, context_window), (None, None, None, None));
        });
        let e = refuse(with(picked, "effort", json!(3)));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["effort"]));
        read(json!({ "id": 23, "op": "harnesses.list" }));
        let r = read(json!({ "id": 23, "op": "harnesses.list", "workspaceId": "ws_1" }));
        op!(r, RuntimeOp::HarnessesList { workspace_id, .. } => assert_eq!(workspace_id.as_deref(), Some("ws_1")));
    }

    // protocol.test.ts:1138
    #[test]
    fn sessions_steer_carries_the_message_for_the_running_turn_and_answers_one_of_four() {
        let steer = json!({ "id": 24, "op": "sessions.steer", "sessionId": "s1", "prompt": "also check the tests", "requestId": "req_2" });
        let r = read(steer.clone());
        op!(r, RuntimeOp::SessionsSteer { session_id, prompt, request_id, .. } => {
            assert_eq!(session_id, "s1");
            assert_eq!(prompt, "also check the tests");
            assert_eq!(request_id.as_deref(), Some("req_2"));
        });
        let r = read(without(steer, "requestId"));
        op!(r, RuntimeOp::SessionsSteer { request_id, .. } => assert_eq!(request_id, None));
        // prompt required
        lacks(json!({ "id": 24, "op": "sessions.steer", "sessionId": "s1" }), "prompt");
        // sessionId required
        lacks(json!({ "id": 24, "op": "sessions.steer", "prompt": "x" }), "sessionId");
    }

    // protocol.test.ts:1159
    #[test]
    fn snapshots_list_snapshots_rollback_parse_and_snapshotlineage_is_the_manifest_plus() {
        let r = read(json!({ "id": 20, "op": "snapshots.list" }));
        op!(r, RuntimeOp::SnapshotsList { name, .. } => assert_eq!(name, None));
        let r = read(json!({ "id": 21, "op": "snapshots.list", "name": "default" }));
        op!(r, RuntimeOp::SnapshotsList { name, .. } => assert_eq!(name.as_deref(), Some("default")));
        let r = read(json!({ "id": 22, "op": "snapshots.rollback", "version": 11 }));
        op!(r, RuntimeOp::SnapshotsRollback { version, .. } => assert_eq!(version, JsNumber(11.0)));
        // version required
        lacks(json!({ "id": 23, "op": "snapshots.rollback" }), "version");
    }

    // protocol.test.ts:1181
    #[test]
    fn workspaces_portreach_names_the_workspace_and_one_guest_port_portreachview_is_the() {
        let req = json!({ "id": 21, "op": "workspaces.portReach", "workspaceId": "ws_1", "port": 3000 });
        let r = read(req.clone());
        op!(r, RuntimeOp::WorkspacesPortReach { workspace_id, port, .. } => {
            assert_eq!(workspace_id, "ws_1");
            assert_eq!(port, 3000);
        });
        lacks(json!({ "id": 21, "op": "workspaces.portReach", "workspaceId": "ws_1" }), "port");
        let e = refuse(with(req.clone(), "port", json!(0)));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["port"]));
        let e = refuse(with(req.clone(), "port", json!(65536)));
        assert_eq!((e.code.as_str(), e.path), ("too_big", path!["port"]));
        let e = refuse(with(req, "port", json!(30.5)));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["port"]));
    }
}

mod thread_provenance {
    use super::*;

    // protocol.test.ts:1495
    #[test]
    fn sessions_start_takes_startedby_and_nothing_else_new() {
        let r = read(json!({ "id": 1, "op": "sessions.start", "workspaceId": "ws_1", "prompt": "go", "startedBy": "cli" }));
        op!(r, RuntimeOp::SessionsStart { started_by, .. } => assert_eq!(serde_json::to_value(&started_by).unwrap(), serde_json::to_value(&Some(json!("cli"))).unwrap()));
        let e = refuse(json!({ "id": 1, "op": "sessions.start", "workspaceId": "ws_1", "prompt": "go", "startedBy": "app" }));
        assert_eq!(e.path, path!["startedBy"]);
    }

    // protocol.test.ts:1500
    #[test]
    fn sessions_start_may_name_who_its_threads_ends_are_told_one_target_or_several_each() {
        let req = json!({ "id": 1, "op": "sessions.start", "workspaceId": "ws_1", "prompt": "build it", "notify": ["thread_parent_0001"] });
        let r = read(req.clone());
        op!(r, RuntimeOp::SessionsStart { notify, .. } => assert_eq!(notify, Some(vec!["thread_parent_0001".to_string()])));
        let r = read(with(req.clone(), "notify", json!(["me", "thread_reviewer_0001"])));
        op!(r, RuntimeOp::SessionsStart { notify, .. } => {
            assert_eq!(notify, Some(vec!["me".to_string(), "thread_reviewer_0001".to_string()]));
        });
        // One target is a list of one, never a bare string, and a list of none names nobody rather than the person.
        let e = refuse(with(req.clone(), "notify", json!("thread_parent_0001")));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["notify"]));
        let e = refuse(with(req.clone(), "notify", json!([])));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["notify"]));
        let e = refuse(with(req.clone(), "notify", json!(7)));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["notify"]));
        // The token a turn's launch carries, which is what me is read against.
        let token = "a".repeat(32);
        let r = read(with(req.clone(), "turnToken", json!(token)));
        op!(r, RuntimeOp::SessionsStart { turn_token, .. } => assert_eq!(turn_token, Some(token.clone())));
        let e = refuse(with(req, "turnToken", json!(7)));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["turnToken"]));
    }

    // protocol.test.ts:1519
    #[test]
    fn sessions_start_may_carry_the_clients_request_id_and_session_start_carries_it_bac() {
        let req = json!({ "id": 1, "op": "sessions.start", "workspaceId": "ws_1", "prompt": "go", "requestId": "req_1" });
        let r = read(req.clone());
        op!(r, RuntimeOp::SessionsStart { request_id, .. } => assert_eq!(request_id.as_deref(), Some("req_1")));
        let e = refuse(with(req, "requestId", json!(7)));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["requestId"]));
    }

    // protocol.test.ts:1545
    #[test]
    fn a_start_carries_the_attempt_it_was_sent_under_and_a_thread_reads_the_attempt_off() {
        let r = read(json!({ "id": 1, "op": "sessions.start", "workspaceId": "ws_1", "prompt": "go", "attempt": "att_1" }));
        op!(r, RuntimeOp::SessionsStart { attempt, .. } => assert_eq!(attempt.as_deref(), Some("att_1")));
    }

    // protocol.test.ts:1769
    #[test]
    fn folded_is_a_mark_a_paired_device_sends_and_the_view_carries_what_an_agents_subag() {
        let r = read(json!({ "id": 1, "op": "sessions.mark", "threadIds": ["thr_a"], "marks": { "folded": true } }));
        op!(r, RuntimeOp::SessionsMark { thread_ids, marks, .. } => {
            assert_eq!(thread_ids, vec!["thr_a".to_string()]);
            assert_eq!(serde_json::to_value(&marks).unwrap(), serde_json::to_value(&json!({ "folded": true })).unwrap());
        });
        let e = refuse(json!({ "id": 1, "op": "sessions.mark", "threadIds": ["thr_a"], "marks": { "folded": "yes" } }));
        assert_eq!(e.path, path!["marks", "folded"]);
    }

    // protocol.test.ts:1806
    #[test]
    fn the_marks_the_settle_the_restore_and_the_search_are_ops_a_paired_device_sends_a() {
        read(json!({ "id": 1, "op": "sessions.mark", "threadIds": ["thr_a"], "marks": { "pinned": true } }));
        read(json!({ "id": 1, "op": "sessions.mark", "threadIds": ["thr_a"], "marks": { "snoozedUntil": null, "section": null } }));
        read(
            json!({ "id": 1, "op": "sessions.mark", "threadIds": ["thr_a"], "marks": { "section": { "name": "working", "whileState": "running s1" } } }),
        );
        refuse(
            json!({ "id": 1, "op": "sessions.mark", "threadIds": ["thr_a"], "marks": { "section": { "name": "pinned", "whileState": "x" } } }),
        );
        let e = refuse(json!({ "id": 1, "op": "sessions.mark", "threadIds": [], "marks": { "pinned": true } }));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["threadIds"]));
        refuse(json!({ "id": 1, "op": "sessions.mark", "threadIds": ["thr_a"], "marks": { "settledAt": 3 } }));
        let r = read(json!({ "id": 1, "op": "sessions.restore", "threadIds": ["thr_a"] }));
        op!(r, RuntimeOp::SessionsRestore { thread_ids, .. } => assert_eq!(thread_ids, vec!["thr_a".to_string()]));
        let r = read(json!({ "id": 1, "op": "sessions.settle", "threadIds": ["thr_a"], "finished": true }));
        op!(r, RuntimeOp::SessionsSettle { finished, .. } => assert_eq!(finished, Some(true)));
        let r = read(json!({ "id": 1, "op": "sessions.search", "query": "canonical" }));
        op!(r, RuntimeOp::SessionsSearch { query, .. } => assert_eq!(query, "canonical"));
        for op in ["sessions.mark", "sessions.settle", "sessions.restore", "sessions.search"] {
            assert!(DEVICE_OPS.contains(&op), "{op}");
        }
        // A thread puts away its own finished children; the runtime holds its token to the threads under it.
        for op in ["sessions.search", "sessions.settle", "sessions.restore"] {
            assert!(THREAD_OPS.contains(&op), "{op}");
        }
        assert!(!THREAD_OPS.contains(&"sessions.mark"));
    }
}

mod workspaces_exec {
    use super::*;

    // protocol.test.ts:1871
    #[test]
    fn takes_a_workspace_and_the_command_as_argv_at_least_one_word_so_quoting_is_the_ru() {
        let r = read(json!({ "id": 1, "op": "workspaces.exec", "workspaceId": "ws_1", "argv": ["grep", "a b", "f"] }));
        op!(r, RuntimeOp::WorkspacesExec { argv, .. } => {
            assert_eq!(argv, vec!["grep".to_string(), "a b".to_string(), "f".to_string()]);
        });
        let e = refuse(json!({ "id": 1, "op": "workspaces.exec", "workspaceId": "ws_1", "argv": [] }));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["argv"]));
        lacks(json!({ "id": 1, "op": "workspaces.exec", "workspaceId": "ws_1", "cmd": "ls" }), "argv");
    }

    // protocol.test.ts:1877
    #[test]
    fn takes_the_folder_the_command_runs_in_as_cwd_absent_when_the_command_runs_in_the() {
        let r =
            read(json!({ "id": 1, "op": "workspaces.exec", "workspaceId": "ws_1", "argv": ["git", "status"], "cwd": "/root/work/proj" }));
        op!(r, RuntimeOp::WorkspacesExec { cwd, .. } => assert_eq!(cwd.as_deref(), Some("/root/work/proj")));
        let r = read(json!({ "id": 1, "op": "workspaces.exec", "workspaceId": "ws_1", "argv": ["git", "status"] }));
        op!(r, RuntimeOp::WorkspacesExec { cwd, .. } => assert_eq!(cwd, None));
    }
}

mod this_computers_folder_listing {
    use super::*;

    // protocol.test.ts:1906
    #[test]
    fn takes_an_ask_with_neither_the_folder_nor_the_hidden_flag_and_vouches_for_a_level() {
        let r = read(json!({ "id": "r1", "op": "host.folders" }));
        op!(r, RuntimeOp::HostFolders { dir, hidden, .. } => assert_eq!((dir, hidden), (None, None)));
        let r = read(json!({ "id": "r1", "op": "host.folders", "dir": "/Users/dev/code", "hidden": true }));
        op!(r, RuntimeOp::HostFolders { dir, hidden, .. } => {
            assert_eq!(dir.as_deref(), Some("/Users/dev/code"));
            assert_eq!(hidden, Some(true));
        });
        let e = refuse(json!({ "id": "r1", "op": "host.folders", "hidden": "yes" }));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["hidden"]));
    }
}

mod the_project_plan {
    use super::*;

    // protocol.test.ts:1929
    #[test]
    fn the_import_names_the_agents_that_travel_and_the_result_says_what_became_of_each() {
        let req = json!({ "id": "r1", "op": "project.import", "workspaceId": "ws_1", "source": "/Users/dev/proj", "dest": "/root/proj", "agents": ["claude"] });
        let r = read(req);
        op!(r, RuntimeOp::ProjectImport { workspace_id, source, dest, agents, .. } => {
            assert_eq!(workspace_id, "ws_1");
            assert_eq!(source, "/Users/dev/proj");
            assert_eq!(dest, "/root/proj");
            assert_eq!(agents, Some(vec!["claude".to_string()]));
        });
    }
}

mod the_project_export {
    use super::*;

    // protocol.test.ts:1956
    #[test]
    fn names_the_workspace_the_folder_on_the_machine_and_the_folder_here_agents_narrow() {
        let req = json!({ "id": "r1", "op": "project.export", "workspaceId": "ws_1", "source": "/root/work/proj", "dest": "/Users/dev/proj", "replace": true, "agents": ["claude", "codex"] });
        let r = read(req);
        op!(r, RuntimeOp::ProjectExport { replace, agents, .. } => {
            assert_eq!(replace, Some(true));
            assert_eq!(agents, Some(vec!["claude".to_string(), "codex".to_string()]));
        });
        let bare =
            json!({ "id": "r2", "op": "project.export", "workspaceId": "ws_1", "source": "/root/work/proj", "dest": "/Users/dev/proj" });
        let r = read(bare.clone());
        op!(r, RuntimeOp::ProjectExport { replace, agents, .. } => assert_eq!((replace, agents), (None, None)));
        lacks(without(bare, "dest"), "dest");
    }
}

mod project_goldens {
    use super::*;

    // protocol.test.ts:2006
    #[test]
    fn workspaces_snapshot_names_the_workspace_and_projectgoldens_list_takes_nothing() {
        let r = read(json!({ "id": 30, "op": "workspaces.snapshot", "workspaceId": "ws_1" }));
        op!(r, RuntimeOp::WorkspacesSnapshot { workspace_id, .. } => assert_eq!(workspace_id, "ws_1"));
        let r = read(json!({ "id": 31, "op": "projectGoldens.list" }));
        op!(r, RuntimeOp::ProjectGoldensList { .. } => ());
        lacks(json!({ "id": 32, "op": "workspaces.snapshot" }), "workspaceId");
    }

    // protocol.test.ts:2013
    #[test]
    fn projectgoldens_remove_names_the_snapshot_replies_with_the_record_and_its_line_an() {
        let r = read(json!({ "id": 33, "op": "projectGoldens.remove", "snapshotId": "snap_project-proj" }));
        op!(r, RuntimeOp::ProjectGoldensRemove { snapshot_id, .. } => assert_eq!(snapshot_id, "snap_project-proj"));
        lacks(json!({ "id": 34, "op": "projectGoldens.remove" }), "snapshotId");
        // The records of wsp's own machines, which a paired computer manages and a thread never deletes.
        assert!(DEVICE_OPS.contains(&"projectGoldens.remove"));
        assert!(!THREAD_OPS.contains(&"projectGoldens.remove"));
    }
}

mod the_newest_release_as_the_host_read_it {
    use super::*;

    // protocol.test.ts:2046
    #[test]
    fn is_asked_for_and_checked_with_no_arguments_and_rides_one_event_every_socket_fold() {
        let r = read(json!({ "id": "r1", "op": "release.get" }));
        op!(r, RuntimeOp::ReleaseGet { .. } => ());
        let r = read(json!({ "id": "r1", "op": "release.check" }));
        op!(r, RuntimeOp::ReleaseCheck { force, .. } => assert_eq!(force, None));
        for op in ["release.get", "release.check"] {
            assert!(!THREAD_OPS.contains(&op), "{op}");
        }
    }

    // protocol.test.ts:2070
    #[test]
    fn restarts_the_host_with_no_arguments_an_op_no_thread_and_no_paired_computer_sends() {
        let r = read(json!({ "id": "r1", "op": "host.restart" }));
        op!(r, RuntimeOp::HostRestart { .. } => ());
        assert!(!THREAD_OPS.contains(&"host.restart"));
        assert!(!DEVICE_OPS.contains(&"host.restart"));
    }
}

mod the_persons_terminal_config {
    use super::*;

    // protocol.test.ts:2088
    #[test]
    fn takes_an_ask_with_or_without_a_scheme_and_vouches_only_for_the_keys_the_pane_hon() {
        let r = read(json!({ "id": "r1", "op": "host.terminalConfig" }));
        op!(r, RuntimeOp::HostTerminalConfig { scheme, .. } => assert_eq!(scheme, None));
        let r = read(json!({ "id": "r1", "op": "host.terminalConfig", "scheme": "light" }));
        op!(r, RuntimeOp::HostTerminalConfig { scheme, .. } => assert_eq!(serde_json::to_value(&scheme).unwrap(), serde_json::to_value(&Some(json!("light"))).unwrap()));
        let e = refuse(json!({ "id": "r1", "op": "host.terminalConfig", "scheme": "sepia" }));
        assert_eq!(e.path, path!["scheme"]);
    }
}

mod bringing_work_back {
    use super::*;

    // protocol.test.ts:2137
    #[test]
    fn the_git_frames_parse_the_base_is_the_callers_to_leave_out_a_read_names_the_remot() {
        let r = read(json!({ "id": 7, "op": "workspaces.bringBack", "workspaceId": "ws_child", "title": "the pricing page" }));
        op!(r, RuntimeOp::WorkspacesBringBack { workspace_id, title, body, .. } => {
            assert_eq!(workspace_id, "ws_child");
            assert_eq!(title.as_deref(), Some("the pricing page"));
            assert_eq!(body, None);
        });
        // A thread's agent pushes and opens its own pull request with git and gh; the op is the person's button alone.
        assert!(!THREAD_OPS.contains(&"workspaces.bringBack"));
    }
}

mod the_doctors_computer_road_on_the_wire {
    use super::*;

    // protocol.test.ts:2229
    #[test]
    fn takes_the_computer_the_id_its_lines_ride_and_the_project_and_refuses_an_id_longe() {
        let r = read(json!({ "id": 1, "op": "places.doctor", "placeId": "p_1", "doctorId": "d_abc" }));
        op!(r, RuntimeOp::PlacesDoctor { place_id, doctor_id, project, .. } => {
            assert_eq!(place_id, "p_1");
            assert_eq!(doctor_id, "d_abc");
            assert_eq!(project, None);
        });
        let r = read(json!({ "id": 2, "op": "places.doctor", "placeId": "p_1", "doctorId": "d_abc", "project": "spoo-landing" }));
        op!(r, RuntimeOp::PlacesDoctor { project, .. } => assert_eq!(project.as_deref(), Some("spoo-landing")));
        lacks(json!({ "id": 3, "op": "places.doctor", "placeId": "p_1" }), "doctorId");
        let e = refuse(json!({ "id": 4, "op": "places.doctor", "placeId": "p_1", "doctorId": "d".repeat(65) }));
        assert_eq!((e.code.as_str(), e.path), ("too_big", path!["doctorId"]));
    }
}

mod localhost_forward_wire_shapes {
    use super::*;

    // port-forwards.test.ts:32
    #[test]
    fn forwards_list_takes_nothing_forwards_stop_names_a_workspace_and_a_bindable_port() {
        let r = read(json!({ "id": 1, "op": "forwards.list" }));
        op!(r, RuntimeOp::ForwardsList { .. } => ());
        let r = read(json!({ "id": 2, "op": "forwards.stop", "workspaceId": "ws_1", "port": 8123 }));
        op!(r, RuntimeOp::ForwardsStop { workspace_id, port, .. } => {
            assert_eq!(workspace_id, "ws_1");
            assert_eq!(serde_json::to_value(port).unwrap(), json!(8123));
        });
        lacks(json!({ "id": 3, "op": "forwards.stop", "workspaceId": "ws_1" }), "port");
        let e = refuse(json!({ "id": 4, "op": "forwards.stop", "workspaceId": "ws_1", "port": 80 }));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["port"]));
    }
}

mod the_agents_report_on_the_wire {
    use super::*;

    // agents-report.test.ts:26
    #[test]
    fn agents_read_names_a_computer_or_a_workspace_never_both_and_is_shut_to_threads_an() {
        let r = read(json!({ "id": "1", "op": "agents.read", "target": { "placeId": "here" } }));
        op!(r, RuntimeOp::AgentsRead { target, .. } => assert_eq!(serde_json::to_value(&target).unwrap(), serde_json::to_value(&json!({ "placeId": "here" })).unwrap()));
        let r = read(json!({ "id": "1", "op": "agents.read", "target": { "workspaceId": "ws_1" } }));
        op!(r, RuntimeOp::AgentsRead { target, .. } => assert_eq!(serde_json::to_value(&target).unwrap(), serde_json::to_value(&json!({ "workspaceId": "ws_1" })).unwrap()));
        let e = refuse(json!({ "id": "1", "op": "agents.read", "target": { "placeId": "here", "workspaceId": "ws_1" } }));
        assert_eq!(e.path.first(), Some(&json!("target")));
        let e = refuse(json!({ "id": "1", "op": "agents.read", "target": {} }));
        assert_eq!(e.path.first(), Some(&json!("target")));
        assert!(!THREAD_OPS.contains(&"agents.read"));
        assert!(!DEVICE_OPS.contains(&"agents.read"));
    }

    // agents-report.test.ts:35
    #[test]
    fn servers_tools_names_one_server_by_its_agent_on_a_computer_or_a_workspace_and_is() {
        let ask = json!({ "id": "1", "op": "servers.tools", "target": { "placeId": "p_spoo" }, "agent": "claude", "name": "airtable" });
        let r = read(ask.clone());
        op!(r, RuntimeOp::ServersTools { agent, name, refresh, .. } => {
            assert_eq!(agent, "claude");
            assert_eq!(name, "airtable");
            assert_eq!(refresh, None);
        });
        let r = read(with(ask.clone(), "refresh", json!(true)));
        op!(r, RuntimeOp::ServersTools { refresh, .. } => assert_eq!(refresh, Some(true)));
        lacks(without(ask, "agent"), "agent");
        assert!(!THREAD_OPS.contains(&"servers.tools"));
        assert!(!DEVICE_OPS.contains(&"servers.tools"));
    }

    // agents-report.test.ts:52
    #[test]
    fn the_sign_in_acts_name_their_target_and_agent_carry_the_code_and_the_key_in_the_f() {
        let acts = vec![
            json!({ "id": "1", "op": "agents.signIn", "target": { "placeId": "p_spoo" }, "agent": "codex" }),
            json!({ "id": "2", "op": "servers.signIn", "target": { "workspaceId": "ws_1" }, "agent": "claude", "name": "notion" }),
            json!({ "id": "3", "op": "agents.signInCode", "signInId": "si_1", "code": "ABCD-1234" }),
            json!({ "id": "4", "op": "agents.signInLine", "target": { "placeId": "here" }, "agent": "opencode" }),
            json!({ "id": "5", "op": "agents.signInLine", "target": { "placeId": "here" }, "agent": "claude", "name": "notion" }),
            json!({ "id": "6", "op": "agents.key", "agent": "claude", "key": "sk-ant-oat01-x" }),
            json!({ "id": "7", "op": "agents.addTools", "target": { "placeId": "here" }, "agent": "codex" }),
            json!({ "id": "8", "op": "agents.signInStop", "signInId": "si_1" }),
        ];
        for act in &acts {
            read(act.clone());
            let word = act["op"].as_str().unwrap();
            assert!(!THREAD_OPS.contains(&word), "{word}");
            assert!(!DEVICE_OPS.contains(&word), "{word}");
        }
        assert!(SECRET_REQUEST_FIELDS.contains(&"code"));
        assert!(SECRET_REQUEST_FIELDS.contains(&"key"));
        assert_eq!(request_secrets(&acts[2]), vec!["ABCD-1234".to_string()]);
        assert_eq!(request_secrets(&acts[5]), vec!["sk-ant-oat01-x".to_string()]);
        lacks(json!({ "id": "8", "op": "agents.signIn", "target": { "placeId": "p" } }), "agent");
    }

    // agents-report.test.ts:92
    #[test]
    fn the_skills_acts_name_a_target_and_a_skill_the_search_and_the_skills_sh_read_name() {
        let acts = vec![
            json!({ "id": "1", "op": "skills.search", "q": "pdf", "limit": 20 }),
            json!({ "id": "2", "op": "skills.get", "skill": "anthropics/skills/pdf" }),
            json!({ "id": "3", "op": "skills.preview", "target": { "placeId": "here" }, "name": "pdf" }),
            json!({ "id": "4", "op": "skills.add", "target": { "workspaceId": "ws_1" }, "skill": "anthropics/skills/pdf", "agents": ["claude"], "project": true }),
            json!({ "id": "5", "op": "skills.remove", "target": { "placeId": "p_spoo" }, "name": "pdf" }),
            json!({ "id": "6", "op": "skills.toggle", "target": { "placeId": "here" }, "name": "pdf", "project": false, "on": false }),
        ];
        for act in &acts {
            read(act.clone());
            let word = act["op"].as_str().unwrap();
            assert!(!THREAD_OPS.contains(&word), "{word}");
            assert!(!DEVICE_OPS.contains(&word), "{word}");
        }
        let e = refuse(json!({ "id": "1", "op": "skills.search", "q": "pdf", "limit": 500 }));
        assert_eq!((e.code.as_str(), e.path), ("too_big", path!["limit"]));
        lacks(json!({ "id": "6", "op": "skills.toggle", "target": { "placeId": "here" }, "name": "pdf" }), "on");
    }

    // agents-report.test.ts:110
    #[test]
    fn the_server_acts_name_a_target_an_agent_and_a_server_the_values_an_add_carries_ar() {
        let acts = vec![
            json!({ "id": "1", "op": "servers.add", "target": { "placeId": "here" }, "agent": "claude", "name": "acme", "command": "npx", "args": ["-y", "@acme/mcp"], "env": { "ACME_KEY": "sk-acme-x" } }),
            json!({ "id": "2", "op": "servers.add", "target": { "workspaceId": "ws_1" }, "agent": "codex", "name": "remote", "project": true, "url": "https://mcp.acme.example/mcp", "headers": { "Authorization": "Bearer tok-acme" } }),
            json!({ "id": "3", "op": "servers.remove", "target": { "placeId": "p_spoo" }, "agent": "claude", "name": "acme", "scope": "home" }),
            json!({ "id": "4", "op": "servers.toggle", "target": { "placeId": "here" }, "agent": "opencode", "name": "acme", "on": false }),
        ];
        for act in &acts {
            read(act.clone());
            let word = act["op"].as_str().unwrap();
            assert!(!THREAD_OPS.contains(&word), "{word}");
            assert!(!DEVICE_OPS.contains(&word), "{word}");
        }
        assert_eq!(request_secrets(&acts[0]), vec!["sk-acme-x".to_string()]);
        assert_eq!(request_secrets(&acts[1]), vec!["Bearer tok-acme".to_string()]);
        let e = refuse(with(acts[2].clone(), "scope", json!("plugin")));
        assert_eq!(e.path, path!["scope"]);
        lacks(json!({ "id": "4", "op": "servers.toggle", "target": { "placeId": "here" }, "agent": "opencode", "name": "acme" }), "on");
    }

    // agents-report.test.ts:146
    #[test]
    fn a_computers_report_names_each_project_it_read_its_project_rows_carry_theirs_a_ta() {
        let r = read(
            json!({ "id": "1", "op": "servers.tools", "target": { "placeId": "p_spoo", "project": "pr_1" }, "agent": "claude", "name": "db" }),
        );
        op!(r, RuntimeOp::ServersTools { target, .. } => assert_eq!(serde_json::to_value(&target).unwrap(), serde_json::to_value(&json!({ "placeId": "p_spoo", "project": "pr_1" })).unwrap()));
        let e = refuse(json!({ "id": "1", "op": "agents.read", "target": { "workspaceId": "ws_1", "project": "pr_1" } }));
        assert_eq!(e.path.first(), Some(&json!("target")));
    }
}

mod each_agents_plugins_in_the_report {
    use super::*;

    // agents-report.test.ts:187
    #[test]
    fn takes_plugins_toggle_with_the_agent_the_plugin_and_the_switch() {
        let r = read(
            json!({ "id": "1", "op": "plugins.toggle", "target": { "placeId": "here" }, "agent": "claude", "plugin": "vercel@claude-plugins-official", "on": false }),
        );
        op!(r, RuntimeOp::PluginsToggle { plugin, on, .. } => {
            assert_eq!(plugin, "vercel@claude-plugins-official");
            assert!(!on);
        });
    }
}

mod the_preferences_record {
    use super::*;

    // preferences.test.ts:193 (the request half; the changed event is another unit's)
    #[test]
    fn the_runtime_takes_preferences_get_and_preferences_set_with_a_patch_and_the_chang() {
        let r = read(json!({ "id": 1, "op": "preferences.get" }));
        op!(r, RuntimeOp::PreferencesGet { .. } => ());
        let r = read(json!({ "id": 1, "op": "preferences.set", "patch": { "sidebarMode": "spaces" } }));
        op!(r, RuntimeOp::PreferencesSet { patch, .. } => assert_eq!(serde_json::to_value(&patch).unwrap(), serde_json::to_value(&json!({ "sidebarMode": "spaces" })).unwrap()));
        let e = refuse(json!({ "id": 1, "op": "preferences.set", "patch": { "sidebarMode": "grid" } }));
        assert_eq!(e.path, path!["patch", "sidebarMode"]);
        lacks(json!({ "id": 1, "op": "preferences.set" }), "patch");
    }
}

mod access_words {
    use super::*;

    // thread-defaults.test.ts:221
    #[test]
    fn a_start_carries_the_word_on_the_wire_and_a_harnesss_own_spelling_is_not_one() {
        let start = json!({ "id": "1", "op": "sessions.start", "workspaceId": "w", "prompt": "hi" });
        let r = read(with(start.clone(), "access", json!("full")));
        op!(r, RuntimeOp::SessionsStart { access, .. } => assert_eq!(serde_json::to_value(&access).unwrap(), serde_json::to_value(&Some(json!("full"))).unwrap()));
        let e = refuse(with(start, "access", json!("bypassPermissions")));
        assert_eq!(e.path, path!["access"]);
        let e = refuse(json!({ "id": "1", "op": "workspaces.start", "url": "u", "access": "acceptEdits" }));
        assert_eq!(e.path, path!["access"]);
    }
}

mod an_agents_setup_on_a_computer {
    use super::*;

    // thread-defaults.test.ts:266
    #[test]
    fn the_wire_takes_a_variable_by_a_shells_name_alone() {
        let set = json!({ "id": "1", "op": "agents.setup", "placeId": "here", "agent": "claude" });
        let r = read(with(set.clone(), "env", json!({ "FOO": "bar" })));
        op!(r, RuntimeOp::AgentsSetup { env, .. } => {
            let env = env.unwrap();
            assert_eq!(env.get("FOO"), Some(&Some("bar".to_string())));
        });
        refuse(with(set.clone(), "env", json!({ "FOO BAR": "x" })));
        // The setup is strict: a field it does not name is refused rather than dropped.
        let e = refuse(with(set, "threads", json!(3)));
        assert_eq!(e.code, "unrecognized_keys");
    }
}

mod the_init_job_view {
    use super::*;

    // init-job.test.ts:163 (the request half; the init.job event is another unit's)
    #[test]
    fn the_event_rides_the_events_channel_beside_the_runtimes_own_and_the_ops_parse_as() {
        read(json!({ "id": 1, "op": "init.get" }));
        // solari is no field of init.keys: zod strips it and the request reads as the bare op.
        reads_with_as(
            parse_runtime_request,
            json!({ "id": 2, "op": "init.keys", "solari": "slr_live_x" }),
            json!({ "id": 2, "op": "init.keys" }),
        );
        let r = read(json!({ "id": 2, "op": "init.keys", "rows": { "logins/codex": "sk-x-fake" } }));
        op!(r, RuntimeOp::InitKeys { rows, .. } => {
            assert_eq!(rows.unwrap().get("logins/codex").map(String::as_str), Some("sk-x-fake"));
        });
        let r = read(json!({ "id": 3, "op": "init.start", "road": "agent", "harness": "claude" }));
        op!(r, RuntimeOp::InitStart { road, harness, .. } => {
            assert_eq!(serde_json::to_value(&road).unwrap(), serde_json::to_value(&json!("agent")).unwrap());
            assert_eq!(harness.as_deref(), Some("claude"));
        });
        let r = read(json!({ "id": 4, "op": "init.answer", "screen": "tools", "ticks": ["gh"], "answers": {} }));
        op!(r, RuntimeOp::InitAnswer { screen, ticks, answers, .. } => {
            assert_eq!(serde_json::to_value(&screen).unwrap(), serde_json::to_value(&json!("tools")).unwrap());
            assert_eq!(ticks, Some(vec!["gh".to_string()]));
            assert!(answers.unwrap().is_empty());
        });
        let r = read(json!({ "id": 4, "op": "init.step", "at": 2 }));
        op!(r, RuntimeOp::InitStep { at, .. } => assert_eq!(at, 2));
        let r = read(json!({ "id": 5, "op": "init.build", "firstWorkspace": "first", "importFolder": "/Users/me/proj" }));
        op!(r, RuntimeOp::InitBuild { first_workspace, import_folder, .. } => {
            assert_eq!(first_workspace.as_deref(), Some("first"));
            assert_eq!(import_folder.as_deref(), Some("/Users/me/proj"));
        });
        let r = read(json!({ "id": 6, "op": "init.cancel" }));
        op!(r, RuntimeOp::InitCancel { .. } => ());
        let r = read(json!({ "id": 7, "op": "init.signInCode", "tool": "gcloud", "code": "4/0Afake" }));
        op!(r, RuntimeOp::InitSignInCode { tool, code, .. } => {
            assert_eq!(tool, "gcloud");
            assert_eq!(code, "4/0Afake");
        });
        let e = refuse(json!({ "id": 7, "op": "init.start", "road": "wizard" }));
        assert_eq!(e.path, path!["road"]);
        let e = refuse(json!({ "id": 8, "op": "init.answer", "screen": "build" }));
        assert_eq!(e.path, path!["screen"]);
        let e = refuse(json!({ "id": 9, "op": "init.step", "at": -1 }));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["at"]));
        // A code is one line typed into a terminal on the machine, so the wire takes neither an empty one nor a flood.
        let e = refuse(json!({ "id": 10, "op": "init.signInCode", "tool": "gcloud", "code": "" }));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["code"]));
        let e = refuse(json!({ "id": 11, "op": "init.signInCode", "tool": "gcloud", "code": "x".repeat(257) }));
        assert_eq!((e.code.as_str(), e.path), ("too_big", path!["code"]));
    }
}

mod the_credential_contracts {
    use super::*;

    // broker-contract.test.ts:73
    #[test]
    fn the_three_runtime_ops_parse_none_on_a_threads_token_and_only_broker_status_on_a() {
        let r = read(json!({ "id": 1, "op": "providers.checkKey", "provider": "boat", "key": "k" }));
        op!(r, RuntimeOp::ProvidersCheckKey { provider, key, .. } => assert_eq!((provider.as_str(), key.as_str()), ("boat", "k")));
        let r = read(json!({ "id": 2, "op": "agents.cloudSignIn", "agent": "codex" }));
        op!(r, RuntimeOp::AgentsCloudSignIn { .. } => ());
        let r = read(json!({ "id": 3, "op": "broker.status" }));
        op!(r, RuntimeOp::BrokerStatus { .. } => ());
        let r = read(json!({ "id": 4, "op": "agents.cloudSignIn", "agent": "claude" }));
        op!(r, RuntimeOp::AgentsCloudSignIn { agent, .. } => assert_eq!(serde_json::to_value(agent).unwrap(), json!("claude")));
        let e = refuse(json!({ "id": 4, "op": "agents.cloudSignIn", "agent": "gemini" }));
        assert_eq!((e.code.as_str(), e.path), ("invalid_enum_value", path!["agent"]));
        let e = refuse(json!({ "id": 5, "op": "providers.checkKey", "provider": "boat", "key": "" }));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["key"]));
        for op in ["providers.checkKey", "agents.cloudSignIn", "broker.status"] {
            assert!(RUNTIME_OPS.contains(&op), "{op}");
            assert!(!THREAD_OPS.contains(&op), "{op}");
            assert_eq!(DEVICE_OPS.contains(&op), op == "broker.status", "{op}");
        }
    }
}

// The refusals the door answers, as zod writes their first issue, each printed by the TS module at a3b870a6e.
mod refusals_as_zod_writes_them {
    use super::*;

    #[test]
    fn an_unknown_op_is_one_discriminator_issue_naming_all_211_ops_in_order() {
        let e = refused_with(parse_runtime_request, json!({ "id": 1, "op": "nope" }));
        assert_eq!((e.code.as_str(), e.path.clone()), ("invalid_union_discriminator", path!["op"]));
        let options: Vec<String> = RUNTIME_OPS.iter().map(|s| s.to_string()).collect();
        assert_eq!(e.detail["options"], json!(options));
        assert_eq!((RUNTIME_OPS[0], RUNTIME_OPS[210]), ("auth", "slates.image"));
        // The op's issue comes first even when origin is wrong too.
        let e = refused_with(parse_runtime_request, json!({ "op": "nope", "origin": "x" }));
        assert_eq!(e.code, "invalid_union_discriminator");
    }

    #[test]
    fn a_missing_id_a_bad_origin_and_a_bare_value() {
        let e = refused_with(parse_runtime_request, json!({ "op": "workspaces.list" }));
        assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_union", path!["id"], "Invalid input"));
        let e = refused_with(parse_runtime_request, json!({ "id": 1, "op": "workspaces.list", "origin": "x" }));
        assert_eq!((e.code.as_str(), e.path.clone()), ("invalid_enum_value", path!["origin"]));
        assert_eq!(e.message, "Invalid enum value. Expected 'here' | 'relayed' | 'paired', received 'x'");
        let e = refused_with(parse_runtime_request, json!(5));
        assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_type", path![], "Expected object, received number"));
    }

    // Unknown keys are dropped (rule 4.14), and the op and id read back.
    #[test]
    fn an_unknown_key_is_dropped_and_the_op_and_id_read_back() {
        let r = reads_with_as(
            parse_runtime_request,
            json!({ "id": 1, "op": "workspaces.list", "junk": 1 }),
            json!({ "id": 1, "op": "workspaces.list" }),
        );
        assert_eq!(r.op.name(), "workspaces.list");
        assert_eq!(serde_json::to_value(r.op.id()).unwrap(), json!(1));
        assert!(r.origin.is_none());
        let r = reads_with(parse_runtime_request, json!({ "id": "a-1", "op": "places.list", "origin": "paired" }));
        assert_eq!((r.op.name(), r.origin), ("places.list", Some(WorkspaceOrigin::Paired)));
        // Every op the list names reads back the name it was read by.
        for op in ["auth", "seal.open", "place.prove", "slates.image", "sessions.start"] {
            assert!(RUNTIME_OPS.contains(&op));
        }
    }
}
