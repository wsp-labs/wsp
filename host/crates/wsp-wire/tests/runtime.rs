// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/{protocol,exit,redact,broker-contract}.test.ts and slate/wire.test.ts: runtime (the op lists, results, responses and request_secrets; the RuntimeRequest union is another file's).
mod common;
use common::*;
use serde_json::{json, Value};
use wsp_wire::wire::runtime::{
    parse_runtime_request, request_secrets, BrokerStatus, BrokerStatusClaude, BrokerStatusCodex, ExecEvent, ProviderKeyWorks,
    RuntimeErrorResponse, RuntimeOp, RuntimeResponse, SessionInterruptOutcome, SessionInterruptResult, SessionStartOutcome,
    SessionStartResult, DEVICE_OPS, RUNTIME_OPS, THREAD_OPS,
};

mod event_replay_wire_fields {
    use super::*;

    // protocol.test.ts:700
    #[test]
    fn the_subscribe_reply_names_the_runtime_s_head_and_stream_and_when_the_cursor_is_g() {
        // Only the response half: the reply's own shape is the events file's.
        let r: RuntimeResponse = reads(json!({ "id": 1, "ok": true, "seq": 7, "gap": true }));
        match r {
            RuntimeResponse::V0(ok) => {
                assert_eq!(ok.rest["seq"], 7);
                assert_eq!(ok.rest["gap"], true);
            }
            other => panic!("{other:?}"),
        }
    }
}

mod runtime_wire_types {
    use super::*;

    // protocol.test.ts:908
    #[test]
    fn a_conversation_kept_outside_wsp_is_read_for_the_picker_by_the_app_alone_no_verb() {
        assert!(!THREAD_OPS.contains(&"conversations.read"));
        assert!(DEVICE_OPS.contains(&"conversations.read"));
        // The op is one the host reads, so a thread's refusal is the list's and not an unknown op's.
        let r = reads_with(parse_runtime_request, json!({ "id": 1, "op": "conversations.read", "agent": "claude", "conversation": "c1" }));
        assert_eq!(r.op.name(), "conversations.read");
        assert!(RUNTIME_OPS.contains(&"conversations.read"));
    }

