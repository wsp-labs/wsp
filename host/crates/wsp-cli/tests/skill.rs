// SPDX-License-Identifier: AGPL-3.0-only
//! Ports of `packages/host/test/skill.test.ts` and the cloud marks blocks of `packages/host/test/cloud.test.ts`, with
//! the TS oracle's recorded readings of `src/skill.ts` and `src/cloud-text.ts`.
#![allow(
    clippy::result_large_err,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::large_enum_variant,
    clippy::regex_creation_in_loops,
    non_snake_case,
    dead_code
)]
use regex::Regex;
use serde_json::Value;
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::git_hosts::THREAD_AGENTS;
use wsp_cli::cloud::CLOUD_BUILT;
use wsp_cli::cloud_text::{cloud_text, skill_for, BOAT_MARK, BOAT_SPAN_END, CLOUD_MARK, CLOUD_SPAN_END, NO_CLOUD_MARK};
use wsp_cli::skill::*;
use wsp_wire::types::place_state::{CLOUD_CAP_DEFAULT, DELETE_AFTER_MS};
use wsp_wire::types::run_block::RUN_BLOCK_WORDS;
use wsp_wire::types::wsp_tools::SLATE_BRIEF;
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::words::thread::{
    background_tasks_line, still_running_line, still_working_line, ANOTHER_AGENT_WORDS, BACKGROUND_WORK_WORDS, COORDINATOR_HANDOFF,
    NOTIFY_CALLER,
};
use wsp_words::words::turn::notify_line;

const ORACLE: &str = include_str!("fixtures/skill_oracle.json");
const PUBLIC_SKILL: &str = include_str!("fixtures/skill_public.md");
const CLOUD_SKILL: &str = include_str!("fixtures/skill_cloud.md");

const BOTH: [bool; 2] = [false, true];
const SCOPES: [(bool, bool); 4] = [(false, true), (true, true), (true, false), (false, false)];

fn oracle() -> Value {
    serde_json::from_str(ORACLE).expect("fixtures/skill_oracle.json is JSON")
}

fn own_cloud() -> bool {
    CLOUD_BUILT && std::env::var("WSP_CLOUD").as_deref() == Ok("1")
}

fn repo_skill_file() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../skills/wsp/SKILL.md")).expect("skills/wsp/SKILL.md reads")
}

fn jslen(text: &str) -> usize {
    text.encode_utf16().count()
}

fn said(text: &str, word: &str) -> bool {
    Regex::new(&format!("(?i)\\b{}\\b", regex::escape(word))).unwrap().is_match(text)
}

fn has(text: &str, needle: &str) {
    assert!(text.contains(needle), "missing: {needle}");
}

fn lacks(text: &str, needle: &str) {
    assert!(!text.contains(needle), "unexpected: {needle}");
}

/// `text.slice(text.indexOf(from), text.indexOf(to))` over the whole text.
fn between<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
    let a = text.find(from).unwrap_or_else(|| panic!("no {from:?}"));
    let b = text.find(to).unwrap_or_else(|| panic!("no {to:?}"));
    if b < a {
        ""
    } else {
        &text[a..b]
    }
}

/// `text.slice(text.indexOf(from), text.indexOf(to, text.indexOf(from)))`.
fn section<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
    let a = text.find(from).unwrap_or_else(|| panic!("no {from:?}"));
    let b = text[a..].find(to).map_or(text.len(), |at| a + at);
    &text[a..b]
}

fn slate_section(text: &str) -> &str {
    section(text, "### slate\n", "\n## ")
}

fn from_heading<'a>(text: &'a str, from: &str) -> &'a str {
    &text[text.find(from).unwrap_or_else(|| panic!("no {from:?}"))..]
}

fn instr(scoped: bool, slate: bool, cloud: bool) -> String {
    instructions_of_for(&THREAD_AGENTS, scoped, slate, cloud)
}

fn skill_in(cloud: bool) -> String {
    wsp_skill_for(cloud)
}

fn strings(value: &Value) -> Vec<&str> {
    value.as_array().expect("an array").iter().map(|v| v.as_str().expect("a string")).collect()
}

#[test]
fn instructions_match_the_ts_oracle_for_all_four_scopes_with_the_cloud_off() {
    // oracle: off.instructions, in the order (false,true), (true,true), (true,false), (false,false)
    let want = oracle()["off"]["instructions"].clone();
    let want = strings(&want);
    assert_eq!(want.len(), 4);
    for ((scoped, slate), want) in SCOPES.into_iter().zip(want) {
        assert_eq!(instr(scoped, slate, false), want, "scoped {scoped}, slate {slate}");
    }
}

#[test]
fn instructions_match_the_ts_oracle_for_all_four_scopes_with_the_cloud_on() {
    // oracle: on.instructions, in the order (false,true), (true,true), (true,false), (false,false)
    let want = oracle()["on"]["instructions"].clone();
    let want = strings(&want);
    assert_eq!(want.len(), 4);
    for ((scoped, slate), want) in SCOPES.into_iter().zip(want) {
        assert_eq!(instr(scoped, slate, true), want, "scoped {scoped}, slate {slate}");
    }
}

