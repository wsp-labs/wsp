// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the runtime ops, the runtime envelope and responses, and the event unions (#2445). Every expected value
//! is what zod 3.25.76 (safeParse, JSON.stringify of its data) or node 22 answered for the same JSON text, run from the
//! bundle of packages/protocol/src. Each test names the TS line it reads and the Rust line that answers otherwise.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_wire::types::check::{issues_of, parse, Check};
use wsp_wire::types::pull_request::PullRequestSeen;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::runtime::{
    request_secrets, RuntimeErrorResponse, RuntimeOkResponse, RuntimeRequest, SessionSteerResult,
};

fn frame(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

/// The first issue parse answers as (code, path, message); None where it reads the frame.
fn first_issue<T: DeserializeOwned + Serialize + Check>(text: &str) -> Option<(String, Value, String)> {
    parse::<T>(&frame(text)).err().map(|e| (e.code, Value::Array(e.path), e.message))
}

/// Every issue as (code, path), in the order they are answered.
fn all_issues<T: DeserializeOwned + Serialize + Check>(text: &str) -> Vec<(String, Value)> {
    issues_of::<T>(&frame(text)).into_iter().map(|e| (e.code, Value::Array(e.path))).collect()
}

fn issue(code: &str, path: Value, message: &str) -> Option<(String, Value, String)> {
    Some((code.to_string(), path, message.to_string()))
}

fn pairs(list: &[(&str, Value)]) -> Vec<(String, Value)> {
    list.iter().map(|(c, p)| (c.to_string(), p.clone())).collect()
}

fn written<T: DeserializeOwned + Serialize + Check>(text: &str) -> String {
    let read = parse::<T>(&frame(text)).unwrap_or_else(|e| panic!("refused {text}: {e:?}"));
    serde_json::to_string(&read).unwrap()
}

// -----------------------------------------------------------------------------------------------------------------
// BLOCKER or MAJOR: a frame the TS refuses, read here
// -----------------------------------------------------------------------------------------------------------------

/// BLOCKER (agents.setup sets the program, arguments and variables an agent is started with). runtime.ts:832
/// `z.object({id, op, placeId, agent}).merge(AgentSetupSet)`: merge takes the strictness of AgentSetupSet (thread-defaults.ts:76
/// `.strict()`), and the intersection at runtime.ts:1025 runs that member on the whole frame, so zod reads `origin` as one
/// more key the object does not name: no frame holding an `origin` is taken for agents.setup. runtime_ops.rs:745 lists
/// "origin" among the keys the strict member allows, and runtime.rs:3801 only checks the keys when no other issue stands.
/// zod also names `origin` among the unrecognized keys, in the order the frame holds them.
#[test]
fn agents_setup_refuses_an_origin_key_as_its_strict_member_does() {
    let with_origin = r#"{"id":"a","op":"agents.setup","placeId":"p","agent":"claude","on":true,"origin":"here"}"#;
    assert_eq!(first_issue::<RuntimeRequest>(with_origin), issue("unrecognized_keys", json!([]), "Unrecognized key(s) in object: 'origin'"));
    let with_both = r#"{"id":"a","op":"agents.setup","placeId":"p","agent":"claude","on":true,"extra":1,"origin":"here"}"#;
    assert_eq!(first_issue::<RuntimeRequest>(with_both), issue("unrecognized_keys", json!([]), "Unrecognized key(s) in object: 'extra', 'origin'"));
    let bad_origin = r#"{"id":"a","op":"agents.setup","placeId":"p","agent":"claude","on":true,"origin":"zzz"}"#;
    assert_eq!(
        all_issues::<RuntimeRequest>(bad_origin),
        pairs(&[("unrecognized_keys", json!([])), ("invalid_enum_value", json!(["origin"]))]),
    );
}

/// MAJOR (and the same on every unit enum the crate reads through serde alone: ticket.issue's purpose guards which road a
/// ticket opens, `origin` rides every request). zod's z.enum takes a string and nothing else; serde reads a unit variant
/// from the one-key map `{"word": null}` as well, so `{"purpose":{"connect":null}}` and `{"origin":{"here":null}}` are
/// read as the word. runtime.ts:1025 (origin), runtime.ts:53 (ticket.issue), wire/devices.ts TicketPurpose; check.rs:450 rules_of reads the Value through
/// serde, workspace.rs WorkspaceOrigin and devices.rs:58 TicketPurpose have no pass of their own. A map holding anything
/// else says the wrong place: `{"connect":{}}` is zod's invalid_type at ["purpose"], here at ["purpose","connect"].
#[test]
fn a_unit_enum_word_must_be_a_string() {
    assert_eq!(
        first_issue::<RuntimeRequest>(r#"{"id":"a","op":"ticket.issue","purpose":{"connect":null}}"#),
        issue("invalid_type", json!(["purpose"]), "Expected 'connect' | 'relay', received object"),
    );
    assert_eq!(
        first_issue::<RuntimeRequest>(r#"{"id":"a","op":"auth","token":"t","origin":{"here":null}}"#),
        issue("invalid_type", json!(["origin"]), "Expected 'here' | 'relayed' | 'paired', received object"),
    );
    assert_eq!(
        first_issue::<RuntimeRequest>(r#"{"id":"a","op":"ticket.issue","purpose":{"connect":{}}}"#),
        issue("invalid_type", json!(["purpose"]), "Expected 'connect' | 'relay', received object"),
    );
    assert_eq!(
        first_issue::<RuntimeRequest>(r#"{"id":"a","op":"auth","token":"t","origin":{"x":1}}"#),
        issue("invalid_type", json!(["origin"]), "Expected 'here' | 'relayed' | 'paired', received object"),
    );
}

/// MAJOR (a struct is read from a JSON array too: serde's positional form). zod's z.object refuses an array with
/// "Expected object, received array"; views/app-procs.ts:25 AppProc has no pass in app_procs.rs:58, so appProcs.set
/// (runtime.ts, `procs: z.array(AppProc)`) takes `[[pid, kind, origins, contents]]` and reads the row from the list.
#[test]
fn an_app_proc_row_is_an_object_not_a_list() {
    assert_eq!(
        first_issue::<RuntimeRequest>(r#"{"id":"a","op":"appProcs.set","procs":[[1,"main",["a"],[1]]]}"#),
        issue("invalid_type", json!(["procs", 0]), "Expected object, received array"),
    );
}

// -----------------------------------------------------------------------------------------------------------------
// DECISION-5 and what it leaves out of the list
// -----------------------------------------------------------------------------------------------------------------

/// DECISION-5, and wider than the spec says: spec/check.md Decision 5 says a rule issue on an earlier key and a type issue
/// on a later one "swap". The rule issue is not moved, it is dropped: RuntimeOp's pass (runtime.rs:1398 and the same line
/// in every arm) runs the member's rules only when no key has a type issue, so the list of issues (what zod's
/// `error.message` hands the client, runtime/src/serve.ts:677) holds the type issues alone. 36 of the 211 ops have a rule
/// on a key that comes before another key (31 inline members, plus seal.open, device.auth, place.join, place.auth and
/// place.prove): pair.redeem, places.doctor, places.set, places.add, events.subscribe, sessions.history, workspaces.exec,
/// agents.key, image.export, slates.event and the rest. runtime.ts:60 pair.redeem, :200 events.subscribe, sessions.history.
#[test]
fn an_inline_member_lists_every_issue_zod_lists() {
    let code = "x".repeat(65);
    // a rule on an earlier key, a type issue on a later one: zod answers the rule first
    assert_eq!(
        all_issues::<RuntimeRequest>(&format!(r#"{{"id":"a","op":"pair.redeem","code":"{code}","name":5}}"#)),
        pairs(&[("too_big", json!(["code"])), ("invalid_type", json!(["name"]))]),
    );
    // the type issue first: the rule issue after it is still listed
    assert_eq!(
        all_issues::<RuntimeRequest>(&format!(r#"{{"op":"pair.redeem","code":"{code}","name":"n"}}"#)),
        pairs(&[("invalid_union", json!(["id"])), ("too_big", json!(["code"]))]),
    );
    assert_eq!(
        all_issues::<RuntimeRequest>(r#"{"id":"a","op":"events.subscribe","after":-1,"stream":null}"#),
        pairs(&[("too_small", json!(["after"])), ("invalid_type", json!(["stream"]))]),
    );
    assert_eq!(
        all_issues::<RuntimeRequest>(r#"{"id":"a","op":"sessions.history","workspaceId":"w","threadId":"t","before":0,"limit":null}"#),
        pairs(&[("too_small", json!(["before"])), ("invalid_type", json!(["limit"]))]),
    );
}

/// MINOR. zod's z.record(EnvName, ...) answers one issue per key whose name is refused, at that key's path, so two bad
/// names are two issues; runtime.rs:729 env_names stops at the first. thread-defaults.ts:73 (AgentSetupSet.env), runtime.ts:832.
#[test]
fn agents_setup_lists_every_variable_name_it_refuses() {
    assert_eq!(
        all_issues::<RuntimeRequest>(r#"{"id":"a","op":"agents.setup","placeId":"p","agent":"claude","on":true,"env":{"a b":"x","c d":"y"}}"#),
        pairs(&[("invalid_string", json!(["env", "a b"])), ("invalid_string", json!(["env", "c d"]))]),
    );
}

/// MINOR. ZodNumber runs every check after a failed `.int()` (the int check marks the parse dirty and goes on), so a
/// fraction past a bound is two issues, the integer one and then the bound's. JsInt fails to read, so the bound never
/// runs: one issue. events.subscribe `after: .int().nonnegative()`, places.add `sshPort: .int().min(1).max(65535)`
/// (runtime.ts:152, :142), and every `.int()` with a bound: Sequenced.seq, ports, RelayPort, ProviderKeyWorks, ...
#[test]
fn a_fraction_past_a_bound_is_two_issues() {
    assert_eq!(
        all_issues::<RuntimeRequest>(r#"{"id":"a","op":"events.subscribe","after":-0.5}"#),
        pairs(&[("invalid_type", json!(["after"])), ("too_small", json!(["after"]))]),
    );
    assert_eq!(
        all_issues::<RuntimeRequest>(r#"{"id":"a","op":"places.add","address":"a","sshPort":70000.5}"#),
        pairs(&[("invalid_type", json!(["sshPort"])), ("too_big", json!(["sshPort"]))]),
    );
    assert_eq!(
        all_issues::<EventUnion>(r#"{"type":"workspace.deleted","workspaceId":"w","seq":0.5}"#),
        pairs(&[("invalid_type", json!(["seq"])), ("too_small", json!(["seq"]))]),
    );
}

// -----------------------------------------------------------------------------------------------------------------
// Written JSON
// -----------------------------------------------------------------------------------------------------------------

/// MINOR. zod's number is a double: `5.0` and `-0` are read as 5 and -0 and JSON.stringify writes `5` and `0`.
/// wsp-frames' RequestId keeps serde_json::Number, which writes `5.0` and `-0.0` (the id is echoed on every reply).
/// runtime.ts reqId (helpers.ts), wsp-frames id.rs:10. node: `{"id":5,"op":"workspaces.list"}`, `{"id":0,"op":"workspaces.list"}`.
#[test]
fn a_request_id_is_written_as_javascript_writes_a_number() {
    let five = parse::<RuntimeRequest>(&frame(r#"{"id":5.0,"op":"workspaces.list"}"#)).unwrap();
    assert_eq!(serde_json::to_string(five.op.id()).unwrap(), "5");
    let zero = parse::<RuntimeRequest>(&frame(r#"{"id":-0,"op":"workspaces.list"}"#)).unwrap();
    assert_eq!(serde_json::to_string(zero.op.id()).unwrap(), "0");
}

/// MINOR. JavaScript holds a key that reads as an array index (`"1"`, `"2"`) ahead of every other key, ascending, whatever
/// order it came in; zod's records, passthrough objects and `request_secrets`' Object.values all follow. The IndexMaps
/// here keep the order sent. node: `request_secrets({"env":{"b":"1","2":"two","1":"one","a":"z"}})` is
/// ["one","two","1","z"]; RuntimeOkResponse writes `{"1":"c","2":"a","id":1,"ok":true,"b":1}`; workspaces.create's envs
/// `{"1":"z","2":"y","b":"x"}`. runtime.rs:4359 request_secrets, runtime.rs:521 RuntimeOkResponse.rest, runtime_ops.rs
/// WorkspacesCreate.envs (spec/runtime_ops.md rule 4.26 says "in the order sent", which is not what JavaScript does).
#[test]
fn a_record_holds_its_index_like_keys_first() {
    assert_eq!(
        request_secrets(&frame(r#"{"env":{"b":"1","2":"two","1":"one","a":"z"}}"#)),
        vec!["one".to_string(), "two".to_string(), "1".to_string(), "z".to_string()],
    );
    assert_eq!(
        written::<RuntimeOkResponse>(r#"{"id":1,"ok":true,"2":"a","b":1,"1":"c"}"#),
        r#"{"1":"c","2":"a","id":1,"ok":true,"b":1}"#,
    );
    let request = parse::<RuntimeRequest>(&frame(r#"{"id":"a","op":"workspaces.create","project":"p","name":"n","envs":{"b":"x","2":"y","1":"z"}}"#)).unwrap();
    let envs = serde_json::to_value(&request).unwrap()["envs"].clone();
    assert_eq!(serde_json::to_string(&envs).unwrap(), r#"{"1":"z","2":"y","b":"x"}"#);
}

/// NIT. zod's object merge skips the key `__proto__` (ParseStatus.mergeObjectSync), in a record and in a passthrough
/// object alike, so it never reaches the data. RuntimeOkResponse.rest and agents.setup's env keep it.
/// node: `{"id":1,"ok":true,"z":2}` and env `{"A":"b"}`.
#[test]
fn a_proto_key_is_not_kept_by_a_record_or_a_passthrough_object() {
    assert_eq!(written::<RuntimeOkResponse>(r#"{"id":1,"ok":true,"__proto__":{"x":1},"z":2}"#), r#"{"id":1,"ok":true,"z":2}"#);
    let request = parse::<RuntimeRequest>(&frame(r#"{"id":1,"op":"agents.setup","placeId":"p","agent":"a","env":{"__proto__":"x","A":"b"}}"#)).unwrap();
    let env = serde_json::to_value(&request).unwrap()["env"].clone();
    assert_eq!(serde_json::to_string(&env).unwrap(), r#"{"A":"b"}"#);
}

/// MINOR. A z.unknown() value is the parsed JavaScript value, written back by JSON.stringify: `5.0`, `-0` and `1e2` are
/// 5, 0 and 100, and keys that read as indexes come first. The serde_json::Value kept here writes `5.0`, `-0.0`, `100.0`
/// and the order sent. daemon-events.ts:88 guest.message `message`, runtime.ts:189 recipes.save `file`.
/// node: `{"type":"guest.message","session":"s","message":{"n":5,"m":0,"k":100}}`.
#[test]
fn a_value_kept_as_sent_is_written_as_javascript_writes_it() {
    assert_eq!(
        written::<DaemonEvent>(r#"{"type":"guest.message","session":"s","message":{"n":5.0,"m":-0,"k":1e2}}"#),
        r#"{"type":"guest.message","session":"s","message":{"n":5,"m":0,"k":100}}"#,
    );
    assert_eq!(
        written::<DaemonEvent>(r#"{"type":"guest.message","session":"s","message":{"b":1,"2":2,"1":3}}"#),
        r#"{"type":"guest.message","session":"s","message":{"1":3,"2":2,"b":1}}"#,
    );
}

/// MAJOR (a value the TS drops is written). pull-request.ts:102 PullRequestSeen is `z.union([Fact, Kept, Unread])`; zod
/// takes the first member that is valid, and a Fact whose `additions` is -1 is not (it is only dirty, so zod goes on),
/// so the next member, PullRequestKept, answers and strips the keys it does not name. serde's untagged enum takes the
/// first member that merely deserializes, the Fact, with `additions: -1` and every key. Seen in every workspace.status
/// and workspace.created event (views/workspace.ts:577 `pr`); types/pull_request.rs:731.
#[test]
fn a_pull_request_seen_is_the_first_member_that_reads_clean() {
    let text = r#"{"number":1,"url":"u","state":"open","host":"h","draft":false,"base":"b","branch":"br","headOid":"o","headSubject":"s","mergeable":"mergeable","mergeState":"m","review":"none","checks":[],"additions":-1,"deletions":0,"changedFiles":0,"commits":0,"readAt":1}"#;
    assert_eq!(written::<PullRequestSeen>(text), r#"{"number":1,"url":"u","state":"open","base":"b","branch":"br","checks":[],"readAt":1}"#);
}

// -----------------------------------------------------------------------------------------------------------------
// Refusal words
// -----------------------------------------------------------------------------------------------------------------

/// MINOR. A key that is a z.enum or an inline object of runtime.ts and gets the wrong type is zod's `Expected 'a' | 'b',
/// received number` or `Expected object, received null`; the Rust types of runtime.rs that stand for them (the inline
/// enums at runtime.rs:4421-4490, the inline structs at :3573-3800) carry no `describe` and answer
/// `Expected unknown, received ...`. workspaces.reviewDraft verdict, agents.key way, agents.cloudSignIn agent, slates.event
/// event, slates.approve scope, places.set spawn and size, workspaces.create agents and sub, slates.write press and
/// slates.event scope; and the Session*Result outcomes. Code and path match; only the message and `expected` differ.
#[test]
fn an_inline_enum_or_object_says_what_zod_expects() {
    assert_eq!(
        first_issue::<RuntimeRequest>(r#"{"id":"a","op":"workspaces.reviewDraft","workspaceId":"w","verdict":5}"#),
        issue("invalid_type", json!(["verdict"]), "Expected 'comment' | 'approve' | 'request_changes', received number"),
    );
    assert_eq!(
        first_issue::<RuntimeRequest>(r#"{"id":"a","op":"agents.key","agent":"claude","key":"k","way":5}"#),
        issue("invalid_type", json!(["way"]), "Expected 'token' | 'key', received number"),
    );
    assert_eq!(
        first_issue::<RuntimeRequest>(r#"{"id":"a","op":"places.set","placeId":"p","spawn":null}"#),
        issue("invalid_type", json!(["spawn"]), "Expected object, received null"),
    );
    assert_eq!(
        first_issue::<SessionSteerResult>(r#"{"outcome":5}"#),
        issue("invalid_type", json!(["outcome"]), "Expected 'accepted' | 'not-running' | 'unsupported' | 'not-found', received number"),
    );
}

/// MINOR. RuntimeOkResponse and RuntimeErrorResponse (runtime.ts:1277-1288) hold `ok: z.literal(true)` and
/// `z.literal(false)`; a missing literal is zod's invalid_literal ("Invalid literal value, expected true"), not "Required".
/// runtime.rs:3872 and :3889 read the key with `missing::<Self>`, which has no word for it.
#[test]
fn a_response_with_no_ok_answers_invalid_literal() {
    assert_eq!(first_issue::<RuntimeOkResponse>(r#"{"id":"a"}"#), issue("invalid_literal", json!(["ok"]), "Invalid literal value, expected true"));
    assert_eq!(first_issue::<RuntimeErrorResponse>(r#"{"id":"a","error":"e"}"#), issue("invalid_literal", json!(["ok"]), "Invalid literal value, expected false"));
}

// -----------------------------------------------------------------------------------------------------------------
// A key z.unknown() or an optional struct takes and the Rust refuses
// -----------------------------------------------------------------------------------------------------------------

/// MAJOR. slate/wire.ts:168 `scope: z.object({ item: SlateWireJson, index: ... })` with SlateWireJson = z.unknown() (:10), which takes undefined, so a
/// scope with no `item` is read and written as sent. runtime.rs:493 SlatesEventScope.item is a required `Value`: the
/// frame is refused with "Required" at ["scope","item"]. (review_core's an_unknown_key_takes_null_and_absence names
/// recipes.save, guest.send and guest.message; this is the fourth.)
#[test]
fn a_slate_event_scope_may_leave_out_item() {
    let text = r#"{"id":"a","op":"slates.event","threadId":"t","version":1,"piece":"p","event":"press","requestId":"r","scope":{"index":1}}"#;
    assert_eq!(first_issue::<RuntimeRequest>(text), None);
}
