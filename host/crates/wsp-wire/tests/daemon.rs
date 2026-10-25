// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/protocol.test.ts, broker-contract.test.ts, callback-relay.test.ts and
//! pty-mode.test.ts, and packages/daemon/test/daemon-ws.test.ts: daemon.
mod common;
use common::*;
use serde_json::{json, Value};
use wsp_wire::wire::daemon::*;
use wsp_wire::wire::daemon_events::{parse_daemon_event, DaemonEvent};
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_wire::wire::machine_link::{DaemonErrorCode, DaemonErrorResponse, DaemonResponse};

fn with(v: &Value, key: &str, val: Value) -> Value {
    let mut out = v.clone();
    out.as_object_mut().unwrap().insert(key.to_string(), val);
    out
}

fn parsed(frame: &Value) -> DaemonRequest {
    parse_daemon_request(frame).unwrap_or_else(|e| panic!("refused {frame}: {e:?}"))
}

mod daemon_wire_types_one_home_for_the_ops_the_daemon_answers {
    use super::*;

    // protocol.test.ts:712
    // The push events of this case are the daemon events' own reader; only the requests and responses are here.
    #[test]
    fn parses_requests_responses_and_push_events() {
        let reqs = [
            json!({"id": 1, "op": "pty.create", "cols": 80, "rows": 24, "shell": "bash"}),
            json!({"id": 2, "op": "pty.attach", "ptyId": "p1"}),
            json!({"id": 3, "op": "pty.write", "ptyId": "p1", "data": "ls\n"}),
            json!({"id": 4, "op": "pty.resize", "ptyId": "p1", "cols": 100, "rows": 30}),
            json!({"id": 5, "op": "pty.kill", "ptyId": "p1"}),
            json!({"id": 6, "op": "pty.list"}),
            json!({"id": 7, "op": "ports.watch"}),
            json!({"id": 8, "op": "manifest.get"}),
            json!({"id": 9, "op": "manifest.record", "cmd": "pnpm dev", "cwd": "/root/app", "port": 3000}),
            json!({"id": 10, "op": "manifest.restartScript"}),
            json!({"id": 11, "op": "inbox.watch"}),
            json!({"id": 12, "op": "inbox.rescan"}),
            json!({"id": 13, "op": "ping"}),
        ];
        for r in reqs {
            let got = reads_with(parse_daemon_request, r.clone());
            assert_eq!(serde_json::to_value(&got).unwrap()["op"], r["op"], "{r}");
        }
        let e = refused_with(parse_daemon_request, json!({"id": 1, "op": "pty.explode"}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_union_discriminator", path!["op"]));

        match reads_with(parse_daemon_response, json!({"id": 1, "ok": true, "ptyId": "p1", "pid": 42})) {
            DaemonResponse::V0(ok) => assert_eq!(ok.rest.get("ptyId"), Some(&json!("p1"))),
            other => panic!("{other:?}"),
        }
        match reads_with(parse_daemon_response, json!({"id": 1, "ok": false, "error": "no such pty"})) {
            DaemonResponse::V1(err) => assert_eq!(err.error, "no such pty"),
            other => panic!("{other:?}"),
        }
    }

    // protocol.test.ts:746
    // The runtime's workspaces.updateDaemon refusal at the end of this case is the runtime writer's.
    #[test]
    fn the_hello_carries_the_daemon_s_version_and_one_without_is_refused() {
        assert!(DAEMON_VERSION >= 2);
        let current = reads_with(parse_daemon_event, json!({"type": "daemon.hello", "root": "/root", "version": DAEMON_VERSION}));
        match current {
            DaemonEvent::DaemonHello { root, version } => assert_eq!((root.as_str(), version), ("/root", DAEMON_VERSION)),
            other => panic!("{other:?}"),
        }
        let e = refused_with(parse_daemon_event, json!({"type": "daemon.hello", "root": "/root"}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["version"]));
        let e = refused_with(parse_daemon_event, json!({"type": "daemon.hello", "root": "/root", "version": "2"}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["version"]));
    }
}

mod daemon_auth_frame {
    use super::*;

    // protocol.test.ts:1232
    #[test]
    fn carries_the_token_and_for_a_socket_meant_for_one_guest_port_that_port() {
        let plain: DaemonAuthRequest = reads(json!({"id": 1, "op": "auth", "token": "t"}));
        assert_eq!((plain.token.as_str(), plain.port), ("t", None));
        let one_port: DaemonAuthRequest = reads(json!({"id": 1, "op": "auth", "token": "t", "port": 3000}));
        assert_eq!(one_port.port, Some(3000));
        let e = refused::<DaemonAuthRequest>(json!({"id": 1, "op": "auth", "token": "t", "port": 0}));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["port"]));
        let e = refused::<DaemonAuthRequest>(json!({"id": 1, "op": "auth", "token": "t", "port": 65536}));
        assert_eq!((e.code.as_str(), e.path), ("too_big", path!["port"]));
        let e = refused::<DaemonAuthRequest>(json!({"id": 1, "op": "auth", "token": "t", "port": "3000"}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["port"]));
        let code: DaemonErrorCode = reads(json!("forbidden"));
        assert_eq!(code, DaemonErrorCode::Forbidden);
    }
}