#[test]
fn agents_line_for_claude_and_codex_matches_the_ts_oracle_with_the_cloud_off() {
    // oracle: off.agentsLine; skill.ts:38-40 says "run takes" with no cloud
    assert_eq!(agents_line_for(&["claude", "codex"], false), oracle()["off"]["agentsLine"].as_str().unwrap());
    assert_eq!(
        agents_line_for(&["claude", "codex"], false),
        "The agents this host runs threads on, the only values run takes as agent: claude, codex."
    );
}

#[test]
fn agents_line_for_claude_and_codex_matches_the_ts_oracle_with_the_cloud_on() {
    // oracle: on.agentsLine; skill.ts:38-40 says "run and fork take" with the cloud
    assert_eq!(agents_line_for(&["claude", "codex"], true), oracle()["on"]["agentsLine"].as_str().unwrap());
    assert_eq!(
        agents_line_for(&["claude", "codex"], true),
        "The agents this host runs threads on, the only values run and fork take as agent: claude, codex."
    );
}

#[test]
fn cloud_text_matches_the_ts_oracle_for_every_recorded_input_with_the_cloud_on() {
    // oracle: cloudText cases [input, on, off], cloud-text.ts:22-27
    let cases = oracle()["cloudText"].clone();
    let cases = cases.as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let [input, on, _off] = [0usize, 1, 2].map(|i| case[i].as_str().unwrap());
        assert_eq!(cloud_text(input, true), on, "input {input:?}");
    }
}

#[test]
fn cloud_text_matches_the_ts_oracle_for_every_recorded_input_with_the_cloud_off() {
    // oracle: cloudText cases [input, on, off], cloud-text.ts:22-27
    let cases = oracle()["cloudText"].clone();
    let cases = cases.as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let [input, _on, off] = [0usize, 1, 2].map(|i| case[i].as_str().unwrap());
        assert_eq!(cloud_text(input, false), off, "input {input:?}");
    }
}

#[test]
fn skill_for_matches_the_ts_oracle_for_every_recorded_input_with_the_cloud_on() {
    // oracle: skillFor, cloud-text.ts:31-51
    // the oracle holds one case: [input, on, off]
    let case = oracle()["skillFor"].clone();
    let [input, on, _off] = [0usize, 1, 2].map(|i| case[i].as_str().unwrap().to_string());
    assert_eq!(skill_for(&input, true), on, "input {input:?}");
}

#[test]
fn skill_for_matches_the_ts_oracle_for_every_recorded_input_with_the_cloud_off() {
    // oracle: skillFor, cloud-text.ts:31-51
    // the oracle holds one case: [input, on, off]
    let case = oracle()["skillFor"].clone();
    let [input, _on, off] = [0usize, 1, 2].map(|i| case[i].as_str().unwrap().to_string());
    assert_eq!(skill_for(&input, false), off, "input {input:?}");
}

#[test]
fn wsp_skill_for_false_is_the_public_skill_file_byte_for_byte() {
    // oracle: off.skill, recorded as fixtures/skill_public.md; skill.ts:18 with the cloud off
    let got = wsp_skill_for(false);
    assert_eq!(got.len(), PUBLIC_SKILL.len());
    assert!(got == PUBLIC_SKILL, "the public skill differs from the TS reading");
    assert!(got == oracle()["off"]["skill"].as_str().unwrap());
}

#[test]
fn wsp_skill_for_true_is_the_cloud_skill_file_byte_for_byte() {
    // oracle: on.skill, recorded as fixtures/skill_cloud.md; skill.ts:18 with the cloud on
    let got = wsp_skill_for(true);
    assert_eq!(got.len(), CLOUD_SKILL.len());
    assert!(got == CLOUD_SKILL, "the cloud skill differs from the TS reading");
    assert!(got == oracle()["on"]["skill"].as_str().unwrap());
}

#[test]
fn wsp_skill_for_is_skill_for_of_the_inlined_text_in_both_readings() {
    // skill.ts:18: `skillFor(text, cloud)`, text being skills/wsp/SKILL.md as the build inlines it
    let file = repo_skill_file();
    assert!(SKILL_TEXT == file, "SKILL_TEXT is not skills/wsp/SKILL.md");
    for cloud in BOTH {
        assert!(wsp_skill_for(cloud) == skill_for(&file, cloud), "cloud {cloud}");
    }
}

#[test]
fn cloud_marks_keep_the_text_and_drop_the_marks_with_the_cloud_on() {
    // cloud.test.ts:171
    let skill = marked_skill();
    let want = [
        "# t",
        "a line",
        "only on a cloud",
        "both cloud ways",
        "## cloud part",
        "under it",
        "```",
        "# not a heading",
        "```",
        "### deeper",
        "## next",
        "kept",
    ]
    .join("\n");
    assert_eq!(skill_for(&skill, true), want);
}

