// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-written rules of the `agents_read` unit (`spec/agents-read.md`): the invariants and the orders a recorded case
//! does not state on its own, and the ports of the TS tests that call these helpers directly.

use wsp_cli::verbs::agents_help::read as a;
use wsp_wire::types::agents_report::AgentsReport;

use super::common::{case, cases, holds, Case};
use super::drivers_agents_read as d;

const UNIT: &str = "agents_read";

fn recorded(func: &str, name: &str) -> Case {
    case(UNIT, func, name, "<cwd>")
}

fn report_of(c: &Case) -> AgentsReport {
    serde_json::from_value(c.args["report"].clone()).expect("a recorded report reads")
}

/// Invariant 7 (packages/host/AGENTS.md): with the cloud off no line names a cloud. A word that names a machine is no
/// thread and is not called a machine, whatever workspaces the host holds; the cloud-on twin below is the setting and
/// not the data.
#[test]
fn invariant_7_cloud_off_a_machine_word_is_no_thread() {
    let off = recorded("agentsTarget", "a word that is a machine's name, the cloud off");
    assert!(!off.cloud);
    let said = off.error.clone().expect("a refusal");
    assert_eq!(said.message, "no thread attic-work");
    assert!(!said.message.contains("machine"));
    holds(UNIT, "agentsTarget", "a word that is a machine's name, the cloud off", d::agents_target);
}

#[test]
fn invariant_7_cloud_on_the_same_word_is_a_machine() {
    let on = recorded("agentsTarget", "a word that is a machine's name, the cloud on");
    assert!(on.cloud);
    assert!(on.error.clone().expect("a refusal").message.contains("attic-work is a machine"));
    holds(UNIT, "agentsTarget", "a word that is a machine's name, the cloud on", d::agents_target);
}

/// Invariant 7 for the rows: no line a list prints, with the cloud off, names a cloud. The one sentence that does
/// (`cloudServerOffLine`) is an `off` text the host writes for a cloud machine's report and the list passes on as it is.
#[test]
fn invariant_7_the_row_functions_add_no_cloud_word_of_their_own() {
    let mut seen = 0;
    for c in cases(UNIT) {
        if !["agentRowLines", "skillRowLines", "serverRowLines", "pluginRowLines", "toolLines"].contains(&c.func.as_str()) {
            continue;
        }
        let args = serde_json::to_string(&c.args).unwrap().to_lowercase();
        if args.contains("cloud") {
            continue;
        }
        let said = c.value.as_ref().map(|v| v.to_string().to_lowercase()).unwrap_or_default();
        assert!(!said.contains("cloud"), "{}: {said}", c.case);
        seen += 1;
    }
    assert!(seen > 100);
}

/// A `--watch` that cannot redraw is refused before the host is dialled: no dial, no frame, nothing printed. A line with
/// nowhere to redraw never starts a host to say so.
#[test]
fn a_watch_with_nowhere_to_redraw_is_refused_before_any_dial() {
    for name in [
        "--watch with --json is refused before any dial",
        "--watch with no terminal to redraw on is refused before any dial",
        "--watch with --json and a terminal is refused for --json first",
        "--watch with a different line's words",
    ] {
        let c = recorded("drawRows", name);
        assert!(c.said.is_empty() && c.frames.is_empty() && c.stdout.is_empty() && c.streamed.is_none(), "{name}");
        assert!(c.error.as_ref().expect("a refusal").kind.as_deref() == Some("usage"), "{name}");
        holds(UNIT, "drawRows", name, d::draw_rows);
    }
}

/// A watch of any length is one dial however many frames it draws, and the signals it hangs are taken off on the way out.
#[test]
fn a_watch_holds_one_dial_over_every_frame() {
    let name = "--watch draws every frame over one dial";
    let c = recorded("drawRows", name);
    assert_eq!(c.said.iter().filter(|s| s.as_str() == "dial").count(), 1);
    assert_eq!(c.said.iter().filter(|s| s.starts_with("frame ")).count(), 2);
    assert_eq!(c.said.iter().filter(|s| s.starts_with("on ")).count(), 3);
    assert_eq!(c.said.iter().filter(|s| s.starts_with("off ")).count(), 3);
    holds(UNIT, "drawRows", name, d::draw_rows);
}

/// A table's lines never end in white space, and what stands under it (the napping line, the refused readers) comes after
/// every row. Read off every recorded report, so a realistic one and a bare one both hold.
#[test]
fn row_tables_never_end_in_white_space_and_the_tail_comes_last() {
    let mut seen = 0;
    for c in cases(UNIT) {
        let lines = match c.func.as_str() {
            "agentRowLines" => a::agent_row_lines(&report_of(&c)),
            "skillRowLines" => a::skill_row_lines(&report_of(&c)),
            "serverRowLines" => a::server_row_lines(&report_of(&c)),
            "pluginRowLines" => a::plugin_row_lines(&report_of(&c)),
            _ => continue,
        };
        for line in &lines {
            assert_eq!(line.as_str(), line.trim_end(), "{}: {line:?}", c.case);
        }
        let report = report_of(&c);
        let refused: Vec<String> = lines.iter().filter(|l| l.starts_with("refused: ")).cloned().collect();
        assert_eq!(refused.len(), report.refused.len(), "{}", c.case);
        if let Some(first) = lines.iter().position(|l| l.starts_with("refused: ")) {
            assert!(lines[first..].iter().all(|l| l.starts_with("refused: ")), "{}: refused lines are last", c.case);
        }
        seen += 1;
    }
    assert!(seen > 100);
}

