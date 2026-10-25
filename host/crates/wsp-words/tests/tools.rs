// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/format.test.ts:549-598 (a turn's activity in one line each), :655-670 (what a tool
//! call answered), :673-708 (the tool call registry), :1725-1965 (the words a relayed permission prompt shows) and
//! init-job.test.ts:270 (threadWorkingLine).

use serde_json::{json, Value};
use std::collections::HashSet;
use wsp_wire::views::session::PermissionOutcome;
use wsp_wire::views::session_events::SessionPermissionEvent;
use wsp_wire::wire::helpers::{PermissionEffect, PermissionOption};
use wsp_words::words::tools::*;

fn js(v: Value) -> String {
    v.to_string()
}

fn wire<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn facts_json(f: &ToolCallFacts) -> Value {
    serde_json::to_value(f).unwrap()
}

fn words_json(w: &PermissionPromptWords) -> Value {
    serde_json::to_value(w).unwrap()
}

// toMatchObject: every key of `want` is in `got` with a matching value, objects matched the same way down.
fn matches(got: &Value, want: &Value) -> bool {
    match (got, want) {
        (Value::Object(g), Value::Object(w)) => w.iter().all(|(k, v)| g.get(k).is_some_and(|gv| matches(gv, v))),
        _ => got == want,
    }
}

fn labels(options: &[PermissionOption]) -> Vec<&str> {
    options.iter().map(|o| o.label.as_str()).collect()
}

mod a_turn_s_activity_in_one_line_each {
    use super::*;

    #[test]
    fn reads_a_shell_call_behind_a_prompt_its_first_line_only_whatever_the_harness_call() {
        assert_eq!(tool_activity_line(Some("Bash"), &js(json!({ "command": "git status" }))), "$ git status");
        assert_eq!(tool_activity_line(Some("Bash"), &js(json!({ "command": "  git   log \n  | head -3" }))), "$ git log");
        assert_eq!(tool_activity_line(Some("command_execution"), &js(json!({ "command": "pnpm test" }))), "$ pnpm test");
    }

    #[test]
    fn reads_a_file_behind_the_verb_that_touched_it_and_a_search_behind_what_it_looked() {
        assert_eq!(
            tool_activity_line(Some("Read"), &js(json!({ "file_path": "packages/engine/src/golden-mcp.ts" }))),
            "reading packages/engine/src/golden-mcp.ts"
        );
        assert_eq!(tool_activity_line(Some("Write"), &js(json!({ "file_path": "src/a.ts" }))), "writing src/a.ts");
        assert_eq!(tool_activity_line(Some("Edit"), &js(json!({ "file_path": "src/a.ts" }))), "editing src/a.ts");
        assert_eq!(tool_activity_line(Some("NotebookEdit"), &js(json!({ "notebook_path": "run.ipynb" }))), "editing run.ipynb");
        assert_eq!(tool_activity_line(Some("Grep"), &js(json!({ "pattern": "shellQuote" }))), "searching code for shellQuote");
        assert_eq!(
            tool_activity_line(Some("WebSearch"), &js(json!({ "query": "solari snapshot" }))),
            "searching the web for solari snapshot"
        );
        assert_eq!(tool_activity_line(Some("WebFetch"), &js(json!({ "url": "https://example.com" }))), "fetching https://example.com");
        assert_eq!(tool_activity_line(Some("Task"), &js(json!({ "description": "review the diff" }))), "agent: review the diff");
        assert_eq!(
            tool_activity_line(Some("spawn_agent"), &js(json!({ "prompt": "review the diff\nand say what broke" }))),
            "agent: review the diff"
        );
    }

    #[test]
    fn counts_the_paths_of_a_change_call_that_carries_several_and_names_the_one_it_carr() {
        let one = json!([{ "kind": "edit", "path": "src/a.ts" }]);
        assert_eq!(tool_activity_line(Some("file_change"), &js(json!({ "changes": one }))), "editing src/a.ts");
        assert_eq!(tool_done_line(Some("file_change"), &js(json!({ "changes": one }))), Some("edited src/a.ts".to_string()));
        let two = json!([{ "kind": "edit", "path": "src/a.ts" }, { "kind": "add", "path": "src/b.ts" }]);
        // plural(2, "file") written out, so this test reaches only tools
        assert_eq!(tool_activity_line(Some("file_change"), &js(json!({ "changes": two }))), "editing 2 files");
    }

    #[test]
    fn says_what_a_call_is_doing_while_it_runs_and_what_it_did_once_its_result_landed_n() {
        let write = js(json!({ "file_path": "kai.txt" }));
        assert_eq!(tool_activity_line(Some("Write"), &write), "writing kai.txt");
        assert_eq!(tool_done_line(Some("Write"), &write), Some("wrote kai.txt".to_string()));
        assert_eq!(tool_activity_line(Some("Read"), &js(json!({ "file_path": "src/a.ts" }))), "reading src/a.ts");
        assert_eq!(tool_done_line(Some("Read"), &js(json!({ "file_path": "src/a.ts" }))), Some("read src/a.ts".to_string()));
        assert_eq!(tool_activity_line(Some("Edit"), &js(json!({ "file_path": "src/a.ts" }))), "editing src/a.ts");
        assert_eq!(tool_done_line(Some("Edit"), &js(json!({ "file_path": "src/a.ts" }))), Some("edited src/a.ts".to_string()));
        assert_eq!(tool_activity_line(Some("Grep"), &js(json!({ "pattern": "shellQuote" }))), "searching code for shellQuote");
        assert_eq!(tool_done_line(Some("Grep"), &js(json!({ "pattern": "shellQuote" }))), Some("searched code for shellQuote".to_string()));
    }

    #[test]
    fn a_call_whose_row_reads_the_same_either_way_has_no_line_of_its_own_for_its_result() {
        assert_eq!(tool_done_line(Some("Bash"), &js(json!({ "command": "git status" }))), None);
        assert_eq!(tool_done_line(Some("Task"), &js(json!({ "description": "review the diff" }))), None);
        assert_eq!(tool_done_line(Some("TodoWrite"), &js(json!({ "todos": [] }))), None);
        assert_eq!(tool_done_line(Some("Write"), "{\"file_pa"), None);
    }

    #[test]
    fn falls_back_to_a_server_tool_s_words_else_the_tool_s_own_name_when_there_is_no_ro() {
        assert_eq!(tool_activity_line(Some("TodoWrite"), &js(json!({ "todos": [] }))), "TodoWrite");
        assert_eq!(tool_activity_line(Some("mcp__wsp__send"), &js(json!({ "id": "t1" }))), "Use wsp's send");
        assert_eq!(tool_activity_line(Some("mcp__wsp__run"), &js(json!({ "message": "go", "title": "Login test" }))), "Use wsp's run");
        assert_eq!(tool_activity_line(Some("Bash"), &js(json!({ "description": "list them" }))), "Bash");
        assert_eq!(tool_activity_line(Some("Bash"), "{\"comm"), "Bash");
        assert_eq!(tool_activity_line(Some("Bash"), &js(Value::Null)), "Bash");
        assert_eq!(tool_activity_line(None, &js(json!({ "command": "ls" }))), "tool");
    }
}

mod what_a_tool_call_answered_in_one_line {
    use super::*;

    #[test]
    fn is_the_result_s_first_line_cut_by_the_rule_that_cuts_the_call_s_own_line() {
        assert_eq!(tool_result_line("On branch main\nnothing to commit", false), Some("On branch main".to_string()));
        assert_eq!(tool_result_line("  total   0 \nfoo", false), Some("total 0".to_string()));
    }

    #[test]
    fn marks_a_failed_result_in_words_with_what_the_harness_said_when_it_said_anything() {
        assert_eq!(tool_result_line("exit 1: no such file", true), Some("failed: exit 1: no such file".to_string()));
        assert_eq!(tool_result_line("", true), Some("failed".to_string()));
        assert_eq!(tool_result_line("\n  \n", true), Some("failed".to_string()));
    }

    #[test]
    fn has_nothing_to_say_for_a_result_that_answered_with_nothing() {
        assert_eq!(tool_result_line("", false), None);
        assert_eq!(tool_result_line("\n  \n", false), None);
    }
}

