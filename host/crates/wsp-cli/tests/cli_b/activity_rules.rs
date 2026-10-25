// SPDX-License-Identifier: AGPL-3.0-only
// spec/activity.md's rules that the recorded lines do not reach: the table ported from
// packages/host/test/activity-table.test.ts, held also to the TS output byte for byte (pure.json), and the words of
// activity, activity end and conversations ported from activity.test.ts and conversations-verb.test.ts.

use serde_json::{json, Value};
use wsp_cli::verbs::activity::{activity_lines, ended_lines, processes_asked};
use wsp_cli::verbs::activity_table::{activity_table, TableOpts};
use wsp_cli::verbs::conversations::conversations_lines;
use wsp_wire::types::conversations::ConversationsAnswer;
use wsp_wire::views::activity::{ActivityEnded, ActivityView};

use crate::common::pure;

fn view(name: &str) -> ActivityView {
    serde_json::from_value(pure()["views"][name].clone()).unwrap_or_else(|e| panic!("view {name}: {e}"))
}

fn table(name: &str, width: Option<usize>, wide: bool) -> String {
    activity_table(&view(name), TableOpts { width, wide })
}

/// Cells as a terminal counts them, for the checks that do not lean on the TS bytes: CJK and emoji take two.
fn cells(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let u = c as u32;
            if (0x1100..=0x115F).contains(&u)
                || (0x2E80..=0xA4CF).contains(&u)
                || (0xAC00..=0xD7A3).contains(&u)
                || (0xF900..=0xFAFF).contains(&u)
                || (0xFE30..=0xFE4F).contains(&u)
                || (0xFF00..=0xFF60).contains(&u)
                || (0x1F300..=0x1FAFF).contains(&u)
                || u == 0x2705
            {
                2
            } else {
                1
            }
        })
        .sum()
}

const AT_80: &str = "lab-mac.local   wsp 1.5 GB   CPU 102%   11 processes   2 threads

                                                                   MEMORY    CPU
wsp  The app and what runs it                                      570 MB   1.4%
  Host :4400 :4420                                                 514 MB   0.3%
    Daemon :59312                                                   56 MB   1.1%
Build: Kept-run tests leave their bash and tail running after th…  452 MB  12.5%
  Codex                                                            210 MB   2.5%
    vite :5174            Dev server                               227 MB  10.0%
    popbar                Terminal                                  15 MB   0.0%
Fix the lab dashboard                                              395 MB   0.4%
  Claude Code                                                      198 MB   0.4%
    mcp-search-console    Tool server                               30 MB   0.0%
      mcp-search-console                                            64 MB   0.0%
    mcp-proxy-for-aws     Tool server                               23 MB   0.0%
      mcp-proxy-for-aws                                             79 MB   0.0%
No thread  Started with no thread wsp knows of                     140 MB  88.0%
  vitest                  Test run                                 140 MB  88.0%";

const LONG: &str = "Build: Kept-run tests leave their bash and tail running after they finish, and a second clause long enough to pass any terminal (#2137)";

#[test]
fn table_names_each_process_folds_the_run_script_and_pump_and_fits_80() {
    let text = table("VIEW", Some(80), false);
    assert_eq!(text, AT_80);
    for line in text.lines() {
        assert!(cells(line) <= 80, "{line}");
    }
    for gone in ["runs/", "tail -n", "claude -p", "uv tool", "69887"] {
        assert!(!text.contains(gone), "{gone} shows: the run script, the pump and command lines fold away");
    }
}

#[test]
fn table_at_160_cuts_only_the_long_title_and_keeps_every_figure_in_one_column() {
    let text = table("VIEW", Some(160), false);
    let lines: Vec<&str> = text.lines().collect();
    for line in &lines {
        assert!(cells(line) <= 160, "{line}");
    }
    let cut: String = LONG.chars().take(71).collect();
    assert_eq!(lines.iter().find(|l| l.starts_with("Build: ")).copied(), Some(format!("{cut}…  452 MB  12.5%").as_str()));
    let rows = &lines[2..];
    let widths: std::collections::BTreeSet<usize> = rows.iter().map(|l| l.chars().count()).collect();
    assert_eq!(widths.len(), 1, "every row the same width:\n{}", rows.join("\n"));
    let mb: std::collections::BTreeSet<usize> =
        rows[1..].iter().map(|l| l.find(" MB").map(|i| l[..i].chars().count()).unwrap_or(0)).collect();
    assert_eq!(mb.len(), 1, "MB in one column");
    let vite = lines.iter().find(|l| l.contains("vite")).unwrap();
    assert!(vite.starts_with("    vite :5174            Dev server  "), "{vite}");
}

#[test]
fn table_drops_the_kind_column_before_names_get_narrower_than_a_name() {
    let text = table("VIEW", Some(40), false);
    for line in text.lines() {
        assert!(cells(line) <= 40, "{line}");
    }
    assert!(!text.contains("Tool server"));
    assert!(text.contains("Claude Code"));
}

