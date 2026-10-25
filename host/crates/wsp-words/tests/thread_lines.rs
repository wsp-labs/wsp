// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/format.test.ts:798-806 (IDLE_REASON), :888-900 (turnCutLine, timedOutLine,
//! stepRetryLine), :933-954 (validatorRefusal), :956-1020 (harnessExitLine, loginPathLine, guestUnusableLine,
//! machineUnreachableLine, execFailedLine, napRefusedLine, machineUnreachedLine), :1127-1186 (mcpServerCommandLine,
//! nextInsideAgentLine, folderRefusalLine, execFolderLine, backgroundTasksLine, taskFinishedLine,
//! providerRoadRetryLine), :1199-1250 (stillWorkingLine, stopFailedLine, sendNowFailedLine, refusalLine, sayOnce,
//! unknownWordLine, runForTheList, foreignFlagLine), :1673-1676 (unknownAgentLine) and redact.test.ts:6-21 (redacted),
//! :39-47 (issuesLine). Skipped: redact.test.ts:25 and :32 (test requestSecrets, absent from the stubs, and redacted is only used incidentally),
//! workspace-state.test.ts:81 (tests sendRefusal in workspace-state, stillWorkingLine is incidental),
//! protocol.test.ts:2188 and exit.test.ts:38 (test the wire's spawn refusals and exit.ts's refusal, refusalLine is
//! incidental). Doubt: timed_out_line takes `times: Option<&str>` where the TS takes a number (`timedOutLine(300, 2)`).

use wsp_words::words::thread::*;

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

mod idle_reason {
    #[test]
    fn writes_the_nap_s_reason_and_reads_its_window_back_and_reads_nothing_out_of_a_rea() {
        use wsp_words::words::thread::idle_reason as reason;
        assert_eq!(reason::of(20.0 * 60_000.0), "idle 20 min");
        assert_eq!(reason::window_in(Some(&reason::of(20.0 * 60_000.0))), Some("20 min".to_string()));
        assert_eq!(reason::window_in(Some(&reason::of(30.0 * 60_000.0))), Some("30 min".to_string()));
        assert_eq!(reason::window_in(None), None);
        assert_eq!(reason::window_in(Some("machine replaced")), None);
        assert_eq!(reason::window_in(Some("idle")), None);
    }
}

mod turn_cut_line {
    use super::*;

    #[test]
    fn names_the_rule_how_long_the_turn_ran_in_the_clock_style_and_the_limit_in_the_wor() {
        const TURN_IDLE_MS: f64 = 600_000.0;
        const TURN_WALL_MS: f64 = 21_600_000.0;
        assert_eq!(turn_cut_line(TurnCutRule::Idle, 900_000.0, TURN_IDLE_MS), "stopped after 15m 00s with no output for 10m");
        assert_eq!(
            turn_cut_line(TurnCutRule::Wall, TURN_WALL_MS, TURN_WALL_MS),
            "stopped after 6h 00m 00s at the 6h turn limit; send to continue where it stopped, or change the limit in Settings > Computers"
        );
    }
}

mod timed_out_line_and_step_retry_line {
    use super::*;

    #[test]
    fn names_the_seconds_says_when_it_happened_twice_and_says_in_words_that_the_step_is() {
        assert_eq!(timed_out_line(300.0, None), "timed out after 300s");
        assert_eq!(timed_out_line(300.0, Some(2.0)), "timed out after 300s, twice");
        assert_eq!(step_retry_line(300.0), "timed out after 300s; trying once more");
    }
}

mod a_refusal_the_host_s_own_validator_wrote {
    use super::*;

    fn issues(rows: serde_json::Value) -> String {
        serde_json::to_string_pretty(&rows).unwrap()
    }

    #[test]
    fn reads_an_op_the_host_does_not_know_as_the_two_builds_differing_and_never_prints_() {
        let refusal = issues(serde_json::json!([{ "code": "invalid_union_discriminator", "options": ["auth", "status.list", "workspaces.exec"], "path": ["op"], "message": "Invalid discriminator value. Expected 'auth' | 'status.list' | 'workspaces.exec'" }]));
        let line = validator_refusal(&refusal);
        assert_eq!(line, Some("the host does not serve this line; it runs another version of wsp, restart it with wsp restart".to_string()));
        let line = line.unwrap();
        assert!(!line.contains("workspaces.exec"), "{line}");
        assert!(!line.contains("discriminator"), "{line}");
    }

    #[test]
    fn names_the_argument_the_host_refused_in_the_words_the_line_was_typed_in() {
        assert_eq!(
            validator_refusal(&issues(serde_json::json!([{ "code": "invalid_type", "expected": "string", "received": "number", "path": ["cwd"], "message": "Expected string, received number" }]))),
            Some("the host would not read --cwd on this line".to_string())
        );
        assert_eq!(
            validator_refusal(&issues(serde_json::json!([{ "code": "invalid_type", "path": ["workspaceId"], "message": "Required" }, { "code": "invalid_type", "path": ["argv", 0], "message": "Required" }]))),
            Some("the host would not read the command on this line".to_string())
        );
    }

    #[test]
    fn says_the_line_alone_where_the_field_it_named_is_one_no_line_carries() {
        assert_eq!(
            validator_refusal(&issues(serde_json::json!([{ "code": "invalid_type", "path": ["turnToken"], "message": "Required" }]))),
            Some("the host would not read this line".to_string())
        );
    }

