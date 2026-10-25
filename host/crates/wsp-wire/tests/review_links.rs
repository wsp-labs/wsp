// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the place link, the machine link and the process frames against zod 3.25.76 and node. Every expected
//! value below was run through the TS bundle (packages/protocol/src/wire/place-link.ts, machine-link.ts, daemon.ts).
mod common;
use common::*;
use serde_json::{json, Value};
use wsp_wire::wire::daemon::{pipe_open_of, ProcEntry, ProcListReply, ProcOwner, ProcScope};
use wsp_wire::wire::machine_link::{
    parse_machine_link_request, workspaces_blocked_by, DaemonResponse, KernelAt, MachineListRow, ModprobeRun, PlaceSysEvent, ProcCost, SysPoint,
    TranscriptsListReply, UsageLogsReply, WorkspaceSysEvent,
};
use wsp_wire::wire::place_link::{
    parse_place_file, place_file_text, JoinMint, PlaceAuthRefusal, PlaceAuthReply,
};
use wsp_wire::wire::parse;

fn place_file_text_of(text: &str) -> String {
    place_file_text(&parse_place_file(text).expect("a place file"))
}

// ---- place file ------------------------------------------------------------------------------------------------

/// place-link.ts:428-447 never checks `joinedAt`, so `null` is kept; place_link.rs:35 reads it through `present`, which
/// refuses null, so the whole file reads as none. node: the file parses and `placeFileText` writes `"joinedAt": null`.
#[test]
fn a_place_file_with_a_null_joined_at_is_still_a_place_file() {
    let text = r#"{"placeId":"p1","name":"n","hostName":"h","hostUrls":["http://a"],"hostPublicKey":"k","keyPath":"/k","joinedAt":null}"#;
    let file = parse_place_file(text).expect("zod-free parse keeps joinedAt whatever it holds");
    assert_eq!(
        place_file_text(&file),
        "{\n  \"placeId\": \"p1\",\n  \"name\": \"n\",\n  \"hostName\": \"h\",\n  \"hostUrls\": [\n    \"http://a\"\n  ],\n  \"hostPublicKey\": \"k\",\n  \"keyPath\": \"/k\",\n  \"joinedAt\": null\n}\n"
    );
}

/// place-link.ts:478 writes the object it parsed, keys in the file's own order; place_link.rs:28 writes the six known
/// keys in struct order, then `joinedAt`, then the rest. node's text for a file written in reverse order:
#[test]
fn place_file_text_keeps_the_keys_in_the_order_the_file_had() {
    let text = r#"{"keyPath":"/k","hostPublicKey":"k","hostUrls":["u"],"hostName":"h","name":"n","placeId":"p1","joinedAt":"x"}"#;
    assert_eq!(
        place_file_text_of(text),
        "{\n  \"keyPath\": \"/k\",\n  \"hostPublicKey\": \"k\",\n  \"hostUrls\": [\n    \"u\"\n  ],\n  \"hostName\": \"h\",\n  \"name\": \"n\",\n  \"placeId\": \"p1\",\n  \"joinedAt\": \"x\"\n}\n"
    );
}

