// SPDX-License-Identifier: AGPL-3.0-only
//! Causes a differential sweep of the shape types in src/types, src/views and the rest of src/wire found after the fix round
//! (#2445, fix round at 4abcbdf7b). A node generator walked the 701 zod schemas of packages/protocol/src (zod 3.25.76, node
//! 22.23.4) and broke each valid value one key at a time and two at a time; 992,126 cases ran against the crate and these
//! are the causes outside the accepted differences, each as the literal input node was given. Every expectation is what
//! zod's safeParse answered.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_wire::types::check::{parse, Check};

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

fn none_differ(found: Vec<Option<String>>) {
    let differ: Vec<String> = found.into_iter().flatten().collect();
    assert!(differ.is_empty(), "{} answer(s) unlike zod's:\n{}", differ.len(), differ.join("\n"));
}

/// zod's z.record drops a __proto__ key (ZodRecord's mergeObjectSync, zod 3.25.76 types.js); JSON.stringify writes the keys that
/// read as array indexes first and a number as JavaScript does (`5.0` is `5`, `-0` is `0`, 2^53+1 is 2^53). `InitKeys`
/// (init-job.ts:142 `z.record(z.boolean())`; src/types/init_job.rs:569) and `SlateWireValues` (slate/wire.ts:21; src/types/slate_wire.rs:16, a
/// map of `serde_json::Value` where `Kept` is due) are plain `IndexMap` aliases, so a field of either type carries neither
/// `kept::record` nor `js_record` (every other `IndexMap` field has both). Left from the first review (its MINOR 14 and 16 listed
/// other fields). Reaches `InitJob`, `InitSetup`, `SlateValuesEvent` and the events that hold them.
#[test]
fn a_record_alias_drops_a_proto_key_and_writes_index_like_keys_first_and_numbers_as_javascript() {
    none_differ(vec![
        read_as::<wsp_wire::types::init_job::InitKeys>(r##"{"__proto__":true,"a":true}"##, r##"{"a":true}"##),
        read_as::<wsp_wire::types::init_job::InitKeys>(r##"{"b":true,"1":true}"##, r##"{"1":true,"b":true}"##),
        read_as::<wsp_wire::types::slate_wire::SlateWireValues>(r##"{"__proto__":"x","a":"x"}"##, r##"{"a":"x"}"##),
        read_as::<wsp_wire::types::slate_wire::SlateWireValues>(r##"{"b":"x","1":"x"}"##, r##"{"1":"x","b":"x"}"##),
        read_as::<wsp_wire::types::slate_wire::SlateWireValues>(
            r##"{"a":5.0,"b":-0,"c":9007199254740993}"##,
            r##"{"a":5,"b":0,"c":9007199254740992}"##,
        ),
        read_as::<wsp_wire::wire::events_union::EventUnion>(
            r##"{"type":"slate.values","workspaceId":"w","threadId":"t","version":1,"revision":1,"values":{"b":1,"__proto__":3}}"##,
            r##"{"type":"slate.values","workspaceId":"w","threadId":"t","version":1,"revision":1,"values":{"b":1}}"##,
        ),
        read_as::<wsp_wire::wire::events_union::EventUnion>(
            r##"{"type":"slate.values","workspaceId":"w","threadId":"t","version":1,"revision":1,"values":{"a":5.0}}"##,
            r##"{"type":"slate.values","workspaceId":"w","threadId":"t","version":1,"revision":1,"values":{"a":5}}"##,
        ),
        read_as::<wsp_wire::types::init_job::InitJob>(
            r##"{"id":"a","road":"manual","phase":"agent","keys":{"__proto__":true,"a":true},"step":0,"stoppable":true,"screens":[],"rows":[],"progress":{"done":0,"total":0},"log":["a"]}"##,
            r##"{"id":"a","road":"manual","phase":"agent","keys":{"a":true},"step":0,"stoppable":true,"screens":[],"rows":[],"progress":{"done":0,"total":0},"log":["a"]}"##,
        ),
    ]);
}

/// zod's ZodArray._parse checks `.max(n)`, `.min(n)` and `.length(n)` on the array before it parses any element, so a 21-item array
/// with one bad item answers `too_big` at the key. keys.rs `push` (src/types/keys.rs:15) runs a key's rule only on a value that read
/// clean, so the Rust answers the element's `invalid_type` at [key, i]. Reaches every bounded array with an element that can fail:
/// AppProc.origins and .contents (app-procs.ts:30,33), WorkspaceTheme.dots (workspace-look.ts), TerminalConfig.palette
/// (project-bundle.ts), SessionSlateEvent.pieces and every type that holds them. Left from the first review, not on its list.
#[test]
fn an_array_bound_answers_before_the_issues_of_its_elements() {
    none_differ(vec![
        refused::<wsp_wire::views::app_procs::AppProc>(
            r##"{"pid":1,"kind":"main","origins":["a","a","a","a","a","a","a","a","a","a","a","a","a","a","a","a","a","a","a","a",null]}"##,
            "too_big",
            json!(["origins"]),
            "Array must contain at most 20 element(s)",
        ),
        refused::<wsp_wire::views::app_procs::AppProc>(
            r##"{"pid":1,"kind":"main","contents":[5,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,"x"]}"##,
            "too_big",
            json!(["contents"]),
            "Array must contain at most 20 element(s)",
        ),
        refused::<wsp_wire::types::workspace_look::WorkspaceTheme>(
            r##"{"dots":[{"angle":0,"radius":0},{"angle":0,"radius":0},{"angle":0,"radius":0},null],"harmony":"single","grain":0,"opacity":0.1,"mode":"auto"}"##,
            "too_big",
            json!(["dots"]),
            "Array must contain at most 3 element(s)",
        ),
        refused::<wsp_wire::views::project_bundle::TerminalConfig>(
            r##"{"files":["a"],"fontFamily":["a"],"fontSize":1,"theme":"a","background":{"r":0,"g":0,"b":0},"foreground":{"r":0,"g":0,"b":0},"palette":[{"r":0,"g":0,"b":0},"x"],"selectionBackground":{"r":0,"g":0,"b":0},"cursorColor":{"r":0,"g":0,"b":0},"cursorStyle":"block","cursorStyleBlink":true,"windowPaddingX":{"left":0,"right":0},"windowPaddingY":{"top":0,"bottom":0},"backgroundOpacity":0,"backgroundBlur":0}"##,
            "too_small",
            json!(["palette"]),
            "Array must contain exactly 16 element(s)",
        ),
        refused::<wsp_wire::types::slate_wire::SessionSlateEvent>(
            r##"{"type":"session.slate","workspaceId":"a","sessionId":"a","at":1,"turnId":"a","threadId":"a","pos":1,"cause":"write","version":1,"by":"agent","pieces":["a","a","a","a","a","a","a","a","a","a","a","a","a","a","a","a","a","a","a","a",null],"run":"a"}"##,
            "too_big",
            json!(["pieces"]),
            "Array must contain at most 20 element(s)",
        ),
    ]);
}

/// zod parses a z.object's keys in schema order, so an absent earlier key (`Required`) is the first issue even when a later key is
/// also wrong, and two wrong keys answer the one the schema lists first. Eight structs that types/ re-exports from wsp_frames
/// have a `check` with no rules and no `pass` (src/types/start.rs:367-430, tree.rs:17, pull_request/pull_request_comment.rs:476,900,
/// pull_request/pr_poll_ms.rs:991), so
/// serde's derive answers: the first key sent that fails. start.ts:92,100, tree.ts, pull-request.ts. Left from the first review
/// (spec/shapes.md rule 6 says its audit of types/ holds); the first review's sweep never broke an absent key together with a
/// later key's type.
#[test]
fn a_re_exported_frames_struct_answers_the_first_key_of_its_schema_not_the_first_one_sent() {
    none_differ(vec![
        refused::<wsp_wire::types::start::GitPrDiffReply>(r##"{"truncated":0,"left":["a"]}"##, "invalid_type", json!(["diff"]), "Required"),
        refused::<wsp_wire::types::start::GitPrDiffReply>(
            r##"{"left":5,"diff":5,"truncated":true}"##,
            "invalid_type",
            json!(["diff"]),
            "Expected string, received number",
        ),
        refused::<wsp_wire::types::pull_request::GitPrMergeReply>(r##"{"autoArmed":0}"##, "invalid_type", json!(["merged"]), "Required"),
        refused::<wsp_wire::types::pull_request::GitPrMergeReply>(
            r##"{"autoArmed":0,"merged":1}"##,
            "invalid_type",
            json!(["merged"]),
            "Expected boolean, received number",
        ),
        refused::<wsp_wire::types::pull_request::GitPrResolveReply>(r##"{"resolved":1}"##, "invalid_type", json!(["threadId"]), "Required"),
        refused::<wsp_wire::types::start::GitPrReviewReply>(r##"{"folded":[5]}"##, "invalid_type", json!(["url"]), "Required"),
        refused::<wsp_wire::types::pull_request::GitRunLogReply>(r##"{"truncated":1}"##, "invalid_type", json!(["lines"]), "Required"),
        refused::<wsp_wire::types::pull_request::GitRunLogReply>(
            r##"{"truncated":1,"lines":5}"##,
            "invalid_type",
            json!(["lines"]),
            "Expected array, received number",
        ),
        refused::<wsp_wire::types::tree::GitStartOnReply>(r##"{"oid":true}"##, "invalid_type", json!(["branch"]), "Required"),
        refused::<wsp_wire::types::pull_request::GitRepoReadReply>(
            r##"{"defaultMethod":[],"autoMerge":true}"##,
            "invalid_type",
            json!(["methods"]),
            "Required",
        ),
        refused::<wsp_wire::types::pull_request::PullRequestReviewRequest>(r##"{"team":0}"##, "invalid_type", json!(["name"]), "Required"),
    ]);
}

/// zod lists `extraKeys` in `for (const key in data)` order, which holds the keys that read as array indexes first and ascending;
/// the Rust lists them as they were sent. The keys of an object a client sends are never numbers today, so this is the refusal's
/// words only. Each `.strict()` object builds its list in its own `let extra` over `obj.keys()`
/// (views/preferences/ssh_behind_kind.rs:440 and 15 more sites that call check.rs:718 `unrecognized`); about 20 types. Left; the
/// first review's MINOR 14 named the written order, not the refusal.
#[test]
fn a_strict_object_lists_its_unknown_keys_in_javascripts_key_order() {
    none_differ(vec![
        refused::<wsp_wire::views::preferences::ProjectLook>(
            r##"{"icon":"folder","hue":"neutral","b":1,"2":2,"1":3,"a":4}"##,
            "unrecognized_keys",
            json!([]),
            "Unrecognized key(s) in object: '1', '2', 'b', 'a'",
        ),
        refused::<wsp_wire::views::preferences::ComputerLook>(
            r##"{"icon":"laptop","b":1,"2":2,"1":3,"a":4}"##,
            "unrecognized_keys",
            json!([]),
            "Unrecognized key(s) in object: '1', '2', 'b', 'a'",
        ),
        refused::<wsp_wire::views::preferences::PreferencesTarget>(
            r##"{"workspace":"a","b":1,"2":2,"1":3,"a":4}"##,
            "unrecognized_keys",
            json!([]),
            "Unrecognized key(s) in object: '1', '2', 'b', 'a'",
        ),
        refused::<wsp_wire::views::session::ThreadMarks>(
            r##"{"pinned":true,"b":1,"2":2,"1":3,"a":4}"##,
            "unrecognized_keys",
            json!([]),
            "Unrecognized key(s) in object: '1', '2', 'b', 'a'",
        ),
    ]);
}

/// zod's ZodRecord reads `for (const key in data)`, so the first bad key is the first index-like one. check.rs `IndexMap::pass`
/// (src/types/check.rs:260, the `for (key, item) in map` loop) walks the map in input order. Same root as the unknown-key list, another
/// code path. Left; refusal words only.
#[test]
fn a_record_names_its_first_bad_key_in_javascripts_key_order() {
    none_differ(vec![
        refused::<wsp_wire::types::thread_defaults::AgentSetupSet>(
            r##"{"on":true,"env":{"b":"a","2":"a","1":"a","a":"a","10":"a","0":"a"}}"##,
            "invalid_string",
            json!(["env", "0"]),
            "a variable's name is letters, digits and _, not starting with a digit",
        ),
        refused::<wsp_wire::views::harness::HarnessCatalog>(
            r##"{"harness":"s","label":"s","source":"harness","version":"s","models":[],"efforts":[],"contextWindows":[],"permissionModes":[],"steers":false,"renames":false,"images":false,"access":{"b":"a","2":"a","1":"a","a":"a","10":"a","0":"a"}}"##,
            "invalid_enum_value",
            json!(["access", "0"]),
            "Invalid enum value. Expected 'ask' | 'auto-edit' | 'full' | 'plan', received '0'",
        ),
    ]);
}