    #[test]
    fn leaves_every_refusal_wsp_writes_itself_alone() {
        assert_eq!(validator_refusal("no workspace nope"), None);
        assert_eq!(validator_refusal("[]"), None);
        assert_eq!(validator_refusal(&serde_json::json!([{ "message": "Required" }]).to_string()), None);
        assert_eq!(validator_refusal(&serde_json::json!({ "code": "invalid_type", "path": ["cwd"] }).to_string()), None);
    }
}

mod harness_exit_line {
    use super::*;

    #[test]
    fn exit_127_names_the_binary_the_shell_could_not_find_and_the_path_it_searched_neve() {
        let path = "/root/.local/bin:/usr/bin:/bin";
        assert_eq!(harness_exit_line("claude", Some(127), Some(path), None), "claude was not found on PATH (exit 127); PATH searched: /root/.local/bin:/usr/bin:/bin");
        assert_eq!(harness_exit_line("codex", Some(127), Some(path), None), "codex was not found on PATH (exit 127); PATH searched: /root/.local/bin:/usr/bin:/bin");
    }

    #[test]
    fn exit_127_with_no_path_exported_says_the_machine_s_own_was_searched() {
        assert_eq!(harness_exit_line("claude", Some(127), None, None), "claude was not found on PATH (exit 127); the launch exported no PATH, the machine's own was searched");
    }

    #[test]
    fn any_other_exit_reads_as_the_code() {
        assert_eq!(harness_exit_line("claude", Some(1), Some("/usr/bin"), None), "claude exited with code 1 before emitting a result");
    }

    #[test]
    fn a_run_that_ended_on_a_signal_says_the_agent_was_killed_and_names_the_signal_wher() {
        let ended = |signal: Option<&str>| HarnessEnded { signal: signal.map(str::to_string), reached: Some(true) };
        assert_eq!(harness_exit_line("claude", None, Some("/usr/bin"), Some(&ended(Some("SIGKILL")))), "claude was killed (SIGKILL) before it answered");
        assert_eq!(harness_exit_line("claude", Some(-1), Some("/usr/bin"), Some(&ended(Some("SIGTERM")))), "claude was killed (SIGTERM) before it answered");
        assert_eq!(harness_exit_line("claude", None, Some("/usr/bin"), Some(&ended(None))), "claude was killed before it answered");
    }

    #[test]
    fn a_launch_that_never_reached_the_agent_says_that_instead_of_naming_a_code() {
        let never = HarnessEnded { signal: None, reached: Some(false) };
        assert_eq!(harness_exit_line("claude", None, Some("/usr/bin"), Some(&never)), "the launch never reached claude: its run ended before the agent said a word");
        assert_eq!(harness_exit_line("claude", Some(127), Some("/usr/bin"), Some(&never)), "claude was not found on PATH (exit 127); PATH searched: /usr/bin");
    }
}

mod login_path_line {
    use super::*;

    #[test]
    fn names_why_the_login_shell_gave_no_path_and_says_the_one_the_launch_handed_the_ho() {
        assert_eq!(login_path_line("/bin/zsh printed nothing"), "login shell: no PATH read (/bin/zsh printed nothing); this host keeps the PATH it was started with");
        assert_eq!(login_path_line("SHELL names no login shell"), "login shell: no PATH read (SHELL names no login shell); this host keeps the PATH it was started with");
    }
}

mod guest_unusable_line {
    use super::*;

    #[test]
    fn names_the_provider_the_machine_it_left_running_and_what_the_guest_said_when_noth() {
        assert_eq!(
            guest_unusable_line("Boat", "bx_tumrjngm", "bash: error while loading shared libraries: libtinfo.so.6: cannot open shared object file: Error 24"),
            "Boat left bx_tumrjngm running but nothing on it can run: bash: error while loading shared libraries: libtinfo.so.6: cannot open shared object file: Error 24"
        );
    }
}

mod machine_unreachable_line {
    use super::*;

    #[test]
    fn quotes_the_provider_s_words_and_names_the_two_roads_open_on_a_running_machine_wi() {
        assert_eq!(
            machine_unreachable_line("Sandbox is not reachable"),
            "the provider cannot reach the machine (Sandbox is not reachable); pause and wake the workspace, or delete it"
        );
    }
}

mod exec_failed_line {
    use super::*;

    #[test]
    fn quotes_the_provider_s_words_says_the_threads_may_still_be_running_and_puts_waiti() {
        assert_eq!(
            exec_failed_line("exec failed"),
            "the provider cannot run commands on the machine (exec failed) while the machine and its threads may still be running; wait, or pause and wake the workspace, or delete it"
        );
    }
}

mod nap_refused_line {
    use super::*;

    #[test]
    fn quotes_the_provider_s_words_says_why_it_will_not_pause_and_names_the_road_left() {
        assert_eq!(
            nap_refused_line("Not pausable"),
            "the provider does not pause this machine (Not pausable): its memory and disk together are over what it pauses; it runs until you delete it"
        );
    }
}

mod machine_unreached_line {
    use super::*;

    #[test]
    fn says_the_machine_could_not_be_reached_from_this_computer_with_the_attempts_count() {
        assert_eq!(machine_unreached_line(6.0, 23_400.0), "the machine could not be reached from this computer after 6 attempts over 23s");
        assert_eq!(machine_unreached_line(1.0, 800.0), "the machine could not be reached from this computer after 1 attempt over 800ms");
    }
}

mod mcp_server_command_line {
    use super::*;