#[test]
fn cloud_marks_drop_a_marked_line_span_and_headings_whole_section_with_the_cloud_off() {
    // cloud.test.ts:175
    let skill = marked_skill();
    let want = ["# t", "a line", "both local ways", "## next", "kept"].join("\n");
    assert_eq!(skill_for(&skill, false), want);
}

fn marked_skill() -> String {
    [
        "# t".to_string(),
        "a line".to_string(),
        "only on a cloud <!-- cloud -->".to_string(),
        "both <!-- cloud -->cloud <!-- /cloud --><!-- no cloud -->local <!-- /no cloud -->ways".to_string(),
        "## cloud part <!-- cloud -->".to_string(),
        "under it".to_string(),
        "```".to_string(),
        "# not a heading".to_string(),
        "```".to_string(),
        "### deeper".to_string(),
        "## next".to_string(),
        "kept".to_string(),
    ]
    .join("\n")
}

#[test]
fn cloud_marks_keep_boats_line_span_and_headings_section_with_the_cloud_on_and_off() {
    // cloud.test.ts:179
    let boat = [
        "# t",
        "sub-threads <!-- boat -->",
        "run <!-- boat -->--on boat <!-- /boat -->here",
        "## Sub-threads on Boat <!-- boat -->",
        "under it",
    ]
    .join("\n");
    let read = ["# t", "sub-threads", "run --on boat here", "## Sub-threads on Boat", "under it"].join("\n");
    assert_eq!(skill_for(&boat, false), read);
    assert_eq!(skill_for(&boat, true), read);
}

#[test]
fn the_public_skill_says_what_wsp_remove_boat_does_with_the_cloud_off() {
    // cloud.test.ts:186, which reads `skillFor(wspSkill(), false)`: the marks gone, read again with the cloud off
    let marked = skill_for(SKILL_TEXT, false);
    let again = skill_for(&skill_in(true), false);
    for text in [&marked, &again] {
        has(text, "`wsp remove boat` deletes every machine this host made on Boat");
        has(text, "`wsp remove boat --forget` takes the key");
    }
}

#[test]
fn the_cloud_skill_keeps_its_text_and_has_no_mark_left() {
    // cloud.test.ts:159 "reads the skill as it always was, the marks gone"
    let skill = wsp_skill_for(true);
    has(&skill, "SOLARI_API_KEY");
    for mark in [CLOUD_MARK, CLOUD_SPAN_END, NO_CLOUD_MARK] {
        lacks(&skill, mark);
    }
    lacks(&skill, "with none joined this run builds nothing");
}

#[test]
fn no_skill_line_or_instruction_names_solari_with_the_cloud_off() {
    // cloud.test.ts:106, for the two readings this file holds: the skill and the instructions
    let skill = wsp_skill_for(false);
    lacks(&skill, "Solari");
    for (scoped, slate) in SCOPES {
        lacks(&instr(scoped, slate, false), "Solari");
    }
}

#[test]
fn the_public_skill_carries_boats_spans_and_no_word_of_solari_and_both_are_kept_with_the_cloud_on() {
    // skill.test.ts:32
    let skill = repo_skill_file();
    let public = skill_for(&skill, false);
    assert!(!Regex::new("(?i)solari|slr_live_").unwrap().is_match(&public));
    has(&skill_for(&skill, true), "SOLARI_API_KEY");
    let span = Regex::new(r"<!-- boat -->(.*?)<!-- /boat -->").unwrap();
    let spans: Vec<&str> = span.captures_iter(&skill).map(|c| c.get(1).unwrap().as_str()).collect();
    let end = format!(" {BOAT_MARK}");
    let lines: Vec<&str> = skill.split('\n').filter(|l| l.ends_with(&end)).map(|l| &l[..l.len() - end.len()]).collect();
    assert!(!spans.is_empty() && !lines.is_empty());
    for text in spans.iter().chain(&lines) {
        assert!(public.contains(text), "the public skill lost: {text}");
    }
    let input = format!("# t\nsub-threads {BOAT_MARK}\nrun {BOAT_MARK}--on boat {BOAT_SPAN_END}here\nSolari {CLOUD_MARK}\n");
    assert_eq!(skill_for(&input, false), "# t\nsub-threads\nrun --on boat here\n");
}