#[test]
fn table_wide_lists_every_process_with_pid_and_whole_command_folding_nothing() {
    let v = view("VIEW");
    let text = activity_table(&v, TableOpts { width: Some(80), wide: true });
    let lines: Vec<&str> = text.lines().collect();
    let all = v.threads.iter().flat_map(|t| t.processes.iter()).chain(v.wsp.iter()).chain(v.unowned.iter());
    for p in all {
        let pid = format!(" {} ", p.pid);
        let line = lines.iter().find(|l| l.contains(&pid)).unwrap_or_else(|| panic!("pid {} listed", p.pid));
        assert!(line.ends_with(&format!("  {}", p.command)), "{line}");
    }
    let tail = lines.iter().find(|l| l.contains("tail -n")).unwrap();
    assert!(tail.starts_with("  tail  ") && tail.contains(" 69890 "), "{tail}");
    assert!(lines[2].trim_start().starts_with("PID") && lines[2].ends_with("MEMORY    CPU  COMMAND"), "{}", lines[2]);
}

#[test]
fn table_names_the_run_script_the_agent_until_the_binary_shows_under_it() {
    let lines: Vec<String> = table("early", Some(80), false).lines().map(String::from).collect();
    assert_eq!(lines.len(), 5, "{lines:?}");
    assert!(lines[3].starts_with("Fix the lab dashboard  ") && lines[3].contains("395 MB"), "{}", lines[3]);
    assert!(lines[4].starts_with("  Agent  ") && lines[4].ends_with("2 MB  0.0%"), "{}", lines[4]);
}

#[test]
fn table_counts_wide_characters_as_two_cells_and_never_cuts_through_one() {
    let text = table("wideTitles", Some(80), false);
    assert!(!text.contains('\u{FFFD}'));
    let rows: Vec<&str> = text.lines().skip(2).collect();
    for line in text.lines() {
        assert!(cells(line) <= 80, "{line}");
    }
    let widths: std::collections::BTreeSet<usize> = rows.iter().map(|l| cells(l)).collect();
    assert_eq!(widths.len(), 1, "{}", rows.join("\n"));
    let cjk = rows.iter().find(|l| l.starts_with("修复")).unwrap();
    assert!(cjk.contains('…') && cjk.ends_with("395 MB   0.4%"), "{cjk}");
    let emoji = rows.iter().find(|l| l.starts_with('🚀')).unwrap();
    assert!(emoji.contains('…') && emoji.ends_with("452 MB  12.5%"), "{emoji}");
}

#[test]
fn table_heads_a_box_wsp_group_with_no_host_as_the_page_does_there() {
    let lines: Vec<String> = table("box", Some(80), false).lines().map(String::from).collect();
    assert!(lines[3].starts_with("wsp  What runs wsp here  ") && lines[3].ends_with("56 MB   1.1%"), "{}", lines[3]);
    assert!(lines[4].starts_with("  Daemon :59312 "), "{}", lines[4]);
    let mac: Vec<String> = table("VIEW", Some(80), false).lines().map(String::from).collect();
    assert!(mac[3].starts_with("wsp  The app and what runs it "), "{}", mac[3]);
}

#[test]
fn table_says_so_when_nothing_runs() {
    assert_eq!(table("empty", Some(80), false), "no thread runs a process on lab-mac.local");
    assert_eq!(table("empty", None, true), "no thread runs a process on lab-mac.local");
}

#[test]
fn table_names_an_outside_claude_code_by_its_agent_under_a_listening_process() {
    let text = table("outside", Some(100), false);
    assert!(!text.contains("2.1.296"), "the version is never a name");
    let line = text.lines().find(|l| l.contains("Claude Code")).unwrap();
    assert!(line.starts_with("    Claude Code  ") && line.ends_with("640 MB  3.0%"), "{line}");
}

#[test]
fn table_ties_sort_by_pid_then_thread_id() {
    let text = table("ties", Some(80), false);
    let order: Vec<&str> = text.lines().skip(3).map(|l| l.trim_start().split(' ').next().unwrap()).collect();
    assert_eq!(order, ["A", "z", "y", "B", "x"], "{text}");
}

#[test]
fn thread_head_uses_thread_figures() {
    let v = view("VIEW");
    let text = activity_table(&v, TableOpts { width: Some(80), wide: false });
    let head = text.lines().find(|l| l.starts_with("Fix the lab dashboard")).unwrap();
    assert!(head.ends_with("395 MB   0.4%"), "the thread's own memory, not its rows' sum: {head}");
}

#[test]
fn table_matches_ts_byte_for_byte() {
    let p = pure();
    for (key, want) in p["tables"].as_object().unwrap() {
        let mut it = key.split(' ');
        let (name, width, mode) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let width = if width == "Infinity" { None } else { Some(width.parse().unwrap()) };
        assert_eq!(Value::from(table(name, width, mode == "wide")), *want, "{key}");
    }
}

#[test]
fn activity_lines_match_ts() {
    let p = pure();
    for (name, want) in p["lines"].as_object().unwrap() {
        assert_eq!(Value::from(activity_lines(&view(name))), *want, "{name}");
    }
}

