// SPDX-License-Identifier: AGPL-3.0-only
//! Inputs where the parse machinery and the runtime ops still answer unlike zod 3.25.76 or node 22 after the fix round,
//! found in the re-review of wsp-wire (#2445, fix round at 4abcbdf7b). Every expectation is what zod's safeParse (JSON.stringify
//! of its data, or `JSON.stringify([issues[0]], null, 2)`) or node answered for the same JSON text, from an esbuild bundle
//! of packages/protocol/src at a3b870a6e.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_wire::types::agents_report::AgentsTarget;
use wsp_wire::types::check::{issues_of, parse, Check};
use wsp_wire::types::pull_request::{GitPrMergeReply, GitPrResolveReply, GitRepoReadReply, GitRunLogReply, PullRequestReviewRequest};
use wsp_wire::types::start::{GitPrDiffReply, GitPrReviewReply};
use wsp_wire::types::thread_defaults::{AgentSetupSet, ProjectOverrides};
use wsp_wire::types::JsNumber;
use wsp_wire::views::app_procs::{AppProc, AppProcList};
use wsp_wire::views::golden_image::Recipe;
use wsp_wire::wire::capabilities::InitSetup;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::limits::PortForward;
use wsp_wire::wire::runtime::RuntimeRequest;

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

/// zod lists exactly these issues, in this order, as (code, path, message).
fn lists<T: DeserializeOwned + Serialize + Check>(text: &str, want: &[(&str, Value, &str)]) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    let got: Vec<(String, Value, String)> = issues_of::<T>(&input).into_iter().map(|e| (e.code, Value::Array(e.path), e.message)).collect();
    let want: Vec<(String, Value, String)> = want.iter().map(|(c, p, m)| (c.to_string(), p.clone(), m.to_string())).collect();
    if got == want {
        None
    } else {
        Some(format!("{} {}: zod lists {want:?}, here {got:?}", name::<T>(), &text[..text.len().min(120)]))
    }
}

/// parse's first issue written as zod's `ZodError.message` of a list of one writes it.
fn issue_text<T: DeserializeOwned + Serialize + Check>(text: &str, node: &str) -> Option<String> {
    let input: Value = serde_json::from_str(text).unwrap();
    match parse::<T>(&input) {
        Ok(_) => Some(format!("{} {text}: zod refused it, read here", name::<T>())),
        Err(e) if e.issue_text() == node => None,
        Err(e) => Some(format!("{} {text}: zod writes {node:?}, here {:?}", name::<T>(), e.issue_text())),
    }
}

fn none_differ(found: Vec<Option<String>>) {
    let differ: Vec<String> = found.into_iter().flatten().collect();
    assert!(differ.is_empty(), "{} answer(s) unlike zod's:\n{}", differ.len(), differ.join("\n"));
}

/// MINOR (left from the first review's MINOR 13, 14 and 16, which the fix round reports fixed for `Kept` only).
/// slate/wire.ts:21 `SlateWireValues = z.record(z.string(), z.unknown())` and :12 `SlateWireDocument`: zod's record drops
/// `__proto__`, writes array-index keys first, and holds the values as JavaScript numbers, so `JSON.stringify` writes
/// `5.0` as 5 and `0.000001` as 0.000001. types/slate_wire.rs:16 `SlateWireValues = IndexMap<String, Value>` and
/// runtime_ops.rs:687 `document` (whose entries are plain `Value`s) write the sent order, keep `__proto__`, and write
/// serde_json's text of each number (`5.0`, `1e-6`). Real traffic: every slates.state and slates.write frame, every
/// slate.values event, a slate's live values.
#[test]
fn a_slate_values_record_is_written_as_zod_writes_a_kept_record() {
    none_differ(vec![
        read_as::<EventUnion>(
            r#"{"type":"slate.values","workspaceId":"w","threadId":"t","version":1,"revision":2,"values":{"__proto__":"x","b":5.0,"2":1,"a":0.000001}}"#,
            r#"{"type":"slate.values","workspaceId":"w","threadId":"t","version":1,"revision":2,"values":{"2":1,"b":5,"a":0.000001}}"#,
        ),
        read_as::<EventUnion>(
            r#"{"type":"slate.values","workspaceId":"w","threadId":"t","version":1,"revision":2,"values":{"k":{"b":5.0,"2":-0,"1":1e2}}}"#,
            r#"{"type":"slate.values","workspaceId":"w","threadId":"t","version":1,"revision":2,"values":{"k":{"1":100,"2":0,"b":5}}}"#,
        ),
        // node writes id first; RuntimeRequest writes op first (see the NIT test), so the expected text is node's with those two swapped.
        read_as::<RuntimeRequest>(
            r#"{"id":1,"op":"slates.state","values":{"__proto__":"x","b":5.0}}"#,
            r#"{"op":"slates.state","id":1,"values":{"b":5}}"#,
        ),
        read_as::<RuntimeRequest>(
            r#"{"id":1,"op":"slates.write","document":{"d":{"x":5.0,"1":2}}}"#,
            r#"{"op":"slates.write","id":1,"document":{"d":{"1":2,"x":5}}}"#,
        ),
    ]);
}