#[test]
fn the_public_skill_teaches_sub_threads_on_boat_right_after_add_and_prices_boat_in_the_costs() {
    // skill.test.ts:44, less the per-class rate rows, which read BOX_CLASSES
    let public = skill_for(&repo_skill_file(), false);
    let heading = Regex::new(r"(?m)^### (.+)$").unwrap();
    let headings: Vec<&str> = heading.captures_iter(&public).map(|c| c.get(1).unwrap().as_str()).collect();
    let add = headings.iter().position(|h| *h == "add").expect("an add heading");
    assert_eq!(headings[add + 1], "Sub-threads on Boat");
    let part = |heading: &str| -> &str {
        let from = public.find(&format!("{heading}\n")).unwrap();
        let to = public[from + heading.len()..].find("\n#").map_or(public.len(), |at| from + heading.len() + at);
        &public[from..to]
    };
    let boat = part("### Sub-threads on Boat");
    has(boat, "`wsp run --on boat --notify me --detach --title \"Fix the upload test\" \"<the brief>\"`");
    has(boat, "`wsp merge in <you> <sub-thread>`");
    has(boat, &format!("deleted {} minutes after it is done", DELETE_AFTER_MS / 60_000));
    has(boat, "12 a minute and 200 a day on the $20 plan");
    let costs = part("## Costs and limits");
    has(costs, "The trial, until the first payment, runs 2 machines at once, sells `2x4` and `4x8` only, and takes 5 starts a minute, 25 an hour and 75 a day.");
    let machines = CLOUD_CAP_DEFAULT.iter().find(|(k, _)| *k == "machines").unwrap().1;
    has(costs, &format!("wsp holds Boat to {machines} machines at once"));
}