    // protocol.test.ts:913
    #[test]
    fn the_device_list_holds_only_ops_the_host_serves_none_of_the_ops_that_start_a_proc() {
        for op in DEVICE_OPS {
            assert!(RUNTIME_OPS.contains(op), "{op}");
        }
        let mut seen = std::collections::HashSet::new();
        assert!(DEVICE_OPS.iter().all(|op| seen.insert(*op)), "DEVICE_OPS holds a duplicate");
        let held: &[&str] = &[
            "sessions.start",
            "sessions.warm",
            "sessions.steer",
            "sessions.interrupt",
            "sessions.rename",
            "sessions.answer",
            "sessions.access",
            "sessions.aside",
            "sessions.rewind",
            "sessions.run",
            "workspaces.exec",
            "workspaces.bringBack",
            "daemon.open",
            "daemon.send",
            "daemon.close",
            "places.add",
            "places.setup",
            "places.choose",
            "places.update",
            "places.holds",
            "places.remove",
            "places.dial",
            "places.set",
            "places.follow",
            "places.skip",
            "places.setupLog",
            "places.estimate",
            "places.loginLanded",
            "places.doctor",
            "places.door",
            "places.mint",
            "places.sshHosts",
            "projects.add",
            "init.keys",
            "image.build",
            "golden.prepare",
            "golden.seal",
            "host.folders",
            "agents.read",
            "servers.tools",
            "servers.icon",
            "agents.signIn",
            "servers.signIn",
            "agents.signInCode",
            "agents.signIns",
            "agents.signInStop",
            "agents.signInLine",
            "agents.key",
            "agents.addTools",
            "agents.setup",
            "skills.search",
            "skills.get",
            "skills.preview",
            "skills.add",
            "skills.remove",
            "skills.toggle",
            "servers.add",
            "servers.remove",
            "servers.toggle",
            "plugins.toggle",
            "project.seed.plan",
            "projects.seed",
            "project.plan",
            "project.import",
            "project.export",
            "pair.issue",
            "pair.redeem",
            "seal.open",
            "device.auth",
            "place.join",
            "place.auth",
            "place.prove",
            "host.restart",
            "editor.list",
            "editor.open",
            "ssh.port",
            "ssh.include",
            "workspaces.commit",
            "workspaces.commitDraft",
            "workspaces.discard",
            "workspaces.fix",
            "workspaces.update",
            "workspaces.merge",
            "workspaces.mergeIn",
            "workspaces.pullRequestSend",
            "workspaces.pullRequestReply",
            "workspaces.pullRequestResolve",
            "workspaces.pullRequestReact",
            "workspaces.start",
            "workspaces.review",
            "workspaces.reviewPost",
            "worktree.make",
            "worktree.remove",
            "sessions.delete",
            "usage.reset",
            "workspaces.resize",
            "recipes.list",
            "recipes.get",
            "recipes.save",
            "recipes.remove",
            "recipes.options",
            "slates.write",
            "slates.read",
            "slates.event",
            "slates.approve",
            "slates.cancel",
            "slates.revoke",
            "appProcs.watch",
            "appProcs.unwatch",
            "appProcs.offer",
            "appProcs.set",
            "activity.read",
            "activity.end",
            "providers.checkKey",
            "agents.cloudSignIn",
        ];
        for op in held {
            assert!(RUNTIME_OPS.contains(op), "{op}");
            assert!(!DEVICE_OPS.contains(op), "{op}");
        }
        // Every op the host serves is on one side or the other, so an op added later is placed on purpose.
        for op in RUNTIME_OPS {
            assert!(DEVICE_OPS.contains(op) || held.contains(op), "{op}");
        }
        // The newest release is read by every window the person has, their phone's included; a restart is not a phone's.
        for op in ["status.list", "workspaces.create", "release.get", "release.check"] {
            assert!(DEVICE_OPS.contains(&op), "{op}");
        }
        // A cap is the person's number on their own computer; no thread and no paired device sets it.
        assert!(!THREAD_OPS.contains(&"places.set"));
        assert!(!THREAD_OPS.contains(&"places.follow"));
        let r = reads_with(parse_runtime_request, json!({ "id": 1, "op": "places.set", "placeId": "p_1", "threads": 1 }));
        match r.op {
            RuntimeOp::PlacesSet { threads, .. } => assert_eq!(threads, Some(1.into())),
            other => panic!("{other:?}"),
        }
        let r = reads_with(parse_runtime_request, json!({ "id": 1, "op": "places.set", "placeId": "solari", "spendPerDayUsd": 0 }));
        match r.op {
            RuntimeOp::PlacesSet { spend_per_day_usd, .. } => assert_eq!(serde_json::to_value(spend_per_day_usd).unwrap(), 0),
            other => panic!("{other:?}"),
        }
        let e = refused_with(parse_runtime_request, json!({ "id": 1, "op": "places.set", "placeId": "p_1", "threads": 0 }));
        assert_eq!(e.path, path!["threads"]);
        assert_eq!(e.code, "too_small");
        let e = refused_with(parse_runtime_request, json!({ "id": 1, "op": "places.set", "placeId": "p_1", "machines": 1.5 }));
        assert_eq!(e.path, path!["machines"]);
        assert_eq!(e.code, "invalid_type");
        let e = refused_with(parse_runtime_request, json!({ "id": 1, "op": "places.set", "placeId": "p_1", "reset": ["everything"] }));
        assert_eq!(e.path, path!["reset", 0]);
        assert_eq!(e.code, "invalid_enum_value");
    }