/// JavaScript writes the keys that read as array indexes first, ahead of every other key of the object. The Rust
/// writes the known keys (place_link.rs:28, machine_link.rs:470) and then the kept map, so an index-like key of the kept
/// map comes after them. node: `{"0":4,"5":3,"placeId":...,"joinedAt":"x","z":1}` and `{"5":3,"id":null,"ok":true,...}`.
#[test]
fn an_index_like_key_of_a_kept_map_is_written_before_the_known_keys() {
    let file = r#"{"placeId":"p1","name":"n","hostName":"h","hostUrls":["u"],"hostPublicKey":"k","keyPath":"/k","joinedAt":"x","z":1,"5":3,"0":4}"#;
    assert_eq!(
        place_file_text_of(file),
        "{\n  \"0\": 4,\n  \"5\": 3,\n  \"placeId\": \"p1\",\n  \"name\": \"n\",\n  \"hostName\": \"h\",\n  \"hostUrls\": [\n    \"u\"\n  ],\n  \"hostPublicKey\": \"k\",\n  \"keyPath\": \"/k\",\n  \"joinedAt\": \"x\",\n  \"z\": 1\n}\n"
    );
    let resp: DaemonResponse = parse(&serde_json::from_str::<Value>(r#"{"id":null,"ok":true,"z":1,"a":2,"5":3}"#).unwrap()).unwrap();
    assert_eq!(serde_json::to_string(&resp).unwrap(), r#"{"5":3,"id":null,"ok":true,"z":1,"a":2}"#);
}

/// A number of a kept key is written as JavaScript writes it: `1.0` is `1`, `5e0` is `5`, `-0` is `0`. The kept values
/// are serde_json values, which keep `1.0`, `5.0` and `-0.0`. node: `"n": 1, "m": 5, "neg": 0, "arr": [2.5, 300]`.
#[test]
fn a_number_of_a_kept_key_is_written_as_javascript_writes_it() {
    let text = r#"{"placeId":"p1","name":"n","hostName":"h","hostUrls":[],"hostPublicKey":"k","keyPath":"/k","n":1.0,"m":5e0,"neg":-0,"arr":[2.50,3e2]}"#;
    assert_eq!(
        place_file_text_of(text),
        "{\n  \"placeId\": \"p1\",\n  \"name\": \"n\",\n  \"hostName\": \"h\",\n  \"hostUrls\": [],\n  \"hostPublicKey\": \"k\",\n  \"keyPath\": \"/k\",\n  \"n\": 1,\n  \"m\": 5,\n  \"neg\": 0,\n  \"arr\": [\n    2.5,\n    300\n  ]\n}\n"
    );
}

// ---- base64 values ---------------------------------------------------------------------------------------------

fn b64(bytes: usize, fill: u8) -> String {
    use wsp_frames::Base64Bytes;
    match bytes {
        32 => Base64Bytes::<32>::from_bytes(&[fill; 32]).as_str().to_owned(),
        44 => Base64Bytes::<44>::from_bytes(&[fill; 44]).as_str().to_owned(),
        64 => Base64Bytes::<64>::from_bytes(&[fill; 64]).as_str().to_owned(),
        _ => unreachable!(),
    }
}

/// helpers.ts:101 is `z.string().max(..).regex(..).refine(..)`, so a null is zod's `Expected string, received null` with
/// expected `string`. The Rust base64 types carry no `describe`, so the same frame says `Expected unknown`.
#[test]
fn a_base64_field_of_the_wrong_json_type_says_it_wanted_a_string() {
    let e = refused::<PlaceAuthRefusal>(json!({ "hostPublicKey": null, "signature": b64(64, 5) }));
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_type", path!["hostPublicKey"], "Expected string, received null"));
    assert_eq!(e.detail.get("expected"), Some(&json!("string")));
}

/// zod runs max, then the regex, then the byte count: 45 characters is `too_big` (maximum 44), `!!!` is `invalid_string`
/// (regex, message `Invalid`), and only a string that passes both and holds the wrong byte count is the `custom`
/// "must be 32 bytes, base64". The Rust answers `custom` for all three.
#[test]
fn a_base64_field_too_long_or_off_the_alphabet_is_zods_max_or_regex_issue() {
    let frame = |nonce: &str| json!({ "nonce": nonce, "hostPublicKey": b64(44, 3), "signature": b64(64, 5), "ephemeral": b64(32, 1) });
    let long = refused::<PlaceAuthReply>(frame(&"A".repeat(45)));
    assert_eq!((long.code.as_str(), long.path.clone(), long.message.as_str()), ("too_big", path!["nonce"], "String must contain at most 44 character(s)"));
    let bang = refused::<PlaceAuthReply>(frame("!!!"));
    assert_eq!((bang.code.as_str(), bang.path.clone(), bang.message.as_str()), ("invalid_string", path!["nonce"], "Invalid"));
}

// ---- JoinMint.expiresAt ----------------------------------------------------------------------------------------

fn mint(expires_at: &str) -> Value {
    json!({ "joins": [{ "url": "http://a", "line": "l" }], "expiresAt": expires_at })
}

/// zod 3.25.76's datetime regex checks the calendar (leap years, days in the month) and the clock (hour 00-23, minute and
/// second 00-59). Each of these is `invalid_string` "Invalid datetime" at ["expiresAt"] in node; the Rust checks the digits'
/// shape only and takes all of them (place_link.rs:61).
#[test]
fn expires_at_must_be_a_real_calendar_time() {
    for bad in [
        "2023-02-29T00:00:00Z",
        "2024-02-30T00:00:00Z",
        "2024-13-01T00:00:00Z",
        "2024-01-01T24:00:00Z",
        "2024-01-01T00:60:00Z",
        "2024-01-01T00:00:60Z",
    ] {
        let e = refused::<JoinMint>(mint(bad));
        assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_string", path!["expiresAt"], "Invalid datetime"), "{bad}");
    }
}

/// The same regex makes the seconds optional (`(:[0-5]\d(\.\d+)?)?`), so node takes `2024-01-01T00:00Z`; the Rust refuses it.
#[test]
fn expires_at_may_leave_out_the_seconds() {
    reads::<JoinMint>(mint("2024-01-01T00:00Z"));
}

/// zod builds the datetime issue as `{code, validation, message}`; the regex and url issues put `validation` first. The
/// Rust writes `validation` first for datetime too (place_link.rs:73). node's text:
#[test]
fn the_datetime_issue_writes_code_before_validation() {
    let e = refused::<JoinMint>(mint("x"));
    assert_eq!(
        e.issue_text(),
        "[\n  {\n    \"code\": \"invalid_string\",\n    \"validation\": \"datetime\",\n    \"message\": \"Invalid datetime\",\n    \"path\": [\n      \"expiresAt\"\n    ]\n  }\n]"
    );
}

// ---- records and passthrough -----------------------------------------------------------------------------------

/// zod's object merge skips the key `__proto__` (ParseStatus.mergeObjectSync), in a record and in a passthrough object
/// alike, so it never reaches the written value. The Rust keeps it. node: labels `{"k":"b"}`, response `{"id":1,"ok":true,"k":2}`.
#[test]
fn a_record_and_a_passthrough_object_drop_a_proto_key_as_zod_does() {
    let row: Value = serde_json::from_str(r#"{"id":"a","state":"starting","labels":{"__proto__":"a","k":"b"}}"#).unwrap();
    reads_as::<MachineListRow>(row, json!({ "id": "a", "state": "starting", "labels": { "k": "b" } }));
    let resp: Value = serde_json::from_str(r#"{"id":1,"ok":true,"__proto__":{"a":1},"k":2}"#).unwrap();
    reads_as::<DaemonResponse>(resp, json!({ "id": 1, "ok": true, "k": 2 }));
}

// ---- tagged requests -------------------------------------------------------------------------------------------

/// DECISION-5. A `cmd` of 16385 units and `timeoutMs: null`: zod reads `cmd` before `timeoutMs`, so its first issue is the
/// `too_big` at ["cmd"]; the Rust reads every key's type first and answers the `invalid_type` at ["timeoutMs"].
#[test]
fn decision_5_a_cmd_that_is_too_long_answers_before_a_later_null_timeout() {
    let e = parse_machine_link_request(&json!({ "id": "x", "op": "machine.exec", "machineId": "m", "cmd": "a".repeat(16385), "timeoutMs": null })).unwrap_err();
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("too_big", path!["cmd"], "String must contain at most 16384 character(s)"));
}

// ---- unions and literals ---------------------------------------------------------------------------------------

/// `status: 1.5` fails only the integer check of the error member, which zod leaves dirty rather than aborted, so the
/// union answers that member's issue: `invalid_type` at ["status"], "Expected integer, received float". check.rs:533
/// counts every `invalid_type` as aborting and answers `invalid_union` at the root.
#[test]
fn a_fractional_status_is_the_integer_issue_not_an_invalid_union() {
    let e = refused::<DaemonResponse>(json!({ "id": 1, "ok": false, "error": "x", "status": 1.5 }));
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_type", path!["status"], "Expected integer, received float"));
}

fn sys_event(extra: Value) -> Value {
    let mut v = json!({
        "type": "workspace.sys", "workspaceId": "w",
        "sample": { "type": "sys.sample", "cpu": 1, "load1": 1, "mem": { "used": 1, "total": 1 }, "disk": { "used": 1, "total": 1 }, "at": 1 },
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k.as_str()] = x.clone();
    }
    v
}

/// `z.literal` answers `invalid_literal` for any value that is not the word, a wrong JSON type included. A `type` of null
/// or 5 is `invalid_literal` at ["type"] in node; the Rust says `invalid_type` "Expected string". A `sample.type` of "zzz"
/// is `invalid_literal` at ["sample","type"] expected "sys.sample"; the Rust says `custom` "unknown variant `zzz`".
#[test]
fn a_sys_event_type_that_is_not_the_literal_is_an_invalid_literal() {
    let e = refused::<WorkspaceSysEvent>(sys_event(json!({ "type": null })));
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_literal", path!["type"], "Invalid literal value, expected \"workspace.sys\""));
    let e = refused::<PlaceSysEvent>({
        let mut v = sys_event(json!({ "type": 5 }));
        v["placeId"] = json!("p");
        v
    });
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_literal", path!["type"], "Invalid literal value, expected \"place.sys\""));
    let mut bad = sys_event(json!({}));
    bad["sample"]["type"] = json!("zzz");
    let e = refused::<WorkspaceSysEvent>(bad);
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_literal", path!["sample", "type"], "Invalid literal value, expected \"sys.sample\""));
}