#[test]
fn wsp_skill_is_the_repos_skill_file_as_this_process_reads_it_with_a_one_line_description() {
    // skill.test.ts:63: the process's own reading, cloud off unless WSP_CLOUD is "1"
    let want = skill_for(&repo_skill_file(), own_cloud());
    let got = wsp_skill();
    assert!(got == want, "wsp_skill() is not the file read through skill_for");
    let lines: Vec<&str> = got.split('\n').collect();
    assert_eq!(lines[0], "---");
    assert_eq!(lines[1], format!("name: {SKILL_NAME}"));
    assert_eq!(lines[3], "---");
    let description = lines[2];
    assert!(Regex::new(r"^description: \S").unwrap().is_match(description));
    assert!(!Regex::new(r#"[:"']"#).unwrap().is_match(&description["description: ".len()..]));
    assert!(jslen(description) < 1024);
}

#[test]
fn the_skill_states_the_install_flags_and_the_agents_the_server_installs_for() {
    // skill.test.ts:74 less the loops over CLI_VERBS and VERBS; MCP_AGENT_IDS is "claude, codex, gemini, opencode"
    for cloud in BOTH {
        let skill = skill_in(cloud);
        has(&skill, "get the server: claude, codex, gemini, opencode.");
        has(&skill, "`--agent` repeats to do several in one call");
        has(&skill, "`--json` answers with one line holding the `server` command every config now runs, what each agent took, its `docs` naming the files the section went into, and a `failures` array");
        has(&skill, "An entry under `installed` with no `path` took the skill and not the server");
        has(&skill, "wsp's own marked section into the instructions the folder it runs in keeps");
        has(&skill, "A second run replaces that section where it stands");
        has(&skill, "`wsp mcp install --agent <id> --remove` takes it back out");
        has(&skill, "off a terminal a run that names none takes every agent whose own command is on the PATH");
    }
}

#[test]
fn the_skill_quotes_the_notify_line_as_the_protocol_prints_it_and_names_every_send_outcome() {
    // skill.test.ts:90
    let result: TurnResult = serde_json::from_value(serde_json::json!({
        "status": "completed", "durationMs": 724000, "costUsd": 0.41, "text": "first line\n<last line of the reply>"
    }))
    .unwrap();
    let line = notify_line("1a2b3c4d-0000", &result, None);
    for cloud in BOTH {
        let skill = skill_in(cloud);
        has(&skill, &format!("`{line}`"));
        for outcome in [SessionStartOutcome::Started, SessionStartOutcome::Steered, SessionStartOutcome::Queued, SessionStartOutcome::Held]
        {
            let name = serde_json::to_value(outcome).unwrap();
            has(&skill, &format!("(outcome `{}`)", name.as_str().unwrap()));
        }
        has(&skill, "`--notify <thread>` names another thread outright, one of your own tree: the lead that started you, a thread beside you under it, or one you started");
    }
}

#[test]
fn the_skill_hands_the_blocking_wait_to_a_shell_script_and_the_instructions_carry_both_roads_to_a_childs_end() {
    // skill.test.ts:97
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let agent_rows = between(&skill, VERBS_HEADING, SHELL_HEADING);
        lacks(agent_rows, "`wsp threads wait");
        let shell = between(&skill, SHELL_HEADING, RULES_HEADING);
        has(shell, "| `wsp threads wait <thread>... [--timeout <s>] [--tail]` | `threads_wait` (threads, timeout) |");
        let wait = between(&skill, "### threads wait", "### stop");
        for words in [
            "wsp run dev --detach --notify me",
            "wsp threads wait 1a2b3c4d 5e6f7a8b --timeout 600",
            "One thread per call",
            "`thread 1a2b3c4d still running after 10m`",
            "This is a shell script's verb",
            "with the agent's reply whole under it",
            "`--tail` prints the last line alone",
            "Do not call it in your own conversation",
            "Never poll `threads`",
        ] {
            has(wait, words);
        }
        let loop_ = between(&skill, "## The loop for building with wsp", "## Where the person steps in");
        has(loop_, "--detach");
        has(loop_, "--notify me");
        has(loop_, "end your turn");
        lacks(loop_, "threads wait");
        lacks(loop_, "nohup wsp run");
        let instructions = instr(false, true, cloud);
        for words in [NOTIFY_CALLER, COORDINATOR_HANDOFF, BACKGROUND_WORK_WORDS] {
            has(&instructions, words);
        }
    }
}

#[test]
fn the_instructions_open_with_what_another_agent_is_and_stay_whole_inside_what_claude_code_keeps() {
    // skill.test.ts:125, for the process's own reading and for both of the cloud's
    assert!(jslen(&instructions(false, true)) <= INSTRUCTIONS_KEPT);
    assert!(jslen(&instructions(true, true)) <= INSTRUCTIONS_KEPT);
    assert!(instructions(false, true).starts_with(&format!("{ANOTHER_AGENT_WORDS}.")));
    has(&instructions(false, true), "in the wsp skill");
    for cloud in BOTH {
        for (scoped, slate) in SCOPES {
            let text = instr(scoped, slate, cloud);
            assert!(jslen(&text) <= INSTRUCTIONS_KEPT, "cloud {cloud}, scoped {scoped}, slate {slate}");
            has(&text, "in the wsp skill");
        }
        assert!(instr(false, true, cloud).starts_with(&format!("{ANOTHER_AGENT_WORDS}.")));
    }
}

#[test]
fn the_instructions_leave_room_under_the_cut_for_a_thread_agent_or_two_more_with_the_cloud_on_or_off() {
    // skill.test.ts:132
    let room = 60;
    for cloud in BOTH {
        for scoped in BOTH {
            let len = jslen(&instr(scoped, true, cloud));
            assert!(len <= INSTRUCTIONS_KEPT - room, "cloud {cloud}, scoped {scoped}: {len}");
        }
    }
}

#[test]
fn a_threads_own_instructions_open_with_its_slate_and_the_others_are_left_as_they_are() {
    // skill.test.ts:147
    for cloud in BOTH {
        let plain = instr(false, true, cloud);
        let without = plain.replacen(&format!("{SLATE_WORDS}. "), "", 1);
        assert_eq!(instr(true, true, cloud), format!("{THREAD_SLATE_WORDS}\n\n{without}"));
        assert_eq!(instr(true, false, cloud), without);
        assert!(!instr(true, false, cloud).to_lowercase().contains("slate"));
        assert!(jslen(THREAD_SLATE_WORDS) < INSTRUCTIONS_KEPT / 2);
        lacks(&plain, THREAD_SLATE_WORDS);
        let kept: String = instr(true, true, cloud).chars().take(INSTRUCTIONS_KEPT).collect();
        has(&kept, &format!("{ANOTHER_AGENT_WORDS}."));
    }
}

#[test]
fn the_slate_words_and_the_skills_slate_section_say_a_request_to_see_lands_on_the_slate() {
    // skill.test.ts:157
    let scoped = instr(true, true, own_cloud());
    has(&scoped, "Once the slate is written, a request to see something lands there.");
    for words in [
        "Once the slate is written, a request to see something lands there.",
        "Build and read it with the slate tools, never wsp from a shell.",
        "A one-off answer or an explanation stays in chat unless they ask to see it.",
        "your first tool call is slate_catalog, then slate_write; a list in chat is not a slate.",
        "Show data fetched with another tool on the slate, not in chat.",
        "Read the sketch a write answers before saying it works, then reply briefly: what you built and what waits on the person.",
    ] {
        has(THREAD_SLATE_WORDS, words);
    }
    let head = &THREAD_SLATE_WORDS[..THREAD_SLATE_WORDS.find("your first tool call").unwrap()];
    for word in ["tick off", "as it goes", "keep an eye on", "what's unread", "live"] {
        has(head, word);
    }
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let section = slate_section(&skill);
        for words in [
            "a request to see something lands there",
            "gets its result there unless they ask in the chat",
            "Code only the slate uses goes in a `<file name=\"x.py\">`, never in the project, and runs as `\"$SLATE_DIR/x.py\"`; project code runs where it is.",
            "give a `<secret name=\"token\" />` input; never ask them to paste it into a file or the chat",
            "Build and read it with the slate tools (`slate_catalog`, `slate_write`, `slate_state`, `slate_read`), never `wsp` from a shell",
            "one heading per section",
            "status and last-checked lines small and muted",
            "actions at the end of their row with one primary per section",
            "mono only for figures, ids, times and paths",
            "every live number with its window and unit",
            "nothing centered but a lone figure or card",
        ] {
            has(section, words);
        }
    }
}