mod the_one_registry_every_client_reads_a_tool_call_from {
    use super::*;

    #[test]
    fn says_which_field_a_call_s_row_shows_what_kind_of_item_it_is_and_what_it_changed() {
        assert_eq!(
            facts_json(&tool_call_facts("Bash", &js(json!({ "command": "git status", "description": "Show working tree status" })))),
            json!({ "itemType": "command_execution", "requestKind": "command", "detail": "Show working tree status", "command": "git status", "description": "Show working tree status" })
        );
        assert_eq!(
            facts_json(&tool_call_facts("command_execution", &js(json!({ "command": "pnpm test" })))),
            json!({ "itemType": "command_execution", "requestKind": "command", "command": "pnpm test" })
        );
        assert_eq!(
            facts_json(&tool_call_facts("Read", &js(json!({ "file_path": "/x/a.ts" })))),
            json!({ "requestKind": "file-read", "detail": "/x/a.ts" })
        );
        assert_eq!(
            facts_json(&tool_call_facts("Edit", &js(json!({ "file_path": "/x/a.ts", "old_string": "a" })))),
            json!({ "itemType": "file_change", "requestKind": "file-change", "detail": "/x/a.ts", "changedFiles": ["/x/a.ts"] })
        );
        assert_eq!(
            facts_json(&tool_call_facts("NotebookEdit", &js(json!({ "notebook_path": "run.ipynb" })))),
            json!({ "itemType": "file_change", "requestKind": "file-change", "detail": "run.ipynb", "changedFiles": ["run.ipynb"] })
        );
        assert_eq!(
            facts_json(&tool_call_facts(
                "file_change",
                &js(json!({ "changes": [{ "kind": "edit", "path": "src/a.ts" }, { "kind": "add", "path": "src/b.ts" }] }))
            )),
            json!({ "itemType": "file_change", "requestKind": "file-change", "changedFiles": ["src/a.ts", "src/b.ts"] })
        );
        assert_eq!(
            facts_json(&tool_call_facts("Grep", &js(json!({ "pattern": "shellQuote", "path": "src" })))),
            json!({ "detail": "shellQuote" })
        );
        assert_eq!(
            facts_json(&tool_call_facts("web_search", &js(json!({ "query": "solari snapshot" })))),
            json!({ "itemType": "web_search", "detail": "solari snapshot" })
        );
        assert_eq!(
            facts_json(&tool_call_facts("Task", &js(json!({ "description": "scan repo", "prompt": "find every caller" })))),
            json!({ "itemType": "collab_agent_tool_call", "detail": "scan repo" })
        );
        assert_eq!(
            facts_json(&tool_call_facts("spawn_agent", &js(json!({ "prompt": "find every caller" })))),
            json!({ "itemType": "collab_agent_tool_call", "detail": "find every caller" })
        );
    }

    #[test]
    fn reads_a_name_with_no_row_by_the_general_field_order_and_an_mcp_call_by_its_name() {
        assert_eq!(facts_json(&tool_call_facts("TodoWrite", &js(json!({ "todos": [] })))), json!({}));
        assert_eq!(
            facts_json(&tool_call_facts("mcp__wsp__send", &js(json!({ "prompt": "hello" })))),
            json!({ "itemType": "mcp_tool_call", "title": "Use wsp's send", "detail": "hello" })
        );
        assert_eq!(
            facts_json(&tool_call_facts("mcp__wsp__run", &js(json!({ "message": "go on", "title": "Login test" })))),
            json!({ "itemType": "mcp_tool_call", "title": "Use wsp's run", "detail": "Login test" })
        );
        assert_eq!(
            facts_json(&tool_call_facts("mcp__wsp__run", &js(json!({ "message": "go on" })))),
            json!({ "itemType": "mcp_tool_call", "title": "Use wsp's run", "detail": "go on" })
        );
        assert_eq!(facts_json(&tool_call_facts("Wombat", &js(json!({ "query": "grey fur" })))), json!({ "detail": "grey fur" }));
    }

    #[test]
    fn shows_input_still_arriving_as_it_stands_and_reads_an_empty_input_as_a_call_with() {
        assert_eq!(
            facts_json(&tool_call_facts("Bash", "{\"comm")),
            json!({ "itemType": "command_execution", "requestKind": "command", "detail": "{\"comm" })
        );
        assert_eq!(facts_json(&tool_call_facts("Bash", "")), json!({ "itemType": "command_execution", "requestKind": "command" }));
        assert_eq!(
            facts_json(&tool_call_facts("Bash", &js(Value::Null))),
            json!({ "itemType": "command_execution", "requestKind": "command", "detail": "null" })
        );
    }

    #[test]
    fn names_the_calls_that_looked_through_the_code_for_the_client_that_folds_them_into() {
        let yes: Vec<bool> = ["Grep", "Glob"].iter().map(|name| is_code_search_tool(Some(name))).collect();
        assert_eq!(yes, vec![true, true]);
        let no: Vec<bool> = [Some("Bash"), Some("Read"), Some("WebSearch"), None].iter().map(|name| is_code_search_tool(*name)).collect();
        assert_eq!(no, vec![false, false, false, false]);
    }
}

mod the_words_a_relayed_permission_prompt_shows {
    use super::*;

    #[test]
    fn words_a_file_write_by_the_file_the_folder_holding_it_and_how_much_is_going_in() {
        // The path and the file's own text are what a person cannot read at a glance; the name, the folder and the size are.
        assert_eq!(
            permission_ask_line("Write", &js(json!({ "file_path": "/Users/dev/wsp-work/index.html", "content": "x".repeat(2_100) })), None),
            "Write index.html in wsp-work (2 KB)"
        );
        assert_eq!(permission_ask_line("Write", &js(json!({ "file_path": "out.txt", "content": "hi" })), None), "Write out.txt (2 B)");
        // The lead is the whole of it: a write's own words carry no code part, since a file's name is not read character by character.
        assert_eq!(permission_prompt_words("Write", &js(json!({ "file_path": "/root/out.txt", "content": "hi" })), None).code, None);
        // The options under it are the question, so the line never asks one.
        let line = permission_ask_line("Write", &js(json!({ "file_path": "/root/out.txt", "content": "hi" })), None);
        assert!(!line.contains('?'), "{line}");
    }

    #[test]
    fn words_a_command_as_the_whole_command_kept_apart_from_the_words_so_a_client_can_d() {
        // A command goes in whole, its later lines and its length alike: one cut anywhere is one nobody can judge.
        let long = format!("cat > out.py <<'PY'\nprint({}1)\nPY", "1 + ".repeat(60));
        assert_eq!(
            permission_ask_line("Bash", &js(json!({ "command": long, "description": "Write and run a sum" })), None),
            format!("Run a command: {long}")
        );
        // The two parts are apart, so a face that blurs two hyphens into one dash never draws the command.
        let gate = "pnpm exec vitest run --minWorkers=1 --maxWorkers=1";
        let words = words_json(&permission_prompt_words("Bash", &js(json!({ "command": gate })), None));
        let want = json!({ "says": "Run a command:", "code": gate, "lead": format!("Run a command: {gate}") });
        assert!(matches(&words, &want), "{words} should match {want}");
    }

    #[test]
    fn words_a_tool_a_server_lends_as_the_server_and_the_tool_and_never_the_server_s_ow() {
        assert_eq!(
            permission_ask_line("mcp__wsp__workspaces", "{}", Some("wsp runs cloud machines called workspaces, forked in seconds")),
            "Use wsp's workspaces"
        );
        assert_eq!(permission_ask_line("mcp__wsp__run", "{}", None), "Use wsp's run");
        assert_eq!(permission_prompt_words("mcp__wsp__workspaces", "{}", Some("wsp runs cloud machines")).code, None);
    }

    #[test]
    fn words_a_skill_as_its_own_name_and_never_the_description_paragraph_the_harness_se() {
        assert_eq!(
            permission_ask_line(
                "Skill",
                &js(json!({ "skill": "agent-browser" })),
                Some("Browser automation CLI for AI agents. Use when the user needs")
            ),
            "Run the skill agent-browser"
        );
    }