    #[test]
    fn names_the_command_every_agent_s_config_now_runs_as_one_shell_line_a_person_can_p() {
        assert_eq!(
            mcp_server_command_line("npx", &["-y", "@wsp-labs/wsp@0.1.2", "mcp", "--state", "/Users/p/.wsp/state.json"]),
            "The server command is npx -y @wsp-labs/wsp@0.1.2 mcp --state /Users/p/.wsp/state.json"
        );
        assert_eq!(
            mcp_server_command_line("/Users/p/.local/bin/wsp", &["mcp", "--state", "/Users/p/my wsp/state.json"]),
            "The server command is /Users/p/.local/bin/wsp mcp --state '/Users/p/my wsp/state.json'"
        );
    }
}

mod next_inside_agent_line {
    use super::*;

    #[test]
    fn names_the_agent_s_own_command_and_what_to_type_at_its_prompt_a_slash_form_as_it_() {
        assert_eq!(next_inside_agent_line("claude", "/wsp set up wsp for me"), "Next: run claude in this folder and say: /wsp set up wsp for me");
        assert_eq!(next_inside_agent_line("codex", "set up wsp for me"), "Next: run codex in this folder and say: set up wsp for me");
    }
}

mod folder_refusal_line {
    use super::*;

    #[test]
    fn carries_this_mac_s_own_reason_after_the_words_that_say_the_level_was_not_read() {
        assert_eq!(
            folder_refusal_line("EACCES: permission denied, scandir '/Users/dev/Documents'"),
            "No folders read. EACCES: permission denied, scandir '/Users/dev/Documents'"
        );
        assert_eq!(
            folder_refusal_line("/etc is outside the folders wsp browses on this computer: /Users/dev"),
            "No folders read. /etc is outside the folders wsp browses on this computer: /Users/dev"
        );
    }

    #[test]
    fn is_one_line_whatever_the_host_said_so_the_slot_the_level_line_shares_keeps_its_h() {
        assert_eq!(
            folder_refusal_line("  EPERM: operation not permitted\n  scandir '/Users/dev/Desktop'  "),
            "No folders read. EPERM: operation not permitted scandir '/Users/dev/Desktop'"
        );
    }
}

mod exec_folder_line {
    use super::*;

    #[test]
    fn names_the_folder_a_failing_command_ran_in_or_the_home_folder_when_it_had_none_of() {
        assert_eq!(exec_folder_line(Some("/root/work/proj")), "ran in /root/work/proj");
        assert_eq!(exec_folder_line(None), "ran in the home folder");
    }
}

mod background_tasks_line {
    use super::*;

    #[test]
    fn counts_the_tasks_the_harness_still_had_running_when_its_process_was_cut() {
        assert_eq!(background_tasks_line(1.0), "ended with 1 background task running");
        assert_eq!(background_tasks_line(2.0), "ended with 2 background tasks running");
    }
}

mod task_finished_line {
    use super::*;

    #[test]
    fn names_the_command_how_it_ended_and_how_long_after_the_reply_in_the_one_duration_() {
        assert_eq!(task_finished_line("pnpm test", "completed", 14.0 * 60_000.0), "`pnpm test` completed, 14m after the reply");
        // Under a minute it is seconds, as a turn's own duration is: a task that ended in twenty seconds says so
        // rather than rounding to nothing.
        assert_eq!(task_finished_line("Sleep 20 seconds then echo done", "completed", 20_400.0), "`Sleep 20 seconds then echo done` completed, 20s after the reply");
        assert_eq!(task_finished_line("gate", "failed", 90_000.0), "`gate` failed, 1m 30s after the reply");
    }

    #[test]
    fn keeps_a_description_the_harness_wrote_with_backticks_in_it_whole_since_the_agent() {
        assert_eq!(task_finished_line("echo `date`", "completed", 1_000.0), "`echo `date`` completed, 1.0s after the reply");
    }
}

mod a_provider_out_of_reach_from_this_computer {
    use super::*;

    #[test]
    fn logs_one_retry_per_line_naming_the_call_the_road_s_own_code_and_the_try_about_to() {
        assert_eq!(provider_road_retry_line("GET /sandboxes/x", "ENOTFOUND", 2.0, 3.0), "GET /sandboxes/x did not leave this computer (ENOTFOUND); try 2 of 3");
        assert_eq!(provider_road_retry_line("POST /sandboxes", "EAI_AGAIN", 3.0, 3.0), "POST /sandboxes did not leave this computer (EAI_AGAIN); try 3 of 3");
    }
}

mod still_working_line {
    use super::*;

    #[test]
    fn names_the_thread_by_its_title_says_the_reply_is_in_but_the_agent_is_still_workin() {
        assert_eq!(
            still_working_line(Some("Fix the port list")),
            "Fix the port list replied, still working; the message runs as its next turn once that process exits"
        );
        // A caller with no title for the thread names it as the person looking at it would: an id names no thread.
        assert_eq!(still_working_line(None), "This thread replied, still working; the message runs as its next turn once that process exits");
        assert_eq!(still_working_line(Some("  ")), still_working_line(None));
        // Nothing in it tells the caller to wait or says the send was refused: the send is never refused.
        let line = still_working_line(None);
        assert!(!regex::Regex::new("wait|refus").unwrap().is_match(&line), "{line}");
    }
}

mod stop_failed_line_and_send_now_failed_line {
    use super::*;

    #[test]
    fn prefix_the_runtime_s_own_words_for_a_stop_or_a_send_now_that_did_not_go_so_the_c() {
        assert_eq!(stop_failed_line("the runtime does not know this session"), "Could not stop: the runtime does not know this session");
        assert_eq!(send_now_failed_line("Workspace is pausing; wake it to send"), "Could not send now: Workspace is pausing; wake it to send");
    }
}