#[test]
fn a_runs_script_is_a_file_in_the_slate_folder_never_in_the_project_in_the_brief_and_the_skill() {
    // skill.test.ts:197, for the two texts that need no verb table or slate catalog: the brief and the skill
    let _ = skill_in(false);
    for cloud in BOTH {
        let skill = skill_in(cloud);
        for (where_, text) in [("Claude slate brief", SLATE_BRIEF), ("skill", slate_section(&skill))] {
            for words in ["<file name=\"x.py\">", "$SLATE_DIR/x.py", "never in the project"] {
                assert!(text.contains(words), "{words} in the {where_}");
            }
        }
    }
}

#[test]
fn the_slates_chart_kinds_and_image_are_said_in_the_instructions_and_the_brief_and_no_dropped_look() {
    // skill.test.ts:212, for the thread instructions, the brief and the skill; the catalog and the served record are skipped
    for cloud in BOTH {
        let scoped = instr(true, true, cloud);
        for (where_, text) in [("Claude slate brief", SLATE_BRIEF), ("thread instructions", scoped.as_str())] {
            for kind in ["chart", "donut", "timeline", "treemap", "diagram", "image"] {
                assert!(said(text, kind), "{kind} in the {where_}");
            }
            for dropped in ["ramp", "outlined"] {
                assert!(!said(text, dropped), "{dropped} in the {where_}");
            }
        }
        assert!(jslen(&scoped) <= INSTRUCTIONS_KEPT);
        let skill = skill_in(cloud);
        let section = slate_section(&skill);
        for kind in ["sparkline", "chart", "donut", "timeline", "treemap"] {
            assert!(!said(section, kind), "{kind} in the skill");
        }
    }
}

#[test]
fn a_send_is_never_refused_for_meeting_a_turn_and_the_steer_the_queue_and_the_reply_tail_are_named() {
    // skill.test.ts:232
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let send = between(&skill, "### send", "### threads wait");
        has(send, "A send is never refused for meeting a turn");
        has(send, "(outcome `steered`)");
        has(send, "(outcome `queued`)");
        has(send, &format!("`{}`", still_working_line(None)));
        has(send, "Two sends keep the order they arrived in");
    }
}

#[test]
fn no_section_of_the_skill_still_teaches_the_wait_or_the_refusal() {
    // skill.test.ts:241
    for cloud in BOTH {
        let skill = skill_in(cloud);
        for words in ["the send is refused", "a `send` before then is refused"] {
            lacks(&skill, words);
        }
        let owns = [
            between(&skill, SHELL_HEADING, RULES_HEADING),
            between(&skill, "### threads wait", "### stop"),
            from_heading(&skill, "## Rules learned the hard way"),
        ]
        .concat();
        let wait = Regex::new("threads.wait").unwrap();
        let elsewhere: Vec<&str> = skill.split('\n').filter(|line| wait.is_match(line) && !owns.contains(line)).collect();
        assert_eq!(elsewhere, Vec::<&str>::new());
    }
}

#[test]
fn the_skill_tells_an_agent_it_can_talk_to_another_thread() {
    // skill.test.ts:255
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let send = between(&skill, "### send", "### threads wait");
        has(send, "Threads talk to each other");
        has(send, "`wsp send <thread> \"<message>\"` (the `send` tool)");
        has(send, "`wsp threads` (the `threads` tool) is where you find that id");
        has(send, "comes back as its own next message");
        has(send, "`--notify me`");
        has(send, "one of your own tree, the lead that started you, a thread beside you under it or one you started, wherever it runs");
        lacks(send, "whoever opened it");
        lacks(send, "one you started or one under it");
        has(send, "goes to the person");
    }
}

#[test]
fn the_skill_says_a_thread_reaches_its_whole_tree_and_stopping_the_lead_stops_the_tree() {
    // skill.test.ts:271
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let opening = &skill[..skill.find(SETUP_HEADING).unwrap()];
        has(opening, "the lead that started it, the threads beside it under that lead and the threads it started, wherever each runs");
        has(opening, "the person's own thread and another lead's tree in the same folder are left out of `threads`");
        has(
            opening,
            "Stopping a thread stops every thread under it, so a child that stops its lead stops its siblings and itself with it.",
        );
        lacks(&skill, "the threads it started and the threads under those, and nothing else");
        let send = between(&skill, "### send", "### threads wait");
        has(send, "A stop cascades: stopping your lead stops every thread under it, your siblings and you among them.");
    }
}

#[test]
fn the_rules_quote_the_line_a_reply_ends_on_when_a_background_command_is_still_running() {
    // skill.test.ts:282
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let rules = from_heading(&skill, "## Rules learned the hard way");
        has(rules, &format!("`{}`", background_tasks_line(1.0)));
        has(rules, &format!("`{}`", still_running_line(1.0)));
    }
}

