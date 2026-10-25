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
use wsp_wire::types::pending_wire::PermissionEffect;

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
