// SPDX-License-Identifier: AGPL-3.0-only
//! Inputs where the joined crate answers unlike zod 3.25.76 or the TS host at a3b870a6e, found in the review of the
//! joined wsp-wire (#2445). Every expectation is what zod's safeParse or the TS code answered for the same JSON.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_wire::types::check::{parse, Check};
use wsp_wire::types::pull_request::{FixResult, PullRequestLabel};
use wsp_wire::types::thread_defaults::ProjectOverrides;
use wsp_wire::types::JsNumber;
use wsp_wire::views::place::PlaceCap;
use wsp_wire::wire::capabilities::Capabilities;
use wsp_wire::wire::daemon::{
    CgroupMemory, DaemonAuthRequest, DaemonRequest, FsImageReply, FsSearchReply, GitCommitReply, ProcListReply, SshStartReply,
};
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::limits::PortForward;
use wsp_wire::wire::machine_link::{DaemonResponse, MachineLinkRequest, PlaceSysEvent, SysPoint};
use wsp_wire::wire::runtime::{BrokerStatus, RuntimeOkResponse, RuntimeRequest, SessionWarmResult};

fn name<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap_or("?")
}

/// zod refused `text` with this first issue; None where parse answers the same.
fn refused<T: DeserializeOwned + Serialize + Check>(text: &str, code: &str, path: Value, message: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), &text[..text.len().min(120)]);
    match parse::<T>(&input) {
        Ok(read) => Some(format!(
            "{label}: zod refused it ({code} at {path} {message:?}), read here as {}",
            serde_json::to_string(&read).unwrap_or_else(|e| format!("<unwritable: {e}>"))
        )),
        Err(e) if e.code == code && Value::Array(e.path.clone()) == path && e.message == message => None,
        Err(e) => Some(format!("{label}: zod answers {code} at {path} {message:?}, here {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

/// zod read `text` and wrote `wrote` (JSON.stringify of its data).
fn read_as<T: DeserializeOwned + Serialize + Check>(text: &str, wrote: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), &text[..text.len().min(120)]);
    match parse::<T>(&input) {
        Ok(read) => match serde_json::to_string(&read) {
            Ok(text) if text == wrote => None,
            Ok(text) => Some(format!("{label}: zod wrote {wrote}, here {text}")),
            Err(e) => Some(format!("{label}: zod wrote {wrote}, here writing failed: {e}")),
        },
        Err(e) => Some(format!("{label}: zod took it and wrote {wrote}, refused here: {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

/// zod took `text` and its data is `wrote`, compared as values (key order aside).
fn takes<T: DeserializeOwned + Serialize + Check>(text: &str, wrote: Value) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), &text[..text.len().min(120)]);
    match parse::<T>(&input) {
        Ok(read) if serde_json::to_value(&read).ok() == Some(wrote.clone()) => None,
        Ok(read) => Some(format!("{label}: zod's data is {wrote}, here {:?}", serde_json::to_value(&read))),
        Err(e) => Some(format!("{label}: zod took it, refused here: {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

fn none_differ(found: Vec<Option<String>>) {
    let differ: Vec<String> = found.into_iter().flatten().collect();
    assert!(differ.is_empty(), "{} answer(s) unlike zod's:\n{}", differ.len(), differ.join("\n"));
}

const SYS_SAMPLE: &str = r#"{"type":"sys.sample","cpu":1,"load1":1,"mem":{"used":1,"total":1},"disk":{"used":1,"total":1},"at":1}"#;

/// The coordinator's ruling of 2026-10-25 (comment 6397505922): a wsp-frames type is re-exported only where its keys,
/// order, field types and checks all match zod. The 40 re-exports in wire/daemon.rs and wire/machine_link.rs were never
/// held to it (spec/shapes.md rule 6 audits only views/ and types/). daemon.ts:186-292, 359-460, 505-512;
/// machine-link.ts SysPoint, UsageLogsReply.
#[test]
fn a_daemon_reply_re_exported_from_wsp_frames_reads_and_writes_as_zod_does() {
    none_differ(vec![
        // z.number() takes a fraction; u64 refuses it.
        read_as::<ProcListReply>(r#"{"at":2.5,"daemon":1,"total":0,"procs":[]}"#, r#"{"at":2.5,"daemon":1,"total":0,"procs":[]}"#),
        // .int() takes 5.0 (rule 3 of spec/check.md); u64 refuses it.
        read_as::<GitCommitReply>(
            r#"{"oid":"a","subject":"s","filesChanged":5.0,"insertions":1,"deletions":1}"#,
            r#"{"oid":"a","subject":"s","filesChanged":5,"insertions":1,"deletions":1}"#,
        ),
        // .optional() refuses null.
        refused::<FsImageReply>(r#"{"size":1,"modified":null}"#, "invalid_type", json!(["modified"]), "Expected number, received null"),
        // SSH_HOST_KEY_LINE (daemon.ts:290): one ed25519 line, no second line for known_hosts.
        refused::<SshStartReply>(
            r#"{"port":22,"hostKey":"ssh-ed25519 AAAA\n@cert-authority * ssh-ed25519 AAAA"}"#,
            "invalid_string",
            json!(["hostKey"]),
            "Invalid",
        ),
        refused::<SshStartReply>(
            r#"{"port":0,"hostKey":"ssh-ed25519 AAAA"}"#,
            "too_small",
            json!(["port"]),
            "Number must be greater than or equal to 1",
        ),
        refused::<FsSearchReply>(
            r#"{"hits":[{"path":"a","line":0}],"truncated":false}"#,
            "too_small",
            json!(["hits", 0, "line"]),
            "Number must be greater than 0",
        ),
        // serde's derive reads a struct from an array by position.
        refused::<CgroupMemory>(r#"["/a",1]"#, "invalid_type", json!([]), "Expected object, received array"),
        // JavaScript writes an integral number with no ".0" (rule 4.25).
        read_as::<SysPoint>(
            r#"{"at":1,"cpu":1,"load1":1,"mem":{"used":1,"total":1},"disk":{"used":1,"total":1}}"#,
            r#"{"at":1,"cpu":1,"load1":1,"mem":{"used":1,"total":1},"disk":{"used":1,"total":1}}"#,
        ),
        // wsp-frames' UsageStore refuses in its own words, as a custom issue.
        refused::<DaemonRequest>(
            r#"{"id":1,"op":"usage.logs","stores":[{"agent":"","format":"claude-jsonl","root":"r"}]}"#,
            "too_small",
            json!(["stores", 0, "agent"]),
            "String must contain at least 1 character(s)",
        ),
    ]);
}

/// JsNumber writes an integral value from 2^53 up through serialize_u128, which serde_json's Value serializer refuses
/// past u64; RuntimeRequest writes itself through to_value. zod takes threads 1e21 (Number.isInteger), parse takes it,
/// and the frame cannot be written back. js_number.rs:65-72, runtime_ops.rs:721.
#[test]
fn a_number_parse_takes_can_be_written_back() {
    let frame: Value = serde_json::from_str(r#"{"id":1,"op":"places.set","placeId":"p","threads":1e21}"#).unwrap();
    let read = parse::<RuntimeRequest>(&frame).expect("zod takes threads 1e21");
    let written = serde_json::to_string(&read);
    assert!(written.is_ok(), "the frame parse took cannot be written: {written:?}");
    let value = serde_json::to_value(JsNumber(1e21));
    assert!(value.is_ok(), "to_value refuses a JsNumber of 1e21: {value:?}");
}

/// spec/check.md Decision 5: an inline union member runs its rules after every key reads clean, so a later key's type
/// issue answers before an earlier key's rule issue. zod walks the member's keys in order (runtime.ts pair.redeem,
/// machine-link.ts machine.exec) and the coordinator's ruling (6397988867, point 1) asks for zod's first issue.
#[test]
fn an_inline_member_answers_its_first_key_that_fails() {
    none_differ(vec![
        refused::<RuntimeRequest>(
            &format!(r#"{{"id":1,"op":"pair.redeem","code":"{}","name":null}}"#, "a".repeat(65)),
            "too_big",
            json!(["code"]),
            "String must contain at most 64 character(s)",
        ),
        refused::<MachineLinkRequest>(
            &format!(r#"{{"id":1,"op":"machine.exec","machineId":"m","cmd":"{}","timeoutMs":null}}"#, "a".repeat(16385)),
            "too_big",
            json!(["cmd"]),
            "String must contain at most 16384 character(s)",
        ),
    ]);
}

/// zod's received word for any number is "number"; "integer" and "float" are only the words of the .int() check.
/// check.rs:602.
#[test]
fn a_fraction_where_a_string_is_due_is_received_as_a_number() {
    none_differ(vec![refused::<PullRequestLabel>(
        r#"{"name":1.5,"color":"c"}"#,
        "invalid_type",
        json!(["name"]),
        "Expected string, received number",
    )]);
}

/// The foundation review's finding 13, for the case it named: ZodNumber's .int() check marks the parse dirty, not
/// aborted (types.cjs:1091-1100), so ZodUnion answers that member's issue. check.rs:533 counts every invalid_type as
/// aborting. views/place.ts:437, machine-link.ts:544.
#[test]
fn a_union_answers_its_member_whose_only_issue_is_the_integer_rule() {
    none_differ(vec![
        refused::<PlaceCap>(r#"{"threads":2.5}"#, "invalid_type", json!(["threads"]), "Expected integer, received float"),
        refused::<DaemonResponse>(
            r#"{"id":1,"ok":false,"error":"x","status":2.5}"#,
            "invalid_type",
            json!(["status"]),
            "Expected integer, received float",
        ),
    ]);
}

/// The foundation review's finding 10, beyond the two keys its test named: types from crate::wire still describe as
/// "unknown" (limits.ts PortForward.kind, runtime.ts SessionWarmResult.warm and BrokerStatus).
#[test]
fn a_key_names_the_type_zod_expects_there() {
    none_differ(vec![
        refused::<PortForward>(
            r#"{"workspaceId":"w","port":2000,"startedAt":"t","name":"n","kind":null}"#,
            "invalid_type",
            json!(["kind"]),
            "Expected 'url' | 'callback' | 'editor', received null",
        ),
        refused::<SessionWarmResult>(
            r#"{"warm":7}"#,
            "invalid_type",
            json!(["warm"]),
            "Expected 'started' | 'standing' | 'none', received number",
        ),
        refused::<BrokerStatus>(
            r#"{"claude":7}"#,
            "invalid_type",
            json!(["claude"]),
            "Expected 'token' | 'key' | 'gateway' | 'login', received number",
        ),
    ]);
}

/// thread-defaults.ts:41: `AfterWorktree.optional()` takes undefined, not null. check.rs:490 (read_trimmed_opt) reads
/// null as absent.
#[test]
fn an_optional_trimmed_key_refuses_null() {
    none_differ(vec![refused::<ProjectOverrides>(
        r#"{"afterWorktree":null}"#,
        "invalid_type",
        json!(["afterWorktree"]),
        "Expected string, received null",
    )]);
}

/// z.unknown() takes every value, null and undefined among them (runtime.ts:189, daemon.ts:930, daemon-events.ts:88).
#[test]
fn an_unknown_key_takes_null_and_absence() {
    none_differ(vec![
        takes::<RuntimeRequest>(
            r#"{"id":1,"op":"recipes.save","name":"r","file":null}"#,
            json!({"id": 1, "op": "recipes.save", "name": "r", "file": null}),
        ),
        takes::<DaemonRequest>(r#"{"id":1,"op":"guest.send"}"#, json!({"id": 1, "op": "guest.send"})),
        takes::<DaemonEvent>(r#"{"type":"guest.message","session":"s"}"#, json!({"type": "guest.message", "session": "s"})),
    ]);
}

/// pull-request.ts:309: FixResult is a discriminatedUnion on outcome over five outcomes; a missing or unknown one is
/// zod's discriminator issue naming all five.
#[test]
fn fix_result_s_outcome_is_its_discriminator() {
    let message = "Invalid discriminator value. Expected 'steered' | 'queued' | 'started' | 'held' | 'updated'";
    none_differ(vec![
        refused::<FixResult>(r#"{"threadId":"t","base":"b","agent":"a"}"#, "invalid_union_discriminator", json!(["outcome"]), message),
        refused::<FixResult>(r#"{"outcome":"nope"}"#, "invalid_union_discriminator", json!(["outcome"]), message),
    ]);
}

/// The local backend builds its capabilities with `sizes` after `templates` (local-backend.ts:158-178; no-provider,
/// fake and place-machine backends the same), and serve.ts:1574 and workspaces.ts:77 send the object as built. The
/// struct writes `sizes` last for every backend, which is the box's and Solari's order only.
#[test]
fn a_local_backend_s_capabilities_write_in_the_order_it_builds_them() {
    let local = r#"{"liveCloneForks":false,"replacesMachine":false,"previewUrls":false,"signedUrls":false,"callbackRelay":false,"diskSnapshots":false,"images":false,"snapshotsAnyLife":false,"snapshotListing":false,"templates":false,"sizes":[],"kept":true,"copies":true,"ownNetwork":false}"#;
    none_differ(vec![read_as::<Capabilities>(local, local)]);
}

/// A missing or wrong literal key is zod's invalid_literal (ZodLiteral, types.cjs:3030): daemon.ts:76 `op`, and the
/// sys events' `type` the integration split by hand (machine-link.ts PlaceSysEvent).
#[test]
fn a_literal_key_answers_invalid_literal() {
    none_differ(vec![
        refused::<DaemonAuthRequest>(
            r#"{"id":"a","token":"a","port":1,"pipe":true}"#,
            "invalid_literal",
            json!(["op"]),
            "Invalid literal value, expected \"auth\"",
        ),
        refused::<PlaceSysEvent>(
            &format!(r#"{{"type":null,"placeId":"p","sample":{SYS_SAMPLE}}}"#),
            "invalid_literal",
            json!(["type"]),
            "Invalid literal value, expected \"place.sys\"",
        ),
    ]);
}

/// limits.ts RelayPort is `.int().min(1024).max(65535)`; the ruling (6397988867, point 2) moves every .int() to JsInt,
/// and RelayPort is still an i64 that refuses 2000.0 and answers 5.0 with "Expected number, received number".
#[test]
fn a_relay_port_reads_every_number_javascript_calls_an_integer() {
    none_differ(vec![
        read_as::<PortForward>(
            r#"{"workspaceId":"w","port":2000.0,"startedAt":"t","name":"n","kind":"url"}"#,
            r#"{"workspaceId":"w","port":2000,"startedAt":"t","name":"n","kind":"url"}"#,
        ),
        refused::<PortForward>(
            r#"{"workspaceId":"w","port":5.0,"startedAt":"t","name":"n","kind":"url"}"#,
            "too_small",
            json!(["port"]),
            "Number must be greater than or equal to 1024",
        ),
    ]);
}

/// runtime.ts RuntimeOkResponse: `id: reqId` is a union of string and number, so a missing id is invalid_union, as the
/// requests answer it.
#[test]
fn a_response_with_no_id_answers_invalid_union() {
    none_differ(vec![refused::<RuntimeOkResponse>(r#"{"ok":true}"#, "invalid_union", json!(["id"]), "Invalid input")]);
}

/// daemon-channel.ts:96 writes `{ ...frame, id }`: op, the call's keys, then id. zod's parse writes id first. The
/// enum writes op, then id, then the keys: neither.
#[test]
fn a_daemon_frame_is_written_as_the_daemon_channel_writes_it() {
    let read = parse::<DaemonRequest>(&json!({"op": "git.status", "cwd": "/w", "id": 1})).unwrap();
    assert_eq!(
        serde_json::to_string(&read).unwrap(),
        r#"{"op":"git.status","cwd":"/w","id":1}"#,
        "the TS host's daemon channel writes this frame so"
    );
}

/// proc_frames rules 1-3 (zod's `.catch`, daemon.ts:359-380): a row's unknown or null kind reads as `other` and an
/// unknown owner rung as no owner, so a daemon a version apart still lists its processes. The host reads proc.list
/// through ProcListReply (runtime/src/activity.ts:144), re-exported from wsp-frames with its strict rows, so one such
/// row refuses the whole list; the rule's tests read ProcSnapshot only.
#[test]
fn a_process_list_reads_a_row_it_does_not_know_as_zod_catches_it() {
    let row = |extra: &str| {
        format!(
            r#"{{"at":1,"daemon":1,"total":1,"procs":[{{"pid":1,"ppid":0,"user":"u","state":"S","comm":"c","cmdline":"c","cpu":1,"rss":1,"startedAt":1{extra}}}]}}"#
        )
    };
    let caught = r#"{"at":1,"daemon":1,"total":1,"procs":[{"pid":1,"ppid":0,"user":"u","state":"S","comm":"c","cmdline":"c","cpu":1,"rss":1,"startedAt":1,"kind":"other"}]}"#;
    none_differ(vec![
        read_as::<ProcListReply>(&row(r#","kind":"nope""#), caught),
        read_as::<ProcListReply>(&row(r#","kind":null"#), caught),
        read_as::<ProcListReply>(
            &row(r#","owner":{"by":"nope","workspace":"w"}"#),
            r#"{"at":1,"daemon":1,"total":1,"procs":[{"pid":1,"ppid":0,"user":"u","state":"S","comm":"c","cmdline":"c","cpu":1,"rss":1,"startedAt":1}]}"#,
        ),
    ]);
}

/// ZodUnion's value is the first member that parses clean (views/place.ts:437). check.rs:458-464 finds no issue
/// (CloudCap reads clean), then serde's untagged read takes the first member that deserializes, ComputerCap, whose
/// `threads >= 1` is broken by the very value it holds.
#[test]
fn a_union_holds_the_member_its_pass_found_clean() {
    none_differ(vec![read_as::<PlaceCap>(r#"{"threads":0,"machines":1,"spendPerDayUsd":1}"#, r#"{"machines":1,"spendPerDayUsd":1}"#)]);
}