#[test]
fn the_skill_says_where_a_thread_on_this_computer_starts_and_its_example_asks_nothing_of_a_folder() {
    // skill.test.ts:288
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let run = between(&skill, "### run", "### send");
        has(run, "wsp run --agent codex \"Say in one line which folder you are in");
        has(run, "`wsp run <project>` opens a thread in the project's folder on this computer");
        has(run, "`--branch <branch>` runs it in a worktree of the project's repo on that branch instead");
        has(run, "from a thread it runs beside you in your own folder");
        lacks(run, "~/wsp-work");
    }
}

#[test]
fn the_skill_says_a_mac_refuses_to_join_and_names_no_user_unit_for_it() {
    // skill.test.ts:300, less the unit words SERVICE_MANAGERS gives
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let start = skill.find("`wsp join <address>...").unwrap();
        let sentence = &skill[start..start + skill[start..].find("`--code-file").unwrap()];
        lacks(sentence, "systemd user unit");
        has(sentence, "a Mac refuses to join");
    }
}

#[test]
fn the_skill_carries_no_em_dash() {
    // skill.test.ts:310
    for cloud in BOTH {
        lacks(&skill_in(cloud), "\u{2014}");
    }
    lacks(&wsp_skill(), "\u{2014}");
}

#[test]
fn the_skill_keeps_the_host_the_agents_own_job_and_the_keys_the_persons() {
    // skill.test.ts:314
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let setup = between(&skill, SETUP_HEADING, "## Verbs and tools");
        let step_two = &setup[setup.find("\n2. ").unwrap()..setup.find("\n3. ").unwrap()];
        has(step_two, "wsp down");
        has(setup, "Starting the host is neither: any command that needs one starts it");
        has(setup, "it reads those keys off the file itself, so nothing about them passes through you");
        has(setup, "The tools show up only after that agent restarts");
        has(setup, "prefer that over `npx @wsp-labs/wsp`");
        let stale = Regex::new(r"\b(?:npm i -g|npx) \S+").unwrap();
        let bad: Vec<&str> = stale
            .find_iter(&skill)
            .map(|m| m.as_str())
            .filter(|m| !Regex::new(r"^(?:npm i -g|npx) @wsp-labs/wsp\b").unwrap().is_match(m))
            .collect();
        assert_eq!(bad, Vec::<&str>::new());
    }
}

#[test]
fn the_setup_section_is_a_runbook_where_every_step_ends_in_its_own_expect_line() {
    // skill.test.ts:326
    let step = Regex::new(r"^\d+\. ").unwrap();
    let expect = Regex::new(r"^ *Expect: ").unwrap();
    let expect_word = Regex::new(r"^ *Expect: \S").unwrap();
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let setup = between(&skill, SETUP_HEADING, "## Verbs and tools");
        let closing = setup.find("\nLast, ").unwrap();
        let lines: Vec<&str> = setup[..closing].split('\n').collect();
        let steps = lines.iter().filter(|l| step.is_match(l)).count();
        assert_eq!(setup.split('\n').filter(|l| expect_word.is_match(l)).count(), steps + 1);
        for (at, line) in lines.iter().enumerate() {
            if !step.is_match(line) {
                continue;
            }
            let next = lines[at + 1..].iter().position(|l| step.is_match(l));
            let body = &lines[at..next.map_or(lines.len(), |n| at + 1 + n)];
            assert_eq!(body.iter().filter(|l| expect.is_match(l)).count(), 1, "{}", &line[..line.len().min(40)]);
        }
        has(setup, "Every step below ends in an `Expect:` line");
        has(setup, "say what to run next inside their own agent");
        has(setup, "In Claude Code the skill is then `/wsp`");
        assert!(setup.trim_end().ends_with("that thread shows in the person's sidebar."));
    }
}