    // protocol.test.ts:1131
    #[test]
    fn sessions_interrupt_answers_one_of_three_outcomes_none_of_them_an_error_reply() {
        for (outcome, want) in [
            ("accepted", SessionInterruptOutcome::Accepted),
            ("not-running", SessionInterruptOutcome::NotRunning),
            ("not-found", SessionInterruptOutcome::NotFound),
        ] {
            let r: SessionInterruptResult = reads(json!({ "outcome": outcome }));
            assert_eq!(r.outcome, want);
        }
        let e = refused::<SessionInterruptResult>(json!({ "outcome": "stopped" }));
        assert_eq!(e.path, path!["outcome"]);
        assert_eq!(e.code, "invalid_enum_value");
        let e = refused::<SessionInterruptResult>(json!({}));
        assert_eq!(e.path, path!["outcome"]);
        assert_eq!(e.code, "invalid_type");
        let r: RuntimeResponse = reads(json!({ "id": 21, "ok": true, "outcome": "not-running" }));
        match r {
            RuntimeResponse::V0(ok) => assert_eq!(ok.rest["outcome"], "not-running"),
            other => panic!("{other:?}"),
        }
    }

    // protocol.test.ts:1150
    #[test]
    fn sessions_start_answers_the_session_and_how_the_start_went_started_steered_into_t() {
        let session = json!({ "id": "s1", "workspaceId": "ws_1", "harness": "claude", "status": "running" });
        for (outcome, want) in
            [("started", SessionStartOutcome::Started), ("steered", SessionStartOutcome::Steered), ("queued", SessionStartOutcome::Queued)]
        {
            let r: SessionStartResult = reads(json!({ "session": session, "outcome": outcome, "turnId": "t1" }));
            assert_eq!(r.outcome, want);
            assert_eq!(r.turn_id, "t1");
            assert_eq!(serde_json::to_value(&r.session).unwrap(), session);
        }
        let e = refused::<SessionStartResult>(json!({ "session": session, "outcome": "accepted", "turnId": "t1" }));
        assert_eq!(e.path, path!["outcome"]);
        assert_eq!(e.code, "invalid_enum_value");
        let e = refused::<SessionStartResult>(json!({ "session": session, "turnId": "t1" }));
        assert_eq!(e.path, path!["outcome"]);
        assert_eq!(e.code, "invalid_type");
        let e = refused::<SessionStartResult>(json!({ "session": session, "outcome": "started" }));
        assert_eq!(e.path, path!["turnId"]);
        assert_eq!(e.code, "invalid_type");
        let e = refused::<SessionStartResult>(json!({ "outcome": "started", "turnId": "t1" }));
        assert_eq!(e.path, path!["session"]);
        assert_eq!(e.code, "invalid_type");
    }
}

mod golden_wire_schemas {
    use super::*;

    // protocol.test.ts:1226
    #[test]
    fn error_replies_may_carry_a_typed_kind() {
        let r: RuntimeErrorResponse = reads(json!({ "id": 1, "ok": false, "error": "refused", "kind": "notFirstLife" }));
        assert_eq!(r.kind.as_deref(), Some("notFirstLife"));
        assert_eq!(r.error, "refused");
    }
}

mod workspaces_exec {
    use super::*;

    // protocol.test.ts:1882
    #[test]
    fn pushes_output_lines_and_one_exit_whose_code_is_null_with_a_reason_when_the_comma() {
        let r: ExecEvent = reads(json!({ "type": "exec.output", "execId": "e1", "text": "hello" }));
        match r {
            ExecEvent::ExecOutput { exec_id, text } => assert_eq!((exec_id.as_str(), text.as_str()), ("e1", "hello")),
            other => panic!("{other:?}"),
        }
        let r: ExecEvent = reads(json!({ "type": "exec.exit", "execId": "e1", "exitCode": 0 }));
        match r {
            ExecEvent::ExecExit { exit_code, error, .. } => assert_eq!((exit_code, error), (Some(0.into()), None)),
            other => panic!("{other:?}"),
        }
        let r: ExecEvent = reads(json!({ "type": "exec.exit", "execId": "e1", "exitCode": null, "error": "deadline" }));
        match r {
            ExecEvent::ExecExit { exit_code, error, .. } => assert_eq!((exit_code, error.as_deref()), (None, Some("deadline"))),
            other => panic!("{other:?}"),
        }
        let e = refused::<ExecEvent>(json!({ "type": "exec.exit", "execId": "e1", "exitCode": 1.5 }));
        assert_eq!(e.path, path!["exitCode"]);
        assert_eq!(e.code, "invalid_type");
    }
}