/// `reportFacts` carries the report's own facts and never a row list or `reach`.
#[test]
fn report_facts_never_carry_rows_or_reach() {
    for c in cases(UNIT).into_iter().filter(|c| c.func == "reportFacts") {
        let facts = serde_json::to_value(a::report_facts(&report_of(&c))).unwrap();
        for key in ["agents", "skills", "servers", "plugins", "reach"] {
            assert!(facts.get(key).is_none(), "{}: {key}", c.case);
        }
        assert_eq!(&facts, c.value.as_ref().unwrap(), "{}", c.case);
    }
}

/// agents-reader.test.ts: "wsp servers and the servers tool print these rows: off, the sentence once per server, and no
/// sign-in command", on a cloud machine's report.
#[test]
fn ts_agents_reader_a_cloud_machines_servers_print_off_with_the_sentence_once() {
    let c = recorded("serverRowLines", "a cloud machine's report: servers that need the person's sign-in are off");
    let lines = a::server_row_lines(&report_of(&c));
    let find = |p: &str| lines.iter().find(|l| l.starts_with(p)).unwrap_or_else(|| panic!("no row {p}")).clone();
    assert!(find("signs ").ends_with(" off"));
    assert!(find("open ").ends_with(" unknown"));
    for name in ["signs", "cf", "linear", "keyed", "aws-mcp"] {
        let sentence =
            format!("{name} runs only on a computer where you signed in to it; a cloud machine holds no sign-in or key of yours.");
        assert_eq!(lines.iter().filter(|l| **l == sentence).count(), 1, "{name}");
    }
    let all = lines.join("\n");
    assert!(!all.contains("mcp login") && !all.contains("mcp auth"));
    holds(UNIT, "serverRowLines", "a cloud machine's report: servers that need the person's sign-in are off", d::server_row_lines);
}

/// agents-signin.test.ts: `addTools({kind: "here"}, "codex")` answers `{file}`, and an agent the catalog does not know
/// is refused by the host with its own sentence, which is passed on.
#[test]
fn ts_agents_signin_add_tools_answers_the_file_and_passes_the_refusal_on() {
    let ok = recorded("addTools", "the answer is cut to its file");
    assert_eq!(ok.value.clone().unwrap(), serde_json::json!({ "file": "~/.codex/config.toml" }));
    assert_eq!(ok.frames, [r#"{"target":{"placeId":"here"},"agent":"codex","id":2,"op":"agents.addTools"}"#]);
    holds(UNIT, "addTools", "the answer is cut to its file", d::add_tools);
    holds(UNIT, "addTools", "an agent the catalog does not know", d::add_tools);
}

/// The names of an environment are written as a JS object writes them: whole-number names first and in numeric order.
/// A TS quirk the Rust keeps (see the spec's unclear list), pinned so a port that keeps insertion order fails here.
#[test]
fn server_values_order_whole_number_names_first() {
    for name in ["a name that is a whole number goes first in the object", "two whole-number names go first in numeric order"] {
        let c = recorded("serverValues", name);
        let keys: Vec<&String> = c.value.as_ref().unwrap()["env"].as_object().unwrap().keys().collect();
        assert_eq!(keys.last().map(|k| k.as_str()), Some("A"), "{name}");
        holds(UNIT, "serverValues", name, d::server_values);
    }
}

/// A thread and a computer together is refused before any frame, by every road that takes a target.
#[test]
fn a_thread_and_a_computer_together_send_no_frame_by_any_road() {
    for (func, name, driver) in [
        ("agentsTarget", "both are refused before any frame", d::agents_target as super::common::Driver),
        ("agentsReport", "both a thread and a computer are refused", d::agents_report),
        ("pluginChanged", "a thread and a computer are refused", d::plugin_changed),
        ("serverToolsOf", "a thread and a computer are refused", d::server_tools_of),
        ("serverChanged", "a thread and a computer are refused", d::server_changed),
    ] {
        let c = recorded(func, name);
        assert!(c.frames.is_empty(), "{func}");
        assert_eq!(c.error.as_ref().expect("a refusal").message.starts_with(a::AIMED_BOTH_LINE), true, "{func}");
        holds(UNIT, func, name, driver);
    }
}

/// `--project` is read against the target before anything is asked of the host: every refusal of `projectAsked` and
/// `toolsProject` is a usage refusal carrying the line's own usage as its fix.
#[test]
fn project_refusals_are_usage_and_carry_the_lines_usage() {
    for c in cases(UNIT).into_iter().filter(|c| c.func == "projectAsked" || c.func == "toolsProject") {
        if let Some(e) = &c.error {
            assert_eq!(e.kind.as_deref(), Some("usage"), "{}", c.case);
            assert_eq!(e.fix.as_deref(), c.args["usage"].as_str(), "{}", c.case);
        }
    }
}