// ---- process rows ----------------------------------------------------------------------------------------------

/// A missing `by` is zod's `Required` whose `expected` is the enum's words joined with ` | `; daemon.rs:957 describes `by`
/// as nothing, so the Rust writes `expected: "unknown"`. The same for a missing `kind` of a scope's pid.
#[test]
fn a_missing_enum_key_of_a_process_owner_or_scope_names_the_words() {
    let e = refused::<ProcOwner>(json!({}));
    assert_eq!(e.detail.get("expected"), Some(&json!("'root' | 'fork' | 'group' | 'env' | 'cgroup' | 'folder' | 'coalition'")));
    let e = refused::<ProcScope>(json!({ "thread": "t", "pids": [{ "pid": 1 }] }));
    assert_eq!(
        e.detail.get("expected"),
        Some(&json!("'agent' | 'tool' | 'dev' | 'test' | 'build' | 'slate' | 'terminal' | 'tab' | 'app' | 'wsp' | 'other'"))
    );
}

fn row(extra: Value) -> Value {
    let mut v = json!({ "pid": 1, "ppid": 0, "user": "u", "state": "S", "comm": "c", "cmdline": "", "cpu": 0, "rss": 0, "startedAt": 0 });
    for (k, x) in extra.as_object().unwrap() {
        v[k.as_str()] = x.clone();
    }
    v
}
fn reply(rows: Vec<Value>) -> Value {
    json!({ "at": 1, "daemon": 1, "total": 1, "procs": rows })
}

