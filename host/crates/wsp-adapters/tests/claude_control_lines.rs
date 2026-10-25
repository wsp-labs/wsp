// SPDX-License-Identifier: AGPL-3.0-only
//! The hard cases of unit claude-control-lines (spec/claude_control_lines.md). Expected values were checked against
//! the TS functions at a3b870a6e.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use wsp_adapters::claude::fields::Obj;
use wsp_adapters::claude::handback::{end_answer, note_line, read_answer, task_ended, HandbackBook, TaskEnd};
use wsp_adapters::claude::permissions::{control_line, interrupt_line, ControlLine};
use wsp_adapters::claude::steers::Steers;
use wsp_adapters::pending::NO_REPORT_LINE;
use wsp_adapters::port::{AdapterEvent, DeltaKind, PermissionAsk, PermissionOption, SubagentState, TurnDelta};
use wsp_wire::wire::helpers::PermissionEffect;

fn obj(v: Value) -> Obj {
    v.as_object().cloned().expect("an object")
}

fn user(uuid: Option<&str>) -> String {
    let mut line = json!({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": "hi"}]}, "parent_tool_use_id": null, "session_id": "s"});
    if let Some(u) = uuid {
        line["uuid"] = json!(u);
    }
    line.to_string()
}

fn lifecycle(uuid: &str, state: &str) -> Obj {
    obj(json!({"type": "command_lifecycle", "command_uuid": uuid, "state": state}))
}

fn init(capabilities: &[&str]) -> Obj {
    obj(json!({"type": "system", "subtype": "init", "session_id": "s", "capabilities": capabilities}))
}

fn told(steers: &mut Steers) -> Vec<AdapterEvent> {
    let seen: Arc<Mutex<Vec<AdapterEvent>>> = Default::default();
    let sink = seen.clone();
    steers.tell_unread("s", &move |e| sink.lock().unwrap().push(e));
    let out = seen.lock().unwrap().clone();
    out
}

#[test]
fn ask_options_are_allow_deny_then_each_mode_once() {
    let line = obj(json!({
        "type": "control_request",
        "request_id": "req-1",
        "request": {
            "subtype": "can_use_tool",
            "tool_name": "Bash",
            "agent_id": "agent-7",
            "description": "",
            "permission_suggestions": [
                {"type": "setMode", "mode": "acceptEdits", "destination": "session"},
                {"type": "addRules", "rules": []},
                {"type": "setMode", "mode": "acceptEdits"},
                {"type": "setMode"},
                {"type": "setMode", "mode": "plan"}
            ]
        }
    }));
    let opt = |id: &str, label: &str, effect: PermissionEffect, mode: Option<&str>| PermissionOption { id: id.into(), label: label.into(), effect, mode: mode.map(String::from) };
    assert_eq!(
        control_line(&line),
        Some(ControlLine::Ask {
            ask: PermissionAsk {
                ask_id: "req-1".into(),
                tool_name: "Bash".into(),
                tool_use_id: None,
                parent_tool_use_id: None,
                input: "null".into(),
                detail: None,
                options: vec![
                    opt("allow", "Allow", PermissionEffect::Allow, None),
                    opt("deny", "Deny", PermissionEffect::Deny, None),
                    opt("mode:acceptEdits", "acceptEdits", PermissionEffect::Mode, Some("acceptEdits")),
                    opt("mode:plan", "plan", PermissionEffect::Mode, Some("plan")),
                ],
            },
            agent_id: Some("agent-7".into()),
        }),
        "absent input is the text null, an empty description is no detail, the agent rides beside the ask"
    );
    assert_eq!(control_line(&obj(json!({"type": "control_request", "request_id": "r", "request": {"subtype": "can_use_tool"}}))), Some(ControlLine::Unknown { request_id: "r".into(), subtype: "can_use_tool".into() }));
    assert_eq!(control_line(&obj(json!({"type": "control_request", "request_id": "r", "request": {}}))), Some(ControlLine::Unknown { request_id: "r".into(), subtype: "".into() }));
    assert_eq!(control_line(&obj(json!({"type": "control_request", "request": {"subtype": "x"}}))), None);
    assert_eq!(control_line(&obj(json!({"type": "control_response", "response": {"subtype": "error", "request_id": "q"}}))), Some(ControlLine::Answer { request_id: "q".into(), error: Some("the CLI refused it without saying why".into()) }));
    assert_eq!(control_line(&obj(json!({"type": "control_cancel_request"}))), None);
}

#[test]
fn interrupt_line_carries_cancel_queued_only_when_asked() {
    assert_eq!(interrupt_line("wsp-interrupt-1", false), r#"{"type":"control_request","request_id":"wsp-interrupt-1","request":{"subtype":"interrupt"}}"#);
    assert_eq!(interrupt_line("wsp-interrupt-2", true), r#"{"type":"control_request","request_id":"wsp-interrupt-2","request":{"subtype":"interrupt","cancel_queued":true}}"#);
}

#[test]
fn steers_start_from_the_channel_then_the_kept_ids() {
    let taken = vec![user(Some("old")), user(None), user(Some("a")), "not json".into(), json!({"type": "control_response"}).to_string(), user(Some("b")), user(Some("a"))];
    let mut steers = Steers::new(&taken, &["c".to_string(), "b".to_string()]);
    assert!(!steers.reports() && !steers.open(), "nothing reported yet: a steer waits on nobody");
    assert!(!steers.read(&init(&["msg_lifecycle_v1"])), "an init is no lifecycle line");
    assert!(steers.reports() && !steers.cancels_queued());
    assert!(steers.open());
    for id in ["a", "b"] {
        steers.read(&lifecycle(id, "started"));
        steers.read(&lifecycle(id, "completed"));
    }
    assert!(steers.open(), "c is still the CLI's to answer");
    assert!(steers.settle());
    assert_eq!(told(&mut steers), vec![AdapterEvent::TurnUnread { session_id: "s".into(), ids: vec!["c".into()] }], "old went with the earlier turn; a and b were read");
}

#[test]
fn unread_is_fixed_at_the_reply_and_told_once() {
    let mut steers = Steers::new(&[], &[]);
    steers.read(&init(&["msg_lifecycle_v1", "interrupt_cancel_queued_v1"]));
    assert!(steers.cancels_queued());
    steers.add("x");
    steers.add("y");
    steers.add("z");
    assert!(steers.read(&lifecycle("x", "queued")));
    assert!(steers.read(&lifecycle("x", "cancelled")));
    assert!(steers.read(&lifecycle("y", "started")));
    assert!(steers.working());
    assert!(steers.read(&lifecycle("nobody", "started")), "an unknown uuid is still a lifecycle line");
    assert!(steers.settle(), "z is still queued");
    steers.read(&lifecycle("z", "started"));
    assert!(!steers.settle(), "nothing left unstarted, but the list stays as fixed");
    assert_eq!(told(&mut steers), vec![AdapterEvent::TurnUnread { session_id: "s".into(), ids: vec!["x".into(), "z".into()] }]);
    assert_eq!(told(&mut steers), vec![], "told once");
}

#[test]
fn a_cli_that_reports_nothing_tells_nothing_unread() {
    let mut steers = Steers::new(&[], &["k".to_string()]);
    steers.add("x");
    assert!(!steers.open());
    assert!(!steers.settle());
    assert_eq!(told(&mut steers), vec![]);
    let mut late = Steers::new(&[], &[]);
    late.add("x");
    late.read(&lifecycle("x", "completed"));
    assert!(late.reports(), "a lifecycle line is a report even without the init's word");
    late.forget("x");
    assert!(!late.open());
}

fn delta(kind: DeltaKind, text: &str) -> TurnDelta {
    TurnDelta::new("s", kind, text)
}

#[test]
fn a_framed_report_is_unframed_and_a_refused_handback_drawn() {
    let mut book = HandbackBook::default();
    let framed = TurnDelta { tool_use_id: Some("call-1".into()), ..delta(DeltaKind::ToolResult, "[Subagent hand-back] Here is the report.\n  line one\n    indented\n\n  not reached\nCLI note") };
    assert_eq!(read_answer(&mut book, framed.clone()), vec![TurnDelta { text: "line one\n  indented".into(), ..framed.clone() }], "the frame ends at the first line without two spaces, an empty one included");
    let bare = TurnDelta { tool_use_id: Some("call-1".into()), ..delta(DeltaKind::ToolResult, "[Subagent hand-back] only") };
    assert_eq!(read_answer(&mut book, bare.clone()), vec![TurnDelta { text: "".into(), ..bare }]);
    let call = TurnDelta { tool_name: Some("SubagentHandback".into()), tool_use_id: Some("hb-1".into()), parent_tool_use_id: Some("call-9".into()), ..delta(DeltaKind::ToolUse, "{}") };
    assert!(wsp_adapters::claude::handback::held_call(&mut book, &call));
    let refused = TurnDelta { tool_use_id: Some("hb-1".into()), parent_tool_use_id: Some("call-9".into()), ..delta(DeltaKind::ToolResult, r#"{"success":false,"message":"hand-back is off"}"#) };
    assert_eq!(read_answer(&mut book, refused.clone()), vec![call.clone(), TurnDelta { text: "hand-back is off".into(), is_error: Some(true), ..refused }]);
    assert!(book.held.is_empty());
    assert!(wsp_adapters::claude::handback::held_call(&mut book, &call));
    let went = TurnDelta { tool_use_id: Some("hb-1".into()), ..delta(DeltaKind::ToolResult, r#"{"success":true}"#) };
    assert_eq!(read_answer(&mut book, went), vec![]);
    let lead_call = TurnDelta { tool_name: Some("SubagentHandback".into()), tool_use_id: Some("hb-2".into()), ..delta(DeltaKind::ToolUse, "{}") };
    assert!(!wsp_adapters::claude::handback::held_call(&mut book, &lead_call), "the lead's own call is drawn as it comes");
}

#[test]
fn task_ended_maps_every_word_and_drops_an_empty_summary() {
    let note = |status: &str, summary: &str| obj(json!({"type": "system", "subtype": "task_notification", "task_id": "t", "status": status, "summary": summary}));
    let end = |state, summary: Option<&str>| Some(TaskEnd { task: "t".into(), state, summary: summary.map(String::from) });
    assert_eq!(task_ended(&note("completed", "built it")), end(SubagentState::Done, Some("built it")));
    assert_eq!(task_ended(&note("failed", "")), end(SubagentState::Failed, None));
    assert_eq!(task_ended(&note("stopped", "x")), end(SubagentState::Stopped, Some("x")));
    assert_eq!(task_ended(&note("killed", "x")), end(SubagentState::Stopped, Some("x")));
    assert_eq!(task_ended(&note("new-word", "x")), end(SubagentState::Failed, Some("x")));
    assert_eq!(task_ended(&obj(json!({"type": "system", "subtype": "task_updated", "task_id": "t", "patch": {"status": "killed"}}))), end(SubagentState::Stopped, None));
    assert_eq!(task_ended(&obj(json!({"type": "system", "subtype": "task_updated", "task_id": "t", "patch": {"status": "completed"}}))), None);
    assert_eq!(task_ended(&obj(json!({"type": "assistant", "subtype": "task_notification", "task_id": "t", "status": "completed"}))), None);
    let withheld = obj(json!({"type": "system", "subtype": "task_notification", "task_id": "t", "status": "completed", "handback": "withheld", "summary": "Tell the user what you found."}));
    assert_eq!(task_ended(&withheld), end(SubagentState::Done, Some(NO_REPORT_LINE)), "a note to the model is never the last line");
    let warned = obj(json!({"type": "system", "subtype": "task_notification", "task_id": "t", "status": "completed", "handback": "send", "handback_report": {"text": "the report", "warning": "check this"}}));
    assert_eq!(task_ended(&warned), end(SubagentState::Done, Some("check this")));
}

#[test]
fn end_answer_says_a_report_once_and_none_in_words() {
    let mut book = HandbackBook::default();
    note_line(&mut book, &obj(json!({"type": "system", "subtype": "task_started", "task_id": "a1", "task_type": "local_agent", "tool_use_id": "call-a"})));
    note_line(&mut book, &obj(json!({"type": "system", "subtype": "task_started", "task_id": "a2", "task_type": "local_agent", "tool_use_id": "call-b", "is_backgrounded": true})));
    note_line(&mut book, &obj(json!({"type": "assistant", "parent_tool_use_id": "call-a", "message": {"content": [{"type": "text", "text": "first"}, {"type": "text", "text": "  the report \n"}]}})));
    let sent = obj(json!({"type": "system", "subtype": "task_notification", "task_id": "a1", "tool_use_id": "call-a", "status": "completed", "handback": "send", "handback_report": {"text": "the report"}}));
    let tool_result = |text: &str, call: &str, failed: bool| TurnDelta { tool_use_id: Some(call.into()), is_error: Some(failed), ..delta(DeltaKind::ToolResult, text) };
    assert_eq!(end_answer(&mut book, &sent, "s"), vec![tool_result("the report", "call-a", false)], "its last words already say it");
    assert!(book.notified.contains("a1"));
    assert_eq!(book.answered.get("call-a").map(String::as_str), Some("the report"));
    let withheld = obj(json!({"type": "system", "subtype": "task_notification", "task_id": "a2", "tool_use_id": "call-b", "status": "failed", "handback": "withheld"}));
    assert_eq!(
        end_answer(&mut book, &withheld, "s"),
        vec![TurnDelta { parent_tool_use_id: Some("call-b".into()), ..delta(DeltaKind::Note, NO_REPORT_LINE) }, tool_result(NO_REPORT_LINE, "call-b", true)]
    );
    let stranger = obj(json!({"type": "system", "subtype": "task_notification", "task_id": "bash-1", "tool_use_id": "call-x", "status": "completed"}));
    assert_eq!(end_answer(&mut book, &stranger, "s"), vec![]);
    assert!(!book.notified.contains("bash-1"), "a background command is none of this");
}

fn option(id: &str, label: &str, effect: PermissionEffect, mode: Option<&str>) -> PermissionOption {
    PermissionOption { id: id.into(), label: label.into(), effect, mode: mode.map(String::from) }
}

fn bash_ask(input: &str, options: Vec<PermissionOption>) -> PermissionAsk {
    PermissionAsk { ask_id: "r1".into(), tool_name: "Bash".into(), tool_use_id: None, parent_tool_use_id: None, input: input.into(), detail: None, options }
}

fn consent() -> Vec<PermissionOption> {
    vec![option("allow", "Allow", PermissionEffect::Allow, None), option("deny", "Deny", PermissionEffect::Deny, None), option("mode:x", "x", PermissionEffect::Mode, None)]
}

// Rule C1: the answer hands the input back as parsed: only a JSON null becomes {}; 0, false, "" and [] stay.
#[test]
fn an_answer_hands_back_any_parsed_input_but_null() {
    let back: Vec<Value> = ["0", "false", "\"\"", "[]", "null"]
        .iter()
        .map(|input| wsp_adapters::claude::permissions::control_answer_line(&bash_ask(input, consent()), "allow", "").unwrap())
        .map(|line| serde_json::from_str::<Value>(&line).unwrap()["response"]["response"]["updatedInput"].clone())
        .collect();
    assert_eq!(back, vec![json!(0), json!(false), json!(""), json!([]), json!({})]);
}

// Rule C2: a mode option with no mode writes its permission update with no mode key, byte for byte.
#[test]
fn a_mode_option_with_no_mode_writes_no_mode_key() {
    let line = wsp_adapters::claude::permissions::control_answer_line(&bash_ask("{}", consent()), "mode:x", "").unwrap();
    assert_eq!(line, r#"{"type":"control_response","response":{"subtype":"success","request_id":"r1","response":{"behavior":"allow","updatedInput":{},"updatedPermissions":[{"type":"setMode","destination":"session"}]}}}"#);
}

// Rule C3: a question offers no allow, so allowing it unasked is refused with the pick's words.
#[test]
fn allowing_a_question_unasked_is_refused() {
    let question = PermissionAsk { ask_id: "q".into(), tool_name: "AskUserQuestion".into(), tool_use_id: None, parent_tool_use_id: None, input: "{}".into(), detail: None, options: vec![option("q0:o0", "A", PermissionEffect::Answer, None)] };
    assert_eq!(wsp_adapters::claude::permissions::control_allow_line(&question).map_err(|e| e.0), Err("allow is not an option on this permission prompt".to_string()));
}

// Rule C4: empty-string ids are kept and ids that are no strings are dropped; a description that is no string is no
// detail; a mode suggestion needs type setMode and a string mode, "" included, each mode once.
#[test]
fn an_ask_keeps_empty_ids_and_reads_only_string_mode_suggestions() {
    let kept = control_line(&obj(json!({"type": "control_request", "request_id": "r1", "request": {"subtype": "can_use_tool", "tool_name": "Bash", "description": "d", "tool_use_id": "", "agent_id": "", "input": {}}})));
    let allow_deny = vec![option("allow", "Allow", PermissionEffect::Allow, None), option("deny", "Deny", PermissionEffect::Deny, None)];
    assert_eq!(
        kept,
        Some(ControlLine::Ask { ask: PermissionAsk { ask_id: "r1".into(), tool_name: "Bash".into(), tool_use_id: Some("".into()), parent_tool_use_id: None, input: "{}".into(), detail: Some("d".into()), options: allow_deny.clone() }, agent_id: Some("".into()) })
    );
    let suggestions = json!([{"type": "setMode", "mode": ""}, {"mode": "plan"}, null, 5, {"type": "setMode", "mode": 5}, {"type": "setMode", "mode": "plan"}, {"type": "setMode", "mode": "plan", "destination": "x"}, {"type": "setMode", "mode": "default"}]);
    let odd = control_line(&obj(json!({"type": "control_request", "request_id": "r1", "request": {"subtype": "can_use_tool", "tool_name": "Bash", "description": 5, "tool_use_id": 7, "agent_id": 9, "input": {}, "permission_suggestions": suggestions}})));
    let mut options = allow_deny;
    options.extend([option("mode:", "", PermissionEffect::Mode, Some("")), option("mode:plan", "plan", PermissionEffect::Mode, Some("plan")), option("mode:default", "default", PermissionEffect::Mode, Some("default"))]);
    assert_eq!(odd, Some(ControlLine::Ask { ask: PermissionAsk { ask_id: "r1".into(), tool_name: "Bash".into(), tool_use_id: None, parent_tool_use_id: None, input: "{}".into(), detail: None, options }, agent_id: None }));
}

// Rule C5: a lifecycle state other than queued or started (absent, a number, a new word) drops the steer, every drop
// is told, the same id again included, and the ones still open follow in the order added.
#[test]
fn every_drop_is_told_in_order_and_never_deduped() {
    let mut steers = Steers::new(&[], &[]);
    steers.read(&init(&["msg_lifecycle_v1"]));
    for id in ["a", "b", "c", "d"] {
        steers.add(id);
    }
    steers.read(&obj(json!({"type": "command_lifecycle", "command_uuid": "a"})));
    steers.read(&obj(json!({"type": "command_lifecycle", "command_uuid": "b", "state": 5})));
    steers.read(&lifecycle("c", "weird"));
    steers.read(&lifecycle("zz", "weird"));
    steers.add("a");
    steers.read(&lifecycle("a", "weird"));
    assert!(steers.settle());
    assert_eq!(told(&mut steers), vec![AdapterEvent::TurnUnread { session_id: "s".into(), ids: vec!["a".into(), "b".into(), "c".into(), "a".into(), "d".into()] }]);
}

// Rule C6: settle on a CLI that reports nothing fixes the unread list empty for good, a later report notwithstanding.
#[test]
fn a_settle_before_any_report_tells_nothing_ever() {
    let mut steers = Steers::new(&[], &[]);
    steers.add("m");
    assert!(!steers.settle());
    steers.read(&lifecycle("m", "queued"));
    assert_eq!((steers.reports(), steers.open()), (true, true));
    assert_eq!(told(&mut steers), vec![]);
}

// Rule C7: steered_ids keeps "" as an id and adds no id twice; a uuid that is no string clears the list.
#[test]
fn steered_ids_keep_the_empty_id_and_clear_on_a_line_with_no_string_uuid() {
    let line = |uuid: Value| json!({"type": "user", "message": {"role": "user", "content": []}, "uuid": uuid}).to_string();
    assert_eq!(wsp_adapters::claude::steers::steered_ids(&[line(json!("")), line(json!("a")), line(json!(""))]), vec!["", "a"]);
    assert_eq!(wsp_adapters::claude::steers::steered_ids(&[line(json!("a")), line(json!(null)), line(json!("b")), line(json!(5)), line(json!("c"))]), vec!["c"]);
}

// Rule C8: task_ended reads the words exactly: a kill is stopped, a status word is case-sensitive, a hand-back of null
// is no report, a blank warning is none.
#[test]
fn a_task_end_reads_each_word_exactly() {
    let note = |extra: Value| {
        let mut line = json!({"type": "system", "subtype": "task_notification", "task_id": "t"});
        line.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        task_ended(&obj(line))
    };
    let end = |state: SubagentState, summary: Option<&str>| Some(TaskEnd { task: "t".into(), state, summary: summary.map(String::from) });
    assert_eq!(task_ended(&obj(json!({"type": "system", "subtype": "task_updated", "task_id": "t", "patch": {"status": "killed"}, "summary": "s"}))), end(SubagentState::Stopped, None));
    assert_eq!(note(json!({"status": "Completed"})), end(SubagentState::Failed, None));
    assert_eq!(note(json!({})), None);
    assert_eq!(note(json!({"status": "completed", "handback": null, "summary": "s"})), end(SubagentState::Done, Some(NO_REPORT_LINE)));
    assert_eq!(note(json!({"status": "completed", "handback": "send", "handback_report": {"text": "T", "warning": ""}})), end(SubagentState::Done, None));
    assert_eq!(note(json!({"status": "completed", "handback": "send", "handback_report": {"text": "T", "warning": "W"}})), end(SubagentState::Done, Some("W")));
}