mod the_exit_code_every_wsp_verb_answers_with {
    use super::*;

    // exit.test.ts:48
    #[test]
    fn the_refused_frame_declares_the_fix_so_a_client_that_parses_the_wire_keeps_it() {
        let frame =
            json!({ "id": 1, "ok": false, "error": "spoo names no user. Type user@spoo.", "kind": "invalid", "fix": "Type user@spoo." });
        let r: RuntimeResponse = reads(frame);
        match r {
            RuntimeResponse::V1(e) => {
                assert_eq!(e.fix.as_deref(), Some("Type user@spoo."));
                assert_eq!(e.kind.as_deref(), Some("invalid"));
                assert_eq!(e.error, "spoo names no user. Type user@spoo.");
            }
            other => panic!("{other:?}"),
        }
    }
}

mod request_secrets {
    use super::*;

    // redact.test.ts:25
    #[test]
    fn reads_the_secret_fields_a_request_carries_one_level_into_a_record_and_nothing_el() {
        for field in ["token", "key", "rows", "code", "passphrase", "env", "envs"] {
            assert!(wsp_wire::wire::runtime::SECRET_REQUEST_FIELDS.contains(&field), "{field}");
        }
        let frame = json!({ "id": 1, "op": "init.keys", "provider": "box", "key": "k1", "env": { "A": "e1" } });
        let mut got = request_secrets(&frame);
        got.sort();
        assert_eq!(got, vec!["e1", "k1"]);
        assert_eq!(request_secrets(&Value::String("not a frame".into())), Vec::<String>::new());
    }

    // redact.test.ts:32
    #[test]
    fn reads_the_sudo_password_an_add_carries_so_a_refusal_that_ever_said_it_is_redacte() {
        // The redaction itself is wsp-words'; this reads the secret the redaction is given.
        let frame = json!({ "id": 2, "op": "places.add", "address": "maya@box", "sudoPassword": "Tq-not-a-real-pw" });
        assert_eq!(request_secrets(&frame), vec!["Tq-not-a-real-pw"]);
    }
}

mod the_slate_s_wire {
    use super::*;

    // slate/wire.test.ts:6
    #[test]
    fn takes_every_op_slate_ops_declares_so_an_op_added_there_alone_is_not_refused_as_u() {
        // SLATE_OPS is the slate unit's; its keys, in its order.
        let slate_ops = [
            "slates.get",
            "slates.write",
            "slates.state",
            "slates.read",
            "slates.catalog",
            "slates.event",
            "slates.approve",
            "slates.cancel",
            "slates.revoke",
            "slates.shown",
            "slates.subscribe",
            "slates.unsubscribe",
            "slates.resolve",
            "slates.image",
        ];
        for op in slate_ops {
            assert!(RUNTIME_OPS.contains(&op), "{op}");
        }
        // And the union reads one of them, so the list is not all the test stands on.
        let r = reads_with(parse_runtime_request, json!({ "id": 1, "op": "slates.get", "threadId": "t1" }));
        assert_eq!(r.op.name(), "slates.get");
    }
}

mod the_credential_contracts {
    use super::*;

    // broker-contract.test.ts:87
    #[test]
    fn the_answers_of_checkkey_and_broker_status_parse() {
        let r: ProviderKeyWorks = reads(json!({ "works": true, "account": "acme", "plan": "team", "running": 2, "limit": 10 }));
        assert_eq!(r.limit, 10.into());
        let e = refused::<ProviderKeyWorks>(json!({ "works": false, "account": "acme", "plan": "team", "running": 2, "limit": 10 }));
        assert_eq!(e.path, path!["works"]);
        assert_eq!(e.code, "invalid_literal");
        let r: BrokerStatus = reads(json!({ "claude": "token", "codex": "access-token", "github": "octocat" }));
        assert_eq!(
            (r.claude, r.codex, r.github.as_deref()),
            (Some(BrokerStatusClaude::Token), Some(BrokerStatusCodex::AccessToken), Some("octocat"))
        );
        let r: BrokerStatus = reads(json!({ "claude": "login" }));
        assert_eq!(r.claude, Some(BrokerStatusClaude::Login));
        let r: BrokerStatus = reads(json!({}));
        assert_eq!((r.claude, r.codex, r.github), (None, None, None));
        let e = refused::<BrokerStatus>(json!({ "claude": "keychain" }));
        assert_eq!(e.path, path!["claude"]);
        assert_eq!(e.code, "invalid_enum_value");
    }
}

