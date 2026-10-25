// SPDX-License-Identifier: AGPL-3.0-only
//! Inputs where wsp-wire's daemon frames and replies, limits, helpers, capabilities, devices and daemon version answer
//! unlike zod 3.25.76 or the TS functions at a3b870a6e (packages/protocol/src/wire/daemon.ts, limits.ts, helpers.ts,
//! devices.ts), found in the review of the joined crate (#2445). Every expectation is what zod's safeParse, JSON.stringify
//! of its data, or node 22.23.4 answered for the same input. Each test names the TS line it reads.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_wire::types::check::{parse, Check};
use wsp_wire::wire::daemon::*;
use wsp_wire::wire::devices::{DaemonChannelEvent, DaemonFrame, DeviceVia};
use wsp_wire::wire::helpers::PermissionEffect;
use wsp_wire::wire::limits::{callback_port_of, host_of, is_http_url, redirects_to_machine};

fn name<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap_or("?")
}

fn as_js(v: &Value) -> Value {
    match v {
        Value::Number(n) => serde_json::to_value(wsp_wire::types::JsNumber(n.as_f64().expect("a JSON number reads as a double"))).unwrap(),
        Value::Array(items) => Value::Array(items.iter().map(as_js).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), as_js(v))).collect()),
        other => other.clone(),
    }
}