mod daemon_files_and_diff_ops {
    use super::*;

    // protocol.test.ts:1243
    #[test]
    fn parses_the_four_requests_and_refuses_a_bad_scope_or_encoding() {
        let reqs = [
            json!({"id": 1, "op": "fs.list", "path": "src", "gitignore": true}),
            json!({"id": 2, "op": "fs.read", "path": "src/index.ts", "encoding": "base64"}),
            json!({"id": 3, "op": "git.status", "cwd": "."}),
            json!({"id": 4, "op": "git.diff", "cwd": ".", "scope": "branch", "path": "src"}),
        ];
        for r in reqs {
            reads_with(parse_daemon_request, r);
        }
        let e = refused_with(parse_daemon_request, json!({"id": 5, "op": "git.diff", "cwd": ".", "scope": "all"}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_enum_value", path!["scope"]));
        let e = refused_with(parse_daemon_request, json!({"id": 6, "op": "fs.read", "path": "x", "encoding": "hex"}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_enum_value", path!["encoding"]));
        let e = refused_with(parse_daemon_request, json!({"id": 7, "op": "fs.list", "path": 7}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["path"]));
    }

    // protocol.test.ts:1256
    #[test]
    fn takes_the_workspace_a_files_or_git_frame_is_for_on_those_seven_ops_and_refuses_one() {
        // A workspace on a computer somebody owns runs no daemon of its own, so the daemon of the computer holding it
        // answers these seven for the workspace the frame names; the path is then the one that workspace sees.
        // The path is the checkout's own, absolute: the daemon answering for a workspace has no working directory
        // inside it, and it refuses a relative path rather than reading one against its own.
        let named = [
            json!({"id": 1, "op": "fs.list", "path": "/root/repo", "machineId": "wsp-a"}),
            json!({"id": 2, "op": "fs.read", "path": "/root/repo/README.md", "machineId": "wsp-a"}),
            json!({"id": 3, "op": "git.status", "cwd": "/root/repo", "machineId": "wsp-a"}),
            json!({"id": 4, "op": "git.diff", "cwd": "/root/repo", "scope": "branch", "machineId": "wsp-a"}),
            json!({"id": 5, "op": "git.push", "cwd": "/root/repo", "base": "main", "machineId": "wsp-a"}),
            json!({"id": 6, "op": "git.pr", "cwd": "/root/repo", "base": "main", "machineId": "wsp-a"}),
            json!({"id": 7, "op": "git.update", "cwd": "/root/repo", "base": "main", "machineId": "wsp-a"}),
        ];
        for r in &named {
            reads_with(parse_daemon_request, r.clone());
        }
        for r in &named {
            let e = refused_with(parse_daemon_request, with(r, "machineId", json!(1)));
            assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["machineId"]), "{r}");
        }
        // And no other op of this road carries one: an exec on a workspace has its own road, over the machine ops.
        let exec = reads_with_as(
            parse_daemon_request,
            json!({"id": 8, "op": "exec", "cmd": "true", "machineId": "wsp-a"}),
            json!({"id": 8, "op": "exec", "cmd": "true"}),
        );
        match exec {
            DaemonRequest::Exec(req) => assert_eq!(req.cmd, "true"),
            other => panic!("{other:?}"),
        }
    }

    // protocol.test.ts:1276
    #[test]
    fn parses_the_typed_replies() {
        let list = json!({"entries": [{"name": "a.ts", "type": "file", "size": 12, "mtime": 1_700_000_000_000_i64}], "truncated": false, "total": 1});
        let got: FsListReply = reads(list.clone());
        assert_eq!((got.entries.len(), got.total, got.truncated), (1, 1, false));
        let e = refused::<FsListReply>(
            json!({"entries": [with(&list["entries"][0], "type", json!("socket"))], "truncated": false, "total": 1}),
        );
        assert_eq!((e.code.as_str(), e.path), ("invalid_enum_value", path!["entries", 0, "type"]));
        let e = refused::<FsListReply>(json!({"entries": list["entries"], "truncated": false}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["total"]));
        let read: FsReadReply = reads(json!({"content": "aGk=", "size": 2, "truncated": false}));
        assert_eq!((read.content.as_str(), read.truncated), ("aGk=", false));
        let status = json!({
            "branch": {"oid": "abc", "head": "main", "upstream": "origin/main", "ahead": 1, "behind": 0},
            "entries": [
                {"xy": ".M", "path": "a.ts"},
                {"xy": "R.", "path": "b.ts", "origPath": "old.ts"},
                {"xy": "??", "path": "new.ts"},
            ],
            "root": "/root/app",
        });
        let got: GitStatusReply = reads(status);
        assert_eq!(got.entries[1].orig_path.as_deref(), Some("old.ts"));
        assert_eq!(got.branch.upstream.as_deref(), Some("origin/main"));
        let stopped = json!({"branch": {"oid": "abc", "head": "fix/cart", "ahead": 3, "behind": 0}, "entries": [], "root": "/root/app", "editsUnread": true});
        let got: GitStatusReply = reads(stopped.clone());
        assert_eq!(got.edits_unread, Some(true));
        let mut uncounted = with(&stopped, "countsUnknown", json!(true));
        uncounted["branch"]["ahead"] = json!(0);
        let got: GitStatusReply = reads(uncounted);
        assert_eq!(got.counts_unknown, Some(true));
        let diff = json!({"base": "main", "files": [{"path": "a.ts", "kind": "modified", "additions": 2, "deletions": 1, "patch": "diff --git a/a.ts b/a.ts\n"}], "truncated": true, "moved": []});
        let got: GitDiffReply = reads(diff);
        assert_eq!((got.files.len(), got.truncated), (1, true));
        let moved = json!({"base": "abc", "files": [], "truncated": false, "moved": ["Checked out pr-889", "Pulled"]});
        let got: GitDiffReply = reads(moved);
        assert_eq!(got.moved, vec!["Checked out pr-889".to_string(), "Pulled".to_string()]);
    }

    // protocol.test.ts:1303
    #[test]
    fn carries_a_typed_error_code_on_refusals() {
        let err = json!({"id": 1, "ok": false, "error": "path escapes the workspace root", "code": "outside-root"});
        let got: DaemonErrorResponse = reads(err.clone());
        assert_eq!(got.code, Some(DaemonErrorCode::OutsideRoot));
        match reads_with(parse_daemon_response, err.clone()) {
            DaemonResponse::V1(refusal) => assert_eq!(refusal.code, Some(DaemonErrorCode::OutsideRoot)),
            other => panic!("{other:?}"),
        }
        let plain: DaemonErrorResponse = reads(json!({"id": 1, "ok": false, "error": "plain"}));
        assert_eq!(plain.code, None);
        let e = refused::<DaemonErrorResponse>(with(&err, "code", json!("whatever")));
        assert_eq!((e.code.as_str(), e.path), ("invalid_enum_value", path!["code"]));
    }
}

mod bringing_work_back {
    use super::*;

    // protocol.test.ts:2137
    // Only the DaemonRequest half: the RuntimeRequest bringBack frame and THREAD_OPS are the runtime writer's.
    #[test]
    fn the_git_frames_parse_the_base_is_the_caller_s_to_leave_out_a_read_names_the_remote() {
        for frame in [
            json!({"id": 1, "op": "git.push", "cwd": "/root/landing", "base": "main"}),
            json!({"id": 2, "op": "git.push", "cwd": "/root/landing"}),
            json!({"id": 3, "op": "git.pr", "cwd": "/root/landing", "base": "main", "title": "the pricing page", "body": "what it does"}),
            json!({"id": 4, "op": "git.prRead", "cwd": "/Users/p", "remote": "git@github.com:o/r.git", "branch": "work"}),
        ] {
            reads_with(parse_daemon_request, frame);
        }
        let e = refused_with(parse_daemon_request, json!({"id": 5, "op": "git.push", "base": "main"}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["cwd"]));
        let e = refused_with(parse_daemon_request, json!({"id": 6, "op": "git.prRead", "cwd": "/Users/p", "branch": "work"}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["remote"]));
        // The frame that read a pull request off the copy's own remote is gone: a read names the remote it is for.
        let e = refused_with(parse_daemon_request, json!({"id": 6, "op": "git.prState", "cwd": "/root/landing"}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_union_discriminator", path!["op"]));
    }

    // protocol.test.ts:2156
    // Only the typed code the daemon refuses with where no command line is signed in; the result shape and its notes
    // are other writers'.
    #[test]
    fn the_result_carries_the_push_and_the_pull_request_or_the_reason_there_is_none() {
        let code: DaemonErrorCode = reads(json!("no-host-cli"));
        assert_eq!(code, DaemonErrorCode::NoHostCli);
    }
}

mod the_credential_contracts {
    use super::*;

    // broker-contract.test.ts:52
    #[test]
    fn a_pipe_s_auth_frame_and_its_pipe_open_parse_and_a_frame_no_daemon_of_pipes_sends_() {
        let auth: DaemonAuthRequest = reads(json!({"id": 0, "op": "auth", "token": "t", "pipe": true}));
        assert_eq!(auth.pipe, Some(true));
        assert_eq!(
            pipe_open_of(&json!({"op": "pipe.open", "door": "gh", "heldMs": 840_000}).to_string()),
            Some(PipeOpen::new(PipeDoor::Gh, 840_000))
        );
        // What daemon 157 sends a pipe auth after its auth reply, recorded off the binary v0.3.6 deploys: a control socket's hello.
        assert_eq!(pipe_open_of(r#"{"type":"daemon.hello","root":"/root","version":157}"#), None);
        assert_eq!(pipe_open_of("not json"), None);
        assert_eq!(pipe_open_of(&json!({"op": "pipe.open", "door": "ssh", "heldMs": 0}).to_string()), None);
    }

    // broker-contract.test.ts:61
    #[test]
    fn each_of_the_six_git_ops_takes_an_op_token_and_an_op_frame_without_one_parses_as_i() {
        fn token_of(r: &DaemonRequest) -> Option<String> {
            match r {
                DaemonRequest::GitPush { token, .. }
                | DaemonRequest::GitStartOn { token, .. }
                | DaemonRequest::GitUpdate { token, .. }
                | DaemonRequest::GitMergeIn { token, .. }
                | DaemonRequest::GitPr { token, .. }
                | DaemonRequest::GitPrCheckout { token, .. } => token.clone(),
                other => panic!("{other:?}"),
            }
        }
        for op in ["git.push", "git.startOn", "git.update", "git.mergeIn", "git.pr", "git.prCheckout"] {
            let frame = json!({"id": 1, "op": op, "cwd": "/root/lab", "branch": "wsp/fix-login", "number": 7});
            assert_eq!(token_of(&parsed(&with(&frame, "token", json!("op-1")))).as_deref(), Some("op-1"), "{op}");
            assert_eq!(token_of(&parsed(&frame)), None, "{op}");
        }
    }
}

mod callback_relay_wire_shapes {
    use super::*;

    // callback-relay.test.ts:74
    #[test]
    fn tunnel_ops_and_events_open_by_id_and_port_base64_data_both_ways_end_from_the_gues() {
        reads_with(parse_daemon_request, json!({"id": 1, "op": "tunnel.open", "tunnelId": "t1", "port": 8976}));
        let e = refused_with(parse_daemon_request, json!({"id": 1, "op": "tunnel.open", "tunnelId": "t1", "port": 0}));
        assert_eq!((e.code.as_str(), e.path), ("too_small", path!["port"]));
        let e = refused_with(parse_daemon_request, json!({"id": 1, "op": "tunnel.open", "tunnelId": "t1", "port": 70000}));
        assert_eq!((e.code.as_str(), e.path), ("too_big", path!["port"]));
        reads_with(parse_daemon_request, json!({"id": 2, "op": "tunnel.write", "tunnelId": "t1", "data": "aGk="}));
        reads_with(parse_daemon_request, json!({"id": 3, "op": "tunnel.close", "tunnelId": "t1"}));
        reads_with(parse_daemon_event, json!({"type": "tunnel.data", "tunnelId": "t1", "data": "aGk="}));
        reads_with(parse_daemon_event, json!({"type": "tunnel.end", "tunnelId": "t1"}));
    }

    // callback-relay.test.ts:121
    #[test]
    fn relay_ports_on_the_wire_are_integers_in_1024_65535_on_browser_open_and_callback_p() {
        for port in [1024, 8976, 65535] {
            reads_with(parse_daemon_event, json!({"type": "callback.port", "port": port}));
            reads_with(parse_daemon_event, json!({"type": "browser.open", "url": "https://x.test/a", "port": port}));
        }
        for (port, code) in [
            (json!(70000), "too_big"),
            (json!(65536), "too_big"),
            (json!(1023), "too_small"),
            (json!(80), "too_small"),
            (json!(0), "too_small"),
            (json!(1.5), "invalid_type"),
            (json!(-1), "too_small"),
        ] {
            let e = refused_with(parse_daemon_event, json!({"type": "callback.port", "port": port}));
            assert_eq!((e.code.as_str(), e.path), (code, path!["port"]), "callback.port {port}");
            let e = refused_with(parse_daemon_event, json!({"type": "browser.open", "url": "https://x.test/a", "port": port}));
            assert_eq!((e.code.as_str(), e.path), (code, path!["port"]), "browser.open {port}");
        }
    }
}

mod pty_mode_daemon_event {
    use super::*;

    // pty-mode.test.ts:6
    #[test]
    fn round_trips_line_and_raw_states_through_the_daemon_event_union() {
        reads_with(parse_daemon_event, json!({"type": "pty.mode", "ptyId": "p1", "mode": "raw", "echo": false, "foreground": "vim"}));
        reads_with(parse_daemon_event, json!({"type": "pty.mode", "ptyId": "p1", "mode": "line", "echo": true, "foreground": "bash"}));
        // foreground is "" when the probe cannot read it; still a valid event
        reads_with(parse_daemon_event, json!({"type": "pty.mode", "ptyId": "p1", "mode": "line", "echo": true, "foreground": ""}));
    }

    // pty-mode.test.ts:16
    #[test]
    fn rejects_unknown_modes_and_missing_fields() {
        let e =
            refused_with(parse_daemon_event, json!({"type": "pty.mode", "ptyId": "p1", "mode": "cooked", "echo": true, "foreground": ""}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_enum_value", path!["mode"]));
        let e = refused_with(parse_daemon_event, json!({"type": "pty.mode", "ptyId": "p1", "mode": "raw", "echo": true}));
        assert_eq!((e.code.as_str(), e.path), ("invalid_type", path!["foreground"]));
    }
}

mod daemon_ws_server {
    use super::*;

    // daemon-ws.test.ts:388
    // The socket half of the case is the daemon's; here a pipe auth's second frame, a control socket's hello, reads as no pipe.
    #[test]
    fn answers_a_pipe_auth_as_a_control_socket_while_it_serves_no_pipes_whose_hello_reads() {
        let hello = json!({"type": "daemon.hello", "root": "/root", "version": DAEMON_VERSION}).to_string();
        assert_eq!(pipe_open_of(&hello), None);
        let event = reads_with(parse_daemon_event, serde_json::from_str(&hello).unwrap());
        assert!(matches!(event, DaemonEvent::DaemonHello { .. }));
    }
}

// The refusals a page's client and the harness read, each printed by the TS module at a3b870a6e.
mod refusals_as_zod_writes_them {
    use super::*;
    use wsp_wire::wire::daemon::{parse_daemon_request, DAEMON_REQUEST_OPS};

    // An unknown op is one issue on ["op"] naming every op in the union's order, which a client reads to say the
    // daemon runs another version.
    #[test]
    fn an_unknown_op_is_one_discriminator_issue_naming_every_op_in_order() {
        let e = refused_with(parse_daemon_request, json!({ "id": 1, "op": "nope" }));
        assert_eq!((e.code.as_str(), e.path.clone()), ("invalid_union_discriminator", path!["op"]));
        let options: Vec<String> = DAEMON_REQUEST_OPS.iter().map(|s| s.to_string()).collect();
        assert_eq!(e.detail["options"], json!(options));
        assert_eq!(DAEMON_REQUEST_OPS.len(), 85);
        assert_eq!((DAEMON_REQUEST_OPS[0], DAEMON_REQUEST_OPS[84]), ("pty.create", "place.update"));
        let quoted: Vec<String> = DAEMON_REQUEST_OPS.iter().map(|o| format!("'{o}'")).collect();
        assert_eq!(e.message, format!("Invalid discriminator value. Expected {}", quoted.join(" | ")));
        // An absent op is the same issue.
        assert_eq!(refused_with(parse_daemon_request, json!({ "id": 1 })).code, "invalid_union_discriminator");
        // A value that is no object is invalid_type at the root.
        let e = refused_with(parse_daemon_request, json!([1]));
        assert_eq!((e.code.as_str(), e.path.clone()), ("invalid_type", path![]));
    }

    // A frame with a known op and no id fails the id's union of string and number: invalid_union at ["id"].
    #[test]
    fn a_known_op_with_no_id_is_an_invalid_union_at_the_id() {
        let e = refused_with(parse_daemon_request, json!({ "op": "ping" }));
        assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_union", path!["id"], "Invalid input"));
        // A fraction is an id: the reply echoes it as sent.
        let r = reads_with(parse_daemon_request, json!({ "id": 2.5, "op": "ping" }));
        assert_eq!(serde_json::to_value(&r).unwrap()["id"], json!(2.5));
    }

    #[test]
    fn the_caps_on_http_post_and_stdio_open() {
        let headers: serde_json::Map<String, Value> = (0..65).map(|i| (format!("h{i}"), json!("v"))).collect();
        let e =
            refused_with(parse_daemon_request, json!({ "id": 1, "op": "http.post", "url": "https://x", "headers": headers, "body": "" }));
        assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("custom", path!["headers"], "at most 64 headers"));
        let sixty_four: serde_json::Map<String, Value> = (0..64).map(|i| (format!("h{i}"), json!("v"))).collect();
        reads_with(parse_daemon_request, json!({ "id": 1, "op": "http.post", "url": "https://x", "headers": sixty_four, "body": "" }));
        let e = refused_with(
            parse_daemon_request,
            json!({ "id": 1, "op": "http.post", "url": "https://x", "headers": { "a": "x".repeat(8193) }, "body": "" }),
        );
        assert_eq!((e.code.as_str(), e.path.clone()), ("too_big", path!["headers", "a"]));
        let e = refused_with(parse_daemon_request, json!({ "id": 1, "op": "stdio.open", "stdioId": "s", "cmd": "" }));
        assert_eq!((e.code.as_str(), e.path.clone()), ("too_small", path!["cmd"]));
        let e = refused_with(parse_daemon_request, json!({ "id": 1, "op": "container.stop", "container": "abc" }));
        assert_eq!((e.code.as_str(), e.path.clone()), ("invalid_string", path!["container"]));
    }

    // daemon.ts:301: a diff reply from a daemon before `moved` reads as none moved, and writes the empty list.
    #[test]
    fn a_diff_reply_with_no_moved_reads_as_none_moved() {
        let got: GitDiffReply = reads_as(
            json!({ "base": null, "files": [], "truncated": false }),
            json!({ "base": null, "files": [], "truncated": false, "moved": [] }),
        );
        assert!(got.moved.is_empty() && got.base.is_none());
    }
}