/// spec/proc_frames.md rules 1 and 2: a row's unknown, null or numeric `kind` reads as `other` and an unknown owner rung
/// reads as absent, so a newer daemon's row never drops the frame. daemon.ts:458 puts ProcEntry in ProcListReply, and
/// node reads all of these; the Rust ProcListReply is wsp-frames' own type, with a strict enum for each, and refuses the reply.
#[test]
fn a_proc_list_reply_reads_a_newer_daemons_rows_as_leniently_as_a_snapshot() {
    let want = |extra: Value| reply(vec![row(extra)]);
    reads_as::<ProcListReply>(reply(vec![row(json!({ "kind": "zzz" }))]), want(json!({ "kind": "other" })));
    reads_as::<ProcListReply>(reply(vec![row(json!({ "kind": null }))]), want(json!({ "kind": "other" })));
    reads_as::<ProcListReply>(reply(vec![row(json!({ "kind": 5 }))]), want(json!({ "kind": "other" })));
    reads_as::<ProcListReply>(reply(vec![row(json!({ "owner": { "by": "zzz", "thread": "t" } }))]), want(json!({})));
    reads_as::<ProcListReply>(reply(vec![row(json!({ "owner": null }))]), want(json!({})));
}

/// serde reads a struct from a JSON array, so the owner `["root"]` reads as `{"by":"root"}`. zod's `.catch(undefined)` takes
/// an array for a wrong type: node writes the row with no owner.
#[test]
fn a_process_owner_that_is_an_array_reads_as_absent() {
    reads_as::<ProcEntry>(row(json!({ "owner": ["root"] })), row(json!({})));
    reads_as::<ProcEntry>(row(json!({ "owner": ["root", "t1", "w1", true] })), row(json!({})));
}