mod the_shape_a_refusal_takes_on_a_terminal {
    use super::*;

    #[test]
    fn is_two_halves_what_happened_and_then_what_to_do_on_one_line() {
        assert_eq!(
            refusal_line("wsp run takes one task; api reads as a second one.", "Name the workspace with --in api."),
            "wsp run takes one task; api reads as a second one. Name the workspace with --in api."
        );
        assert_eq!(refusal_line("a.", "b.").split('\n').count(), 1);
        // A first half written for the app too, where nothing follows it, closes itself here.
        assert_eq!(
            refusal_line("no agent called codx; the catalog knows claude, codex", "Name one of those."),
            "no agent called codx; the catalog knows claude, codex. Name one of those."
        );
        assert_eq!(
            refusal_line("wsp thread read takes one thread. ", "usage: wsp thread read <thread>"),
            "wsp thread read takes one thread. usage: wsp thread read <thread>"
        );
    }

    #[test]
    fn says_the_command_s_name_once_whether_or_not_the_sentence_opens_with_it() {
        assert_eq!(say_once("wsp run: ", "wsp run takes one task"), "wsp run takes one task");
        assert_eq!(say_once("wsp delete: ", "no workspace nope"), "wsp delete: no workspace nope");
        assert_eq!(say_once("", "no workspace nope"), "no workspace nope");
        // The name is matched whole: a sentence that merely opens with the same letters keeps its prefix.
        assert_eq!(say_once("wsp stop: ", "wsp stopped nothing"), "wsp stop: wsp stopped nothing");
        assert_eq!(say_once("wsp stop: ", "wsp stop"), "wsp stop");
    }

    #[test]
    fn answers_a_word_no_command_has_with_the_word_and_where_the_list_is_and_never_with() {
        assert_eq!(refusal_line(&unknown_word_line("ls"), &run_for_the_list("wsp --help")), "unknown command: ls. Run wsp --help for the list.");
        assert_eq!(refusal_line(&unknown_word_line("ls"), &run_for_the_list("wsp --help")).split('\n').count(), 1);
    }
}

mod foreign_flag_line {
    use super::*;

    #[test]
    fn names_the_verb_or_verbs_that_read_the_flag_then_the_one_that_does_not() {
        assert_eq!(foreign_flag_line("--tick", &["wsp recipe"], "wsp recipe scan"), "--tick belongs to wsp recipe; wsp recipe scan does not read it");
        assert_eq!(foreign_flag_line("--agent", &["wsp fork", "wsp run"], "wsp send"), "--agent belongs to wsp fork and wsp run; wsp send does not read it");
    }
}

mod unknown_agent_line {
    use super::*;

    #[test]
    fn names_the_id_nobody_knows_and_the_ids_the_catalog_does_so_a_typo_is_a_sentence() {
        assert_eq!(unknown_agent_line("codx", &strs(&["claude", "codex"])), "no agent called codx; the catalog knows claude, codex");
    }
}

mod redacted {
    use super::*;

    #[test]
    fn blanks_every_hidden_value_wherever_it_appears_and_its_trimmed_form_and_skips_emp() {
        assert_eq!(redacted("key sk-ant-x-1 then sk-ant-x-1 again", &["sk-ant-x-1"]), "key <redacted> then <redacted> again");
        assert_eq!(redacted("the door read sk-ant-x-2.", &["  sk-ant-x-2\n"]), "the door read <redacted>.");
        assert_eq!(redacted("nothing here", &["", "   "]), "nothing here");
    }

    #[test]
    fn never_blanks_a_value_shorter_than_four_characters_once_trimmed_so_a_stray_space_() {
        assert_eq!(redacted("unauthorized: ws-80 refused", &[" ", "u", "80", " ab "]), "unauthorized: ws-80 refused");
        assert_eq!(redacted("the key abcd was refused", &["abcd"]), "the key <redacted> was refused");
    }

    #[test]
    fn blanks_the_longest_value_first_so_a_shorter_one_that_is_its_prefix_leaves_no_tai() {
        let key = "sk-ant-x-probe-key-12345";
        assert_eq!(redacted(&format!("Box refused {key}"), &[&key[..12], key]), "Box refused <redacted>");
    }
}

mod issues_line {
    use super::*;