// Edges no TS test pins, each printed by the TS module at a3b870a6e.
mod edges_the_ts_module_printed {
    use super::*;
    use wsp_wire::wire::runtime::{asker_of, task_stop_refused_line, task_stop_unsupported_line, workspace_id_of};

    // runtime.ts:1037-1043: the fields in SECRET_REQUEST_FIELDS' order; a string as itself; an object or array one
    // level down, its string values only, in order; nothing deeper; an empty string is still a secret.
    #[test]
    fn request_secrets_reads_one_level_in_the_fields_order() {
        let frame = json!({ "token": "a", "key": { "x": "b", "y": 3, "z": { "w": "c" } }, "rows": ["d", "e"], "env": null, "code": 5, "headers": { "h": "f" }, "passphrase": "", "other": "z" });
        assert_eq!(request_secrets(&frame), vec!["a", "b", "d", "e", "", "f"]);
        for not_a_frame in [json!("x"), Value::Null, json!([1]), json!(5)] {
            assert!(request_secrets(&not_a_frame).is_empty(), "{not_a_frame}");
        }
        assert_eq!(request_secrets(&json!({ "sudoPassword": "pw", "envs": { "A": "1" } })), vec!["1", "pw"]);
    }

    // runtime.ts:1254-1258: the first string of the event's workspaceId, workspace.id, status.id, forward.workspaceId.
    #[test]
    fn workspace_id_of_reads_the_first_string_in_its_order() {
        assert_eq!(workspace_id_of(&json!({ "workspaceId": 5, "workspace": { "id": "a" } })).as_deref(), Some("a"));
        assert_eq!(workspace_id_of(&json!({ "status": { "id": "s" }, "forward": { "workspaceId": "f" } })).as_deref(), Some("s"));
        assert_eq!(workspace_id_of(&json!({ "forward": { "workspaceId": "f" } })).as_deref(), Some("f"));
        assert_eq!(workspace_id_of(&json!({})), None);
        assert_eq!(workspace_id_of(&Value::Null), None);
    }

    // runtime.ts:1262-1266: both ids strings, or none; nothing else of askedBy is kept.
    #[test]
    fn asker_of_needs_both_ids() {
        let asker = asker_of(&json!({ "askedBy": { "threadId": "t", "rootThreadId": "r", "x": 1 } })).unwrap();
        assert_eq!((asker.thread_id.as_str(), asker.root_thread_id.as_str()), ("t", "r"));
        assert!(asker_of(&json!({ "askedBy": { "threadId": "t" } })).is_none());
        assert!(asker_of(&json!({})).is_none());
    }

    #[test]
    fn the_task_stop_lines() {
        assert_eq!(task_stop_refused_line("claude", "busy"), "claude would not stop it: busy");
        assert_eq!(task_stop_unsupported_line("codex"), "Stop is not available for codex subagents; stop the thread to stop them all");
    }

    // The three lists hold 199, 50 and 84 ops; every thread op and device op is one the host serves.
    #[test]
    fn the_op_lists_are_the_counts_the_door_reads() {
        parse_runtime_request(&json!({ "id": 1, "op": "auth", "token": "t" })).unwrap();
        assert_eq!((RUNTIME_OPS.len(), THREAD_OPS.len(), DEVICE_OPS.len()), (199, 50, 84));
        for op in THREAD_OPS.iter().chain(DEVICE_OPS) {
            assert!(RUNTIME_OPS.contains(op), "{op}");
        }
    }
}