/// MINOR (left from the first review's finding 14, "fixed for every generated record"). init-job.ts:142
/// `InitKeys = z.record(z.boolean())`: zod's record drops `__proto__` and writes index-like keys first, ascending, then the
/// rest in the order sent. types/init_job.rs:569 `InitKeys = IndexMap<String, bool>` has no `js_record` or
/// `kept::record`, so `InitSetup.keys`, `InitJob.keys` and the `init.job` event write the order sent and keep `__proto__`.
#[test]
fn an_init_keys_record_is_written_as_zod_writes_a_record() {
    let base = |keys: &str| {
        format!(
            r#"{{"keys":{keys},"keyProvider":"a","home":"a","agents":[],"pricing":null,"place":{{"id":"a","name":"a","kind":"computer"}},"forksOn":"a","buildRefusal":"a","job":null}}"#
        )
    };
    let want = |keys: &str| {
        format!(
            r#"{{"keys":{keys},"keyProvider":"a","home":"a","agents":[],"pricing":null,"place":{{"id":"a","name":"a","kind":"computer"}},"forksOn":"a","buildRefusal":"a","job":null}}"#
        )
    };
    none_differ(vec![
        read_as::<InitSetup>(&base(r#"{"__proto__":true}"#), &want("{}")),
        read_as::<InitSetup>(
            &base(
                r#"{"b":true,"10":true,"a":true,"2":true,"01":true,"4294967294":true,"4294967295":true,"-1":true,"1.0":true,"0":true,"1":true}"#,
            ),
            &want(
                r#"{"0":true,"1":true,"2":true,"10":true,"4294967294":true,"b":true,"a":true,"01":true,"4294967295":true,"-1":true,"1.0":true}"#,
            ),
        ),
    ]);
}

/// MINOR (left from MINOR 2, "a fraction past a bound is two issues", which the fix round reports fixed through
/// `Check::dirty`). runtime.ts:729 / limits.ts:153 `RelayPort = z.number().int().min(1024).max(65535)`: `port: 2.5` is the
/// integer issue and then the bound, and `65535.5` the integer issue and then `too_big`. `RelayPort` is a newtype with its
/// own `check()` and no `dirty`, so the bound never runs on the fraction and `issues_of` lists one issue. `parse` answers
/// the first, which matches; the list does not (serve.ts:679 sends zod's whole list, and `issuesLine` logs every issue).
/// Seen in `forwards.stop`, `PortForward.port` and the `forward.open` and `forward.close` events.
#[test]
fn a_fraction_in_a_relay_port_is_two_issues() {
    none_differ(vec![
        lists::<RuntimeRequest>(
            r#"{"id":1,"op":"forwards.stop","workspaceId":"w","port":2.5}"#,
            &[
                ("invalid_type", json!(["port"]), "Expected integer, received float"),
                ("too_small", json!(["port"]), "Number must be greater than or equal to 1024"),
            ],
        ),
        lists::<RuntimeRequest>(
            r#"{"id":1,"op":"forwards.stop","workspaceId":"w","port":65535.5}"#,
            &[
                ("invalid_type", json!(["port"]), "Expected integer, received float"),
                ("too_big", json!(["port"]), "Number must be less than or equal to 65535"),
            ],
        ),
        lists::<PortForward>(
            r#"{"workspaceId":"w","port":0.5,"startedAt":"s","name":"n","kind":"url"}"#,
            &[
                ("invalid_type", json!(["port"]), "Expected integer, received float"),
                ("too_small", json!(["port"]), "Number must be greater than or equal to 1024"),
            ],
        ),
    ]);
}

/// MINOR (new class: the first review did not sweep issue order inside a record). zod's record walks `for in` over the
/// parsed object, so a record's issues come in JavaScript's key order: array-index keys ascending, then the rest as sent.
/// runtime.ts:832 + thread-defaults.ts:69 `AgentSetupSet.env = z.record(EnvName, ...)`. `check.rs:257` (`IndexMap::pass`)
/// and `runtime_ops.rs:794` (`env_key`) walk the `serde_json::Value` map, which holds the order sent, so with two refused
/// names zod's first issue is the index-like one and the Rust first issue is the earlier-sent one.
#[test]
fn a_record_lists_its_issues_in_javascript_key_order() {
    let frame = r#"{"id":1,"op":"agents.setup","placeId":"p","agent":"claude","on":true,"env":{"b!":null,"2":null}}"#;
    let name = "a variable's name is letters, digits and _, not starting with a digit";
    none_differ(vec![
        refused::<RuntimeRequest>(frame, "invalid_string", json!(["env", "2"]), name),
        lists::<RuntimeRequest>(frame, &[("invalid_string", json!(["env", "2"]), name), ("invalid_string", json!(["env", "b!"]), name)]),
    ]);
}

/// MINOR (new: the fix round's number rule reaches `received`). zod's invalid_literal issue carries `received` as the
/// JavaScript value, so `ZodError.message` writes a number as JavaScript does: 5, 1e+21, 0.000001, 0, 9007199254740992.
/// check.rs:843, :867 and :871 keep the `serde_json::Value` the frame held, so `issue_text()` writes serde_json's text of it
/// (`5.0`, `1e-6`, `-0.0`, `9007199254740993`; `1e21` comes out right). runtime.ts:57 `here: z.literal(true).optional()`; the same in every
/// `z.literal(true)` key (`workspaces.landing` `sized`, the `places.*` `choices`).
#[test]
fn a_number_received_by_a_literal_is_written_as_javascript_writes_it() {
    let at = |received: &str| {
        format!("[\n  {{\n    \"received\": {received},\n    \"code\": \"invalid_literal\",\n    \"expected\": true,\n    \"path\": [\n      \"here\"\n    ],\n    \"message\": \"Invalid literal value, expected true\"\n  }}\n]")
    };
    none_differ(vec![
        issue_text::<RuntimeRequest>(r#"{"id":1,"op":"pair.issue","here":5.0}"#, &at("5")),
        issue_text::<RuntimeRequest>(r#"{"id":1,"op":"pair.issue","here":1e21}"#, &at("1e+21")),
        issue_text::<RuntimeRequest>(r#"{"id":1,"op":"pair.issue","here":0.000001}"#, &at("0.000001")),
        issue_text::<RuntimeRequest>(r#"{"id":1,"op":"pair.issue","here":-0}"#, &at("0")),
        issue_text::<RuntimeRequest>(r#"{"id":1,"op":"pair.issue","here":9007199254740993}"#, &at("9007199254740992")),
    ]);
}

/// MINOR (left from the first review: rule 6 literal keys were only checked for their words). golden-image.ts:229
/// `version: z.literal(1)`: zod compares with `===`, so the JSON numbers `1.0`, `1e0` and `10e-1` are the literal. literal.rs:68
/// `LiteralValue for i64` (:68) is `v.as_i64() == Some(self)`, and serde_json holds those three as floats, which `as_i64` refuses.
/// The one numeric literal the crate reads (`literal!(LitV1, 1)`, views/golden_image/agents_bin.rs:6); a recipe file a person edits.
#[test]
fn a_numeric_literal_takes_every_text_javascript_reads_as_that_number() {
    let at =
        |version: &str| format!(r#"{{"version":{version},"at":"a","tick":"used","histories":[],"rows":[],"custom":[],"engine":true}}"#);
    let want = r#"{"version":1,"at":"a","tick":"used","histories":[],"rows":[],"custom":[],"engine":true}"#;
    none_differ(vec![read_as::<Recipe>(&at("1.0"), want), read_as::<Recipe>(&at("1e0"), want), read_as::<Recipe>(&at("10e-1"), want)]);
}

/// MINOR (left from the first review's M2: the `wsp_frames` re-exports are held to zod's first issue). pull-request.ts:517,
/// 520, 514, 565, 390 and start.ts (GitPrDiffReply, GitPrReviewReply): seven shapes `types/pull_request/mod.rs:15-33` and
/// `types/start.rs:367,409` still re-export from wsp-frames, whose derived
/// `Deserialize` answers the first error in the order the input holds its keys, with no schema pass. A frame with a
/// missing earlier key and a refused later key answers the later key; zod answers the earlier key (Required). The same
/// shapes list one issue where zod lists each.
#[test]
fn a_pull_request_reply_answers_its_first_key_in_schema_order() {
    none_differ(vec![
        refused::<GitPrMergeReply>(r#"{"autoArmed":"x"}"#, "invalid_type", json!(["merged"]), "Required"),
        refused::<GitRepoReadReply>(r#"{"defaultMethod":"zzz","autoMerge":true}"#, "invalid_type", json!(["methods"]), "Required"),
        refused::<PullRequestReviewRequest>(r#"{"team":"x"}"#, "invalid_type", json!(["name"]), "Required"),
        refused::<GitRunLogReply>(r#"{"truncated":"x"}"#, "invalid_type", json!(["lines"]), "Required"),
        refused::<GitPrResolveReply>(r#"{"resolved":"x"}"#, "invalid_type", json!(["threadId"]), "Required"),
        refused::<GitPrDiffReply>(r#"{"truncated":"x","left":[]}"#, "invalid_type", json!(["diff"]), "Required"),
        refused::<GitPrReviewReply>(r#"{"folded":"x"}"#, "invalid_type", json!(["url"]), "Required"),
        lists::<GitRepoReadReply>(
            r#"{"methods":[1,"merge",2],"defaultMethod":"merge","autoMerge":true}"#,
            &[
                ("invalid_type", json!(["methods", 0]), "Expected 'merge' | 'squash' | 'rebase', received number"),
                ("invalid_type", json!(["methods", 2]), "Expected 'merge' | 'squash' | 'rebase', received number"),
            ],
        ),
        lists::<GitPrMergeReply>(
            r#"{"merged":"x","autoArmed":"x"}"#,
            &[
                ("invalid_type", json!(["merged"]), "Expected boolean, received string"),
                ("invalid_type", json!(["autoArmed"]), "Expected boolean, received string"),
            ],
        ),
    ]);
}

/// NIT (left from M5 and the first review's MINOR 13; spec/check.md Decision 1). serde_json writes a `Value` number with its own
/// shortest form, so a number that goes through a `Value` differs from node in the three places Decision 1 now lists: below
/// 1e-5, at 2^64 and past, and at -2^63 and below. node 22.23.4 writes `String(0.000005)` as 0.000005, `String(0.0000012)` as
/// 0.0000012, `String(-(2**63))` as -9223372036854776000 and `String(2**64)` as 18446744073709552000; a `Value`
/// writes 5e-6, 1.2e-6, -9.223372036854776e+18 and 1.8446744073709552e+19. Between those, `String(0.00001)` is 0.00001,
/// `String(0.000015)` is 0.000015 and `String(2**63)` is 9223372036854776000, and a `Value` writes the same text. Writing a `JsNumber` straight is exact everywhere.
#[test]
fn a_number_through_a_value_differs_only_where_decision_1_says() {
    let through = |n: f64| serde_json::to_string(&serde_json::to_value(JsNumber(n)).unwrap()).unwrap();
    let straight = |n: f64| serde_json::to_string(&JsNumber(n)).unwrap();
    let inside = [(0.00001, "0.00001"), (0.000015, "0.000015"), (2f64.powi(63), "9223372036854776000")];
    for (n, node) in inside {
        assert_eq!((through(n), straight(n)), (node.to_string(), node.to_string()));
    }
    let outside = [
        (0.000005, "0.000005", "5e-6"),
        (0.0000012, "0.0000012", "1.2e-6"),
        (-(2f64.powi(63)), "-9223372036854776000", "-9.223372036854776e+18"),
        (-9223372036854775808.0 - 2048.0, "-9223372036854778000", "-9.223372036854778e+18"),
        (2f64.powi(64), "18446744073709552000", "1.8446744073709552e+19"),
    ];
    for (n, node, value) in outside {
        assert_eq!((straight(n), through(n)), (node.to_string(), value.to_string()));
    }
}

/// NIT (the first review's "issues_of lists every issue zod lists"). runtime.ts:1025 an intersection parses both sides, so a
/// frame that is not an object is two `invalid_type` issues at the root, one from each side. runtime_op_pass_b.rs:1056
/// returns the left side's one. serve.ts:652 answers a non-object frame before the schema, so the wire never lists them.
#[test]
fn a_frame_that_is_no_object_lists_an_issue_for_each_side_of_the_intersection() {
    let object = ("invalid_type", json!([]), "Expected object, received null");
    none_differ(vec![lists::<RuntimeRequest>("null", &[object.clone(), object])]);
}

fn repeat(item: &str, n: usize, bad_at: usize, bad: &str) -> String {
    let items: Vec<&str> = (0..n).map(|i| if i == bad_at { bad } else { item }).collect();
    format!("[{}]", items.join(","))
}

fn place_prove_with_agent_versions(map: &str) -> String {
    format!(
        r#"{{"id":"a","op":"place.prove","signature":"BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBw==","report":{{"name":"a","platform":"darwin","arch":"a","os":"a","shape":{{"cpu":1,"memMb":1}},"login":{{}},"runsWorkspaces":true,"engine":"none","daemonVersion":0,"wsp":["a"],"agents":[],"agentVersions":{map},"dialed":"https://example.com/a"}}}}"#
    )
}

/// MINOR (left from MINOR 1, "first issue in zod's order"; the fix round runs a key's rule on the value read, which a key
/// whose items fail never yields). zod's ZodArray checks the length first (dirty, so it goes on) and then each item, so an
/// array past its cap with a bad item answers `too_big` at the array and then the item's issue. check.rs:210 `Vec::pass`
/// reads the items only, the cap is the key's rule, and `push` (keys.rs:15) runs a rule only on a value that read clean
/// or has a `dirty` value, which `Vec` has none of: the cap never runs and the item's issue answers first. Seen in
/// `projects.icons` hashes (max 1000), `slates.resolve` paths (max 200), `AppProc` origins and contents (max 20) and so
/// `appProcs.set` below the list, `workspaces.look` `theme.dots` (max 3), and `place.prove` `report.agents` and `logins`,
/// `ProcScope.pids`, `SessionSlateEvent.pieces`. The list itself, `AppProcList` (and so `appProcs.set` procs past 200), has a
/// pass of its own and answers right: two controls below.
#[test]
fn an_array_past_its_cap_answers_the_cap_before_its_items() {
    let paths = repeat(r#""a""#, 201, 3, "5");
    let hashes = repeat(&format!("\"{}\"", "0".repeat(64)), 1001, 2, r#""zz""#);
    let procs = repeat(r#"{"pid":1,"kind":"tab"}"#, 201, 5, r#"{"pid":"x","kind":"tab"}"#);
    let origins = repeat(r#""https://a.test""#, 21, 2, "5");
    let contents = repeat("1", 21, 1, "-1");
    none_differ(vec![
        refused::<RuntimeRequest>(
            &format!(r#"{{"id":1,"op":"slates.resolve","threadId":"t","paths":{paths}}}"#),
            "too_big",
            json!(["paths"]),
            "Array must contain at most 200 element(s)",
        ),
        refused::<RuntimeRequest>(
            &format!(r#"{{"id":1,"op":"projects.icons","hashes":{hashes}}}"#),
            "too_big",
            json!(["hashes"]),
            "Array must contain at most 1000 element(s)",
        ),
        refused::<RuntimeRequest>(
            &format!(r#"{{"id":1,"op":"appProcs.set","procs":{procs}}}"#),
            "too_big",
            json!(["procs"]),
            "Array must contain at most 200 element(s)",
        ),
        refused::<AppProcList>(&procs, "too_big", json!([]), "Array must contain at most 200 element(s)"),
        refused::<AppProc>(
            &format!(r#"{{"pid":1,"kind":"tab","origins":{origins}}}"#),
            "too_big",
            json!(["origins"]),
            "Array must contain at most 20 element(s)",
        ),
        refused::<AppProc>(
            &format!(r#"{{"pid":1,"kind":"tab","contents":{contents}}}"#),
            "too_big",
            json!(["contents"]),
            "Array must contain at most 20 element(s)",
        ),
        refused::<RuntimeRequest>(
            &format!(r#"{{"id":1,"op":"appProcs.set","procs":[{{"pid":1,"kind":"main","origins":{origins}}}]}}"#),
            "too_big",
            json!(["procs", 0, "origins"]),
            "Array must contain at most 20 element(s)",
        ),
        refused::<RuntimeRequest>(
            r#"{"id":1,"op":"workspaces.look","workspaceId":"w","theme":{"dots":[{"angle":0,"radius":0},{"angle":0,"radius":0},"x",{"angle":0,"radius":0}],"harmony":"single","grain":0,"opacity":0.1,"mode":"auto"},"glyph":"terminal"}"#,
            "too_big",
            json!(["theme", "dots"]),
            "Array must contain at most 3 element(s)",
        ),
        // the same gap on a record's refine (place-link.ts agentVersions, `.refine(keys <= 32)`): the item answers first, as zod
        // does, but the refine zod lists after it is never run, so `issues_of` lists one issue of two.
        lists::<RuntimeRequest>(
            &place_prove_with_agent_versions(&format!(
                "{{{}}}",
                (0..33)
                    .map(|i| if i == 2 { format!(r#""k{i}":"{}""#, "x".repeat(65)) } else { format!(r#""k{i}":"v""#) })
                    .collect::<Vec<_>>()
                    .join(",")
            )),
            &[
                ("too_big", json!(["report", "agentVersions", "k2"]), "String must contain at most 64 character(s)"),
                ("custom", json!(["report", "agentVersions"]), "at most 32 agents"),
            ],
        ),
        lists::<RuntimeRequest>(
            &format!(r#"{{"id":1,"op":"slates.resolve","threadId":"t","paths":{paths}}}"#),
            &[
                ("too_big", json!(["paths"]), "Array must contain at most 200 element(s)"),
                ("invalid_type", json!(["paths", 3]), "Expected string, received number"),
            ],
        ),
    ]);
}

/// MINOR (new class). zod's strict object collects the unknown keys with `for (const key in ctx.data)`, so they come in
/// JavaScript's key order, array-index keys first. thread-defaults.ts:76 (`AgentSetupSet`, `ProjectOverrides` `.strict()`),
/// agents-report.ts:32 (`AgentsTarget`), runtime.ts:832 (`agents.setup`). runtime_op_pass_b.rs:1060 and thread_defaults.rs:74,
/// :127, :181 (and the same line in views/session.rs:391 and views/preferences) list the unknown keys of the
/// `serde_json::Value` object, which holds the order the frame has them.
#[test]
fn the_unknown_keys_of_a_strict_object_are_listed_in_javascript_key_order() {
    none_differ(vec![
        refused::<RuntimeRequest>(
            r#"{"id":1,"op":"agents.setup","placeId":"p","agent":"claude","on":true,"b":1,"2":1,"a":1}"#,
            "unrecognized_keys",
            json!([]),
            "Unrecognized key(s) in object: '2', 'b', 'a'",
        ),
        refused::<RuntimeRequest>(
            r#"{"id":1,"op":"agents.setup","placeId":"p","agent":"claude","on":true,"b":1,"origin":"here","2":1}"#,
            "unrecognized_keys",
            json!([]),
            "Unrecognized key(s) in object: '2', 'b', 'origin'",
        ),
        refused::<AgentsTarget>(
            r#"{"workspaceId":"w","b":1,"2":1}"#,
            "unrecognized_keys",
            json!([]),
            "Unrecognized key(s) in object: '2', 'b'",
        ),
        refused::<AgentSetupSet>(r#"{"on":true,"b":1,"2":1}"#, "unrecognized_keys", json!([]), "Unrecognized key(s) in object: '2', 'b'"),
        refused::<ProjectOverrides>(
            r#"{"agent":"x","b":1,"2":1}"#,
            "unrecognized_keys",
            json!([]),
            "Unrecognized key(s) in object: '2', 'b'",
        ),
    ]);
}
