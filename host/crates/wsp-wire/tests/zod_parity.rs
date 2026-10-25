// SPDX-License-Identifier: AGPL-3.0-only
//! Inputs where `parse` answers unlike zod 3.25.76 on the TS schemas at a3b870a6e. Every expectation is what zod's
//! safeParse returned for the same JSON: the first issue's code, path and message, the text zod wrote, or the
//! value it read. Found in the review of #2445.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_wire::types::agents_report::{AgentRoad, AgentsProject, AgentsTarget, McpRowTransport, SignInLine};
use wsp_wire::types::check::{parse, Check};
use wsp_wire::types::project_look::ProjectIconHash;
use wsp_wire::types::pull_request::{FixResult, GitPrReadReply, PullRequestLabel, PullRequestReaction, PullRequestState};
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::types::start::{GithubLink, ReviewBlock};
use wsp_wire::types::thread_defaults::{AgentSetupSet, EnvName, ProjectOverrides, TrackerRepo};
use wsp_wire::types::usage::LimitWindow;
use wsp_wire::views::golden_image::RecipeSource;
use wsp_wire::views::place::{PlaceView, SudoPassword};
use wsp_wire::views::preferences::{Preferences, ThemeId};
use wsp_wire::views::session_events::{SessionEvent, SessionPlanEvent};
use wsp_wire::views::workspace::WorktreeFolder;
use wsp_wire::views::workspace_kind::WorkspaceKind;