/// zod refused `text` with this first issue; None where parse answers the same.
fn refused<T: DeserializeOwned + Serialize + Check>(text: &str, code: &str, path: Value, message: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), &text[..text.len().min(110)]);
    match parse::<T>(&input) {
        Ok(read) => Some(format!(
            "{label}: zod refused it ({code} at {path} {message:?}), read here as {}",
            serde_json::to_string(&read).unwrap_or_else(|e| format!("<unwritable: {e}>"))
        )),
        Err(e) if e.code == code && Value::Array(e.path.clone()) == path && e.message == message => None,
        Err(e) => Some(format!("{label}: zod answers {code} at {path} {message:?}, here {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

/// zod took `text` and JSON.stringify of its data is `wrote`; compared as values (key order and number text aside).
fn reads_as<T: DeserializeOwned + Serialize + Check>(text: &str, wrote: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), &text[..text.len().min(110)]);
    match parse::<T>(&input) {
        Ok(read) if as_js(&serde_json::to_value(&read).unwrap()) == as_js(&serde_json::from_str(wrote).unwrap()) => None,
        Ok(read) => Some(format!("{label}: zod wrote {wrote}, here {}", serde_json::to_string(&read).unwrap())),
        Err(e) => Some(format!("{label}: zod took it, refused here: {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

/// zod took `text` and JSON.stringify of its data is `wrote`, byte for byte.
fn writes<T: DeserializeOwned + Serialize + Check>(text: &str, wrote: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), &text[..text.len().min(110)]);
    match parse::<T>(&input) {
        Ok(read) => match serde_json::to_string(&read) {
            Ok(got) if got == wrote => None,
            Ok(got) => Some(format!("{label}: zod wrote {wrote}, here {got}")),
            Err(e) => Some(format!("{label}: zod wrote {wrote}, here writing failed: {e}")),
        },
        Err(e) => Some(format!("{label}: zod took it, refused here: {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

fn none_differ(found: Vec<Option<String>>) {
    let differ: Vec<String> = found.into_iter().flatten().collect();
    assert!(differ.is_empty(), "{} answer(s) unlike zod's:\n{}", differ.len(), differ.join("\n"));
}

fn op_refused(text: &str, code: &str, path: Value, message: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("DaemonRequest {}", &text[..text.len().min(110)]);
    match parse_daemon_request(&input) {
        Ok(read) => {
            Some(format!("{label}: zod refused it ({code} at {path} {message:?}), read here as {}", serde_json::to_string(&read).unwrap()))
        }
        Err(e) if e.code == code && Value::Array(e.path.clone()) == path && e.message == message => None,
        Err(e) => Some(format!("{label}: zod answers {code} at {path} {message:?}, here {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// BLOCKER: a guard that accepts what the TS refuses
// ---------------------------------------------------------------------------------------------------------------

/// BLOCKER. daemon.ts:290-292: SSH_HOST_KEY_LINE, min 1, max SSH_KEY_MAX 1024 on hostKey. The comment above it says why:
/// a second line or a `@cert-authority` marker is a key this computer's ssh trusts for every wsp- alias. wsp-frames'
/// SshStartReply is `{port: u16, host_key: String}` with no rule at all, and daemon.rs re-exports it with a Check that
/// is `Ok(())` (spec/daemon.md rule 9 says the bounds are read while deserializing; this one has none).
#[test]
fn an_ssh_host_key_is_one_ed25519_line_of_at_most_1024_units() {
    let long = format!(r#"{{"port":22,"hostKey":"ssh-ed25519 {}"}}"#, "A".repeat(1030));
    none_differ(vec![
        refused::<SshStartReply>(
            r#"{"port":22,"hostKey":"ssh-ed25519 AAAA\n@cert-authority * ssh-ed25519 BBBB"}"#,
            "invalid_string",
            json!(["hostKey"]),
            "Invalid",
        ),
        refused::<SshStartReply>(
            r#"{"port":22,"hostKey":"@cert-authority * ssh-ed25519 AAAA"}"#,
            "invalid_string",
            json!(["hostKey"]),
            "Invalid",
        ),
        refused::<SshStartReply>(r#"{"port":22,"hostKey":"ssh-rsa AAAA"}"#, "invalid_string", json!(["hostKey"]), "Invalid"),
        refused::<SshStartReply>(r#"{"port":22,"hostKey":"ssh-ed25519 AAAA\n"}"#, "invalid_string", json!(["hostKey"]), "Invalid"),
        refused::<SshStartReply>(
            r#"{"port":22,"hostKey":""}"#,
            "too_small",
            json!(["hostKey"]),
            "String must contain at least 1 character(s)",
        ),
        refused::<SshStartReply>(&long, "too_big", json!(["hostKey"]), "String must contain at most 1024 character(s)"),
    ]);
}

/// BLOCKER by the brief's rule (a guard accepts what the TS refuses); the reach is small, only a host with a character
/// IDNA disallows. limits.ts:114-122 isHttpUrl runs `new URL(url)`, which in node refuses a host holding U+3164 HANGUL
/// FILLER, U+115F, U+2061 or U+17B4; the `url` crate maps them away and accepts, and host_of answers "ab.com" where
/// node answers undefined. A fuzz over every code point in a host label found 23,567 such code points (the most are
/// code points node's tables leave unassigned); these four have been assigned for decades.
#[test]
fn a_host_node_refuses_is_refused_here() {
    let mut found = Vec::new();
    for c in ['\u{3164}', '\u{115f}', '\u{2061}', '\u{17b4}'] {
        let url = format!("http://a{c}b.com/");
        if is_http_url(&url) {
            found.push(format!("is_http_url({url:?}): node answers false, here true"));
        }
        if host_of(&url).is_some() {
            found.push(format!("host_of({url:?}): node answers undefined, here {:?}", host_of(&url)));
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

// ---------------------------------------------------------------------------------------------------------------
// MAJOR
// ---------------------------------------------------------------------------------------------------------------

/// MAJOR. daemon.ts:292 `port: z.number().int().min(1).max(65535)`. wsp-frames reads a u16: 0 passes, -1 and 65536 are
/// `invalid_type` "Expected unknown, received number", 22.0 is refused.
#[test]
fn an_ssh_port_is_an_integer_from_1_to_65535() {
    none_differ(vec![
        refused::<SshStartReply>(
            r#"{"port":0,"hostKey":"ssh-ed25519 AAAA"}"#,
            "too_small",
            json!(["port"]),
            "Number must be greater than or equal to 1",
        ),
        refused::<SshStartReply>(
            r#"{"port":-1,"hostKey":"ssh-ed25519 AAAA"}"#,
            "too_small",
            json!(["port"]),
            "Number must be greater than or equal to 1",
        ),
        refused::<SshStartReply>(
            r#"{"port":65536,"hostKey":"ssh-ed25519 AAAA"}"#,
            "too_big",
            json!(["port"]),
            "Number must be less than or equal to 65535",
        ),
        refused::<SshStartReply>(
            r#"{"port":22.5,"hostKey":"ssh-ed25519 AAAA"}"#,
            "invalid_type",
            json!(["port"]),
            "Expected integer, received float",
        ),
        reads_as::<SshStartReply>(r#"{"port":22.0,"hostKey":"ssh-ed25519 AAAA"}"#, r#"{"port":22,"hostKey":"ssh-ed25519 AAAA"}"#),
    ]);
}

/// MAJOR. daemon.ts:508 `at: z.array(z.string().min(1).max(1024)).max(8)` on HttpBearer, which http.post's bearer is. wsp-frames'
/// bounded_list caps each entry's length and the count but not the minimum, so an empty key name is read.
#[test]
fn a_bearer_key_name_is_at_least_one_unit() {
    none_differ(vec![
        refused::<HttpBearer>(
            r#"{"file":"abc","at":[""],"field":"f"}"#,
            "too_small",
            json!(["at", 0]),
            "String must contain at least 1 character(s)",
        ),
        op_refused(
            r#"{"id":1,"op":"http.post","url":"http://a.test/x","headers":{},"body":"","bearer":{"file":"abc","at":[""],"field":"f"}}"#,
            "too_small",
            json!(["bearer", "at", 0]),
            "String must contain at least 1 character(s)",
        ),
    ]);
}

/// MAJOR (written JSON). daemon.ts:195-200 `files: z.record(z.string(), z.string())`: zod writes the keys in the order read
/// (integer-like ones first, ascending, as a JavaScript object holds them). wsp-frames' FsHashReply keeps `files` in a
/// BTreeMap, so a host that reads and writes a reply sorts it.
#[test]
fn an_fs_hash_reply_writes_its_files_in_the_order_it_read_them() {
    none_differ(vec![
        writes::<FsHashReply>(r#"{"files":{"b":"1","a":"2"}}"#, r#"{"files":{"b":"1","a":"2"}}"#),
        writes::<FsHashReply>(r#"{"files":{"2":"x","b":"1","1":"y","a":"2"}}"#, r#"{"files":{"1":"y","2":"x","b":"1","a":"2"}}"#),
    ]);
}

/// MAJOR (written JSON). daemon.ts:195-200 `outside`, `past` and `special` are `.optional()` arrays: zod keeps an empty
/// one it read. FsHashReply's `Vec` skips an empty list when it writes.
#[test]
fn an_fs_hash_reply_keeps_an_empty_list_it_read() {
    none_differ(vec![
        writes::<FsHashReply>(r#"{"files":{},"outside":[],"past":[],"special":[]}"#, r#"{"files":{},"outside":[],"past":[],"special":[]}"#),
        writes::<FsHashReply>(
            r#"{"files":{},"outside":["a"],"past":[],"special":["b"]}"#,
            r#"{"files":{},"outside":["a"],"past":[],"special":["b"]}"#,
        ),
    ]);
}

/// MAJOR (written JSON). daemon.ts:161 `prunable: z.boolean().optional()` and :175 `truncated: z.boolean().optional()`:
/// zod keeps an explicit `false`. wsp-frames reads both as a plain `bool` that is skipped when false.
#[test]
fn an_optional_flag_read_as_false_is_written_as_false() {
    none_differ(vec![
        writes::<GitWorktree>(r#"{"path":"p","prunable":false}"#, r#"{"path":"p","prunable":false}"#),
        writes::<GitWorktreesReply>(r#"{"worktrees":[{"path":"p","prunable":false}]}"#, r#"{"worktrees":[{"path":"p","prunable":false}]}"#),
        writes::<GitBranchesReply>(r#"{"branches":[],"truncated":false}"#, r#"{"branches":[],"truncated":false}"#),
    ]);
}

/// MAJOR. daemon.ts:483-490 DaemonExecRequest is `{id, op: z.literal("exec"), cmd, timeoutMs, stdin}`. The Rust struct
/// holds no `op` (it is a member of the tagged enum), so read on its own it takes a frame with no op and one with
/// another op, which zod refuses with invalid_literal. daemon.ts:912 uses the same schema as a member of DaemonRequest,
/// and packages/runtime/test/box-launch.test.ts:31 parses it alone.
#[test]
fn an_exec_request_read_alone_wants_op_exec() {
    none_differ(vec![
        refused::<DaemonExecRequest>(r#"{"id":1,"cmd":"x"}"#, "invalid_literal", json!(["op"]), "Invalid literal value, expected \"exec\""),
        refused::<DaemonExecRequest>(
            r#"{"id":1,"op":"ping","cmd":"x"}"#,
            "invalid_literal",
            json!(["op"]),
            "Invalid literal value, expected \"exec\"",
        ),
        refused::<DaemonExecRequest>(
            r#"{"id":1,"op":5,"cmd":"x"}"#,
            "invalid_literal",
            json!(["op"]),
            "Invalid literal value, expected \"exec\"",
        ),
    ]);
}

/// MAJOR (written JSON). The same schema writes `op`: `{"id":1,"op":"exec","cmd":"x"}`. The struct writes
/// `{"id":1,"cmd":"x"}`, a frame with no op, which a daemon answers unknown op.
#[test]
fn an_exec_request_written_alone_carries_op_exec() {
    none_differ(vec![writes::<DaemonExecRequest>(r#"{"id":1,"op":"exec","cmd":"x"}"#, r#"{"id":1,"op":"exec","cmd":"x"}"#)]);
}

// ---------------------------------------------------------------------------------------------------------------
// The 33 wsp-frames re-exports (daemon.rs:18-24). Each case below is one zod answered and the Rust did not.
// ---------------------------------------------------------------------------------------------------------------

/// MINOR (refuses what zod takes). The coordinator's ruling (6397505922): a wsp-frames type is re-exported only where
/// its field types match. zod's `z.number()` takes a fraction, a negative and a huge number; `.int()` takes 5.0; these
/// shapes are u64, u32, u16 or i64 in wsp-frames. daemon.ts line in each label. Not listed (process rows, another
/// reviewer's): ProcListReply.at, .daemon, .total and its rows.
#[test]
fn a_re_exported_reply_reads_every_number_zod_reads() {
    none_differ(vec![
        // daemon.ts:122 size, mtime
        reads_as::<FsEntry>(
            r#"{"name":"n","type":"file","size":2.5,"mtime":-1.5}"#,
            r#"{"name":"n","type":"file","size":2.5,"mtime":-1.5}"#,
        ),
        // :126 total
        reads_as::<FsListReply>(r#"{"entries":[],"truncated":false,"total":2.5}"#, r#"{"entries":[],"truncated":false,"total":2.5}"#),
        // :189 size, modified, inode, changed
        reads_as::<FsImageReply>(
            r#"{"size":2.5,"modified":-1,"inode":0.5,"changed":-2.5}"#,
            r#"{"size":2.5,"modified":-1,"inode":0.5,"changed":-2.5}"#,
        ),
        // :182 bytes (.int())
        reads_as::<FsWriteReply>(r#"{"bytes":-1}"#, r#"{"bytes":-1}"#),
        // :261 additions, deletions are u32 over z.number()
        reads_as::<GitDiffFile>(
            r#"{"path":"p","kind":"added","additions":-1,"deletions":2.5,"patch":"x"}"#,
            r#"{"path":"p","kind":"added","additions":-1,"deletions":2.5,"patch":"x"}"#,
        ),
        reads_as::<GitDiffReply>(
            r#"{"base":null,"files":[{"path":"p","kind":"added","additions":-1,"deletions":2.5,"patch":"x"}],"truncated":false}"#,
            r#"{"base":null,"files":[{"path":"p","kind":"added","additions":-1,"deletions":2.5,"patch":"x"}],"truncated":false,"moved":[]}"#,
        ),
        // :269 filesChanged, insertions, deletions (.int()): -1 and 5.0
        reads_as::<GitCommitReply>(
            r#"{"oid":"o","subject":"s","filesChanged":-1,"insertions":5.0,"deletions":0}"#,
            r#"{"oid":"o","subject":"s","filesChanged":-1,"insertions":5,"deletions":0}"#,
        ),
        // :152 files (.int())
        reads_as::<GitRestoreReply>(r#"{"before":"b","files":-3}"#, r#"{"before":"b","files":-3}"#),
        // :156 dropped (.int().nonnegative()): 5.0
        reads_as::<GitCheckpointDropReply>(r#"{"dropped":5.0}"#, r#"{"dropped":5}"#),
        // :167 committed (.int().nonnegative()): 5.0
        reads_as::<GitLocalBranch>(r#"{"name":"n","oid":"o","committed":5.0}"#, r#"{"name":"n","oid":"o","committed":5}"#),
        reads_as::<GitBranchesReply>(
            r#"{"branches":[{"name":"n","oid":"o","committed":5.0}]}"#,
            r#"{"branches":[{"name":"n","oid":"o","committed":5}]}"#,
        ),
        // :140 number (.int())
        reads_as::<HostItem>(
            r#"{"kind":"issue","number":-1,"title":"t","body":"b","url":"u"}"#,
            r#"{"kind":"issue","number":-1,"title":"t","body":"b","url":"u"}"#,
        ),
        reads_as::<GitPrListReply>(
            r#"{"items":[{"kind":"issue","number":-1,"title":"t","body":"b","url":"u"}]}"#,
            r#"{"items":[{"kind":"issue","number":-1,"title":"t","body":"b","url":"u"}]}"#,
        ),
        // :512 status (.int()) is a u16
        reads_as::<HttpPostReply>(r#"{"status":70000,"body":""}"#, r#"{"status":70000,"body":""}"#),
        reads_as::<HttpPostReply>(r#"{"status":-1,"body":""}"#, r#"{"status":-1,"body":""}"#),
        // :436 memory (.int().nonnegative()): 5.0
        reads_as::<CgroupMemory>(r#"{"cgroup":"c","memory":5.0}"#, r#"{"cgroup":"c","memory":5}"#),
        // :92 heldMs (.int().nonnegative()): 5.0
        reads_as::<PipeOpen>(r#"{"op":"pipe.open","door":"http","heldMs":5.0}"#, r#"{"op":"pipe.open","door":"http","heldMs":5}"#),
        // :209 line (.int().positive()) is a u32
        reads_as::<FsSearchReply>(
            r#"{"hits":[{"path":"p","line":4294967296}],"truncated":false}"#,
            r#"{"hits":[{"path":"p","line":4294967296}],"truncated":false}"#,
        ),
        refused::<FsSearchReply>(
            r#"{"hits":[{"path":"p","line":0}],"truncated":false}"#,
            "too_small",
            json!(["hits", 0, "line"]),
            "Number must be greater than 0",
        ),
    ]);
}

/// MINOR (accepts what zod refuses). Every `.optional()` key of these shapes refuses null in zod (`Expected string,
/// received null`); wsp-frames' `Option` with a default reads null as absent. daemon.ts line in each label.
#[test]
fn a_re_exported_reply_refuses_null_on_an_optional_key() {
    let nul = |ty: &str, text: &str, path: Value, what: &str| -> Option<String> {
        let message = format!("Expected {what}, received null");
        let r = match ty {
            "FsImageReply" => refused::<FsImageReply>(text, "invalid_type", path, &message),
            "FsSearchReply" => refused::<FsSearchReply>(text, "invalid_type", path, &message),
            "GitBranchesReply" => refused::<GitBranchesReply>(text, "invalid_type", path, &message),
            "GitLocalBranch" => refused::<GitLocalBranch>(text, "invalid_type", path, &message),
            "GitDiffFile" => refused::<GitDiffFile>(text, "invalid_type", path, &message),
            "GitPrListReply" => refused::<GitPrListReply>(text, "invalid_type", path, &message),
            "GitWorktree" => refused::<GitWorktree>(text, "invalid_type", path, &message),
            "HttpPostReply" => refused::<HttpPostReply>(text, "invalid_type", path, &message),
            _ => unreachable!(),
        };
        r
    };
    none_differ(vec![
        // :189
        nul("FsImageReply", r#"{"size":1,"modified":null}"#, json!(["modified"]), "number"),
        nul("FsImageReply", r#"{"size":1,"inode":null}"#, json!(["inode"]), "number"),
        nul("FsImageReply", r#"{"size":1,"changed":null}"#, json!(["changed"]), "number"),
        nul("FsImageReply", r#"{"size":1,"mediaType":null}"#, json!(["mediaType"]), "string"),
        nul("FsImageReply", r#"{"size":1,"content":null}"#, json!(["content"]), "string"),
        nul("FsImageReply", r#"{"size":1,"svg":null}"#, json!(["svg"]), "boolean"),
        // :209-212
        nul("FsSearchReply", r#"{"hits":[{"path":"p","line":null}],"truncated":false}"#, json!(["hits", 0, "line"]), "number"),
        nul("FsSearchReply", r#"{"hits":[{"path":"p","text":null}],"truncated":false}"#, json!(["hits", 0, "text"]), "string"),
        // :167-175
        nul("GitBranchesReply", r#"{"current":null,"branches":[]}"#, json!(["current"]), "string"),
        nul(
            "GitBranchesReply",
            r#"{"branches":[{"name":"n","oid":"o","committed":1,"upstream":null}]}"#,
            json!(["branches", 0, "upstream"]),
            "string",
        ),
        nul("GitLocalBranch", r#"{"name":"n","oid":"o","committed":1,"upstream":null}"#, json!(["upstream"]), "string"),
        nul("GitLocalBranch", r#"{"name":"n","oid":"o","committed":1,"worktree":null}"#, json!(["worktree"]), "string"),
        // :261
        nul("GitDiffFile", r#"{"path":"p","kind":"k","additions":1,"deletions":1,"patch":"x","blob":null}"#, json!(["blob"]), "string"),
        // :143
        nul("GitPrListReply", r#"{"items":[],"note":null}"#, json!(["note"]), "string"),
        nul("GitPrListReply", r#"{"items":[],"noCliFor":null}"#, json!(["noCliFor"]), "string"),
        // :161
        nul("GitWorktree", r#"{"path":"p","branch":null}"#, json!(["branch"]), "string"),
        nul("GitWorktree", r#"{"path":"p","head":null}"#, json!(["head"]), "string"),
        // :512
        nul("HttpPostReply", r#"{"status":1,"body":"","contentType":null}"#, json!(["contentType"]), "string"),
        nul("HttpPostReply", r#"{"status":1,"body":"","sessionId":null}"#, json!(["sessionId"]), "string"),
    ]);
}

/// MINOR. A string rule of zod answers too_small or too_big at its own path; wsp-frames' `bounded` and `bounded_list`
/// answer `custom` in their own words, and a list entry's issue is at the list. daemon.ts:508 (HttpBearer),
/// :502 (UsageStore). The lengths count UTF-16 units on both sides (66 units of 33 emoji cross 64). One case is
/// already pinned in review_core.rs (UsageStore agent empty); the rest are the same cause.
#[test]
fn a_re_exported_rule_answers_zod_s_issue_not_a_custom_one() {
    let long = |n: usize| "x".repeat(n);
    none_differ(vec![
        refused::<HttpBearer>(
            r#"{"file":"ab","at":[],"field":"f"}"#,
            "too_small",
            json!(["file"]),
            "String must contain at least 3 character(s)",
        ),
        refused::<HttpBearer>(
            &format!(r#"{{"file":"{}","at":[],"field":"f"}}"#, long(4097)),
            "too_big",
            json!(["file"]),
            "String must contain at most 4096 character(s)",
        ),
        refused::<HttpBearer>(
            &format!(r#"{{"file":"abc","at":["{}"],"field":"f"}}"#, long(1025)),
            "too_big",
            json!(["at", 0]),
            "String must contain at most 1024 character(s)",
        ),
        refused::<HttpBearer>(
            r#"{"file":"abc","at":["x","x","x","x","x","x","x","x","x"],"field":"f"}"#,
            "too_big",
            json!(["at"]),
            "Array must contain at most 8 element(s)",
        ),
        refused::<HttpBearer>(
            r#"{"file":"abc","at":[],"field":""}"#,
            "too_small",
            json!(["field"]),
            "String must contain at least 1 character(s)",
        ),
        refused::<HttpBearer>(
            &format!(r#"{{"file":"abc","at":[],"field":"{}"}}"#, long(256)),
            "too_big",
            json!(["field"]),
            "String must contain at most 255 character(s)",
        ),
        refused::<UsageStore>(
            r#"{"agent":"a","format":"claude-jsonl","root":""}"#,
            "too_small",
            json!(["root"]),
            "String must contain at least 1 character(s)",
        ),
        refused::<UsageStore>(
            &format!(r#"{{"agent":"{}","format":"claude-jsonl","root":"r"}}"#, long(65)),
            "too_big",
            json!(["agent"]),
            "String must contain at most 64 character(s)",
        ),
        refused::<UsageStore>(
            &format!(r#"{{"agent":"{}","format":"claude-jsonl","root":"r"}}"#, "😀".repeat(33)),
            "too_big",
            json!(["agent"]),
            "String must contain at most 64 character(s)",
        ),
        refused::<UsageStore>(
            &format!(r#"{{"agent":"a","format":"claude-jsonl","root":"{}"}}"#, long(4097)),
            "too_big",
            json!(["root"]),
            "String must contain at most 4096 character(s)",
        ),
    ]);
}

/// MINOR. Every key of these shapes answers a wrong JSON type with the type zod expects ("Expected string, received
/// number"); the wsp-frames types describe nothing, so the issue says "Expected unknown". About 25 types in all
/// (the lead's finding 8 names the generated ones); these are the daemon's.
#[test]
fn a_re_exported_key_names_the_type_zod_expects() {
    none_differ(vec![
        refused::<FsEntry>(
            r#"{"name":"n","type":"file","size":"1","mtime":1}"#,
            "invalid_type",
            json!(["size"]),
            "Expected number, received string",
        ),
        refused::<FsEntry>(
            r#"{"name":"n","type":5,"size":1,"mtime":1}"#,
            "invalid_type",
            json!(["type"]),
            "Expected 'file' | 'dir' | 'symlink', received number",
        ),
        refused::<FsListReply>(
            r#"{"entries":[],"truncated":"x","total":1}"#,
            "invalid_type",
            json!(["truncated"]),
            "Expected boolean, received string",
        ),
        refused::<GitCheckpointReply>(
            r#"{"ref":"r","commit":"c","changed":"x","based":true}"#,
            "invalid_type",
            json!(["changed"]),
            "Expected boolean, received string",
        ),
        refused::<CgroupMemory>(r#"{"cgroup":"c","memory":"x"}"#, "invalid_type", json!(["memory"]), "Expected number, received string"),
        refused::<PipeOpen>(
            r#"{"op":"pipe.open","door":5,"heldMs":1}"#,
            "invalid_type",
            json!(["door"]),
            "Expected 'http' | 'gh', received number",
        ),
        refused::<PipeOpen>(
            r#"{"op":"pipe.open","door":"http","heldMs":2.5}"#,
            "invalid_type",
            json!(["heldMs"]),
            "Expected integer, received float",
        ),
        refused::<GitLocalBranch>(
            r#"{"name":"n","oid":"o","committed":1.5}"#,
            "invalid_type",
            json!(["committed"]),
            "Expected integer, received float",
        ),
        refused::<HttpPostReply>(r#"{"status":200.5,"body":""}"#, "invalid_type", json!(["status"]), "Expected integer, received float"),
    ]);
}

/// MINOR. zod answers the first failing key in the schema's order whatever order the input lists them; the wsp-frames
/// types read in the input's order and stop at the first error. daemon.ts line in each label.
#[test]
fn a_re_exported_reply_answers_the_first_key_of_its_schema() {
    none_differ(vec![
        // :436 cgroup before memory
        refused::<CgroupMemory>(r#"{"memory":"1","cgroup":5}"#, "invalid_type", json!(["cgroup"]), "Expected string, received number"),
        // :148 ref before commit
        refused::<GitCheckpointReply>(
            r#"{"commit":5,"ref":5,"changed":true,"based":true}"#,
            "invalid_type",
            json!(["ref"]),
            "Expected string, received number",
        ),
        // :195 files before outside
        refused::<FsHashReply>(
            r#"{"outside":{},"files":[],"past":["a"],"special":["a"]}"#,
            "invalid_type",
            json!(["files"]),
            "Expected object, received array",
        ),
        // :161 path before branch
        refused::<GitWorktreesReply>(
            r#"{"worktrees":[{"branch":5,"path":5}]}"#,
            "invalid_type",
            json!(["worktrees", 0, "path"]),
            "Expected string, received number",
        ),
        // :132 files before truncated
        refused::<FsFilesReply>(r#"{"truncated":"true","files":{}}"#, "invalid_type", json!(["files"]), "Expected array, received object"),
    ]);
}

/// MINOR. An array where an object is due is `Expected object, received array` in zod; serde's derive reads a struct
/// from an array by position, so the wsp-frames types answer `custom` "invalid length 0, expected struct X with N
/// elements" (review_core.rs pins CgroupMemory only; the cause is every re-export).
#[test]
fn a_re_exported_reply_refuses_an_array_as_no_object() {
    let msg = "Expected object, received array";
    none_differ(vec![
        refused::<FsFilesReply>("[]", "invalid_type", json!([]), msg),
        refused::<FsHashReply>("[]", "invalid_type", json!([]), msg),
        refused::<GitCheckpointReply>("[]", "invalid_type", json!([]), msg),
        refused::<GitDiscardReply>("[]", "invalid_type", json!([]), msg),
        refused::<GitWorktreesReply>("[]", "invalid_type", json!([]), msg),
        refused::<GitWorktreesReply>(r#"{"worktrees":[[]]}"#, "invalid_type", json!(["worktrees", 0]), msg),
        refused::<PipeOpen>("[]", "invalid_type", json!([]), msg),
        refused::<HttpBearer>("[]", "invalid_type", json!([]), msg),
        refused::<UsageStore>("[]", "invalid_type", json!([]), msg),
        op_refused(r#"{"id":1,"op":"usage.logs","stores":[[]]}"#, "invalid_type", json!(["stores", 0]), msg),
        op_refused(
            r#"{"id":1,"op":"http.post","url":"http://a.test/x","headers":{},"body":"","bearer":[]}"#,
            "invalid_type",
            json!(["bearer"]),
            msg,
        ),
    ]);
}

// ---------------------------------------------------------------------------------------------------------------
// Local types
// ---------------------------------------------------------------------------------------------------------------

/// MINOR. z.literal answers invalid_literal for any value that is not it, a number or null included (types.cjs
/// ZodLiteral): daemon.ts:78 `op: z.literal("auth")`, devices.ts:26 `kind: z.literal("account")`, daemon.ts:92
/// `op: z.literal("pipe.open")`. The Rust reads the key as a String first, so a non-string is invalid_type, and
/// PipeOpen's own tag answers `Required` or invalid_enum_value.
#[test]
fn a_literal_key_answers_invalid_literal_for_any_value() {
    let auth = "Invalid literal value, expected \"auth\"";
    let account = "Invalid literal value, expected \"account\"";
    let pipe = "Invalid literal value, expected \"pipe.open\"";
    none_differ(vec![
        refused::<DaemonAuthRequest>(r#"{"id":1,"op":5,"token":"t"}"#, "invalid_literal", json!(["op"]), auth),
        refused::<DaemonAuthRequest>(r#"{"id":1,"op":null,"token":"t"}"#, "invalid_literal", json!(["op"]), auth),
        refused::<DeviceVia>(
            r#"{"kind":5,"relayDeviceId":"r","fingerprint":"f","publicKey":"p","admittedBy":"a"}"#,
            "invalid_literal",
            json!(["kind"]),
            account,
        ),
        refused::<DeviceVia>(
            r#"{"kind":null,"relayDeviceId":"r","fingerprint":"f","publicKey":"p","admittedBy":"a"}"#,
            "invalid_literal",
            json!(["kind"]),
            account,
        ),
        refused::<PipeOpen>(r#"{"door":"http","heldMs":1}"#, "invalid_literal", json!(["op"]), pipe),
        refused::<PipeOpen>(r#"{"op":"zzz","door":"http","heldMs":1}"#, "invalid_literal", json!(["op"]), pipe),
        refused::<PipeOpen>(r#"{"op":5,"door":"http","heldMs":1}"#, "invalid_literal", json!(["op"]), pipe),
    ]);
}

/// MINOR. daemon.ts:934 `message: z.unknown()` on guest.reply, as :930 on guest.send: z.unknown() takes an absent key.
/// review_core.rs pins guest.send; guest.reply is the same cause with its own field.
#[test]
fn a_guest_reply_takes_an_absent_message() {
    none_differ(vec![reads_as::<DaemonRequest>(
        r#"{"id":1,"op":"guest.reply","session":"s"}"#,
        r#"{"id":1,"op":"guest.reply","session":"s"}"#,
    )]);
}

/// MINOR. daemon.ts:968-971 GuestCliMessage is `z.union([{stream, text}, {exit: z.number().int()}])`. A fraction in
/// `exit` is the second member's only issue, which zod's union keeps because `.int()` marks the parse dirty, not
/// aborted (the lead's finding 7 pins PlaceCap and DaemonResponse; this union has the same cause).
#[test]
fn a_cli_message_with_a_fractional_exit_answers_the_integer_rule() {
    none_differ(vec![refused::<GuestCliMessage>(r#"{"exit":1.5}"#, "invalid_type", json!(["exit"]), "Expected integer, received float")]);
}

/// MINOR. helpers.ts:11 `PermissionEffect = z.enum([...])`: a null or a number is `Expected 'allow' | 'deny' | 'mode' |
/// 'answer', received null`. PermissionEffect has no `describe`, so it says "Expected unknown" (PermissionOption.effect
/// the same).
#[test]
fn a_permission_effect_names_its_words() {
    let msg = |got: &str| format!("Expected 'allow' | 'deny' | 'mode' | 'answer', received {got}");
    none_differ(vec![
        refused::<PermissionEffect>("null", "invalid_type", json!([]), &msg("null")),
        refused::<PermissionEffect>("5", "invalid_type", json!([]), &msg("number")),
        refused::<wsp_wire::wire::helpers::PermissionOption>(
            r#"{"id":"a","label":"l","effect":5}"#,
            "invalid_type",
            json!(["effect"]),
            &msg("number"),
        ),
    ]);
}

/// DECISION-5. daemon.ts:516-959: zod walks an op's keys in order, so the first key that fails by type or by rule is
/// the answer. The enum runs its rules after every key has read clean, so a rule on an earlier key answers after a
/// type issue on a later one. The sweep found 95 such cases in 26 ops (ports.watch, usage.logs, transcripts.list,
/// transcripts.read, proc.kill, git.push, git.pr, git.prRead, git.prView, git.runLog, git.prMerge, git.issueRead,
/// git.prCheckout, git.prDiff, git.prReview, git.prReply, git.prResolve, git.prReact, git.update, git.startOn,
/// git.mergeIn, tunnel.open, ssh.start, http.post, guest.open, place.update); review_core.rs pins the same rule for
/// pair.redeem and machine.exec.
#[test]
fn an_op_answers_its_first_key_that_fails_by_type_or_by_rule() {
    let zeros = "0".repeat(64);
    none_differ(vec![
        op_refused(
            r#"{"id":1,"op":"tunnel.open","tunnelId":"t","port":0,"machineId":5}"#,
            "too_small",
            json!(["port"]),
            "Number must be greater than or equal to 1",
        ),
        op_refused(
            r#"{"id":1,"op":"git.prRead","cwd":"c","remote":"r","number":-1,"seen":5}"#,
            "too_small",
            json!(["number"]),
            "Number must be greater than or equal to 0",
        ),
        op_refused(
            &format!(r#"{{"id":1,"op":"place.update","uploadId":"","seq":"1","last":true,"data":"d","sha256":"{zeros}"}}"#),
            "too_small",
            json!(["uploadId"]),
            "String must contain at least 1 character(s)",
        ),
        op_refused(
            &format!(r#"{{"id":1,"op":"guest.open","kind":"cli","token":"{}","turnToken":5,"argv":[],"cwd":""}}"#, "x".repeat(513)),
            "too_big",
            json!(["token"]),
            "String must contain at most 512 character(s)",
        ),
        op_refused(r#"{"id":1,"op":"proc.kill","pid":0,"signal":5}"#, "too_small", json!(["pid"]), "Number must be greater than 0"),
        op_refused(r#"{"id":1,"op":"http.post","url":"ftp://x","headers":5,"body":""}"#, "custom", json!(["url"]), "http or https URL"),
        op_refused(
            r#"{"id":1,"op":"ssh.start","authorizedKey":"","machineId":5}"#,
            "too_small",
            json!(["authorizedKey"]),
            "String must contain at least 1 character(s)",
        ),
        op_refused(
            r#"{"id":1,"op":"transcripts.read","root":"","dirs":5,"session":"s","last":1}"#,
            "too_small",
            json!(["root"]),
            "String must contain at least 1 character(s)",
        ),
        op_refused(
            r#"{"id":1,"op":"git.prReply","cwd":"c","remote":"r","number":-1,"body":5}"#,
            "too_small",
            json!(["number"]),
            "Number must be greater than or equal to 0",
        ),
        op_refused(
            &format!(r#"{{"id":1,"op":"git.push","cwd":"c","token":"{}","machineId":5}}"#, "x".repeat(513)),
            "too_big",
            json!(["token"]),
            "String must contain at most 512 character(s)",
        ),
    ]);
}

/// MINOR (written JSON). A reqId that is a number is written as JavaScript writes it: 1, 100, 0. wsp-frames'
/// RequestId (helpers.rs `ReqId`) keeps the float JSON read, so `1.0`, `1e2` and `-0` come back `1.0`, `100.0`, `-0.0`
/// (DaemonAuthRequest and every op). Ids a JavaScript client writes are never these; a Python or Go client could.
#[test]
fn a_request_id_is_written_as_javascript_writes_a_number() {
    none_differ(vec![
        writes::<DaemonAuthRequest>(r#"{"id":1.0,"op":"auth","token":"t"}"#, r#"{"id":1,"op":"auth","token":"t"}"#),
        writes::<DaemonAuthRequest>(r#"{"id":1e2,"op":"auth","token":"t"}"#, r#"{"id":100,"op":"auth","token":"t"}"#),
        writes::<DaemonAuthRequest>(r#"{"id":-0,"op":"auth","token":"t"}"#, r#"{"id":0,"op":"auth","token":"t"}"#),
    ]);
}

/// MINOR (written JSON). devices.ts:201 DaemonFrame and :215 `event` are `.passthrough()`: the parsed object holds its
/// known key and the rest, and JavaScript lists integer-like keys first in ascending order, before `op` and `type`.
/// `rest` is ordered that way (js_record) but `op` and `type` are struct fields written before it. Also `__proto__`:
/// zod's parse drops that own key, the Rust keeps it.
#[test]
fn a_passthrough_frame_lists_integer_like_keys_before_its_known_key() {
    none_differ(vec![
        writes::<DaemonFrame>(r#"{"op":"x","10":1,"2":2,"b":3,"a":4}"#, r#"{"2":2,"10":1,"op":"x","b":3,"a":4}"#),
        writes::<DaemonChannelEvent>(
            r#"{"type":"daemon.event","channel":"c","event":{"type":"t","b":1,"2":2,"a":3,"1":1}}"#,
            r#"{"type":"daemon.event","channel":"c","event":{"1":1,"2":2,"type":"t","b":1,"a":3}}"#,
        ),
        writes::<DaemonFrame>(r#"{"op":"x","__proto__":{"a":1}}"#, r#"{"op":"x"}"#),
    ]);
}

/// MINOR. daemon.ts:99-107 pipeOpenOf is JSON.parse then PipeOpen.safeParse. JSON.parse reads 5.0, 5e0, 1E3 and -0 as
/// the integers they are, and any nesting depth node's stack takes; the Rust reads heldMs as a u64 (a float refuses it)
/// and serde_json stops at depth 128, so a frame that carries a deep unknown key reads as no pipe.
#[test]
fn a_pipe_open_text_reads_as_json_parse_reads_it() {
    let deep = |n: usize| format!(r#"{{"op":"pipe.open","door":"http","heldMs":1,"x":{}{}}}"#, "[".repeat(n), "]".repeat(n));
    let mut found = Vec::new();
    for (text, wrote) in [
        (r#"{"op":"pipe.open","door":"http","heldMs":5.0}"#.to_string(), r#"{"op":"pipe.open","door":"http","heldMs":5}"#),
        (r#"{"op":"pipe.open","door":"http","heldMs":5e0}"#.to_string(), r#"{"op":"pipe.open","door":"http","heldMs":5}"#),
        (r#"{"op":"pipe.open","door":"http","heldMs":1E3}"#.to_string(), r#"{"op":"pipe.open","door":"http","heldMs":1000}"#),
        (r#"{"op":"pipe.open","door":"http","heldMs":-0}"#.to_string(), r#"{"op":"pipe.open","door":"http","heldMs":0}"#),
        (deep(200), r#"{"op":"pipe.open","door":"http","heldMs":1}"#),
    ] {
        match pipe_open_of(&text) {
            Some(open) if serde_json::to_string(&open).unwrap() == wrote => {}
            other => found.push(format!("pipe_open_of({}): node answers {wrote}, here {other:?}", &text[..text.len().min(70)])),
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// MINOR. limits.ts:114-122: node's `new URL` takes a host that starts with, or holds, an Arabic-Indic digit
/// (U+0661), a letter of Arabic Extended-B (U+0870), and a path, query, fragment or userinfo holding U+180E, which is
/// not in `\s` since Unicode 6.3. The url crate refuses the first two (its bidi check), and is_http_url refuses
/// U+180E everywhere because of a workaround meant for a host (limits.rs:99). 58 code points of the first kind in a
/// host label, 207 as the first character.
#[test]
fn a_host_node_takes_is_taken_here() {
    let mut found = Vec::new();
    for (url, host) in [
        ("http://\u{661}.com", Some("xn--9hb.com")),
        ("http://a\u{661}.com", Some("xn--a-bqc.com")),
        ("http://a\u{870}b.com/", Some("xn--ab-h8e.com")),
        ("http://a.com/\u{180e}", Some("a.com")),
        ("http://a.com?\u{180e}", Some("a.com")),
        ("http://a.com#\u{180e}", Some("a.com")),
        ("http://u\u{180e}@a.com", Some("a.com")),
    ] {
        if !is_http_url(url) {
            found.push(format!("is_http_url({url:?}): node answers true, here false"));
        }
        if host_of(url).as_deref() != host {
            found.push(format!("host_of({url:?}): node answers {host:?}, here {:?}", host_of(url)));
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// MINOR. limits.ts:143-150 hostOf is `new URL(url).host`: node maps U+1E9E (capital sharp s) to "ss"; the url crate
/// keeps the sharp s (punycode `xn--ab-gia`). And a U+180E in the host is a refusal in node (undefined) where the url
/// crate drops it ("a.com"), the other half of the workaround above.
#[test]
fn a_host_is_mapped_as_node_maps_it() {
    let mut found = Vec::new();
    for (url, host) in [
        ("http://a\u{1e9e}b.com", Some("assb.com")),
        ("http://a\u{180e}b.com", None),
        ("http://\u{180e}a.com", None),
        ("http://a.com\u{180e}", None),
    ] {
        if host_of(url).as_deref() != host {
            found.push(format!("host_of({url:?}): node answers {host:?}, here {:?}", host_of(url)));
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// BLOCKER by the brief's rule (a guard accepts what the TS refuses), same cause as the host test above and the same
/// small reach. limits.ts:172-188: `new URL(redirect).hostname` of `localhost` plus U+2061, U+3164 or a soft-hyphen-like
/// code point is not `localhost` in node, so the sign-in page does not come back to the machine and callbackPortOf has
/// no port; the `url` crate maps the character away, so redirects_to_machine answers true and callback_port_of 5000.
/// A fuzz found 24 such code points after `localhost` and one in an IPv4 label (U+1CCF1, Unicode 16, which flips on a
/// newer node).
#[test]
fn a_redirect_to_a_host_node_does_not_read_as_loopback_is_not_loopback_here() {
    let mut found = Vec::new();
    for c in ["%E2%81%A1", "%E3%85%A4", "%E1%85%9F", "%E1%9E%B4"] {
        let url = format!("https://auth.example.com/authorize?redirect_uri=http%3A%2F%2Flocalhost{c}%3A5000%2Fcb");
        if redirects_to_machine(&url) {
            found.push(format!("redirects_to_machine(..localhost{c}:5000..): node answers false, here true"));
        }
        if callback_port_of(&url).is_some() {
            found.push(format!("callback_port_of(..localhost{c}:5000..): node answers undefined, here {:?}", callback_port_of(&url)));
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}