    #[test]
    fn falls_back_to_the_harness_s_own_phrase_under_the_tool_s_name_for_a_kind_with_no() {
        assert_eq!(
            permission_ask_line("Read", &js(json!({ "file_path": "/root/out.txt" })), Some("out.txt")),
            "Permission for Read: out.txt"
        );
        assert_eq!(permission_ask_line("Read", "{}", None), "Permission for Read");
        assert_eq!(permission_ask_line("Read", "{}", Some("")), "Permission for Read");
        // Input that is not an object yet, and a call whose input carries none of what its rule needs, fall back the same way.
        assert_eq!(permission_ask_line("Bash", "{\"comm", Some("rm -rf build")), "Permission for Bash: rm -rf build");
        assert_eq!(permission_ask_line("Write", "{}", Some("out.txt")), "Permission for Write: out.txt");
    }

    #[test]
    fn folds_a_file_s_body_away_and_shows_the_rest_of_the_input_under_the_lead() {
        let body = "line\n".repeat(400);
        let write = permission_prompt_words("Write", &js(json!({ "file_path": "/Users/dev/wsp-work/index.html", "content": body })), None);
        assert_eq!(write.lead, "Write index.html in wsp-work (2 KB)");
        // The file is behind the fold, whole, and nowhere else: the buttons stay in reach.
        assert_eq!(serde_json::to_value(&write.body).unwrap(), json!({ "label": "show the file", "text": body }));
        assert_eq!(write.rest, "");
        // The lead already carries the command, and the harness's phrase for it is never the row's words.
        assert_eq!(
            permission_prompt_words("Bash", &js(json!({ "command": "rm -rf build", "description": "Clean the build" })), None).rest,
            ""
        );
        assert_eq!(permission_prompt_words("Skill", &js(json!({ "skill": "agent-browser" })), Some("Browser automation CLI")).rest, "");
        // What the lead does not carry still shows, each field apart from the next: this is the row consent is given on.
        assert_eq!(permission_prompt_words("Bash", &js(json!({ "command": "ls", "timeout": 5_000 })), None).rest, "timeout: 5000");
        assert_eq!(
            permission_prompt_words("mcp__wsp__send", &js(json!({ "threadId": "thr_1", "message": "go" })), None).rest,
            "threadId: thr_1\nmessage: go"
        );
        // A field a tool really calls description is the call's own, not the harness's paragraph, so it shows like any other.
        assert_eq!(
            permission_prompt_words(
                "mcp__linear__create_issue",
                &js(json!({ "title": "Fix login", "description": "The button does\n\nnothing on Safari." })),
                None
            )
            .rest,
            "title: Fix login\ndescription: The button does nothing on Safari."
        );
        // A tool that carries no file body folds nothing away, and input that is not an object is shown as it stands.
        assert_eq!(
            serde_json::to_value(&permission_prompt_words("Bash", &js(json!({ "command": "ls" })), None).body).unwrap(),
            Value::Null
        );
        assert!(permission_prompt_words("Bash", &js(json!({ "command": "ls" })), None).body.is_none());
        assert_eq!(permission_prompt_words("Bash", "not json", None).rest, "not json");
    }

    #[test]
    fn says_how_a_closed_prompt_closed_naming_the_option_only_where_it_says_something_t() {
        let allow = ("Allow", PermissionEffect::Allow);
        let deny = ("Deny", PermissionEffect::Deny);
        let mode = ("Allow, then Accept edits", PermissionEffect::Mode);
        // "Allowed: Allow" and "Denied: Deny" name one fact twice; the mode pick is the one that says more.
        assert_eq!(permission_outcome_line(PermissionOutcome::Allowed, Some(allow)), "Allowed");
        assert_eq!(permission_outcome_line(PermissionOutcome::Allowed, None), "Allowed");
        assert_eq!(permission_outcome_line(PermissionOutcome::Denied, Some(deny)), "Denied");
        assert_eq!(permission_outcome_line(PermissionOutcome::Allowed, Some(mode)), "Allowed: Allow, then Accept edits");
        // Nobody picked either of these, so neither names an option, whatever it is handed.
        assert_eq!(permission_outcome_line(PermissionOutcome::Unanswered, None), "Nobody answered; denied");
        assert_eq!(permission_outcome_line(PermissionOutcome::Cancelled, None), "Cancelled with the turn");
        assert_eq!(permission_outcome_line(PermissionOutcome::Unanswered, Some(deny)), "Nobody answered; denied");
        assert_eq!(permission_outcome_line(PermissionOutcome::Cancelled, Some(mode)), "Cancelled with the turn");
    }