fn proc(pid: u32, name: &str) -> Value {
    json!({ "pid": pid, "ppid": 1, "depth": 0, "startedAt": 1, "kind": "dev", "name": name, "command": format!("{name} --x"), "memory": 1048576, "cpu": 0, "ports": [] })
}

fn ended(v: Value) -> ActivityEnded {
    serde_json::from_value(v).expect("ended parses")
}

#[test]
fn ended_lines_a_freed_kept_agent_wins_over_what_ended() {
    let p = pure();
    assert_eq!(
        ended_lines(&ended(json!({ "pid": 1, "ended": [], "forced": [], "left": [], "freed": "11111111-2222" }))),
        p["ended"]["freed"]
    );
    assert_eq!(
        ended_lines(&ended(json!({ "pid": 1, "ended": [proc(1, "x")], "forced": [1], "left": [], "freed": "t" }))),
        p["ended"]["freedWithEnded"]
    );
}

#[test]
fn ended_lines_nothing_ended_says_the_process_is_gone() {
    assert_eq!(
        ended_lines(&ended(json!({ "pid": 42, "ended": [], "forced": [], "left": [] }))),
        "nothing was signalled: the process listed as 42 is gone"
    );
}

#[test]
fn ended_lines_count_what_was_under_and_name_kill_and_what_stayed() {
    let p = pure();
    assert_eq!(ended_lines(&ended(json!({ "pid": 42, "ended": [proc(42, "node")], "forced": [], "left": [] }))), "ended 42 node");
    assert_eq!(
        ended_lines(&ended(
            json!({ "pid": 42, "ended": [proc(42, "node"), proc(43, "esbuild"), proc(44, "sh")], "forced": [43], "left": [] })
        )),
        p["ended"]["under"]
    );
    assert_eq!(
        ended_lines(&ended(json!({ "pid": 42, "ended": [proc(42, "node"), proc(43, "esbuild")], "forced": [42, 43], "left": [43] }))),
        p["ended"]["left"]
    );
}

#[test]
fn processes_asked_takes_stop_keep_or_nothing() {
    assert_eq!(processes_asked(None, None).unwrap(), None);
    assert_eq!(processes_asked(Some("stop"), None).unwrap().as_deref(), Some("stop"));
    assert_eq!(processes_asked(Some("keep"), None).unwrap().as_deref(), Some("keep"));
    let e = processes_asked(Some("maybe"), None).unwrap_err();
    assert_eq!(e.message, "--processes takes one of stop, keep, and got \"maybe\". Name one of those.");
    assert_eq!(e.exit_code(), 3);
}

fn answer(v: Value) -> ConversationsAnswer {
    serde_json::from_value(v).expect("answer parses")
}

fn held() -> Value {
    json!({ "agent": "claude", "said": "Claude Code's conversations could not be read: at most 64 entries, got 74", "fix": "Ask again in a moment.", "why": "at most 64 entries, got 74" })
}

#[test]
fn conversations_say_none_only_where_every_store_answered() {
    assert_eq!(conversations_lines(&answer(json!({ "rows": [], "held": [] }))), ["no Claude Code or Codex conversations in this folder"]);
}

#[test]
fn conversations_leave_the_held_line_to_speak_where_no_row_came_back() {
    assert_eq!(
        conversations_lines(&answer(json!({ "rows": [], "held": [held()] }))),
        ["Claude Code's conversations could not be read: at most 64 entries, got 74. Ask again in a moment."]
    );
}

#[test]
fn conversations_print_rows_above_the_held_line() {
    let lines = conversations_lines(&answer(
        json!({ "rows": [{ "agent": "codex", "id": "01a12813-cd12-7a12-9b13-e76892906ff0", "title": "lab terminal thread", "cwd": "/work/acme/lab", "lastAt": 0, "origin": "terminal", "live": false }], "held": [held()] }),
    ));
    assert_eq!(lines.last().unwrap(), "Claude Code's conversations could not be read: at most 64 entries, got 74. Ask again in a moment.");
    assert!(lines.iter().any(|l| l.contains("lab terminal thread")));
    assert!(!lines.iter().any(|l| l.starts_with("no ")));
    assert_eq!(Value::from(lines), pure()["conversations"]["rowsAndHeld"]);
}

#[test]
fn conversations_mark_a_thread_an_open_one_and_wsp_and_print_the_title_raw() {
    std::env::set_var("TZ", "UTC");
    let lines = conversations_lines(&answer(json!({ "rows": [
        { "agent": "claude", "id": "a", "title": "t\tab\nnew", "cwd": "/w", "lastAt": 1791673748000u64, "bytes": 0, "origin": "wsp", "live": false },
        { "agent": "claude", "id": "b", "title": "x", "cwd": "/w", "lastAt": 1791673748123u64, "bytes": 1536, "origin": "terminal", "live": true, "thread": "0123456789abcdef" },
        { "agent": "codex", "id": "c", "title": "", "cwd": "/w", "lastAt": -1, "branch": "main", "origin": "script", "live": false }
    ], "held": [] })));
    assert_eq!(Value::from(lines), pure()["conversations"]["marks"]);
}