#[test]
fn the_instructions_are_what_another_agent_is_the_slate_what_wsp_is_the_agents_and_the_roads_to_a_childs_end() {
    // skill.test.ts:344, with instructions() and instructions_of() at the process's own reading
    let own = own_cloud();
    assert_eq!(instructions(false, true), instructions_of(&THREAD_AGENTS, false, true));
    assert_eq!(instructions(true, true), instructions_of(&THREAD_AGENTS, true, true));
    assert_eq!(instructions(true, false), instructions_of(&THREAD_AGENTS, true, false));
    assert_eq!(instructions(false, true), instr(false, true, own));
    assert_eq!(agents_line(&["claude", "codex"]), agents_line_for(&["claude", "codex"], own));
    for cloud in BOTH {
        let text = instr(false, true, cloud);
        assert!(text.starts_with(&format!("{ANOTHER_AGENT_WORDS}. {SLATE_WORDS}. Agents work on a project, a folder on one computer")));
        assert!(jslen(&format!("{SLATE_WORDS}.")) < 220);
        lacks(SLATE_WORDS, "This thread");
        has(SLATE_WORDS, "naming the thread");
        assert_eq!(text.find(&format!("{SLATE_WORDS}.")), Some(ANOTHER_AGENT_WORDS.len() + 2));
        let skill = skill_in(cloud);
        assert!(jslen(slate_section(&skill)) as f64 / 4.0 < 600.0);
        has(&text, "Setup, every verb and the rules are in the wsp skill");
        lacks(&text, "The road is a health check");
        has(&skill, "The road is a health check");
        assert_eq!(text.split('\n').filter(|l| !l.starts_with("- ")).count(), 1);
        lacks(&text, "## ");
        let want_head =
            [format!("{ANOTHER_AGENT_WORDS}."), format!("{SLATE_WORDS}."), ESSENTIALS.to_string(), agents_line_for(&THREAD_AGENTS, cloud)]
                .join(" ");
        assert_eq!(text.split('\n').next().unwrap(), want_head);
        let tail: Vec<&str> = text.split('\n').skip(1).collect();
        assert_eq!(
            tail,
            vec![
                format!("- {NOTIFY_CALLER}, so nothing is polled."),
                format!("- {COORDINATOR_HANDOFF}."),
                format!("- {BACKGROUND_WORK_WORDS}."),
            ]
        );
    }
}

#[test]
fn the_rules_for_running_work_are_thirteen_lines_twelve_with_no_cloud_and_the_instructions_carry_three_of_them() {
    // skill.test.ts:367, once per cloud reading, where the TS reads CLOUD_ON
    for cloud in BOTH {
        let skill = skill_in(cloud);
        let from = skill.find(&format!("\n{RULES_HEADING}\n")).expect("the rules heading");
        let to = skill[from + 1..].find("\n## ").map_or(skill.len(), |at| from + 1 + at);
        let section = &skill[from..to];
        let rules: Vec<&str> = section.split('\n').filter(|l| l.starts_with("- ")).collect();
        assert_eq!(rules.len(), if cloud { 13 } else { 12 }, "cloud {cloud}");
        assert_eq!(*rules.last().unwrap(), format!("- {RUN_BLOCK_WORDS}."));
        has(rules[0], NOTIFY_CALLER);
        has(rules[1], COORDINATOR_HANDOFF);
        assert_eq!(rules[2], format!("- {ANOTHER_AGENT_WORDS}."));
        let text = instr(false, true, cloud);
        let carried: Vec<&str> = text.split('\n').skip(1).collect();
        assert_eq!(carried.len(), 3);
        for (line, words) in carried.iter().zip([NOTIFY_CALLER, COORDINATOR_HANDOFF, BACKGROUND_WORK_WORDS]) {
            has(line, words);
        }
        let ends = Regex::new(r"\.$").unwrap();
        let history = Regex::new(r"#\d|\bticket\b|\b20\d\d\b").unwrap();
        for rule in &rules {
            assert!(ends.is_match(rule), "{rule}");
            assert!(!history.is_match(rule), "{rule}");
        }
        has(section, "A thread on this computer runs in the project's folder, beside any other thread there");
        has(section, "`wsp run <project> --branch <branch>`");
        has(section, "a quick subtask or a second harness");
        if cloud {
            has(section, "Fork a cloud machine for builds that run beside each other");
            has(section, "anything that should not touch this computer");
            has(section, "`wsp pause <workspace>`");
        } else {
            lacks(section, "--from");
        }
        has(section, "on 2 vCPU and 4 GB one thread runs tests or a build at a time");
        has(section, "its own git worktree");
        has(section, "`pnpm install --offline`");
        has(section, BACKGROUND_WORK_WORDS);
        has(section, "a send into a thread whose turn is still running opens no second turn");
        has(section, "Restarting the host cuts every turn running anywhere");
        has(section, "shows in their sidebar");
        has(section, "`--title`");
    }
}

#[test]
fn the_instructions_name_every_agent_the_host_has_an_adapter_for_and_no_other_catalog_agent() {
    // skill.test.ts:412
    for cloud in BOTH {
        let text = instr(false, true, cloud);
        let named = |id: &str| Regex::new(&format!(r"\b{}\b", regex::escape(id))).unwrap().is_match(&text);
        for id in THREAD_AGENTS {
            assert!(named(id), "{id}");
        }
        for agent in catalog_agents() {
            if !THREAD_AGENTS.contains(&agent.id.as_str()) {
                assert!(!named(&agent.id), "{}", agent.id);
            }
        }
        has(&text, &format!(" as agent: {}.", THREAD_AGENTS.join(", ")));
    }
    assert_eq!(
        agents_line_for(&["claude", "codex"], false),
        "The agents this host runs threads on, the only values run takes as agent: claude, codex."
    );
    assert_eq!(
        agents_line_for(&["claude", "codex"], true),
        "The agents this host runs threads on, the only values run and fork take as agent: claude, codex."
    );
}