    #[test]
    fn reads_a_call_that_asks_the_person_as_the_question_it_is_with_one_answer_per_choi() {
        let input = js(json!({
            "questions": [
                {
                    "question": "This working directory is not a repository. What should I do?",
                    "header": "Directory",
                    "options": [
                        { "label": "Clone it", "description": "Fetch the remote into this folder." },
                        { "label": "Start fresh", "description": "Run git init here." },
                        { "label": "Stop", "description": "Do nothing and wait." },
                    ],
                    "multiSelect": false,
                },
            ],
        }));
        let asked = asked_questions(QUESTION_TOOL, &input).unwrap();
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].header, "Directory");
        assert_eq!(asked[0].question, "This working directory is not a repository. What should I do?");
        assert!(!asked[0].multi_select);
        let pairs: Vec<(&str, &str)> = asked[0].options.iter().map(|o| (o.label.as_str(), o.description.as_str())).collect();
        assert_eq!(
            pairs,
            vec![
                ("Clone it", "Fetch the remote into this folder."),
                ("Start fresh", "Run git init here."),
                ("Stop", "Do nothing and wait."),
            ]
        );
        // Three choices, three options on the wire, every one an answer: nothing here offers to allow or refuse a call
        // that only asks.
        let options = question_options(QUESTION_TOOL, &input);
        assert_eq!(options.len(), 3);
        assert_eq!(labels(&options), vec!["Clone it", "Start fresh", "Stop"]);
        let effects: HashSet<PermissionEffect> = options.iter().map(|o| o.effect).collect();
        assert_eq!(effects, HashSet::from([PermissionEffect::Answer]));
        let ids: Vec<&str> = options.iter().map(|o| o.id.as_str()).collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
        // The whole prompt is the question; no raw input is left anywhere on it.
        let words = permission_prompt_words(QUESTION_TOOL, &input, None);
        assert_eq!(words.questions, Some(asked.clone()));
        assert_eq!(words.rest, "");
        assert!(words.body.is_none());
        assert_eq!(words.lead, "This working directory is not a repository. What should I do?");
        // The call itself reads as the question too: the tool's own name is no word a person knows.
        assert_eq!(tool_activity_line(Some(QUESTION_TOOL), &input), "This working directory is not a repository. What should I do?");
        let activity = tool_activity_line(Some(QUESTION_TOOL), &input);
        assert!(!activity.contains(QUESTION_TOOL), "{activity}");
        // And the transcript's own row for it reads the same way: words a person knows, then the question under them.
        let facts = tool_call_facts(QUESTION_TOOL, &input);
        assert_eq!(facts.title, Some("Asked you".to_string()));
        // The prompt under that row is the question, whole, so the row itself repeats none of it.
        assert_eq!(facts.detail, None);
        // Every tool a person already knows the word for keeps the harness's own name.
        assert_eq!(tool_call_facts("Bash", &js(json!({ "command": "ls" }))).title, None);
        for raw in ["multiSelect", "questions:", "{", "}"] {
            assert!(!words.lead.contains(raw), "{}", words.lead);
        }
        // Every other kind of call keeps its own words and carries no questions.
        assert_eq!(permission_prompt_words("Bash", &js(json!({ "command": "ls" })), None).questions, None);
        assert_eq!(asked_questions("Bash", &js(json!({ "command": "ls" }))), None);
        assert_eq!(question_options("Bash", &js(json!({ "command": "ls" }))), Vec::<PermissionOption>::new());
    }

    #[test]
    fn turns_a_pick_into_the_answer_the_harness_reads_off_the_call_s_own_input_one_labe() {
        let one_value = json!({
            "questions": [{ "question": "Tabs or spaces?", "header": "Indent", "options": [{ "label": "Tabs" }, { "label": "Spaces" }], "multiSelect": false }],
        });
        let one = js(one_value.clone());
        let options = question_options(QUESTION_TOOL, &one);
        let spaces = options[1].id.clone();
        let answer = Value::Object(question_answer_input(QUESTION_TOOL, &one, std::slice::from_ref(&spaces)).unwrap());
        let want = json!({ "answers": { "Tabs or spaces?": "Spaces" } });
        assert!(matches(&answer, &want), "{answer} should match {want}");
        // The call's own fields ride back with it: the harness matches the answer to the question it asked.
        assert_eq!(answer["questions"], one_value["questions"]);
        // One pick closes one prompt, so a question that takes several answers sends them as one id.
        let many = js(json!({
            "questions": [{ "question": "Which checks?", "header": "Checks", "options": [{ "label": "Types" }, { "label": "Tests" }, { "label": "Lint" }], "multiSelect": true }],
        }));
        let picks = question_options(QUESTION_TOOL, &many);
        let both = picked_option_id(&[picks[0].id.clone(), picks[2].id.clone()]);
        let picked = picked_options(&picks, &both).unwrap();
        assert_eq!(labels(&picked), vec!["Types", "Lint"]);
        let answer = Value::Object(question_answer_input(QUESTION_TOOL, &many, &[picks[0].id.clone(), picks[2].id.clone()]).unwrap());
        let want = json!({ "answers": { "Which checks?": ["Types", "Lint"] } });
        assert!(matches(&answer, &want), "{answer} should match {want}");
        // A pick naming anything this prompt does not carry names nothing at all.
        assert_eq!(picked_options(&picks, "q9:o9"), None);
        assert_eq!(picked_options(&picks, &picked_option_id(&[picks[0].id.clone(), "q9:o9".to_string()])), None);
        // An ordinary prompt's own ids still resolve through the one road every pick takes.
        let plain: Vec<PermissionOption> = wire(json!([{ "id": "allow", "label": "Allow", "effect": "allow" }]));
        let ids: Vec<String> = picked_options(&plain, "allow").unwrap().into_iter().map(|o| o.id).collect();
        assert_eq!(ids, vec!["allow".to_string()]);
        // A closed question says what was picked, since "Allowed" says nothing about an answer.
        assert_eq!(
            permission_outcome_line(PermissionOutcome::Allowed, Some(("Types, Lint", PermissionEffect::Answer))),
            "You answered: Types, Lint"
        );
    }

    #[test]
    fn an_answer_typed_in_other_is_the_answer_the_harness_reads_beside_the_ticked_label() {
        let form = js(json!({
            "questions": [
                { "question": "Which one?", "header": "Pick", "options": [{ "label": "A" }, { "label": "B" }], "multiSelect": false },
                { "question": "Which checks?", "header": "Checks", "options": [{ "label": "Types" }, { "label": "Lint" }], "multiSelect": true },
                { "question": "A name?", "header": "Name", "options": [], "multiSelect": false },
            ],
        }));
        let options = question_options(QUESTION_TOOL, &form);
        // Words carrying the join and the id's own colon still travel as one pick each.
        let picks =
            vec![other_option_id(0, "C | or D: either"), options[2].id.clone(), other_option_id(1, "Format"), other_option_id(2, "Webby")];
        let picked = picked_options(&options, &picked_option_id(&picks)).unwrap();
        assert_eq!(labels(&picked), vec!["C | or D: either", "Types", "Format", "Webby"]);
        let answer = Value::Object(question_answer_input(QUESTION_TOOL, &form, &picks[..]).unwrap());
        let want = json!({ "answers": { "Which one?": "C | or D: either", "Which checks?": ["Types", "Format"], "A name?": "Webby" } });
        assert!(matches(&answer, &want), "{answer} should match {want}");
        // A consent prompt takes no typed answer: its options are the only picks it has.
        let consent: Vec<PermissionOption> = wire(json!([{ "id": "allow", "label": "Allow", "effect": "allow" }]));
        assert_eq!(picked_options(&consent, &other_option_id(0, "sure")), None);
        assert_eq!(permission_outcome_line(PermissionOutcome::Allowed, Some(("Webby", PermissionEffect::Answer))), "You answered: Webby");
    }

    #[test]
    fn a_deny_the_person_gave_a_reason_for_tells_the_agent_the_reason_in_their_own_word() {
        assert_eq!(denied_line(None), PERMISSION_DENIED_LINE);
        assert_eq!(denied_line(Some("  ")), PERMISSION_DENIED_LINE);
        assert_eq!(
            denied_line(Some("Run only the thread-rows file")),
            format!("{PERMISSION_DENIED_LINE}, and said what to do instead: Run only the thread-rows file")
        );
    }

    #[test]
    fn shows_nobody_a_tool_result_the_harness_marked_its_own_note_to_the_agent_and_titl() {
        let note = "Async agent launched successfully. (This tool result is internal metadata, never quote or paste any part of it, including the agentId below, into a user-facing reply.)\nagentId: a057760";
        assert!(internal_tool_result(note));
        assert_eq!(tool_result_line(note, false), None);
        // What a person may read is untouched.
        assert!(!internal_tool_result("acpi\nadduser.conf"));
        assert_eq!(tool_result_line("acpi\nadduser.conf", false), Some("acpi".to_string()));
        // The fold's own line says what was launched, off the call that launched it.
        assert_eq!(
            subagent_task_line(&js(json!({ "description": "count alpha files", "prompt": "run ls" }))),
            Some("count alpha files".to_string())
        );
        assert_eq!(subagent_task_line(&js(json!({ "prompt": "run ls" }))), None);
        assert_eq!(subagent_asker_line("count alpha files"), "count alpha files asks");
        // A prompt drawn in a thread that did not raise it names the thread that did, in the row grammar's two parts
        // under a middle dot, so it reads as one line of the same family as the asker line above it.
        assert_eq!(waiting_asker_line("read the file"), "read the file asks; this thread waits on the answer");
    }

    #[test]
    fn has_one_deny_line_the_person_s_own_and_it_points_the_agent_at_no_other_access_mo() {
        assert_eq!(PERMISSION_DENIED_LINE, "the person denied this in the chat");
        for word in ["access", "bypass", "mode", "start the thread"] {
            assert!(!PERMISSION_DENIED_LINE.contains(word), "{word}");
        }
        // Added: the TS test reads only the constant, which is already filled, so this reaches the stub that words it.
        assert_eq!(denied_line(None), PERMISSION_DENIED_LINE);
    }

    #[test]
    fn what_a_thread_says_it_is_waiting_on_is_the_prompt_row_s_own_lead_worded_in_one_p() {
        let ask: SessionPermissionEvent = wire(
            json!({ "workspaceId": "w", "sessionId": "s", "askId": "a1", "options": [], "toolName": "Write", "input": "{\"file_path\":\"/root/out.txt\",\"content\":\"hi\"}", "detail": "out.txt" }),
        );
        assert_eq!(asking_line(&ask), permission_ask_line(&ask.tool_name, &ask.input, ask.detail.as_deref()));
        // The whole prompt is in hand, so the lead reads the call's own fields and not only the phrase the harness named.
        let line = asking_line(&ask);
        assert!(line.contains("out.txt"), "{line}");
        let bare: SessionPermissionEvent = wire(
            json!({ "workspaceId": "w", "sessionId": "s", "askId": "a1", "options": [], "toolName": "mcp__wsp__workspaces", "input": "{}" }),
        );
        assert_eq!(asking_line(&bare), permission_ask_line(&bare.tool_name, &bare.input, bare.detail.as_deref()));
    }

    #[test]
    fn a_mode_option_reads_as_an_allow_that_also_stops_the_asking_in_the_picker_s_own_w() {
        assert_eq!(permission_mode_option_label("Accept edits"), "Allow, then Accept edits");
    }

    #[test]
    fn the_access_menu_says_what_a_pick_does_to_the_turn_running_now_before_the_pick_is() {
        assert_eq!(access_reach_line(true), "Applies to the turn running now");
        assert_eq!(access_reach_line(false), "Applies from the thread's next turn");
        // Said of the pick, not of a failure: the line stands over the list before anything has been picked.
        let failure = regex::Regex::new("could not|failed|unsupported").unwrap();
        for moves in [true, false] {
            let line = access_reach_line(moves);
            assert!(!failure.is_match(&line), "{line}");
        }
    }

    #[test]
    fn a_pick_a_harness_said_it_would_take_and_then_refused_is_a_refusal_in_two_halves() {
        assert_eq!(ACCESS_REFUSED_SAID, "The turn refused it.");
        assert_eq!(ACCESS_REFUSED_FIX, "The next turn runs at it.");
        assert_eq!(ACCESS_REFUSED_LINE, format!("{ACCESS_REFUSED_SAID} {ACCESS_REFUSED_FIX}"));
        // Both halves: what happened, then where the pick stands, which is the shape every refusal in this app has. A
        // message carries no access, so neither half may say one does.
        let next_turn = regex::Regex::new("next turn").unwrap();
        assert!(next_turn.is_match(ACCESS_REFUSED_FIX));
        let message = regex::Regex::new("message").unwrap();
        assert!(!message.is_match(ACCESS_REFUSED_LINE));
        let reach = access_reach_line(false);
        assert!(!message.is_match(&reach), "{reach}");
        // The slot is one line that truncates from the right; the render test measures the paint, this holds the budget
        // the measurement was against, so the half carrying the answer is never the half that is cut.
        assert!(ACCESS_REFUSED_LINE.encode_utf16().count() < 54);
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::words::tools::*;

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }

    use wsp_wire::views::session::PermissionOutcome;
    use wsp_wire::views::session_events::SessionPermissionEvent;
    use wsp_wire::wire::helpers::{PermissionEffect, PermissionOption};

    fn opt_answer(id: &str, label: &str) -> PermissionOption {
        PermissionOption { id: id.into(), label: label.into(), effect: PermissionEffect::Answer, mode: None }
    }

    fn opt_allow(id: &str, label: &str) -> PermissionOption {
        PermissionOption { id: id.into(), label: label.into(), effect: PermissionEffect::Allow, mode: None }
    }

    fn ask(tool_name: &str, input: &str, detail: Option<&str>) -> SessionPermissionEvent {
        let mut v = serde_json::json!({ "workspaceId": "w", "sessionId": "s", "askId": "a1", "toolName": tool_name, "input": input, "options": [] });
        if let Some(d) = detail {
            v["detail"] = serde_json::json!(d);
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn a_server_tool_is_mcp_then_a_server_then_the_rest() {
        assert_eq!(server_tool("mcp__wsp__run"), Some(ServerTool { server: "wsp".into(), tool: "run".into() })); // JSON.stringify(serverTool("mcp__wsp__run") ?? null)
        assert_eq!(server_tool("mcp__a__b__c"), Some(ServerTool { server: "a".into(), tool: "b__c".into() })); // JSON.stringify(serverTool("mcp__a__b__c") ?? null)
        assert_eq!(server_tool("mcp____x"), None); // JSON.stringify(serverTool("mcp____x") ?? null)
        assert_eq!(server_tool("mcp__srv__"), Some(ServerTool { server: "srv".into(), tool: "".into() })); // JSON.stringify(serverTool("mcp__srv__") ?? null)
        assert_eq!(server_tool("mcp_x__y"), None); // JSON.stringify(serverTool("mcp_x__y") ?? null)
        assert_eq!(server_tool("Bash"), None); // JSON.stringify(serverTool("Bash") ?? null)
        assert_eq!(server_tool(""), None); // JSON.stringify(serverTool("") ?? null)
        assert_eq!(server_tool_words("mcp__wsp__run_a_thing"), Some("Use wsp's run a thing".to_string())); // serverToolWords("mcp__wsp__run_a_thing")
        assert_eq!(server_tool_words("mcp__srv__"), Some("Use srv's ".to_string())); // serverToolWords("mcp__srv__")
        assert_eq!(server_tool_words("Read"), None); // serverToolWords("Read")
    }

    #[test]
    fn every_row_reads_its_call_at_both_moments() {
        assert_eq!(tool_activity_line(Some("Bash"), "{\"command\":\"  ls -la\\n  && echo\"}"), "$ ls -la"); // toolActivityLine("Bash", "{\"command\":\"  ls -la\\n  && echo\"}")
        assert_eq!(tool_done_line(Some("Bash"), "{\"command\":\"  ls -la\\n  && echo\"}"), None); // toolDoneLine("Bash", "{\"command\":\"  ls -la\\n  && echo\"}")
        assert_eq!(tool_activity_line(Some("Bash"), "{\"command\":\"   \"}"), "Bash"); // toolActivityLine("Bash", "{\"command\":\"   \"}")
        assert_eq!(tool_done_line(Some("Bash"), "{\"command\":\"   \"}"), None); // toolDoneLine("Bash", "{\"command\":\"   \"}")
        assert_eq!(tool_activity_line(Some("Bash"), "{bad"), "Bash"); // toolActivityLine("Bash", "{bad")
        assert_eq!(tool_done_line(Some("Bash"), "{bad"), None); // toolDoneLine("Bash", "{bad")
        assert_eq!(tool_activity_line(Some("Bash"), "[1]"), "Bash"); // toolActivityLine("Bash", "[1]")
        assert_eq!(tool_done_line(Some("Bash"), "[1]"), None); // toolDoneLine("Bash", "[1]")
        assert_eq!(tool_activity_line(Some("Read"), "{\"file_path\":\"/a/b.ts\"}"), "reading /a/b.ts"); // toolActivityLine("Read", "{\"file_path\":\"/a/b.ts\"}")
        assert_eq!(tool_done_line(Some("Read"), "{\"file_path\":\"/a/b.ts\"}"), Some("read /a/b.ts".to_string())); // toolDoneLine("Read", "{\"file_path\":\"/a/b.ts\"}")
        assert_eq!(tool_activity_line(Some("Write"), "{\"file_path\":\"/a/b.ts\"}"), "writing /a/b.ts"); // toolActivityLine("Write", "{\"file_path\":\"/a/b.ts\"}")
        assert_eq!(tool_done_line(Some("Write"), "{\"file_path\":\"/a/b.ts\"}"), Some("wrote /a/b.ts".to_string())); // toolDoneLine("Write", "{\"file_path\":\"/a/b.ts\"}")
        assert_eq!(tool_activity_line(Some("NotebookEdit"), "{\"notebook_path\":\"/n.ipynb\"}"), "editing /n.ipynb"); // toolActivityLine("NotebookEdit", "{\"notebook_path\":\"/n.ipynb\"}")
        assert_eq!(tool_done_line(Some("NotebookEdit"), "{\"notebook_path\":\"/n.ipynb\"}"), Some("edited /n.ipynb".to_string())); // toolDoneLine("NotebookEdit", "{\"notebook_path\":\"/n.ipynb\"}")
        assert_eq!(tool_activity_line(Some("file_change"), "{\"changes\":[{\"path\":\"/x\"}]}"), "editing /x"); // toolActivityLine("file_change", "{\"changes\":[{\"path\":\"/x\"}]}")
        assert_eq!(tool_done_line(Some("file_change"), "{\"changes\":[{\"path\":\"/x\"}]}"), Some("edited /x".to_string())); // toolDoneLine("file_change", "{\"changes\":[{\"path\":\"/x\"}]}")
        assert_eq!(
            tool_activity_line(Some("file_change"), "{\"changes\":[{\"path\":\"/x\"},{\"path\":\"\"},{\"path\":\"/y\"},null]}"),
            "editing 2 files"
        ); // toolActivityLine("file_change", "{\"changes\":[{\"path\":\"/x\"},{\"path\":\"\"},{\"path\":\"/y\"},null]}")
        assert_eq!(
            tool_done_line(Some("file_change"), "{\"changes\":[{\"path\":\"/x\"},{\"path\":\"\"},{\"path\":\"/y\"},null]}"),
            Some("edited 2 files".to_string())
        ); // toolDoneLine("file_change", "{\"changes\":[{\"path\":\"/x\"},{\"path\":\"\"},{\"path\":\"/y\"},null]}")
        assert_eq!(tool_activity_line(Some("file_change"), "{\"changes\":[]}"), "file_change"); // toolActivityLine("file_change", "{\"changes\":[]}")
        assert_eq!(tool_done_line(Some("file_change"), "{\"changes\":[]}"), None); // toolDoneLine("file_change", "{\"changes\":[]}")
        assert_eq!(tool_activity_line(Some("Grep"), "{\"pattern\":\"foo\\nbar\"}"), "searching code for foo"); // toolActivityLine("Grep", "{\"pattern\":\"foo\\nbar\"}")
        assert_eq!(tool_done_line(Some("Grep"), "{\"pattern\":\"foo\\nbar\"}"), Some("searched code for foo".to_string())); // toolDoneLine("Grep", "{\"pattern\":\"foo\\nbar\"}")
        assert_eq!(tool_activity_line(Some("WebFetch"), "{\"url\":\"https://x\"}"), "fetching https://x"); // toolActivityLine("WebFetch", "{\"url\":\"https://x\"}")
        assert_eq!(tool_done_line(Some("WebFetch"), "{\"url\":\"https://x\"}"), Some("fetched https://x".to_string())); // toolDoneLine("WebFetch", "{\"url\":\"https://x\"}")
        assert_eq!(tool_activity_line(Some("Task"), "{\"description\":\"do it\"}"), "agent: do it"); // toolActivityLine("Task", "{\"description\":\"do it\"}")
        assert_eq!(tool_done_line(Some("Task"), "{\"description\":\"do it\"}"), None); // toolDoneLine("Task", "{\"description\":\"do it\"}")
        assert_eq!(tool_activity_line(Some("spawn_agent"), "{\"prompt\":\"child\"}"), "agent: child"); // toolActivityLine("spawn_agent", "{\"prompt\":\"child\"}")
        assert_eq!(tool_done_line(Some("spawn_agent"), "{\"prompt\":\"child\"}"), None); // toolDoneLine("spawn_agent", "{\"prompt\":\"child\"}")
        assert_eq!(tool_activity_line(Some("AskUserQuestion"), "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}"), "Which?"); // toolActivityLine("AskUserQuestion", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}")
        assert_eq!(tool_done_line(Some("AskUserQuestion"), "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}"), None); // toolDoneLine("AskUserQuestion", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}")
        assert_eq!(tool_activity_line(Some("mcp__wsp__run"), "{}"), "Use wsp's run"); // toolActivityLine("mcp__wsp__run", "{}")
        assert_eq!(tool_done_line(Some("mcp__wsp__run"), "{}"), None); // toolDoneLine("mcp__wsp__run", "{}")
        assert_eq!(tool_activity_line(Some("Unknown"), "{}"), "Unknown"); // toolActivityLine("Unknown", "{}")
        assert_eq!(tool_done_line(Some("Unknown"), "{}"), None); // toolDoneLine("Unknown", "{}")
        assert_eq!(tool_activity_line(None, "{}"), "tool"); // toolActivityLine(undefined, "{}")
        assert_eq!(tool_done_line(None, "{}"), None); // toolDoneLine(undefined, "{}")
    }

    #[test]
    fn tool_call_facts_keep_the_ts_key_order() {
        assert_eq!(js(&tool_call_facts("Bash", "{\"command\":\"ls\",\"description\":\"list\"}")), "{\"itemType\":\"command_execution\",\"requestKind\":\"command\",\"detail\":\"list\",\"command\":\"ls\",\"description\":\"list\"}"); // JSON.stringify(toolCallFacts("Bash", "{\"command\":\"ls\",\"description\":\"list\"}"))
        assert_eq!(js(&tool_call_facts("Bash", "")), "{\"itemType\":\"command_execution\",\"requestKind\":\"command\"}"); // JSON.stringify(toolCallFacts("Bash", ""))
        assert_eq!(
            js(&tool_call_facts("Bash", "{partial")),
            "{\"itemType\":\"command_execution\",\"requestKind\":\"command\",\"detail\":\"{partial\"}"
        ); // JSON.stringify(toolCallFacts("Bash", "{partial"))
        assert_eq!(
            js(&tool_call_facts("Edit", "{\"file_path\":\"/a\",\"old_string\":\"x\"}")),
            "{\"itemType\":\"file_change\",\"requestKind\":\"file-change\",\"detail\":\"/a\",\"changedFiles\":[\"/a\"]}"
        ); // JSON.stringify(toolCallFacts("Edit", "{\"file_path\":\"/a\",\"old_string\":\"x\"}"))
        assert_eq!(
            js(&tool_call_facts("file_change", "{\"changes\":[{\"path\":\"/a\"},{\"path\":\"/b\"}]}")),
            "{\"itemType\":\"file_change\",\"requestKind\":\"file-change\",\"changedFiles\":[\"/a\",\"/b\"]}"
        ); // JSON.stringify(toolCallFacts("file_change", "{\"changes\":[{\"path\":\"/a\"},{\"path\":\"/b\"}]}"))
        assert_eq!(
            js(&tool_call_facts("mcp__srv__do", "{\"title\":\"T\",\"message\":\"M\"}")),
            "{\"itemType\":\"mcp_tool_call\",\"title\":\"Use srv's do\",\"detail\":\"T\"}"
        ); // JSON.stringify(toolCallFacts("mcp__srv__do", "{\"title\":\"T\",\"message\":\"M\"}"))
        assert_eq!(
            js(&tool_call_facts("mcp__srv__do", "{\"message\":\"M\"}")),
            "{\"itemType\":\"mcp_tool_call\",\"title\":\"Use srv's do\",\"detail\":\"M\"}"
        ); // JSON.stringify(toolCallFacts("mcp__srv__do", "{\"message\":\"M\"}"))
        assert_eq!(js(&tool_call_facts("Unknown", "{\"url\":\"u\",\"description\":\"d\"}")), "{\"detail\":\"u\"}"); // JSON.stringify(toolCallFacts("Unknown", "{\"url\":\"u\",\"description\":\"d\"}"))
        assert_eq!(js(&tool_call_facts("AskUserQuestion", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}")), "{\"title\":\"Asked you\"}"); // JSON.stringify(toolCallFacts("AskUserQuestion", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}"))
        assert!(is_code_search_tool(Some("Grep"))); // isCodeSearchTool("Grep")
        assert!(is_code_search_tool(Some("Glob"))); // isCodeSearchTool("Glob")
        assert!(!is_code_search_tool(Some("Read"))); // isCodeSearchTool("Read")
        assert!(!is_code_search_tool(None)); // isCodeSearchTool(undefined)
    }

    #[test]
    fn results_answers_and_subagent_lines() {
        assert_eq!(tool_result_line("This tool result is internal metadata; x", false), None); // toolResultLine("This tool result is internal metadata; x")
        assert_eq!(tool_result_line("  \n", false), None); // toolResultLine("  \n")
        assert_eq!(tool_result_line("  \n", true), Some("failed".to_string())); // toolResultLine("  \n", true)
        assert_eq!(tool_result_line("\nfirst  line\nsecond", true), Some("failed: first line".to_string())); // toolResultLine("\nfirst  line\nsecond", true)
        assert_eq!(tool_answered_line(Some("Edit"), "{\"file_path\":\"/a\"}", "ok", None), Some("edited /a".to_string())); // toolAnsweredLine("Edit", "{\"file_path\":\"/a\"}", { text: "ok" })
        assert_eq!(tool_answered_line(Some("Edit"), "{\"file_path\":\"/a\"}", "bad", Some(true)), Some("failed: bad".to_string())); // toolAnsweredLine("Edit", "{\"file_path\":\"/a\"}", { text: "bad", isError: true })
        assert_eq!(tool_answered_line(Some("Bash"), "{\"command\":\"x\"}", "", None), None); // toolAnsweredLine("Bash", "{\"command\":\"x\"}", { text: "" })
        assert_eq!(subagent_task_line("{\"description\":\" a\\n b \"}"), Some("a".to_string())); // subagentTaskLine("{\"description\":\" a\\n b \"}")
        assert_eq!(subagent_task_line("nope"), None); // subagentTaskLine("nope")
        assert_eq!(subagent_prompt("{\"prompt\":\"  whole\\nprompt \"}"), Some("  whole\nprompt ".to_string()));
        // subagentPrompt("{\"prompt\":\"  whole\\nprompt \"}")
    }

    #[test]
    fn question_picks_encode_join_and_answer_as_the_ts_does() {
        assert_eq!(other_option_id(2, "a b|c:d/\u{e9}?&=+~*'()!.-_"), "q2:other:a%20b%7Cc%3Ad%2F%C3%A9%3F%26%3D%2B~*'()!.-_"); // otherOptionId(2, "a b|c:d/é?&=+~*'()!.-_")
        assert_eq!(js(&asked_questions("AskUserQuestion", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}")), "[{\"key\":\"Which?\",\"index\":0,\"header\":\"Pick\",\"question\":\"Which?\",\"options\":[{\"id\":\"q0:o0\",\"label\":\"A\",\"description\":\"first\"},{\"id\":\"q0:o1\",\"label\":\"B\",\"description\":\"\"}],\"multiSelect\":true},{\"key\":\"Second?\",\"index\":2,\"header\":\"\",\"question\":\"Second?\",\"options\":[{\"id\":\"q2:o0\",\"label\":\"Y\",\"description\":\"\"}],\"multiSelect\":false}]"); // JSON.stringify(askedQuestions("AskUserQuestion", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}") ?? null)
        assert_eq!(js(&asked_questions("AskUserQuestion", "{\"questions\":[]}")), "null"); // JSON.stringify(askedQuestions("AskUserQuestion", '{"questions":[]}') ?? null)
        assert_eq!(js(&asked_questions("Bash", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}")), "null"); // JSON.stringify(askedQuestions("Bash", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}") ?? null)
        assert_eq!(js(&question_options("AskUserQuestion", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}")), "[{\"id\":\"q0:o0\",\"label\":\"A\",\"effect\":\"answer\"},{\"id\":\"q0:o1\",\"label\":\"B\",\"effect\":\"answer\"},{\"id\":\"q2:o0\",\"label\":\"Y\",\"effect\":\"answer\"}]"); // JSON.stringify(questionOptions("AskUserQuestion", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}"))
        assert_eq!(js(&picked_options(&[opt_answer("q0:o0", "A"), opt_answer("q0:o1", "B")], "q0:o1|q0:other:hi%20there")), "[{\"id\":\"q0:o1\",\"label\":\"B\",\"effect\":\"answer\"},{\"id\":\"q0:other:hi%20there\",\"label\":\"hi there\",\"effect\":\"answer\"}]"); // JSON.stringify(pickedOptions([{id:"q0:o0",label:"A",effect:"answer"},{id:"q0:o1",label:"B",effect:"answer"}], "q0:o1|q0:other:hi%20there") ?? null)
        assert_eq!(js(&picked_options(&[opt_answer("q0:o0", "A")], "q0:other:%E0%A4%A")), "null"); // JSON.stringify(pickedOptions([{id:"q0:o0",label:"A",effect:"answer"}], "q0:other:%E0%A4%A") ?? null)
        assert_eq!(js(&picked_options(&[opt_allow("allow", "Allow")], "q0:other:x")), "null"); // JSON.stringify(pickedOptions([{id:"allow",label:"Allow",effect:"allow"}], "q0:other:x") ?? null)
        assert_eq!(js(&picked_options(&[], "")), "null"); // JSON.stringify(pickedOptions([], "") ?? null)
        assert_eq!(picked_option_id(&[] as &[&str]), ""); // pickedOptionId([])
        assert_eq!(picked_option_id(&["a", "b"]), "a|b"); // pickedOptionId(["a","b"])
        assert_eq!(
            js(&question_answer_input(
                "AskUserQuestion",
                "{\"answers\":{\"old\":1},\"questions\":[{\"question\":\"Q1\",\"options\":[{\"label\":\"A\"},{\"label\":\"B\"}]}]}",
                &["q0:o1", "q0:other:typed", "q9:o0"]
            )),
            "{\"answers\":{\"Q1\":\"B\"},\"questions\":[{\"question\":\"Q1\",\"options\":[{\"label\":\"A\"},{\"label\":\"B\"}]}]}"
        ); // JSON.stringify(questionAnswerInput("AskUserQuestion", "{\"answers\":{\"old\":1},\"questions\":[{\"question\":\"Q1\",\"options\":[{\"label\":\"A\"},{\"label\":\"B\"}]}]}", ["q0:o1", "q0:other:typed", "q9:o0"]) ?? null)
        assert_eq!(
            js(&question_answer_input(
                "AskUserQuestion",
                "{\"answers\":{\"old\":1},\"questions\":[{\"question\":\"Q1\",\"options\":[{\"label\":\"A\"},{\"label\":\"B\"}]}]}",
                &["zz"]
            )),
            "null"
        ); // JSON.stringify(questionAnswerInput("AskUserQuestion", "{\"answers\":{\"old\":1},\"questions\":[{\"question\":\"Q1\",\"options\":[{\"label\":\"A\"},{\"label\":\"B\"}]}]}", ["zz"]) ?? null)
    }

    #[test]
    fn every_ask_kind_leads_its_prompt_its_own_way() {
        assert_eq!(
            permission_ask_line("Write", "{\"file_path\":\"/a/b/c.txt\",\"content\":\"h\u{e9}llo\"}", Some("the detail")),
            "Write c.txt in b (6 B)"
        ); // permissionAskLine("Write", "{\"file_path\":\"/a/b/c.txt\",\"content\":\"héllo\"}", "the detail")
        assert_eq!(permission_ask_line("Write", "{\"file_path\":\"c.txt\"}", Some("the detail")), "Write c.txt"); // permissionAskLine("Write", "{\"file_path\":\"c.txt\"}", "the detail")
        assert_eq!(
            permission_ask_line("Edit", "{\"file_path\":\"/r/x.ts\",\"edits\":[1,2]}", Some("the detail")),
            "Edit x.ts in r (2 places)"
        ); // permissionAskLine("Edit", "{\"file_path\":\"/r/x.ts\",\"edits\":[1,2]}", "the detail")
        assert_eq!(permission_ask_line("Edit", "{\"file_path\":\"/r/x.ts\",\"edits\":[1]}", Some("the detail")), "Edit x.ts in r"); // permissionAskLine("Edit", "{\"file_path\":\"/r/x.ts\",\"edits\":[1]}", "the detail")
        assert_eq!(permission_ask_line("Bash", "{\"command\":\"rm -rf x\"}", Some("the detail")), "Run a command: rm -rf x"); // permissionAskLine("Bash", "{\"command\":\"rm -rf x\"}", "the detail")
        assert_eq!(
            permission_ask_line("command_execution", "{\"command\":\"ls\",\"cwd\":\"/w\"}", Some("the detail")),
            "Run a command: ls"
        ); // permissionAskLine("command_execution", "{\"command\":\"ls\",\"cwd\":\"/w\"}", "the detail")
        assert_eq!(permission_ask_line("file_change", "{\"changes\":[{\"path\":\"/a/b\"}]}", Some("the detail")), "Change b in a"); // permissionAskLine("file_change", "{\"changes\":[{\"path\":\"/a/b\"}]}", "the detail")
        assert_eq!(
            permission_ask_line("file_change", "{\"changes\":[{\"path\":\"/a/b\"},{\"path\":\"/c/d\"},{\"nope\":1}]}", Some("the detail")),
            "Change 2 files: b, d"
        ); // permissionAskLine("file_change", "{\"changes\":[{\"path\":\"/a/b\"},{\"path\":\"/c/d\"},{\"nope\":1}]}", "the detail")
        assert_eq!(permission_ask_line("WebFetch", "{\"url\":\"https://e\"}", Some("the detail")), "Fetch a page: https://e"); // permissionAskLine("WebFetch", "{\"url\":\"https://e\"}", "the detail")
        assert_eq!(permission_ask_line("Skill", "{\"skill\":\"unslop\"}", Some("the detail")), "Run the skill unslop"); // permissionAskLine("Skill", "{\"skill\":\"unslop\"}", "the detail")
        assert_eq!(permission_ask_line("mcp__wsp__run", "{}", Some("the detail")), "Use wsp's run"); // permissionAskLine("mcp__wsp__run", "{}", "the detail")
        assert_eq!(permission_ask_line("Weird", "{}", Some("the detail")), "Permission for Weird: the detail"); // permissionAskLine("Weird", "{}", "the detail")
        assert_eq!(permission_ask_line("Weird", "not json", Some("the detail")), "Permission for Weird: the detail"); // permissionAskLine("Weird", "not json", "the detail")
        assert_eq!(permission_ask_line("Weird", "{}", Some("")), "Permission for Weird"); // permissionAskLine("Weird", "{}", "")
        assert_eq!(asking_line(&ask("Bash", "{\"command\":\"ls\"}", Some("d"))), "Run a command: ls");
        // askingLine({toolName:"Bash",input:"{\"command\":\"ls\"}",detail:"d"})
    }

    #[test]
    fn prompt_words_carry_the_rest_body_note_and_edit() {
        assert_eq!(js(&permission_prompt_words("Write", "{\"file_path\":\"/a/b.txt\",\"content\":\"body\",\"mode\":420,\"extra\":\"  two\\n lines \"}", None)), "{\"lead\":\"Write b.txt in a (4 B)\",\"says\":\"Write b.txt in a (4 B)\",\"rest\":\"mode: 420\\nextra: two lines\",\"body\":{\"label\":\"show the file\",\"text\":\"body\"},\"file\":\"/a/b.txt\"}"); // JSON.stringify(permissionPromptWords("Write", "{\"file_path\":\"/a/b.txt\",\"content\":\"body\",\"mode\":420,\"extra\":\"  two\\n lines \"}", undefined))
        assert_eq!(js(&permission_prompt_words("Edit", "{\"file_path\":\"/a/b.txt\",\"edits\":[{\"old_string\":\"\\n\",\"new_string\":\"\"},{\"old_string\":\"x\",\"new_string\":\"y\"}],\"replace_all\":true,\"z\":[1,\"a\"],\"o\":{\"k\":[1,{\"n\":null}]}}", None)), "{\"lead\":\"Edit b.txt in a (2 places)\",\"says\":\"Edit b.txt in a (2 places)\",\"rest\":\"z: 1, a\\no: {\\\"k\\\":[1,{\\\"n\\\":null}]}\",\"file\":\"/a/b.txt\",\"edit\":{\"old\":\"\\n\",\"next\":\"\"}}"); // JSON.stringify(permissionPromptWords("Edit", "{\"file_path\":\"/a/b.txt\",\"edits\":[{\"old_string\":\"\\n\",\"new_string\":\"\"},{\"old_string\":\"x\",\"new_string\":\"y\"}],\"replace_all\":true,\"z\":[1,\"a\"],\"o\":{\"k\":[1,{\"n\":null}]}}", undefined))
        assert_eq!(js(&permission_prompt_words("command_execution", "{\"command\":\"ls\",\"cwd\":\"/w\",\"reason\":\"x\"}", Some("because I must"))), "{\"lead\":\"Run a command: ls\",\"says\":\"Run a command:\",\"code\":\"ls\",\"rest\":\"reason: x\",\"note\":\"because I must\"}"); // JSON.stringify(permissionPromptWords("command_execution", "{\"command\":\"ls\",\"cwd\":\"/w\",\"reason\":\"x\"}", "because I must"))
        assert_eq!(
            js(&permission_prompt_words("command_execution", "{\"command\":\"ls\"}", Some(""))),
            "{\"lead\":\"Run a command: ls\",\"says\":\"Run a command:\",\"code\":\"ls\",\"rest\":\"\"}"
        ); // JSON.stringify(permissionPromptWords("command_execution", "{\"command\":\"ls\"}", ""))
        assert_eq!(js(&permission_prompt_words("WebFetch", "{\"url\":\"https://e\",\"prompt\":\"read the docs\"}", None)), "{\"lead\":\"Fetch a page: https://e\",\"says\":\"Fetch a page:\",\"code\":\"https://e\",\"rest\":\"\",\"note\":\"read the docs\"}"); // JSON.stringify(permissionPromptWords("WebFetch", "{\"url\":\"https://e\",\"prompt\":\"read the docs\"}", undefined))
        assert_eq!(js(&permission_prompt_words("AskUserQuestion", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}", None)), "{\"lead\":\"Which?\",\"says\":\"Which?\",\"rest\":\"\",\"questions\":[{\"key\":\"Which?\",\"index\":0,\"header\":\"Pick\",\"question\":\"Which?\",\"options\":[{\"id\":\"q0:o0\",\"label\":\"A\",\"description\":\"first\"},{\"id\":\"q0:o1\",\"label\":\"B\",\"description\":\"\"}],\"multiSelect\":true},{\"key\":\"Second?\",\"index\":2,\"header\":\"\",\"question\":\"Second?\",\"options\":[{\"id\":\"q2:o0\",\"label\":\"Y\",\"description\":\"\"}],\"multiSelect\":false}]}"); // JSON.stringify(permissionPromptWords("AskUserQuestion", "{\"questions\":[{\"question\":\"Which?\",\"header\":\"Pick\",\"options\":[{\"label\":\"A\",\"description\":\"first\"},{\"label\":\"B\"},{\"nolabel\":1},\"junk\"],\"multiSelect\":true},{\"question\":\"  \"},{\"question\":\"Second?\",\"options\":[{\"label\":\"Y\"}]}],\"other\":1}", undefined))
        assert_eq!(
            js(&permission_prompt_words("Bash", "not json", Some("d"))),
            "{\"lead\":\"Permission for Bash: d\",\"says\":\"Permission for Bash: d\",\"rest\":\"not json\"}"
        ); // JSON.stringify(permissionPromptWords("Bash", "not json", "d"))
        assert_eq!(
            js(&permission_prompt_words("Bash", "{\"command\":\"ls\",\"tags\":[\"a\",1,true]}", None)),
            "{\"lead\":\"Run a command: ls\",\"says\":\"Run a command:\",\"code\":\"ls\",\"rest\":\"tags: [\\\"a\\\",1,true]\"}"
        ); // JSON.stringify(permissionPromptWords("Bash", "{\"command\":\"ls\",\"tags\":[\"a\",1,true]}", undefined))
    }

    #[test]
    fn outcome_denied_and_access_lines() {
        assert_eq!(permission_outcome_line(PermissionOutcome::Allowed, None), "Allowed"); // permissionOutcomeLine("allowed", undefined)
        assert_eq!(permission_outcome_line(PermissionOutcome::Allowed, Some(("Allow all", PermissionEffect::Mode))), "Allowed: Allow all"); // permissionOutcomeLine("allowed", {"label":"Allow all","effect":"mode"})
        assert_eq!(
            permission_outcome_line(PermissionOutcome::Allowed, Some(("Option A", PermissionEffect::Answer))),
            "You answered: Option A"
        ); // permissionOutcomeLine("allowed", {"label":"Option A","effect":"answer"})
        assert_eq!(permission_outcome_line(PermissionOutcome::Denied, Some(("No", PermissionEffect::Mode))), "Denied: No"); // permissionOutcomeLine("denied", {"label":"No","effect":"mode"})
        assert_eq!(permission_outcome_line(PermissionOutcome::Denied, Some(("No", PermissionEffect::Deny))), "Denied"); // permissionOutcomeLine("denied", {"label":"No","effect":"deny"})
        assert_eq!(permission_outcome_line(PermissionOutcome::Unanswered, None), "Nobody answered; denied"); // permissionOutcomeLine("unanswered", undefined)
        assert_eq!(permission_outcome_line(PermissionOutcome::Cancelled, Some(("x", PermissionEffect::Mode))), "Cancelled with the turn"); // permissionOutcomeLine("cancelled", {"label":"x","effect":"mode"})
        assert_eq!(denied_line(None), "the person denied this in the chat"); // deniedLine(undefined)
        assert_eq!(denied_line(Some("")), "the person denied this in the chat"); // deniedLine("")
        assert_eq!(denied_line(Some("   ")), "the person denied this in the chat"); // deniedLine("   ")
        assert_eq!(
            denied_line(Some("  do this instead  ")),
            "the person denied this in the chat, and said what to do instead: do this instead"
        ); // deniedLine("  do this instead  ")
        assert_eq!(permission_mode_option_label("plan"), "Allow, then plan"); // permissionModeOptionLabel("plan")
        assert_eq!(access_reach_line(true), "Applies to the turn running now"); // accessReachLine(true)
        assert_eq!(access_reach_line(false), "Applies from the thread's next turn");
        // accessReachLine(false)
    }
}
