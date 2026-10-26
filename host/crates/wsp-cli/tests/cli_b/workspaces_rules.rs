// SPDX-License-Identifier: AGPL-3.0-only
// spec/workspaces-git.md and spec/workspaces-life.md rules that guard what a delete takes away: a question answered
// no and a tool call without confirm send no delete, and the host's own check goes out before any question.

use crate::common::{case, holds, tool_holds};

fn ops(frames: &[String]) -> Vec<String> {
    frames.iter().map(|f| serde_json::from_str::<serde_json::Value>(f).unwrap()["op"].as_str().unwrap().to_string()).collect()
}

/// A delete is the one frame without `check`; the check is the one with it.
fn deletes(frames: &[String]) -> usize {
    frames.iter().filter(|f| f.contains("\"op\":\"sessions.delete\"") && !f.contains("\"check\":true")).count()
}

#[test]
fn delete_answered_no_sends_no_delete() {
    let c = case("workspaces_life", "hand", "delete at a terminal answered no");
    holds(&c);
    assert_eq!(deletes(&c.text.frames), 0, "{:?}", c.text.frames);
    assert_eq!(c.text.exit, 1);
    assert!(c.text.stdout.is_empty() && c.json.stdout.is_empty(), "a kept thread prints nothing on stdout");
    let yes = case("workspaces_life", "hand", "delete at a terminal answered yes");
    holds(&yes);
    assert_eq!(deletes(&yes.text.frames), 1);
}

#[test]
fn delete_checks_with_the_host_before_it_asks() {
    for name in ["thread unconfirmed", "thread in the folder unconfirmed", "sub-thread unconfirmed"] {
        let c = case("workspaces_life", "delete", name);
        holds(&c);
        assert_eq!(deletes(&c.text.frames), 0, "{name}");
        assert_eq!(ops(&c.text.frames).last().map(String::as_str), Some("sessions.delete"), "{name}: the check is the last frame");
    }
}

#[test]
fn tool_delete_without_confirm_sends_no_delete() {
    for name in
        ["unconfirmed", "thread unconfirmed", "thread in the folder unconfirmed", "sub-thread unconfirmed", "unconfirmed never made"]
    {
        let c = case("workspaces_life", "delete", name);
        tool_holds(&c);
        let result = c.tool_result.as_ref().unwrap();
        assert_eq!(result["isError"], true, "{name}: an unconfirmed delete answers as an error");
        for asked in c.tool_asked.as_ref().unwrap().as_array().unwrap() {
            let deleting = asked["op"] == "workspaces.delete" || (asked["op"] == "sessions.delete" && asked.get("check").is_none());
            assert!(!deleting, "{name}: sent {asked}");
        }
    }
}

#[test]
fn discard_checks_before_it_asks_and_keeps_the_file_off_a_terminal() {
    let c = case("workspaces_git", "discard", "discarded");
    holds(&c);
    let checks: Vec<&String> = c.text.frames.iter().filter(|f| f.contains("\"op\":\"workspaces.discard\"")).collect();
    assert_eq!(checks.len(), 2);
    assert!(checks[0].contains("\"check\":true") && !checks[1].contains("\"check\""));
}
