// SPDX-License-Identifier: AGPL-3.0-only
//! Inputs where the daemon side of the wire still answers unlike zod 3.25.76 or the TS host after the fix round, found in
//! the re-review of wsp-wire (#2445, fix round at 4abcbdf7b). Every expectation is what zod's safeParse, JSON.parse or the
//! TS function answered for the same input (node 22.23.4, an esbuild bundle of packages/protocol/src at a3b870a6e).

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_wire::types::check::{issues_of, parse, Check};
use wsp_wire::types::pull_request::GitRepoReadReply;
use wsp_wire::types::start::{GitPrDiffReply, GitPrReviewReply};
use wsp_wire::types::tree::GitStartOnReply;
use wsp_wire::wire::daemon::{pipe_open_of, DaemonRequest, ProcListReply, ProcScope, SshStartReply};
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::machine_link::ProcContainer;

fn name<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap_or("?")
}

/// zod refused `text` with this first issue; None where parse answers the same.
fn refused<T: DeserializeOwned + Serialize + Check>(text: &str, code: &str, path: Value, message: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), text.chars().take(120).collect::<String>());
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
    let label = format!("{} {}", name::<T>(), text.chars().take(120).collect::<String>());
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
    let label = format!("{} {}", name::<T>(), text.chars().take(120).collect::<String>());
    match parse::<T>(&input) {
        Ok(read) if serde_json::to_value(&read).ok() == Some(wrote.clone()) => None,
        Ok(read) => Some(format!("{label}: zod's data is {wrote}, here {:?}", serde_json::to_value(&read))),
        Err(e) => Some(format!("{label}: zod took it, refused here: {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

/// zod listed exactly `want` (code, path, message) for `text`; `issues_of` is what the host reads when it sends every issue.
fn listed<T: DeserializeOwned + Serialize + Check>(text: &str, want: &[(&str, Value, &str)]) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let label = format!("{} {}", name::<T>(), text.chars().take(120).collect::<String>());
    let got: Vec<(String, Value, String)> = issues_of::<T>(&input).into_iter().map(|e| (e.code, Value::Array(e.path), e.message)).collect();
    let want: Vec<(String, Value, String)> = want.iter().map(|(c, p, m)| (c.to_string(), p.clone(), m.to_string())).collect();
    (got != want).then(|| format!("{label}: zod lists {want:?}, here {got:?}"))
}

fn none_differ(found: Vec<Option<String>>) {
    let differ: Vec<String> = found.into_iter().flatten().collect();
    assert!(differ.is_empty(), "{} answer(s) unlike zod's:\n{}", differ.len(), differ.join("\n"));
}

/// MINOR 1 / MINOR 2, left open on the daemon side. zod's `.int()` leaves the parse dirty and goes on, so a fraction past
/// a bound is two issues (daemon.ts:550, 581, 595, 598, 628, 886, 905, 396; daemon-events.ts:49, 52, 63; limits.ts:153).
/// The first issue is right everywhere; `issues_of` lists only the integer one for a key read through `daemon_checks::push`
/// or `list` (daemon_checks.rs:23, 47), which never run the key's bound on the dirty value, for `RelayPort`, which has no
/// `dirty` (limits.rs:56), and for the integers of a list inside `ProcScope` (`Vec` has no `dirty`, check.rs:160).
/// A scalar key of a reply or of the exec request does it right (`SshStartReply.port`, `DaemonExecRequest.timeoutMs`).
#[test]
fn a_fraction_past_a_bound_is_two_issues_on_a_daemon_frame() {
    none_differ(vec![
        listed::<DaemonRequest>(
            r#"{"id":1,"op":"tunnel.open","tunnelId":"t","port":0.5}"#,
            &[
                ("invalid_type", json!(["port"]), "Expected integer, received float"),
                ("too_small", json!(["port"]), "Number must be greater than or equal to 1"),
            ],
        ),
        listed::<DaemonRequest>(
            r#"{"id":1,"op":"tunnel.open","tunnelId":"t","port":65535.5}"#,
            &[
                ("invalid_type", json!(["port"]), "Expected integer, received float"),
                ("too_big", json!(["port"]), "Number must be less than or equal to 65535"),
            ],
        ),
        listed::<DaemonRequest>(
            r#"{"id":1,"op":"ports.watch","roots":[-0.5]}"#,
            &[
                ("invalid_type", json!(["roots", 0]), "Expected integer, received float"),
                ("too_small", json!(["roots", 0]), "Number must be greater than or equal to 0"),
            ],
        ),
        listed::<DaemonRequest>(
            r#"{"id":1,"op":"proc.inspect","pid":-0.5}"#,
            &[
                ("invalid_type", json!(["pid"]), "Expected integer, received float"),
                ("too_small", json!(["pid"]), "Number must be greater than 0"),
            ],
        ),
        listed::<DaemonRequest>(
            r#"{"id":1,"op":"proc.kill","pid":-0.5,"signal":"TERM"}"#,
            &[
                ("invalid_type", json!(["pid"]), "Expected integer, received float"),
                ("too_small", json!(["pid"]), "Number must be greater than 0"),
            ],
        ),
        listed::<DaemonRequest>(
            r#"{"id":1,"op":"proc.scopes","scopes":[{"thread":"t","agents":[-0.5]}]}"#,
            &[
                ("invalid_type", json!(["scopes", 0, "agents", 0]), "Expected integer, received float"),
                ("too_small", json!(["scopes", 0, "agents", 0]), "Number must be greater than or equal to 0"),
            ],
        ),
        listed::<DaemonRequest>(
            r#"{"id":1,"op":"http.post","url":"http://a.b/","headers":{},"body":"","timeoutMs":-0.5}"#,
            &[
                ("invalid_type", json!(["timeoutMs"]), "Expected integer, received float"),
                ("too_small", json!(["timeoutMs"]), "Number must be greater than 0"),
            ],
        ),
        listed::<DaemonRequest>(
            r#"{"id":1,"op":"transcripts.read","root":"r","dirs":[],"session":"s","last":-0.5}"#,
            &[
                ("invalid_type", json!(["last"]), "Expected integer, received float"),
                ("too_small", json!(["last"]), "Number must be greater than 0"),
            ],
        ),
        listed::<DaemonEvent>(
            r#"{"type":"callback.port","port":2.5}"#,
            &[
                ("invalid_type", json!(["port"]), "Expected integer, received float"),
                ("too_small", json!(["port"]), "Number must be greater than or equal to 1024"),
            ],
        ),
        listed::<DaemonEvent>(
            r#"{"type":"browser.open","url":"http://a.b/","port":70000.5}"#,
            &[
                ("invalid_type", json!(["port"]), "Expected integer, received float"),
                ("too_big", json!(["port"]), "Number must be less than or equal to 65535"),
            ],
        ),
        listed::<ProcScope>(
            r#"{"thread":"t","groups":[-0.5]}"#,
            &[
                ("invalid_type", json!(["groups", 0]), "Expected integer, received float"),
                ("too_small", json!(["groups", 0]), "Number must be greater than or equal to 0"),
            ],
        ),
    ]);
}

/// MINOR 14 and 16, left open for the three records of a daemon frame (`pty.create` and `stdio.open` env, `http.post`
/// headers: daemon.ts:530, 898, 907). zod's record drops `__proto__` and JavaScript writes the keys that read as array
/// indexes first; `DaemonRequest` keeps `__proto__` and writes the keys in the order they came (proc_sort.rs:370, 592, 600
/// are plain `IndexMap`s; `kept::record` and `js_record` exist and `FsHashReply.files` uses them). `headers` also counts
/// the dropped key toward its 64, so a frame zod takes is refused (daemon_checks.rs:182, proc_scope_pid.rs:216).
#[test]
fn a_daemon_frame_record_drops_a_proto_key_and_writes_index_keys_first() {
    let mut proto_and_64 = String::from(r#"{"id":1,"op":"http.post","url":"http://a.b/","headers":{"__proto__":"p""#);
    let mut kept = serde_json::Map::new();
    for i in 0..64 {
        proto_and_64.push_str(&format!(r#","k{i}":"v""#));
        kept.insert(format!("k{i}"), json!("v"));
    }
    proto_and_64.push_str(r#"},"body":""}"#);
    none_differ(vec![
        read_as::<DaemonRequest>(
            r#"{"id":1,"op":"pty.create","env":{"b":"x","10":"y","a":"z","2":"w"}}"#,
            r#"{"op":"pty.create","env":{"2":"w","10":"y","b":"x","a":"z"},"id":1}"#,
        ),
        read_as::<DaemonRequest>(
            r#"{"id":1,"op":"pty.create","env":{"b":"x","__proto__":"p"}}"#,
            r#"{"op":"pty.create","env":{"b":"x"},"id":1}"#,
        ),
        read_as::<DaemonRequest>(
            r#"{"id":1,"op":"stdio.open","stdioId":"s","cmd":"c","env":{"b":"x","10":"y","__proto__":"p","2":"w"}}"#,
            r#"{"op":"stdio.open","stdioId":"s","cmd":"c","env":{"2":"w","10":"y","b":"x"},"id":1}"#,
        ),
        read_as::<DaemonRequest>(
            r#"{"id":1,"op":"http.post","url":"http://a.b/","headers":{"b":"x","10":"y","__proto__":"p","2":"w"},"body":""}"#,
            r#"{"op":"http.post","url":"http://a.b/","headers":{"2":"w","10":"y","b":"x"},"body":"","id":1}"#,
        ),
        takes::<DaemonRequest>(&proto_and_64, json!({"op": "http.post", "url": "http://a.b/", "headers": kept, "body": "", "id": 1})),
    ]);
}

/// MINOR 1, the rest of what the fix round's notes call a known difference: a string key with three rules lists each one
/// it breaks (daemon.ts:292). `SshStartReply.hostKey` is `.min(1).max(SSH_KEY_MAX).regex(...)`; `rule_host_key` returns the
/// first failing rule only (daemon_replies.rs:1155), so `issues_of` lists one issue for an empty key and for a key past 1,024
/// units that is also not one line. `parse` answers the first issue and is right. The one consumer that reads every issue,
/// `sshStarted` (host/src/relay.ts:249), gets the same answer either way, so this is only the listing.
#[test]
fn an_ssh_host_key_lists_every_rule_it_breaks() {
    let astral = format!(r#"{{"port":22,"hostKey":"ssh-ed25519 AAAA {}a"}}"#, "\u{1F600}".repeat(504));
    none_differ(vec![
        listed::<SshStartReply>(
            r#"{"port":22,"hostKey":""}"#,
            &[
                ("too_small", json!(["hostKey"]), "String must contain at least 1 character(s)"),
                ("invalid_string", json!(["hostKey"]), "Invalid"),
            ],
        ),
        listed::<SshStartReply>(
            &astral,
            &[
                ("too_big", json!(["hostKey"]), "String must contain at most 1024 character(s)"),
                ("invalid_string", json!(["hostKey"]), "Invalid"),
            ],
        ),
        listed::<SshStartReply>(
            r#"{"port":0.5,"hostKey":""}"#,
            &[
                ("invalid_type", json!(["port"]), "Expected integer, received float"),
                ("too_small", json!(["port"]), "Number must be greater than or equal to 1"),
                ("too_small", json!(["hostKey"]), "String must contain at least 1 character(s)"),
                ("invalid_string", json!(["hostKey"]), "Invalid"),
            ],
        ),
    ]);
}

/// `pipeOpenOf` (daemon.ts:98) reads its text with JSON.parse and keeps the frame whatever an ignored key holds. serde_json
/// refuses a lone surrogate escape and a number past f64 in any key, so `pipe_open_of` answers None for a text node reads
/// as a PipeOpen (json_text.rs:9 handles depth only). The daemon never writes such a text today.
#[test]
fn a_pipe_open_text_json_parse_takes_is_taken_here() {
    let open = |door: &str, held: i64| Some(format!(r#"{{"op":"pipe.open","door":"{door}","heldMs":{held}}}"#));
    let read = |text: &str| pipe_open_of(text).map(|p| serde_json::to_string(&p).unwrap());
    let mut differ = Vec::new();
    for (text, want) in [
        (r#"{"op":"pipe.open","door":"http","heldMs":1,"zzz":"\ud800"}"#, open("http", 1)),
        (r#"{"op":"pipe.open","door":"http","heldMs":1,"s":"\ude00\ud83d"}"#, open("http", 1)),
        (r#"{"op":"pipe.open","door":"gh","heldMs":2,"zzz":1e999}"#, open("gh", 2)),
    ] {
        let got = read(text);
        if got != want {
            differ.push(format!("pipeOpenOf {text}: node answers {want:?}, here {got:?}"));
        }
    }
    assert!(differ.is_empty(), "{} answer(s) unlike node's:\n{}", differ.len(), differ.join("\n"));
}

/// M2 / MINOR 1, left for the four daemon replies that are still `pub use wsp_frames::...` (types/start.rs:367, 409,
/// types/tree.rs:15; the fifth, `GitPrCheckoutReply`, has one key and is right). start.ts:92, 100 and tree.ts:12 are zod
/// objects, which read their keys in schema order and list every issue. The frames type has no `pass`, so serde's one error
/// answers, and serde walks the keys in the order the input gives them and reports a missing key last. A frame with two broken
/// keys in the other order answers the later key; `issues_of` stops at the first error, so a list of two bad items lists one.
/// The first review found this on the 40 re-exports in src/wire and spec/shapes.md rule 6 passed these four.
#[test]
fn a_daemon_reply_still_re_exported_from_wsp_frames_answers_the_first_key_of_its_schema() {
    none_differ(vec![
        refused::<GitPrDiffReply>(
            r#"{"truncated":"x","diff":5,"left":[]}"#,
            "invalid_type",
            json!(["diff"]),
            "Expected string, received number",
        ),
        refused::<GitPrDiffReply>(r#"{"truncated":null,"left":{}}"#, "invalid_type", json!(["diff"]), "Required"),
        refused::<GitStartOnReply>(r#"{"oid":5,"branch":6}"#, "invalid_type", json!(["branch"]), "Expected string, received number"),
        refused::<GitStartOnReply>(r#"{"oid":5}"#, "invalid_type", json!(["branch"]), "Required"),
        refused::<GitPrReviewReply>(r#"{"folded":5,"url":6}"#, "invalid_type", json!(["url"]), "Expected string, received number"),
        listed::<GitPrDiffReply>(
            r#"{"truncated":null,"left":{}}"#,
            &[
                ("invalid_type", json!(["diff"]), "Required"),
                ("invalid_type", json!(["truncated"]), "Expected boolean, received null"),
                ("invalid_type", json!(["left"]), "Expected array, received object"),
            ],
        ),
        listed::<GitPrDiffReply>(
            r#"{"diff":"a","truncated":true,"left":[1,2]}"#,
            &[
                ("invalid_type", json!(["left", 0]), "Expected string, received number"),
                ("invalid_type", json!(["left", 1]), "Expected string, received number"),
            ],
        ),
        listed::<GitPrReviewReply>(
            r#"{}"#,
            &[("invalid_type", json!(["url"]), "Required"), ("invalid_type", json!(["folded"]), "Required")],
        ),
        listed::<GitStartOnReply>(
            r#"{"oid":5,"branch":6}"#,
            &[
                ("invalid_type", json!(["branch"]), "Expected string, received number"),
                ("invalid_type", json!(["oid"]), "Expected string, received number"),
            ],
        ),
    ]);
}

/// MINOR 7, left for a frames re-export (types/pull_request/mod.rs:21): `GitRepoReadReply` has no `pass`, so serde reads the
/// enum words `methods` and `defaultMethod` hold, and serde reads the map `{"merge":null}` as the word `merge`. zod's
/// `z.enum` takes a string only (pull-request.ts GitRepoReadReply), so the reply is refused with invalid_type. An object that
/// names no word answers invalid_enum_value here and invalid_type in zod. GitPrMergeReply, GitPrResolveReply and GitRunLogReply
/// of the same file (the other frames re-exports) list one issue where zod lists each, the cause the test above names.
#[test]
fn a_repo_read_reply_refuses_an_enum_word_given_as_a_map() {
    none_differ(vec![
        refused::<GitRepoReadReply>(
            r#"{"methods":[{"merge":null}],"defaultMethod":"merge","autoMerge":true}"#,
            "invalid_type",
            json!(["methods", 0]),
            "Expected 'merge' | 'squash' | 'rebase', received object",
        ),
        refused::<GitRepoReadReply>(
            r#"{"methods":["merge"],"defaultMethod":{"squash":null},"autoMerge":true}"#,
            "invalid_type",
            json!(["defaultMethod"]),
            "Expected 'merge' | 'squash' | 'rebase', received object",
        ),
        refused::<GitRepoReadReply>(
            r#"{"methods":[{"a":1}],"defaultMethod":"merge","autoMerge":true}"#,
            "invalid_type",
            json!(["methods", 0]),
            "Expected 'merge' | 'squash' | 'rebase', received object",
        ),
    ]);
}

/// M1 read for its own reason, not only on the words the old tests try. zod's `.catch` takes anything that is not a whole
/// value of the enum, and a map is not a string (daemon.ts:380, 382; machine-link.ts:647): `kind: {"agent":null}` reads as
/// `other`, an owner whose `by` is `{"root":null}` reads as no owner, `placedBy: {"label":null}` reads as absent. The Rust
/// catch is `serde_json::from_value::<ProcKind>(..).unwrap_or(Other)` (git_pr_review_comment.rs:309, 315; helpers.rs `or_none`),
/// and serde reads the map `{"agent":null}` as the unit variant `agent`. The same cause as MINOR 7, new on the catch keys.
#[test]
fn a_catch_key_given_an_enum_as_a_map_reads_as_zod_catches_it() {
    none_differ(vec![
        read_as::<ProcListReply>(
            r#"{"at":1,"daemon":1,"total":1,"procs":[{"pid":1,"ppid":1,"user":"u","state":"S","comm":"c","cmdline":"x","cpu":0,"rss":1,"startedAt":1,"kind":{"agent":null}}]}"#,
            r#"{"at":1,"daemon":1,"total":1,"procs":[{"pid":1,"ppid":1,"user":"u","state":"S","comm":"c","cmdline":"x","cpu":0,"rss":1,"startedAt":1,"kind":"other"}]}"#,
        ),
        read_as::<ProcListReply>(
            r#"{"at":1,"daemon":1,"total":1,"procs":[{"pid":1,"ppid":1,"user":"u","state":"S","comm":"c","cmdline":"x","cpu":0,"rss":1,"startedAt":1,"owner":{"by":{"root":null},"thread":"t"}}]}"#,
            r#"{"at":1,"daemon":1,"total":1,"procs":[{"pid":1,"ppid":1,"user":"u","state":"S","comm":"c","cmdline":"x","cpu":0,"rss":1,"startedAt":1}]}"#,
        ),
        read_as::<ProcContainer>(
            r#"{"id":"i","name":"n","image":"im","state":"s","placedBy":{"label":null}}"#,
            r#"{"id":"i","name":"n","image":"im","state":"s"}"#,
        ),
    ]);
}