    #[test]
    fn flattens_a_schema_s_issues_to_one_line_of_path_and_message() {
        let key = |s: &str| PathPart::Key(s.to_string());
        let issues = vec![
            Issue { path: vec![key("provider")], message: "Expected string, received number".to_string() },
            Issue { path: vec![key("rows"), key("a")], message: "Expected string, received number".to_string() },
        ];
        let line = issues_line(&issues);
        assert_eq!(line, "provider: Expected string, received number; rows.a: Expected string, received number");
        assert!(!line.contains('\n'), "{line}");
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::words::thread::*;

    use wsp_wire::wire::capabilities::WorkspaceSize;

    #[test]
    fn limits_and_cut_turns() {
        assert_eq!(fmt_limit(0.0), "0m"); // fmtLimit(0)
        assert_eq!(fmt_limit(59999.0), "1m"); // fmtLimit(59999)
        assert_eq!(fmt_limit(60000.0), "1m"); // fmtLimit(60000)
        assert_eq!(fmt_limit(90000.0), "2m"); // fmtLimit(90000)
        assert_eq!(fmt_limit(3600000.0), "1h"); // fmtLimit(3600000)
        assert_eq!(fmt_limit(7200000.0), "2h"); // fmtLimit(7200000)
        assert_eq!(fmt_limit(5400000.0), "90m"); // fmtLimit(5400000)
        assert_eq!(fmt_limit(30000.0), "1m"); // fmtLimit(30000)
        assert_eq!(turn_cut_line(TurnCutRule::Idle, 600000.0, 600000.0), "stopped after 10m 00s with no output for 10m"); // turnCutLine("idle", 600000, 600000)
        assert_eq!(turn_cut_line(TurnCutRule::Wall, 3600000.0, 3600000.0), "stopped after 1h 00m 00s at the 1h turn limit; send to continue where it stopped, or change the limit in Settings > Computers"); // turnCutLine("wall", 3600000, 3600000)
        assert_eq!(timed_out_line(300.0, Some(3.0)), "timed out after 300s"); // timedOutLine(300, 3)
        assert_eq!(step_retry_line(120.0), "timed out after 120s; trying once more"); // stepRetryLine(120)
    }

    #[test]
    fn a_harness_that_ended_early_says_what_ended_it() {
        assert_eq!(harness_exit_line("claude", Some(127), None, None), "claude was not found on PATH (exit 127); the launch exported no PATH, the machine's own was searched"); // harnessExitLine("claude", 127, undefined)
        assert_eq!(harness_exit_line("claude", Some(127), Some("/usr/bin:/bin"), Some(&HarnessEnded { signal: Some("SIGKILL".into()), reached: None })), "claude was not found on PATH (exit 127); PATH searched: /usr/bin:/bin"); // harnessExitLine("claude", 127, "/usr/bin:/bin", {signal:"SIGKILL"})
        assert_eq!(harness_exit_line("claude", Some(1), None, Some(&HarnessEnded { signal: Some("SIGTERM".into()), reached: None })), "claude was killed (SIGTERM) before it answered"); // harnessExitLine("claude", 1, undefined, {signal:"SIGTERM"})
        assert_eq!(harness_exit_line("claude", Some(0), None, None), "claude exited with code 0 before emitting a result"); // harnessExitLine("claude", 0, undefined)
        assert_eq!(harness_exit_line("claude", None, None, Some(&HarnessEnded { signal: None, reached: Some(false) })), "the launch never reached claude: its run ended before the agent said a word"); // harnessExitLine("claude", null, undefined, {reached:false})
        assert_eq!(harness_exit_line("claude", None, None, Some(&HarnessEnded { signal: None, reached: Some(true) })), "claude was killed before it answered"); // harnessExitLine("claude", null, undefined, {reached:true})
        assert_eq!(harness_exit_line("claude", None, None, None), "claude was killed before it answered"); // harnessExitLine("claude", null, undefined)
    }

    #[test]
    fn the_validator_refusal_reads_only_an_issue_list() {
        assert_eq!(validator_refusal(""), None); // validatorRefusal("")
        assert_eq!(validator_refusal("not json"), None); // validatorRefusal("not json")
        assert_eq!(validator_refusal("{}"), None); // validatorRefusal("{}")
        assert_eq!(validator_refusal("[]"), None); // validatorRefusal("[]")
        assert_eq!(validator_refusal("[1]"), None); // validatorRefusal("[1]")
        assert_eq!(validator_refusal("[{\"code\":\"x\"}]"), None); // validatorRefusal("[{\"code\":\"x\"}]")
        assert_eq!(validator_refusal("[\n  {\n    \"code\": \"invalid_union_discriminator\",\n    \"path\": [\n      \"op\"\n    ],\n    \"message\": \"m\"\n  }\n]"), Some("the host does not serve this line; it runs another version of wsp, restart it with wsp restart".to_string())); // validatorRefusal("[\n  {\n    \"code\": \"invalid_union_discriminator\",\n    \"path\": [\n      \"op\"\n    ],\n    \"message\": \"m\"\n  }\n]")
        assert_eq!(validator_refusal("[\n  {\n    \"code\": \"invalid_union_discriminator\",\n    \"path\": [\n      \"op\"\n    ]\n  },\n  {\n    \"code\": \"invalid_type\",\n    \"path\": [\n      \"threadId\"\n    ]\n  }\n]"), Some("the host would not read the thread on this line".to_string())); // validatorRefusal("[\n  {\n    \"code\": \"invalid_union_discriminator\",\n    \"path\": [\n      \"op\"\n    ]\n  },\n  {\n    \"code\": \"invalid_type\",\n    \"path\": [\n      \"threadId\"\n    ]\n  }\n]")
        assert_eq!(validator_refusal("[\n  {\n    \"code\": \"invalid_type\",\n    \"path\": [\n      \"cwd\",\n      0\n    ]\n  },\n  {\n    \"code\": \"invalid_type\",\n    \"path\": [\n      \"sessionId\"\n    ]\n  },\n  {\n    \"code\": \"invalid_type\",\n    \"path\": [\n      \"threadId\"\n    ]\n  },\n  {\n    \"code\": \"too_big\",\n    \"path\": [\n      \"prompt\"\n    ]\n  }\n]"), Some("the host would not read --cwd and the thread and the message on this line".to_string())); // validatorRefusal("[\n  {\n    \"code\": \"invalid_type\",\n    \"path\": [\n      \"cwd\",\n      0\n    ]\n  },\n  {\n    \"code\": \"invalid_type\",\n    \"path\": [\n      \"sessionId\"\n    ]\n  },\n  {\n    \"code\": \"invalid_type\",\n    \"path\": [\n      \"threadId\"\n    ]\n  },\n  {\n    \"code\": \"too_big\",\n    \"path\": [\n      \"prompt\"\n    ]\n  }\n]")
        assert_eq!(validator_refusal("[\n  {\n    \"code\": \"invalid_type\",\n    \"path\": [\n      \"mystery\"\n    ]\n  }\n]"), Some("the host would not read this line".to_string())); // validatorRefusal("[\n  {\n    \"code\": \"invalid_type\",\n    \"path\": [\n      \"mystery\"\n    ]\n  }\n]")
        assert_eq!(validator_refusal("[\n  {\n    \"code\": \"custom\",\n    \"path\": []\n  }\n]"), Some("the host would not read this line".to_string())); // validatorRefusal("[\n  {\n    \"code\": \"custom\",\n    \"path\": []\n  }\n]")
    }

    #[test]
    fn disk_lines_cut_their_share_to_a_tenth_never_up() {
        assert_eq!(login_path_line("timed out"), "login shell: no PATH read (timed out); this host keeps the PATH it was started with"); // loginPathLine("timed out")
        assert_eq!(disk_full_line(89.99_f64), "its disk is 89.9% full: a stop snapshots the disk and fails once it is full, so free space on it (build output, caches) before it naps"); // diskFullLine(89.99)
        assert_eq!(disk_full_line(90_f64), "its disk is 90% full: a stop snapshots the disk and fails once it is full, so free space on it (build output, caches) before it naps"); // diskFullLine(90)
        assert_eq!(disk_full_line(95.25_f64), "its disk is 95.2% full: a stop snapshots the disk and fails once it is full, so free space on it (build output, caches) before it naps"); // diskFullLine(95.25)
        assert_eq!(disk_full_line(99.999_f64), "its disk is 99.9% full: a stop snapshots the disk and fails once it is full, so free space on it (build output, caches) before it naps"); // diskFullLine(99.999)
        assert_eq!(disk_full_line(100_f64), "its disk is 100% full: a stop snapshots the disk and fails once it is full, so free space on it (build output, caches) before it naps"); // diskFullLine(100)
        assert_eq!(disk_mark_line(&DiskMark { machine: "spoo".into(), on: "Boat".into(), pct: 92.37_f64, used_bytes: 148000000000_f64, size_bytes: 160000000000_f64, freed_bytes: 0_f64, machine_goes: false, r#move: None }), "The disk of spoo is 92.3% used (148 of 160 GB) and still filling. Boat cannot save a stop of a full disk, and deletes one whose stops keep failing with its disk. wsp found none of its own staging or build caches there to clear. Free space on its disk."); // diskMarkLine({"machine":"spoo","on":"Boat","pct":92.37,"usedBytes":148000000000,"sizeBytes":160000000000,"freedBytes":0,"machineGoes":false})
        assert_eq!(disk_mark_line(&DiskMark { machine: "spoo".into(), on: "Boat".into(), pct: 95_f64, used_bytes: 152000000000_f64, size_bytes: 160000000000_f64, freed_bytes: 2500000000_f64, machine_goes: true, r#move: Some(DiskMove { thread: "t 1".into(), name: "large".into(), cpu: 8_f64, mem_mb: 16384_f64, disk_gb: 320_f64 }) }), "The disk of spoo is 95% used (152 of 160 GB) and still filling. Boat cannot save a stop of a full disk, and deletes one whose stops keep failing with its disk. wsp cleared 2.5 GB of its own staging and build caches there. Free space on its disk, or let the thread finish: its machine is deleted once it is done. Or move it to large (8 vCPU, 16 GB, 320 GB disk): wsp resize 't 1' --size 8x16 stops it and resumes it at that size, with its files, which ends a turn running there."); // diskMarkLine({"machine":"spoo","on":"Boat","pct":95,"usedBytes":152000000000,"sizeBytes":160000000000,"freedBytes":2500000000,"machineGoes":true,"move":{"thread":"t 1","name":"large","cpu":8,"memMb":16384,"diskGb":320}})
    }

    #[test]
    fn resize_and_folder_lines() {
        assert_eq!(resize_landed_line("w", &serde_json::from_value::<WorkspaceSize>(serde_json::json!({ "cpu": 2.0, "memMb": 4096.0 })).unwrap(), &serde_json::from_value::<WorkspaceSize>(serde_json::json!({ "cpu": 4.0, "memMb": 8192.0 })).unwrap()), "w came back at 2x4, not the 4x8 asked for"); // resizeLandedLine("w", {cpu:2,memMb:4096}, {cpu:4,memMb:8192})
        assert_eq!(resized_line("w", &serde_json::from_value::<WorkspaceSize>(serde_json::json!({ "cpu": 2.0, "memMb": 512.0 })).unwrap()), "w runs at 2x0.5 now, on a machine of that size"); // resizedLine("w", {cpu:2,memMb:512})
        assert_eq!(resize_word_line(""), "\"\" names no size"); // resizeWordLine("")
        assert_eq!(resize_word_line("4x"), "\"4x\" names no size"); // resizeWordLine("4x")
        assert_eq!(resize_word_line("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\" names no size"); // resizeWordLine("a\"b\\c\n")
        assert_eq!(machine_unreached_line(1.0, 30000.0), "the machine could not be reached from this computer after 1 attempt over 30s"); // machineUnreachedLine(1, 30000)
        assert_eq!(mcp_server_command_line("/opt/my wsp/bin/wsp", &["mcp", "--state", "it's"]), "The server command is '/opt/my wsp/bin/wsp' mcp --state 'it'\\''s'"); // mcpServerCommandLine("/opt/my wsp/bin/wsp", ["mcp", "--state", "it's"])
        assert_eq!(mcp_server_command_line("wsp", &[] as &[&str]), "The server command is wsp"); // mcpServerCommandLine("wsp", [])
        assert_eq!(next_inside_agent_line("claude", "/mcp"), "Next: run claude in this folder and say: /mcp"); // nextInsideAgentLine("claude", "/mcp")
        assert_eq!(folder_level_line("/h", 0, 0.0), "No folders in /h."); // folderLevelLine({dir:"/h",folders:[],hidden:0})
        assert_eq!(folder_level_line("/h", 1, 3.0), "1 folder in /h, 3 hidden."); // folderLevelLine({dir:"/h",folders:[1],hidden:3})
        assert_eq!(folder_level_line("/h", 2, 0.0), "2 folders in /h."); // folderLevelLine({dir:"/h",folders:[1,2],hidden:0})
        assert_eq!(folder_level_line("/h", 0, 2.0), "No folders in /h, 2 hidden."); // folderLevelLine({dir:"/h",folders:[],hidden:2})
        assert_eq!(folder_refusal_line("  EACCES:\n  permission\tdenied  "), "No folders read. EACCES: permission denied"); // folderRefusalLine("  EACCES:\n  permission\tdenied  ")
        assert_eq!(exec_folder_line(None), "ran in the home folder"); // execFolderLine(undefined)
    }

    #[test]
    fn the_idle_reason_reads_back_only_its_own_shape() {
        assert_eq!(idle_reason::of(0.0), "idle 0 min"); // IDLE_REASON.of(0)
        assert_eq!(idle_reason::of(89999.0), "idle 1 min"); // IDLE_REASON.of(89999)
        assert_eq!(idle_reason::of(90000.0), "idle 2 min"); // IDLE_REASON.of(90000)
        assert_eq!(idle_reason::of(1200000.0), "idle 20 min"); // IDLE_REASON.of(1200000)
        assert_eq!(idle_reason::window_in(None), None); // IDLE_REASON.windowIn(undefined)
        assert_eq!(idle_reason::window_in(Some("idle 20 min")), Some("20 min".to_string())); // IDLE_REASON.windowIn("idle 20 min")
        assert_eq!(idle_reason::window_in(Some("idle 20 min ")), None); // IDLE_REASON.windowIn("idle 20 min ")
        assert_eq!(idle_reason::window_in(Some("idle  20 min")), None); // IDLE_REASON.windowIn("idle  20 min")
        assert_eq!(idle_reason::window_in(Some("idle 0 min")), Some("0 min".to_string())); // IDLE_REASON.windowIn("idle 0 min")
        assert_eq!(idle_reason::window_in(Some("napped")), None); // IDLE_REASON.windowIn("napped")
        assert_eq!(idle_reason::window_in(Some("idle min")), None); // IDLE_REASON.windowIn("idle min")
    }

    #[test]
    fn provider_waits_and_the_words_doors_quote() {
        assert_eq!(still_running_line(1.0), "1 background task still running; another line comes when this turn ends"); // stillRunningLine(1)
        assert_eq!(workspace_asleep_again_line("w"), "w is asleep again"); // workspaceAsleepAgainLine("w")
        assert_eq!(workspace_stays_awake_line("w", None), "w stays awake"); // workspaceStaysAwakeLine("w")
        assert_eq!(workspace_stays_awake_line("w", Some(3725000.0)), "w stays awake, naps in 1h 2m"); // workspaceStaysAwakeLine("w", 3725000)
        assert_eq!(provider_road_retry_line("POST /x", "ECONNRESET", 2.0, 3.0), "POST /x did not leave this computer (ECONNRESET); try 2 of 3"); // providerRoadRetryLine("POST /x", "ECONNRESET", 2, 3)
        assert_eq!(create_answer_lost_line("Boat", "socket hang up", "b-1"), "the answer to Boat's create was lost (socket hang up); wsp asked again and found b-1, which this create goes on with."); // createAnswerLostLine("Boat", "socket hang up", "b-1")
        assert_eq!(snapshot_listed_wait_line("s1", 5000.0, 65000.0), "the provider lists snapshot s1 yet answered \"Snapshot not found\"; asking again in 5.0s (1m 5s so far)"); // snapshotListedWaitLine("s1", 5000, 65000)
        assert_eq!(image_served_wait_line("Boat", 5000.0), "waiting for Boat to serve the image; asking again in 5.0s"); // imageServedWaitLine("Boat", 5000)
        assert_eq!(snapshot_listed_refused_line("s1", 120000.0), "the provider lists snapshot s1 yet answered \"Snapshot not found\" for 2m; it is not gone, so ask again in a minute"); // snapshotListedRefusedLine("s1", 120000)
        assert_eq!(background_work_words(Some("the Bash tool's run_in_background")), "Start long work with the Bash tool's run_in_background, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends"); // backgroundWorkWords("the Bash tool's run_in_background")
        assert_eq!(background_work_words(None), "Start long work with your harness's own background road, which wakes you when it ends, and never with a shell &, which nothing tracks and which wakes nobody when it ends"); // backgroundWorkWords()
        assert_eq!(still_working_line(None), "This thread replied, still working; the message runs as its next turn once that process exits"); // stillWorkingLine(undefined)
        assert_eq!(still_working_line(Some("")), "This thread replied, still working; the message runs as its next turn once that process exits"); // stillWorkingLine("")
        assert_eq!(still_working_line(Some("  ")), "This thread replied, still working; the message runs as its next turn once that process exits"); // stillWorkingLine("  ")
        assert_eq!(still_working_line(Some("  Fix the build  ")), "Fix the build replied, still working; the message runs as its next turn once that process exits"); // stillWorkingLine("  Fix the build  ")
    }

    #[test]
    fn a_refusal_is_two_halves_joined_once() {
        assert_eq!(refusal_line("it failed", "Try again."), "it failed. Try again."); // refusalLine("it failed", "Try again.")
        assert_eq!(refusal_line("it failed.", "Try again."), "it failed. Try again."); // refusalLine("it failed.", "Try again.")
        assert_eq!(refusal_line("it failed!", "x"), "it failed! x"); // refusalLine("it failed!", "x")
        assert_eq!(refusal_line("why?", "x"), "why? x"); // refusalLine("why?", "x")
        assert_eq!(refusal_line("note:", "x"), "note: x"); // refusalLine("note:", "x")
        assert_eq!(refusal_line("trailing  \n\t", "x"), "trailing. x"); // refusalLine("trailing  \n\t", "x")
        assert_eq!(refusal_line("", "x"), ". x"); // refusalLine("", "x")
        assert_eq!(refusal_line("ends;", "x"), "ends;. x"); // refusalLine("ends;", "x")
        assert_eq!(refusal_line("it failed", ""), "it failed. "); // refusalLine("it failed", "")
        assert_eq!(say_once("wsp new: ", "wsp new takes one name"), "wsp new takes one name"); // sayOnce("wsp new: ", "wsp new takes one name")
        assert_eq!(say_once("wsp new: ", "wsp new"), "wsp new"); // sayOnce("wsp new: ", "wsp new")
        assert_eq!(say_once("wsp new: ", "wsp newer"), "wsp new: wsp newer"); // sayOnce("wsp new: ", "wsp newer")
        assert_eq!(say_once("wsp new:", "x"), "wsp new:x"); // sayOnce("wsp new:", "x")
        assert_eq!(say_once("", "x"), "x"); // sayOnce("", "x")
        assert_eq!(say_once(": ", "x"), ": x"); // sayOnce(": ", "x")
        assert_eq!(unknown_word_line("frob"), "unknown command: frob."); // unknownWordLine("frob")
        assert_eq!(run_for_the_list("wsp help"), "Run wsp help for the list."); // runForTheList("wsp help")
    }

    #[test]
    fn redaction_takes_the_longest_value_first_and_skips_short_ones() {
        assert_eq!(redacted("key=abcd1234 again abcd1234", &["abcd1234"]), "key=<redacted> again <redacted>"); // redacted("key=abcd1234 again abcd1234", ["abcd1234"])
        assert_eq!(redacted("abc and abcdef", &["abc"]), "abc and abcdef"); // redacted("abc and abcdef", ["abc"])
        assert_eq!(redacted("the abcdef and abcd", &["abcd", "abcdef"]), "the <redacted> and <redacted>"); // redacted("the abcdef and abcd", ["abcd", "abcdef"])
        assert_eq!(redacted("pad ' tok1 ' and tok1", &[" tok1 "]), "pad '<redacted>' and <redacted>"); // redacted("pad ' tok1 ' and tok1", [" tok1 "])
        assert_eq!(redacted("nothing", &[] as &[&str]), "nothing"); // redacted("nothing", [])
        assert_eq!(redacted("a <redacted> b", &["redacted"]), "a <<redacted>> b"); // redacted("a <redacted> b", ["redacted"])
    }

    #[test]
    fn issues_flags_and_agents_lines() {
        assert_eq!(issues_line(&[]), ""); // issuesLine([])
        assert_eq!(issues_line(&[Issue { path: vec![PathPart::Key("a".into()), PathPart::Index(0.0), PathPart::Key("b".into())], message: "Required".into() }, Issue { path: vec![], message: "Bad".into() }]), "a.0.b: Required; : Bad"); // issuesLine([{path:["a",0,"b"],message:"Required"},{path:[],message:"Bad"}])
        assert_eq!(foreign_flag_line("--x", &["run"] as &[&str], "wsp new"), "--x belongs to run; wsp new does not read it"); // foreignFlagLine("--x", ["run"], "wsp new")
        assert_eq!(foreign_flag_line("--x", &["run","send"] as &[&str], "wsp new"), "--x belongs to run and send; wsp new does not read it"); // foreignFlagLine("--x", ["run","send"], "wsp new")
        assert_eq!(foreign_flag_line("--x", &["run","send","fork"] as &[&str], "wsp new"), "--x belongs to run, send and fork; wsp new does not read it"); // foreignFlagLine("--x", ["run","send","fork"], "wsp new")
        assert_eq!(foreign_flag_line("--x", &[] as &[&str], "wsp new"), "--x belongs to undefined; wsp new does not read it"); // foreignFlagLine("--x", [], "wsp new")
        assert_eq!(unknown_agent_line("gpt", &[] as &[&str]), "no agent called gpt; the catalog knows "); // unknownAgentLine("gpt", [])
        assert_eq!(no_adapter_line("x", &[] as &[&str]), "no adapter registered for harness \"x\"; agents on this host: none"); // noAdapterLine("x", [])
        assert_eq!(no_adapter_line("x", &["claude", "codex"]), "no adapter registered for harness \"x\"; agents on this host: claude, codex"); // noAdapterLine("x", ["claude", "codex"])
    }
}