/// zod refused `input` with this first issue; None where parse answers the same.
fn refused<T: DeserializeOwned + Serialize + Check>(input: Value, code: &str, path: Value, message: &str) -> Option<String> {
    let label = format!("{} {input}", std::any::type_name::<T>().rsplit("::").next().unwrap_or("?"));
    match parse::<T>(&input) {
        Ok(read) => Some(format!("{label}: zod refused it ({code} at {path}), read here as {}", serde_json::to_string(&read).unwrap())),
        Err(e) if e.code == code && Value::Array(e.path.clone()) == path && e.message == message => None,
        Err(e) => Some(format!("{label}: zod answers {code} at {path} {message:?}, here {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

/// zod read `text` and wrote `wrote`.
fn read_as<T: DeserializeOwned + Serialize + Check>(text: &str, wrote: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    match parse::<T>(&input) {
        Ok(read) if serde_json::to_string(&read).unwrap() == wrote => None,
        Ok(read) => Some(format!("{text}: zod wrote {wrote}, here {}", serde_json::to_string(&read).unwrap())),
        Err(e) => Some(format!("{text}: zod read it as {wrote}, refused here: {} at {:?} {:?}", e.code, e.path, e.message)),
    }
}

fn none_differ(found: Vec<Option<String>>) {
    let differ: Vec<String> = found.into_iter().flatten().collect();
    assert!(differ.is_empty(), "{} answer(s) unlike zod's:\n{}", differ.len(), differ.join("\n"));
}

const ENV_NAME: &str = "a variable's name is letters, digits and _, not starting with a digit";

fn preferences(extra: Value) -> Value {
    let mut p = json!({"theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {}, "projectLook": {}, "projectOrder": [], "labs": false});
    p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    p
}

fn place_view(extra: Value) -> Value {
    let mut p = json!({"id": "s", "kind": "computer", "name": "s", "default": false});
    p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    p
}

/// project-look.ts:14, views/place.ts:69, thread-defaults.ts:58, preferences.ts ThemeId, the hash on a recipe folder
/// (recipe-file.ts:72) and on preferences.projectIcon (preferences.ts:113): the rules of a schema written as a Rust
/// type alias, and of every field of that type.
#[test]
fn a_schema_written_as_an_alias_keeps_its_rules() {
    none_differ(vec![
        refused::<ProjectIconHash>(json!("../../etc/passwd"), "invalid_string", json!([]), "Invalid"),
        refused::<SudoPassword>(json!("pw\nid"), "invalid_string", json!([]), "Invalid"),
        refused::<EnvName>(json!("1X"), "invalid_string", json!([]), ENV_NAME),
        refused::<TrackerRepo>(json!("no-slash"), "invalid_string", json!([]), "Invalid"),
        refused::<ThemeId>(json!(""), "too_small", json!([]), "String must contain at least 1 character(s)"),
        refused::<RecipeFile>(
            json!({"name": "r", "folders": {"k": {"from": "f", "image": "../x"}}}),
            "invalid_string",
            json!(["folders", "k", "image"]),
            "Invalid",
        ),
        refused::<Preferences>(
            preferences(json!({"projectIcon": {"k": "../x"}})),
            "invalid_string",
            json!(["projectIcon", "k"]),
            "Invalid",
        ),
    ]);
}

/// zod's ZodObject._parse walks the shape's keys in order and ZodArray checks its items' rules in key order too:
/// the first issue is the first key of the schema that fails, whatever the input's key order, whether serde or a
/// check would refuse it, and wherever the missing key is.
#[test]
fn the_first_issue_is_the_first_key_of_the_schema_that_fails() {
    none_differ(vec![
        refused::<ReviewBlock>(
            json!({"comments": [], "summary": 5, "verdict": "nope"}),
            "invalid_enum_value",
            json!(["verdict"]),
            "Invalid enum value. Expected 'comment' | 'approve' | 'request_changes', received 'nope'",
        ),
        refused::<ReviewBlock>(
            json!({"verdict": "approve", "summary": "s", "comments": [{"path": "a", "line": 0, "side": "left", "body": "b"}]}),
            "too_small",
            json!(["comments", 0, "line"]),
            "Number must be greater than 0",
        ),
        refused::<AgentsProject>(json!({"name": "n", "path": true}), "invalid_type", json!(["id"]), "Required"),
        refused::<Preferences>(
            preferences(json!({"sidebarWidth": 0, "appFont": "x".repeat(129)})),
            "too_small",
            json!(["sidebarWidth"]),
            "Number must be greater than 0",
        ),
    ]);
}

/// `.optional()` alone takes undefined, not null (usage.ts:17, pull-request.ts PullRequestLabel).
#[test]
fn an_optional_key_refuses_null() {
    none_differ(vec![
        refused::<LimitWindow>(
            json!({"kind": "week", "usedPercent": 1, "resetsAt": null}),
            "invalid_type",
            json!(["resetsAt"]),
            "Expected number, received null",
        ),
        refused::<PullRequestLabel>(
            json!({"name": "n", "color": "c", "description": null}),
            "invalid_type",
            json!(["description"]),
            "Expected string, received null",
        ),
    ]);
}

/// `.int()` is Number.isInteger on the number JSON.parse read: `5.0` is 5, `-0` is an integer, 1e21 is one, and
/// 2^53+1 is read as 2^53 (pull-request.ts PullRequestReaction.count).
#[test]
fn an_int_field_reads_every_number_javascript_calls_an_integer() {
    let e21: Value = serde_json::from_str(r#"{"content":"+1","count":1e21,"mine":true}"#).unwrap();
    none_differ(vec![
        read_as::<PullRequestReaction>(r#"{"content":"+1","count":5.0,"mine":true}"#, r#"{"content":"+1","count":5,"mine":true}"#),
        read_as::<PullRequestReaction>(r#"{"content":"+1","count":-0,"mine":true}"#, r#"{"content":"+1","count":0,"mine":true}"#),
        read_as::<PullRequestReaction>(
            r#"{"content":"+1","count":9007199254740993,"mine":true}"#,
            r#"{"content":"+1","count":9007199254740992,"mine":true}"#,
        ),
        parse::<PullRequestReaction>(&e21)
            .err()
            .map(|e| format!("1e21: zod reads it as an integer, refused here: {} {:?}", e.code, e.message)),
    ]);
}

/// A discriminatedUnion answers the member's own issue at the member's own path (session-events.ts:635,
/// golden-image.ts:161, pull-request.ts:309).
#[test]
fn an_issue_inside_a_union_member_keeps_its_path() {
    none_differ(vec![
        refused::<SessionEvent>(
            json!({"type": "session.end", "workspaceId": 5, "sessionId": "s", "exitCode": null, "sawResult": true}),
            "invalid_type",
            json!(["workspaceId"]),
            "Expected string, received number",
        ),
        refused::<RecipeSource>(
            json!({"kind": "used", "sessions": 1, "calls": 1.5}),
            "invalid_type",
            json!(["calls"]),
            "Expected integer, received float",
        ),
        refused::<FixResult>(json!({"outcome": "updated"}), "invalid_type", json!(["base"]), "Required"),
    ]);
}

/// zod reads no array as an object; serde's derive reads a struct, and an internally tagged enum, from a sequence.
#[test]
fn an_array_is_not_an_object() {
    none_differ(vec![
        refused::<GitPrReadReply>(json!([]), "invalid_type", json!([]), "Expected object, received array"),
        refused::<AgentsProject>(json!(["a", "b", "c"]), "invalid_type", json!([]), "Expected object, received array"),
        refused::<McpRowTransport>(json!(["stdio", "x"]), "invalid_type", json!([]), "Expected object, received array"),
    ]);
}

/// z.record(keySchema, ...) parses each key with its schema (thread-defaults.ts:75, recipe-file.ts:58).
#[test]
fn a_record_s_keys_keep_their_rules() {
    none_differ(vec![
        refused::<AgentSetupSet>(json!({"env": {"1X": "v"}}), "invalid_string", json!(["env", "1X"]), ENV_NAME),
        refused::<RecipeFile>(
            json!({"name": "r", "agents": {"": {}}}),
            "too_small",
            json!(["agents", ""]),
            "String must contain at least 1 character(s)",
        ),
    ]);
}

/// PlaceView's road and dialled carry their own rules (views/place.ts PlaceRoad, PlaceDialled); its check names them
/// as nested checks and never runs them.
#[test]
fn a_nested_check_the_doc_comment_names_runs() {
    none_differ(vec![
        refused::<PlaceView>(
            place_view(json!({"road": {"back": {"boxPort": 70000}}})),
            "too_big",
            json!(["road", "back", "boxPort"]),
            "Number must be less than or equal to 65535",
        ),
        refused::<PlaceView>(
            place_view(json!({"dialled": {"at": "t", "answered": true, "roundTripMs": -1}})),
            "too_small",
            json!(["dialled", "roundTripMs"]),
            "Number must be greater than or equal to 0",
        ),
    ]);
}

/// A re-exported wsp-frames enum is read by its serde words, and zod names those words (pull-request.ts:12,
/// workspace-kind.ts).
#[test]
fn a_reexported_enum_names_its_wire_words() {
    none_differ(vec![
        refused::<PullRequestState>(
            json!("x"),
            "invalid_enum_value",
            json!([]),
            "Invalid enum value. Expected 'open' | 'merged' | 'closed', received 'x'",
        ),
        refused::<WorkspaceKind>(json!(null), "invalid_type", json!([]), "Expected 'cloud' | 'local' | 'place', received null"),
    ]);
}

/// The word zod expects at a key whose type `describe` does not answer for: a `z.literal(true)` (wsp-frames' True)
/// and an array of objects (views/workspace.ts WorktreeFolder.gone, session-events.ts SessionPlanEvent.steps).
#[test]
fn every_key_describes_what_zod_expects_there() {
    none_differ(vec![
        refused::<WorktreeFolder>(
            json!({"path": "p", "made": true, "gone": 7}),
            "invalid_literal",
            json!(["gone"]),
            "Invalid literal value, expected true",
        ),
        refused::<SessionPlanEvent>(
            json!({"type": "session.plan", "workspaceId": "w", "sessionId": "s", "steps": 7}),
            "invalid_type",
            json!(["steps"]),
            "Expected array, received number",
        ),
    ]);
}

/// thread-defaults.ts:37: AfterWorktree's `.min(1, message)` answers with its own words.
#[test]
fn a_rule_s_own_message_is_kept() {
    none_differ(vec![refused::<ProjectOverrides>(
        json!({"afterWorktree": "  "}),
        "too_small",
        json!(["afterWorktree"]),
        "the after-worktree command is a shell line; --reset after-worktree takes it away",
    )]);
}

/// JSON.stringify([issue], null, 2) as zod built the issue: ZodEnum's invalid_type opens with expected and received,
/// and an undefined `received` is left out.
#[test]
fn issue_text_is_zod_s_text() {
    let road = parse::<AgentRoad>(&json!(null)).map(|_| ()).unwrap_err().issue_text();
    let host = parse::<GithubLink>(&json!({"repo": "r", "kind": "issue", "number": 1, "url": "u"})).map(|_| ()).unwrap_err().issue_text();
    assert_eq!(
        (road.as_str(), host.as_str()),
        (
            "[\n  {\n    \"expected\": \"'wsp' | 'own' | 'shim' | 'none'\",\n    \"received\": \"null\",\n    \"code\": \"invalid_type\",\n    \"path\": [],\n    \"message\": \"Expected 'wsp' | 'own' | 'shim' | 'none', received null\"\n  }\n]",
            "[\n  {\n    \"code\": \"invalid_literal\",\n    \"expected\": \"github.com\",\n    \"path\": [\n      \"host\"\n    ],\n    \"message\": \"Invalid literal value, expected \\\"github.com\\\"\"\n  }\n]",
        )
    );
}

/// ZodUnion returns the first member that parsed with issues (status dirty) before it gives up: a strict member's
/// unrecognized key is such an issue (agents-report.ts:32).
#[test]
fn a_union_answers_its_first_dirty_member() {
    none_differ(vec![refused::<AgentsTarget>(
        json!({"placeId": "p", "workspaceId": "w"}),
        "unrecognized_keys",
        json!([]),
        "Unrecognized key(s) in object: 'workspaceId'",
    )]);
}

/// A JavaScript object holds integer-like keys first, ascending, and ZodRecord's `for in` writes them so.
#[test]
fn a_record_writes_integer_keys_first_as_javascript_does() {
    none_differ(vec![read_as::<SignInLine>(
        r#"{"command":"c","env":{"b":"x","10":"x","2":"x"}}"#,
        r#"{"command":"c","env":{"2":"x","10":"x","b":"x"}}"#,
    )]);
}

/// JavaScript's `\s` is not char::is_whitespace: U+0085 is not in it and U+FEFF is (TrackerRepo's regex,
/// thread-defaults.ts; ReviewBlock's verdict preprocess, start.ts:196).
#[test]
fn a_regex_s_white_space_is_javascript_s() {
    none_differ(vec![
        read_as::<ProjectOverrides>("{\"trackers\":[\"a\u{85}/b\"]}", "{\"trackers\":[\"a\u{85}/b\"]}"),
        refused::<ProjectOverrides>(json!({"trackers": ["a\u{feff}/b"]}), "invalid_string", json!(["trackers", 0]), "Invalid"),
        refused::<ReviewBlock>(
            json!({"verdict": "request\u{85}changes", "summary": "s", "comments": []}),
            "invalid_enum_value",
            json!(["verdict"]),
            "Invalid enum value. Expected 'comment' | 'approve' | 'request_changes', received 'request\u{85}changes'",
        ),
    ]);
}