// ---- shapes held to wsp-frames ---------------------------------------------------------------------------------

/// The wsp-frames shapes carry no `describe`, so a wrong JSON type reads `Expected unknown, received null` where zod says
/// `Expected number, received null`, with expected `number`. node's issue for `cpuMs: null`:
#[test]
fn a_frames_shape_names_the_type_it_wanted_when_the_json_type_is_wrong() {
    let e = refused::<ProcCost>(json!({ "cpuMs": null, "held": 1 }));
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_type", path!["cpuMs"], "Expected number, received null"));
    assert_eq!(e.detail.get("expected"), Some(&json!("number")));
}

/// serde reads a struct from a JSON array; zod refuses it with `Expected object, received array`. `pipe_open_of` guards the
/// pipe a daemon opens, and it takes `["pipe.open","http",0]` as a PipeOpen.
#[test]
fn a_frames_shape_refuses_an_array_where_zod_wants_an_object() {
    let e = refused::<ProcCost>(json!([1, 2]));
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_type", path![], "Expected object, received array"));
    assert!(pipe_open_of(r#"["pipe.open","http",0]"#).is_none());
}

/// zod's `.int()` and `z.number()` take what JavaScript reads as an integer or a number: `5.0`, `1e3`, `-1` for an int with
/// no minimum, and a fraction where the schema is `z.number()`. The wsp-frames fields are u64 and i64, which serde refuses
/// for `5.0`. node takes every one of these.
#[test]
fn a_frames_shape_takes_the_numbers_zod_takes() {
    let held: Value = serde_json::from_str(r#"{"cpuMs":1,"held":5.0}"#).unwrap();
    parse::<ProcCost>(&held).expect("held 5.0 is an integer to zod");
    parse::<ProcCost>(&json!({ "cpuMs": 1, "held": -1 })).expect("held has no minimum in zod");
    let at = json!({ "at": 1.5, "cpu": 1, "load1": 1, "mem": { "used": 1, "total": 1 }, "disk": { "used": 1, "total": 1 } });
    parse::<SysPoint>(&at).expect("at is a plain number in zod");
    assert!(pipe_open_of(r#"{"op":"pipe.open","door":"http","heldMs":5.0}"#).is_some());
    assert!(pipe_open_of(r#"{"op":"pipe.open","door":"gh","heldMs":1e3}"#).is_some());
}

/// zod's `.optional()` takes undefined and refuses `null`. The wsp-frames `Option` fields read `null` as absent. node
/// refuses all three with `Expected string, received null` at the key.
#[test]
fn a_frames_shape_refuses_null_for_an_optional_key() {
    let e = refused::<ProcListReply>(reply(vec![row(json!({ "pty": null }))]));
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_type", path!["procs", 0, "pty"], "Expected string, received null"));
    let e = refused::<TranscriptsListReply>(json!({ "rows": [{ "id": "a", "cwd": "/a", "branch": null, "lastAt": 1, "bytes": 2 }] }));
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_type", path!["rows", 0, "branch"], "Expected string, received null"));
    let usage = json!({ "rows": [{ "agent": "a", "session": "s", "at": 1, "model": "m", "folder": null,
        "tokens": { "input": 1, "output": 1, "cached": 1, "cacheWrite": 1, "reasoning": 1 } }], "limits": [] });
    let e = refused::<UsageLogsReply>(usage);
    assert_eq!((e.code.as_str(), e.path.clone(), e.message.as_str()), ("invalid_type", path!["rows", 0, "folder"], "Expected string, received null"));
}

/// A whole-number f64 of a wsp-frames shape is written `1.0`, `0.0`, `50.0`; JavaScript writes `1`, `0`, `50`. node:
/// `{"at":1,"cpu":0,"load1":1,...}`, `{"cpuMs":12,"held":4}`, `"cost":2`, `"usedPercent":50`.
#[test]
fn a_frames_shape_writes_a_whole_number_without_a_point_zero() {
    reads_exact::<SysPoint>(json!({ "at": 1, "cpu": 0, "load1": 1, "mem": { "used": 1, "total": 2 }, "disk": { "used": 1, "total": 2 } }));
    reads_exact::<ProcCost>(json!({ "cpuMs": 12, "held": 4 }));
    reads_exact::<UsageLogsReply>(json!({
        "rows": [{ "agent": "a", "session": "s", "at": 1, "model": "m",
            "tokens": { "input": 1, "output": 1, "cached": 1, "cacheWrite": 1, "reasoning": 1 }, "cost": 2 }],
        "limits": [{ "agent": "a", "at": 1, "primary": { "usedPercent": 50 } }]
    }));
}

/// A request id is `z.union([z.string(), z.number()])`, a plain number, so `5.0` and `-0` are written `5` and `0`. The Rust
/// keeps the number's input text. node: `{"id":5,"ok":true}` and `{"id":0,"ok":true}`.
#[test]
fn a_numeric_request_id_is_written_as_javascript_writes_a_number() {
    for (input, want) in [(r#"{"id":5.0,"ok":true}"#, r#"{"id":5,"ok":true}"#), (r#"{"id":-0,"ok":true}"#, r#"{"id":0,"ok":true}"#)] {
        let v: Value = serde_json::from_str(input).unwrap();
        let read: DaemonResponse = parse(&v).unwrap();
        assert_eq!(serde_json::to_string(&read).unwrap(), want);
    }
}

// ---- the kernel check ------------------------------------------------------------------------------------------

/// `controllers.split(/\s+/)` splits on JavaScript's white space: it takes U+FEFF and not U+0085. The Rust splits with
/// `split_whitespace`, which takes U+0085 and not U+FEFF. node: `memory\u{feff}cpu` has both controllers, so the check goes on
/// and the answer is none; `memory\u{85}cpu` is one word, so memory is missing.
#[test]
fn the_controller_list_splits_on_javascript_white_space() {
    let blocked = |controllers: &str| {
        let files = [("/sys/fs/cgroup/cgroup.controllers", controllers.to_owned()), ("/proc/filesystems", "nodev\toverlay\n".to_owned())];
        let read = move |path: &str| files.iter().find(|(p, _)| *p == path).map(|(_, t)| t.clone());
        let modprobe = |_: &[&str]| ModprobeRun::Failed { error: "not run".into() };
        workspaces_blocked_by(&KernelAt { platform: "linux", read: &read, modprobe: &modprobe, euid: Some(0) })
    };
    assert_eq!(blocked("memory\u{feff}cpu"), None);
    assert_eq!(
        blocked("memory\u{85}cpu"),
        Some("this computer's cgroup root offers no memory controller, which wsp needs to run workspaces here".to_string())
    );
}
