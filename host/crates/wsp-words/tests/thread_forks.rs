// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/format.test.ts:271-304 (memory near full), :369-418 (the catalog source line),
//! :1107-1125 (goneWords, NOT_GONE), :1187-1197 (record restored, name refusals), :1252-1272 (daemon words),
//! :1274-1300 (the nap's vault words), :1302-1311 (imageHomeKeptLine), :1313-1338 (a create's own words) and
//! rewind-words.test.ts:6-23 (rewindNote and the rewind refusals).

use regex::Regex;
use serde_json::{json, Value};
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};
use wsp_words::words::computer::THIS_COMPUTER;
use wsp_words::words::thread::*;

fn wire<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn size(v: Value) -> WorkspaceSize {
    wire(v)
}

fn offers_of(v: Value) -> Vec<MachineSizeOffer> {
    wire(v)
}

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

mod a_machine_that_stopped_answering_with_its_memory_near_full {
    use super::*;

    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    fn offers() -> Vec<MachineSizeOffer> {
        offers_of(json!([
            { "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.11 },
            { "cpu": 2, "memMb": 8192, "rateUsdPerHour": 0.15 },
            { "cpu": 4, "memMb": 16384, "rateUsdPerHour": 0.3 },
        ]))
    }

    #[test]
    fn near_full_is_the_measured_share_on_the_number_only() {
        assert_eq!(MEMORY_NEAR_FULL, 0.9);
        assert!(memory_near_full(3.59 * GIB, 3.94 * GIB));
        assert!(memory_near_full(90.0, 100.0));
        assert!(!memory_near_full(89.0, 100.0));
        assert!(!memory_near_full(0.0, 0.0));
    }

    #[test]
    fn the_line_carries_the_last_figures_and_says_the_work_took_the_memory_never_that_t() {
        assert_eq!(
            out_of_memory_line(&MemoryReading { used: 3.59 * GIB, total: 3.94 * GIB, load1: 6.42 }),
            "Out of memory (3.6 GB of 3.9 GB used, load 6.4) when the thread's computer last answered; the work there took the memory, not a fault in the computer",
        );
    }

    #[test]
    fn the_row_form_says_the_unit_once_when_both_sides_share_it_so_the_sidebar_s_second() {
        assert_eq!(
            out_of_memory_row_line(&MemoryReading { used: 3.59 * GIB, total: 3.94 * GIB, load1: 6.42 }),
            "out of memory, 3.6 of 3.9 GB",
        );
        assert_eq!(
            out_of_memory_row_line(&MemoryReading { used: 900.0 * 1024.0 * 1024.0, total: 3.94 * GIB, load1: 1.0 }),
            "out of memory, 900 MB of 3.9 GB",
        );
    }

    #[test]
    fn the_size_line_names_the_smallest_offer_with_more_memory_and_its_rate_or_that_the() {
        let offers = offers();
        assert_eq!(
            bigger_size_line(&size(json!({ "cpu": 2, "memMb": 4096 })), &offers),
            "A thread on 2\u{a0}vCPU,\u{a0}8\u{a0}GB ($0.15/hr) fits more; pick it when you start the next one",
        );
        assert_eq!(
            bigger_size_line(&size(json!({ "cpu": 2, "memMb": 8192 })), &offers),
            "A thread on 4\u{a0}vCPU,\u{a0}16\u{a0}GB ($0.30/hr) fits more; pick it when you start the next one",
        );
        assert_eq!(
            bigger_size_line(&size(json!({ "cpu": 4, "memMb": 16384 })), &offers),
            "No size with more memory is offered; run less in the thread at once",
        );
        // Order in the table does not pick the offer; memory does.
        let mut reversed = offers.clone();
        reversed.reverse();
        let line = bigger_size_line(&size(json!({ "cpu": 2, "memMb": 4096 })), &reversed);
        assert!(line.contains("2\u{a0}vCPU,\u{a0}8\u{a0}GB"), "{line}");
    }
}

mod where_the_composer_s_model_lists_came_from_in_one_line {
    use super::*;

    fn table(extra: Value) -> HarnessCatalog {
        let mut base = json!({
            "harness": "codex",
            "label": "Codex",
            "source": "table",
            "version": "app-server 0.153.0, 2026-09-07",
            "models": [{ "value": "gpt-5.6-sol", "label": "GPT-5.6-Sol" }],
            "efforts": [],
            "contextWindows": [],
            "permissionModes": [],
            "steers": false,
            "renames": false,
            "images": false,
        });
        for (k, v) in extra.as_object().unwrap() {
            base[k] = v.clone();
        }
        wire(base)
    }

    #[test]
    fn names_the_agent_s_own_binary_and_its_own_pin_when_its_table_stood_in_never_anoth() {
        assert_eq!(catalog_source_line(&table(json!({})), THIS_COMPUTER), "codex table, app-server 0.153.0, 2026-09-07");
        assert_eq!(
            catalog_source_line(
                &table(json!({ "harness": "claude", "label": "Claude Code", "version": "--help 2.1.257, 2026-09-05" })),
                THIS_COMPUTER,
            ),
            "claude table, --help 2.1.257, 2026-09-05",
        );
        // One line at the popup's width: 48 characters of the 10px mono the footer draws in, measured in Chromium.
        assert!(js_len(&catalog_source_line(&table(json!({})), THIS_COMPUTER)) <= 48);
    }

    #[test]
    fn says_the_adapter_s_own_reason_where_the_binary_answered_and_named_one_in_place_o() {
        assert_eq!(
            catalog_source_line(&table(json!({ "refusal": codex_not_signed_in_line("codex login") })), THIS_COMPUTER),
            "Codex is not signed in where this thread runs; run codex login there, app-server 0.153.0, 2026-09-07",
        );
    }

    #[test]
    fn a_table_with_no_pin_of_its_own_claims_none_and_the_binary_that_answered_carries() {
        assert_eq!(
            catalog_source_line(&table(json!({ "harness": "gemini", "label": "Gemini CLI", "version": null })), THIS_COMPUTER),
            "gemini table",
        );
        assert_eq!(
            catalog_source_line(&table(json!({ "source": "harness", "version": "0.153.0" })), THIS_COMPUTER),
            "Codex 0.153.0 on this computer",
        );
        assert_eq!(catalog_source_line(&table(json!({ "source": "harness", "version": null })), "hetzner"), "Codex on hetzner",);
        // The reason belongs to the fallback: a binary that filled the lists has nothing to explain.
        assert_eq!(
            catalog_source_line(&table(json!({ "source": "harness", "version": "0.153.0", "refusal": "not signed in" })), THIS_COMPUTER,),
            "Codex 0.153.0 on this computer",
        );
    }

    #[test]
    fn an_empty_model_list_reads_as_the_source_that_gave_it_what_the_binary_reported_or() {
        assert_eq!(no_models_line(&table(json!({ "source": "harness", "models": [] }))), "Codex reported no models");
        assert_eq!(no_models_line(&table(json!({ "models": [] }))), "No model in the Codex table");
    }

    #[test]
    fn neither_foot_says_machine_the_word_the_app_never_uses_to_a_person() {
        let kinds: Vec<HarnessCatalog> = vec![
            table(json!({ "source": "harness", "version": "2.1.257" })),
            table(json!({ "source": "harness", "version": null })),
            table(json!({ "refusal": codex_not_signed_in_line("codex login") })),
            table(json!({ "models": [] })),
            table(json!({})),
        ];
        let machine = Regex::new("(?i)machine").unwrap();
        for r#where in [THIS_COMPUTER, "hetzner"] {
            for catalog in &kinds {
                {
                    let line = catalog_source_line(catalog, r#where);
                    assert!(!machine.is_match(&line), "{line}");
                }
            }
        }
    }
}

mod gone_words {
    use super::*;

    #[test]
    fn names_the_machine_alone_when_nobody_saw_the_provider_lose_it() {
        assert_eq!(gone_words("m1", None), "machine m1 is gone at the provider");
    }

    #[test]
    fn names_the_call_that_found_it_gone_and_the_second_it_did_quoting_the_provider_s_a() {
        // Date.parse("2026-09-07T01:21:10.500Z")
        let at = 1788744070500.0;
        assert_eq!(
            gone_words("sb_1", Some(&GoneSighting { by: GoneSeenBy::Pause, at, answer: some("404 Not found") })),
            "machine sb_1 is gone at the provider: the pause found it gone at 2026-09-07T01:21:10Z (404 Not found)",
        );
        assert_eq!(
            gone_words("sb_1", Some(&GoneSighting { by: GoneSeenBy::StatusPoll, at, answer: None })),
            "machine sb_1 is gone at the provider: the status poll found it gone at 2026-09-07T01:21:10Z",
        );
        assert_eq!(
            gone_words("sb_1", Some(&GoneSighting { by: GoneSeenBy::Sweep, at, answer: some("") })),
            "machine sb_1 is gone at the provider: the sweep found it gone at 2026-09-07T01:21:10Z",
        );
    }
}

mod not_gone {
    use super::*;

    #[test]

    fn says_the_record_follows_the_state_read_whichever_state_it_holds_the_machine_in() {
        // Only a constant: NOT_GONE already holds its value, so this test does not fail against todo!().
        assert_eq!(NOT_GONE, "not gone at the provider after all; the record follows the state read");
    }
}

mod a_record_the_sweep_restored_and_a_name_a_fork_cannot_take {
    use super::*;

    #[test]
    fn names_the_machine_the_workspace_and_the_verb_that_removes_it() {
        assert_eq!(RECORD_RESTORED, "record restored from the provider's listing");
        assert_eq!(
            record_restored_line("sbx_1", "first", "ws_1"),
            "reap: recorded sbx_1 as workspace first (ws_1): a machine from this setup that no record claimed; it bills until wsp delete first",
        );
    }

    #[test]
    fn refuses_a_taken_name_and_a_name_being_deleted_in_words_a_person_can_act_on() {
        assert_eq!(name_taken_refusal("first"), "first is already a workspace; pick another name, or delete it first");
        assert_eq!(name_deleting_refusal("first"), "first is being deleted; wait for the delete to finish, then fork it again",);
    }
}

mod daemon_updating_and_daemon_update_failed {
    use super::*;

    #[test]

    fn says_what_is_being_done_and_that_it_failed_in_fixed_words_no_daemon_named_no_rea() {
        assert_eq!(DAEMON_UPDATING, "updating the helper");
        assert_eq!(DAEMON_UPDATE_FAILED, "could not update the helper");
        // The row's second line fits about thirty characters at the default sidebar width (measured in Chromium at
        // 159px), and a deploy's own reason is an npm log hundreds wide that names the daemon in its own words.
        for line in [DAEMON_UPDATING, DAEMON_UPDATE_FAILED] {
            assert!(!line.contains("daemon"));
            assert!(js_len(line) <= 30);
        }
    }

    #[test]

    fn has_its_own_pair_for_the_first_daemon_a_machine_takes_since_nothing_that_machine() {
        assert_eq!(DAEMON_INSTALLING, "installing the helper");
        assert_eq!(DAEMON_INSTALL_FAILED, "could not install the helper");
        for line in [DAEMON_INSTALLING, DAEMON_INSTALL_FAILED] {
            assert!(!line.contains("daemon"));
            assert!(!line.contains("updat"));
            assert!(js_len(line) <= 30);
        }
    }
}

mod the_nap_s_words_when_its_vault_was_not_stored {
    use super::*;

    #[test]
    fn vaultovercapline_reads_the_export_and_the_cap_in_the_one_byte_rule() {
        assert_eq!(vault_over_cap_line(797_760_137.0, 209_715_200.0), "the export was 761 MB, over the 200 MB cap");
        assert_eq!(vault_over_cap_line(6_000.0, 5_000.0), "the export was 6 KB, over the 5 KB cap");
    }

    #[test]
    fn the_verdict_says_what_the_nap_did_and_what_stands_which_is_nothing_where_no_nap() {
        assert_eq!(vault_kept_line(Some("2026-09-08T07:10:04.444Z")), "the nap saved no backup; what was saved before is kept",);
        // A workspace whose first nap failed has no earlier backup to keep, and the row beside this one says "no
        // backup" with no day: a line promising one kept would be the app inventing a file.
        assert_eq!(vault_kept_line(None), "the nap saved no backup; nothing is saved off the machine");
        // Whatever refused the export stays on the line's title: a shell's own words are evidence, not a sentence.
        for why in [
            vault_over_cap_line(797_760_137.0, 209_715_200.0),
            "vault enumeration failed: ls: /root: No such file or directory".to_string(),
        ] {
            for w in [Some("2026-09-08T07:10:04.444Z"), None] {
                let line = vault_kept_line(w);
                assert!(!line.contains(&why), "{line}");
            }
        }
    }

    #[test]
    fn vaultstaleline_is_the_one_word_every_surface_shows_for_a_machine_whose_files_are() {
        let refused = vault_over_cap_line(677_178_573.0, 209_715_200.0);
        assert_eq!(vault_stale_line(Some("2026-09-08T07:10:04.444Z"), Some(&refused)), some("no backup since 2026-09-08"),);
        // The day is the whole of it: the vault that stands can be days old, and the row has about thirty characters.
        assert_eq!(vault_stale_line(Some("2026-09-01T23:59:59Z"), Some(&refused)), some("no backup since 2026-09-01"),);
        assert!(js_len(&vault_stale_line(Some("2026-09-08T07:10:04.444Z"), Some(&refused)).unwrap()) <= 29);
        assert_eq!(vault_stale_line(None, Some(&refused)), some("no backup"));
        assert_eq!(vault_stale_line(Some("2026-09-08T07:10:04.444Z"), None), None);
        assert_eq!(vault_stale_line(None, None), None);
    }
}

mod a_create_s_own_words {
    use super::*;

    #[test]
    fn the_fork_s_line_names_the_workspace_and_the_computer_it_starts_on_and_nothing_of() {
        assert_eq!(starting_line("spoo-fix", "hetzner"), "starting spoo-fix on hetzner");
        assert_eq!(starting_line("clone-test", "ascii"), "starting clone-test on ascii");
        for word in ["fork", "golden", "image", "machine"] {
            let line = starting_line("spoo-fix", "hetzner");
            assert!(!line.contains(word), "{line}");
        }
        // The row draws this line while a fork boots, and it holds thirty mono characters.
        assert!(js_len(&starting_line("clone-test", "ascii")) <= 30);
    }

    #[test]
    fn every_line_of_the_log_is_written_one_way_lower_case_no_full_stop_no_machine_s_id() {
        let lines =
            [starting_line("clone-test", "box"), hostname_set_line("clone-test"), HOSTNAME_KEPT.to_string(), CREATE_READY.to_string()];
        let fk = Regex::new(r"\bfk_").unwrap();
        for line in &lines {
            let first: String = line.chars().take(1).collect();
            assert_eq!(first, first.to_lowercase());
            assert!(!line.ends_with('.'));
            assert!(!fk.is_match(line), "{line}");
        }
        assert_eq!(hostname_set_line("clone-test"), "hostname set to clone-test");
        assert_eq!(CREATE_READY, "ready");
    }

    #[test]

    fn a_refused_hostname_reads_as_what_it_means_for_the_workspace_with_no_shell_s_word() {
        // Only constants: HOSTNAME_KEPT already holds its value, so this test does not fail against todo!().
        assert_eq!(HOSTNAME_KEPT, "hostname not set; the workspace keeps the machine's own name");
        assert!(!HOSTNAME_KEPT.contains("sethostname"));
        assert!(!HOSTNAME_KEPT.contains("failed"));
    }
}

mod the_words_a_rewind_says {
    use super::*;

    #[test]
    fn names_what_goes_before_the_click_and_that_undo_brings_the_files_back_and_never_t() {
        assert_eq!(
            rewind_note(&RewindNote { turns: 3.0, files: true, cuts_conversation: true, agent: "Claude Code".to_string(), kept: None }),
            "The 3 turns after this reply leave the conversation, and the files go back to how they stood at this reply. If another thread has worked in this folder since this reply, the files stay as they are. Undo rewind puts them back until the next turn in this folder ends; the conversation does not come back.",
        );
        assert_eq!(
            rewind_note(&RewindNote { turns: 1.0, files: false, cuts_conversation: true, agent: "Codex".to_string(), kept: None }),
            "The turn after this reply leaves the conversation; the files stay as they are.",
        );
        // An agent that keeps its own history: only the files move, and the note says why the turns stay.
        assert_eq!(
            rewind_note(&RewindNote { turns: 2.0, files: true, cuts_conversation: false, agent: "Cursor".to_string(), kept: None }),
            "Cursor keeps its own history, so the conversation stays and the files go back to how they stood at this reply. If another thread has worked in this folder since this reply, the files stay as they are. Undo rewind puts them back until the next turn in this folder ends.",
        );
    }

    #[test]
    fn refuses_in_one_sentence_each_naming_what_to_do() {
        assert_eq!(
            rewind_children_line(&["fix the port list", "write the migration"]),
            "stop the threads under this one first (fix the port list, write the migration); a rewind never stops them for you",
        );
        assert_eq!(
            rewind_beside_line("move the pricing table"),
            "another thread in this folder is working (move the pricing table), and its files would go back too; let its turn end or stop it first",
        );
        assert_eq!(
            rewind_no_anchor_line("Claude Code"),
            "Claude Code left no point in that reply to cut its conversation at, so only the files can go back",
        );
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::words::thread::*;

    use serde_json::json;
    use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};

    fn wire<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
        serde_json::from_value(v).unwrap()
    }

    fn catalog(v: serde_json::Value) -> wsp_wire::views::harness::HarnessCatalog {
        let mut base = json!({ "models": [], "efforts": [], "contextWindows": [], "permissionModes": [], "steers": false, "renames": false, "images": false });
        for (k, v) in v.as_object().unwrap() {
            base[k] = v.clone();
        }
        wire(base)
    }

    fn taken(names: &[&str]) -> indexmap::IndexSet<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn asides_and_rewinds_say_what_moves_and_what_stays() {
        assert_eq!(aside_unsupported_line("codex"), "codex takes no side question; send it as a message and the thread keeps it"); // asideUnsupportedLine("codex")
        assert_eq!(aside_wall_line(120000.0), "the side question had no answer after 2m and was stopped"); // asideWallLine(120000)
        assert_eq!(rewind_children_line(&[] as &[&str]), "stop the threads under this one first (); a rewind never stops them for you"); // rewindChildrenLine([])
        assert_eq!(rewind_children_line(&["a", "b"]), "stop the threads under this one first (a, b); a rewind never stops them for you"); // rewindChildrenLine(["a", "b"])
        assert_eq!(
            rewind_beside_line("t"),
            "another thread in this folder is working (t), and its files would go back too; let its turn end or stop it first"
        ); // rewindBesideLine("t")
        assert_eq!(
            rewind_no_anchor_line("codex"),
            "codex left no point in that reply to cut its conversation at, so only the files can go back"
        ); // rewindNoAnchorLine("codex")
        assert_eq!(rewind_kept_line("codex keeps it", true), "codex keeps it; the files went back and every turn stays"); // rewindKeptLine("codex keeps it", true)
        assert_eq!(rewind_kept_line("codex keeps it", false), "codex keeps it; every turn stays"); // rewindKeptLine("codex keeps it", false)
        assert_eq!(rewind_note(&RewindNote { turns: 1.0, files: true, cuts_conversation: true, agent: "claude".into(), kept: None }), "The turn after this reply leaves the conversation, and the files go back to how they stood at this reply. If another thread has worked in this folder since this reply, the files stay as they are. Undo rewind puts them back until the next turn in this folder ends; the conversation does not come back."); // rewindNote({"turns":1,"files":true,"cutsConversation":true,"agent":"claude"})
        assert_eq!(
            rewind_note(&RewindNote { turns: 3.0, files: false, cuts_conversation: true, agent: "claude".into(), kept: None }),
            "The 3 turns after this reply leave the conversation; the files stay as they are."
        ); // rewindNote({"turns":3,"files":false,"cutsConversation":true,"agent":"claude"})
        assert_eq!(rewind_note(&RewindNote { turns: 2.0, files: true, cuts_conversation: false, agent: "Codex".into(), kept: None }), "Codex keeps its own history, so the conversation stays and the files go back to how they stood at this reply. If another thread has worked in this folder since this reply, the files stay as they are. Undo rewind puts them back until the next turn in this folder ends."); // rewindNote({"turns":2,"files":true,"cutsConversation":false,"agent":"Codex"})
        assert_eq!(rewind_note(&RewindNote { turns: 2.0, files: true, cuts_conversation: true, agent: "x".into(), kept: Some("codex keeps its history".into()) }), "Codex keeps its history, so the conversation stays and the files go back to how they stood at this reply. If another thread has worked in this folder since this reply, the files stay as they are. Undo rewind puts them back until the next turn in this folder ends."); // rewindNote({"turns":2,"files":true,"cutsConversation":true,"agent":"x","kept":"codex keeps its history"})
        assert_eq!(rewind_note(&RewindNote { turns: 2.0, files: false, cuts_conversation: false, agent: "x".into(), kept: Some("".into()) }), ", so the conversation stays and the files go back to how they stood at this reply. If another thread has worked in this folder since this reply, the files stay as they are. Undo rewind puts them back until the next turn in this folder ends.");
        // rewindNote({"turns":2,"files":false,"cutsConversation":false,"agent":"x","kept":""})
    }

    #[test]
    fn a_forks_branch_is_numbered_past_every_name_taken() {
        assert_eq!(forked_from_line("T"), "Forked from T"); // forkedFromLine("T")
        assert_eq!(fork_of_line("T"), "Fork of T"); // forkOfLine("T")
        assert_eq!(fork_dialog_line("T"), "A new thread with this conversation up to here. T does not change."); // forkDialogLine("T")
        assert_eq!(fork_folder_note("T"), "The files as they are now, shared with T."); // forkFolderNote("T")
        assert_eq!(fork_where_line(None), "in this folder"); // forkWhereLine(undefined)
        assert_eq!(fork_where_line(Some("")), "on "); // forkWhereLine("")
        assert_eq!(fork_where_line(Some("main")), "on main"); // forkWhereLine("main")
        assert_eq!(fork_branch_name(None, &taken(&[]), "0123456789"), "fork-01234567"); // forkBranchName(undefined, new Set(), "0123456789")
        assert_eq!(fork_branch_name(Some(""), &taken(&["fork-01234567"]), "0123456789"), "fork-01234567-2"); // forkBranchName("", new Set(["fork-01234567"]), "0123456789")
        assert_eq!(fork_branch_name(Some("main"), &taken(&["main-fork", "main-fork-2", "main-fork-4"]), "t"), "main-fork-3"); // forkBranchName("main", new Set(["main-fork", "main-fork-2", "main-fork-4"]), "t")
        assert_eq!(fork_branch_name(Some("main"), &taken(&["main-fork-2"]), "t"), "main-fork"); // forkBranchName("main", new Set(["main-fork-2"]), "t")
        assert_eq!(at_not_a_turn_line("0"), "--at takes a finished turn's number from 1, not 0"); // atNotATurnLine("0")
        assert_eq!(fork_agent_line("claude", "codex"), "a fork runs on its thread's agent, claude, not codex"); // forkAgentLine("claude", "codex")
        assert_eq!(fork_agent_fix("codex"), "Leave the agent out, or open a new thread on codex."); // forkAgentFix("codex")
        assert_eq!(forks_not_line("cursor"), "cursor cannot fork a thread"); // forksNotLine("cursor")
        assert_eq!(fork_turns_line(0.0), "that thread has no finished turn to fork from"); // forkTurnsLine(0)
        assert_eq!(fork_turns_fix(0.0), "Wait for its first turn to end."); // forkTurnsFix(0)
        assert_eq!(fork_turns_line(1.0), "that thread has 1 finished turn"); // forkTurnsLine(1)
        assert_eq!(fork_turns_fix(1.0), "Name one from 1 to 1 with --at."); // forkTurnsFix(1)
        assert_eq!(fork_turns_line(3.0), "that thread has 3 finished turns"); // forkTurnsLine(3)
        assert_eq!(fork_turns_fix(3.0), "Name one from 1 to 3 with --at."); // forkTurnsFix(3)
        assert_eq!(fork_no_turn_line("t9"), "that thread holds no turn t9"); // forkNoTurnLine("t9")
        assert_eq!(fork_no_anchor_line("codex"), "codex left no point in that turn to fork its conversation at"); // forkNoAnchorLine("codex")
        assert_eq!(fork_refused_line("codex", "why"), "codex could not fork the conversation: why"); // forkRefusedLine("codex", "why")
        assert_eq!(thread_runs_on_line("claude", "codex"), "this thread runs on claude; open a new thread to run codex");
        // threadRunsOnLine("claude", "codex")
    }

    #[test]
    fn codex_and_catalog_lines() {
        assert_eq!(codex_not_signed_in_line("codex login"), "Codex is not signed in where this thread runs; run codex login there"); // codexNotSignedInLine("codex login")
        assert_eq!(
            codex_missing_env_line("OPENAI_KEY"),
            "Codex's model provider reads its key from the environment variable OPENAI_KEY, which is not set on this machine"
        ); // codexMissingEnvLine("OPENAI_KEY")
        assert_eq!(
            catalog_source_line(&catalog(json!({"harness":"codex","label":"Codex","source":"harness","version":"0.153.0"})), "spoo"),
            "Codex 0.153.0 on spoo"
        ); // catalogSourceLine({"harness":"codex","label":"Codex","source":"harness","version":"0.153.0"}, "spoo")
        assert_eq!(
            no_models_line(&catalog(json!({"harness":"codex","label":"Codex","source":"harness","version":"0.153.0"}))),
            "Codex reported no models"
        ); // noModelsLine({"harness":"codex","label":"Codex","source":"harness","version":"0.153.0"})
        assert_eq!(
            catalog_source_line(&catalog(json!({"harness":"codex","label":"Codex","source":"harness","version":null})), "spoo"),
            "Codex on spoo"
        ); // catalogSourceLine({"harness":"codex","label":"Codex","source":"harness","version":null}, "spoo")
        assert_eq!(
            no_models_line(&catalog(json!({"harness":"codex","label":"Codex","source":"harness","version":null}))),
            "Codex reported no models"
        ); // noModelsLine({"harness":"codex","label":"Codex","source":"harness","version":null})
        assert_eq!(
            catalog_source_line(&catalog(json!({"harness":"codex","label":"Codex","source":"table","version":null})), "spoo"),
            "codex table"
        ); // catalogSourceLine({"harness":"codex","label":"Codex","source":"table","version":null}, "spoo")
        assert_eq!(
            no_models_line(&catalog(json!({"harness":"codex","label":"Codex","source":"table","version":null}))),
            "No model in the Codex table"
        ); // noModelsLine({"harness":"codex","label":"Codex","source":"table","version":null})
        assert_eq!(
            catalog_source_line(
                &catalog(
                    json!({"harness":"codex","label":"Codex","source":"table","version":"pinned 0.150","refusal":"codex gave nothing"})
                ),
                "spoo"
            ),
            "codex gave nothing, pinned 0.150"
        ); // catalogSourceLine({"harness":"codex","label":"Codex","source":"table","version":"pinned 0.150","refusal":"codex gave nothing"}, "spoo")
        assert_eq!(
            no_models_line(&catalog(
                json!({"harness":"codex","label":"Codex","source":"table","version":"pinned 0.150","refusal":"codex gave nothing"})
            )),
            "No model in the Codex table"
        ); // noModelsLine({"harness":"codex","label":"Codex","source":"table","version":"pinned 0.150","refusal":"codex gave nothing"})
        assert_eq!(
            catalog_source_line(&catalog(json!({"harness":"codex","label":"Codex","source":"table","version":"0.150"})), "spoo"),
            "codex table, 0.150"
        ); // catalogSourceLine({"harness":"codex","label":"Codex","source":"table","version":"0.150"}, "spoo")
        assert_eq!(
            no_models_line(&catalog(json!({"harness":"codex","label":"Codex","source":"table","version":"0.150"}))),
            "No model in the Codex table"
        ); // noModelsLine({"harness":"codex","label":"Codex","source":"table","version":"0.150"})
        assert_eq!(codex_reconnect_line(185000.0), "stopped after 3m 05s of Codex reconnecting to its model provider with no answer");
        // codexReconnectLine(185000)
    }

    #[test]
    fn memory_lines_read_the_share_and_the_next_size_up() {
        assert!(!memory_near_full(0_f64, 0_f64)); // memoryNearFull({used:0,total:0})
        assert!(memory_near_full(9_f64, 10_f64)); // memoryNearFull({used:9,total:10})
        assert!(!memory_near_full(8.99_f64, 10_f64)); // memoryNearFull({used:8.99,total:10})
        assert!(!memory_near_full(1_f64, 0_f64)); // memoryNearFull({used:1,total:0})
        assert!(memory_near_full(10_f64, 10_f64)); // memoryNearFull({used:10,total:10})
        assert_eq!(out_of_memory_line(&MemoryReading { used: 3.59 * 1073741824.0, total: 3.94 * 1073741824.0, load1: 6.25 }), "Out of memory (3.6 GB of 3.9 GB used, load 6.3) when the thread's computer last answered; the work there took the memory, not a fault in the computer"); // outOfMemoryLine({used:3.59*1024**3,total:3.94*1024**3,load1:6.25})
        assert_eq!(out_of_memory_line(&MemoryReading { used: 100.0, total: 200.0, load1: 0.05 }), "Out of memory (100 B of 200 B used, load 0.1) when the thread's computer last answered; the work there took the memory, not a fault in the computer"); // outOfMemoryLine({used:100,total:200,load1:0.05})
        assert_eq!(
            out_of_memory_row_line(&MemoryReading { used: 3.59 * 1073741824.0, total: 3.94 * 1073741824.0, load1: 1.0 }),
            "out of memory, 3.6 of 3.9 GB"
        ); // outOfMemoryRowLine({used:3.59*1024**3,total:3.94*1024**3,load1:1})
        assert_eq!(
            bigger_size_line(&serde_json::from_value::<WorkspaceSize>(serde_json::json!({ "cpu": 2.0, "memMb": 4096.0 })).unwrap(), &[]),
            "No size with more memory is offered; run less in the thread at once"
        ); // biggerSizeLine({cpu:2,memMb:4096}, [])
        assert_eq!(
            bigger_size_line(
                &serde_json::from_value::<WorkspaceSize>(serde_json::json!({ "cpu": 2.0, "memMb": 4096.0 })).unwrap(),
                &[
                    serde_json::from_value::<MachineSizeOffer>(serde_json::json!({ "cpu": 8.0, "memMb": 16384.0, "rateUsdPerHour": 0.4 }))
                        .unwrap(),
                    serde_json::from_value::<MachineSizeOffer>(serde_json::json!({ "cpu": 2.0, "memMb": 4096.0, "rateUsdPerHour": 0.1 }))
                        .unwrap(),
                    serde_json::from_value::<MachineSizeOffer>(serde_json::json!({ "cpu": 4.0, "memMb": 8192.0, "rateUsdPerHour": 0.2 }))
                        .unwrap(),
                    serde_json::from_value::<MachineSizeOffer>(serde_json::json!({ "cpu": 6.0, "memMb": 8192.0, "rateUsdPerHour": 0.3 }))
                        .unwrap()
                ]
            ),
            "A thread on 4\u{a0}vCPU,\u{a0}8\u{a0}GB ($0.20/hr) fits more; pick it when you start the next one"
        ); // biggerSizeLine({cpu:2,memMb:4096}, [{cpu:8,memMb:16384,rateUsdPerHour:0.4},{cpu:2,memMb:4096,rateUsdPerHour:0.1},{cpu:4,memMb:8192,rateUsdPerHour:0.2},{cpu:6,memMb:8192,rateUsdPerHour:0.3}])
    }

    #[test]
    fn vault_and_create_lines() {
        assert_eq!(vault_over_cap_line(3221225472.0, 2147483648.0), "the export was 3 GB, over the 2 GB cap"); // vaultOverCapLine(3*1024**3, 2*1024**3)
        assert_eq!(store_unread_line("codex", "EACCES"), "could not read codex on the machine, so nothing from it travelled: EACCES"); // storeUnreadLine("codex", "EACCES")
        assert_eq!(
            state_entry_refusal("../x", "a symlink"),
            "the agents' state from the machine holds ../x, which is a symlink; nothing of it was landed"
        ); // stateEntryRefusal("../x", "a symlink")
        assert_eq!(vault_kept_line(None), "the nap saved no backup; nothing is saved off the machine"); // vaultKeptLine({})
        assert_eq!(vault_kept_line(Some("2026-10-01T00:00:00.000Z")), "the nap saved no backup; what was saved before is kept"); // vaultKeptLine({vaultedAt:"2026-10-01T00:00:00.000Z"})
        assert_eq!(vault_stale_line(None, None), None); // vaultStaleLine({})
        assert_eq!(vault_stale_line(None, Some("too big")), Some("no backup".to_string())); // vaultStaleLine({vaultRefused:"too big"})
        assert_eq!(vault_stale_line(Some("2026-10-01T23:59:59.999Z"), Some("x")), Some("no backup since 2026-10-01".to_string())); // vaultStaleLine({vaultRefused:"x",vaultedAt:"2026-10-01T23:59:59.999Z"})
        assert_eq!(vault_stale_line(Some("short"), Some("")), Some("no backup since short".to_string())); // vaultStaleLine({vaultRefused:"",vaultedAt:"short"})
        assert_eq!(hostname_set_line("spoo"), "hostname set to spoo"); // hostnameSetLine("spoo")
        assert_eq!(starting_line("w", "Boat"), "starting w on Boat"); // startingLine("w", "Boat")
        assert_eq!(copying_folder_line("/a", "/b"), "copying /a to /b"); // copyingFolderLine("/a", "/b")
        assert_eq!(copy_first_line("Boat", "w", 0.0), "building your image on Boat first, about ten minutes, then w forks from it"); // copyFirstLine("Boat", "w", 0)
        assert_eq!(
            copy_first_line("Boat", "w", 0.09000000000000001),
            "building your image on Boat first, about ten minutes, billed at $0.09/hr while it builds, then w forks from it"
        ); // copyFirstLine("Boat", "w", 0.09000000000000001)
    }

    #[test]
    fn a_moment_is_utc_to_the_second() {
        assert_eq!(iso_seconds(0.0), "1970-01-01T00:00:00Z"); // isoSeconds(0)
        assert_eq!(iso_seconds(1788744070500.0), "2026-09-07T01:21:10Z"); // isoSeconds(1788744070500)
        assert_eq!(iso_seconds(1788744070999.0), "2026-09-07T01:21:10Z"); // isoSeconds(1788744070999)
        assert_eq!(iso_seconds(-1.0), "1969-12-31T23:59:59Z"); // isoSeconds(-1)
        assert_eq!(gone_words("m1", None), "machine m1 is gone at the provider"); // goneWords("m1")
        assert_eq!(
            gone_words("m1", Some(&GoneSighting { by: GoneSeenBy::StatusPoll, at: 1788744070500.0, answer: None })),
            "machine m1 is gone at the provider: the status poll found it gone at 2026-09-07T01:21:10Z"
        ); // goneWords("m1", {"by":"status poll","at":1788744070500})
        assert_eq!(
            gone_words("m1", Some(&GoneSighting { by: GoneSeenBy::RecordLoad, at: 0.0, answer: Some("".into()) })),
            "machine m1 is gone at the provider: the record load found it gone at 1970-01-01T00:00:00Z"
        ); // goneWords("m1", {"by":"record load","at":0,"answer":""})
        assert_eq!(
            gone_words("m1", Some(&GoneSighting { by: GoneSeenBy::Sweep, at: 0.0, answer: Some("404 Not Found".into()) })),
            "machine m1 is gone at the provider: the sweep found it gone at 1970-01-01T00:00:00Z (404 Not Found)"
        ); // goneWords("m1", {"by":"sweep","at":0,"answer":"404 Not Found"})
        assert_eq!(
            gone_unconfirmed_line("m1", "404"),
            "machine m1 answered 404 when this host started; wsp is asking the provider again before it calls it gone"
        ); // goneUnconfirmedLine("m1", "404")
        assert_eq!(
            record_restored_line("m1", "w", "id1"),
            "reap: recorded m1 as workspace w (id1): a machine from this setup that no record claimed; it bills until wsp delete w"
        ); // recordRestoredLine("m1", "w", "id1")
        assert_eq!(name_taken_refusal("w"), "w is already a workspace; pick another name, or delete it first"); // nameTakenRefusal("w")
        assert_eq!(name_deleting_refusal("w"), "w is being deleted; wait for the delete to finish, then fork it again"); // nameDeletingRefusal("w")
        assert_eq!(
            delete_refused_line("w", "m1", "running"),
            "w's machine m1 is still running after three asks; run wsp delete again or delete it at the provider"
        ); // deleteRefusedLine("w", "m1", "running")
    }

    #[test]
    fn places_and_threads_named_by_a_word() {
        assert_eq!(
            build_place_ask_line(&["a", "b"]),
            "a, b run workspaces and none of them is the default place; name the one to build your image on with --on"
        ); // buildPlaceAskLine(["a", "b"])
        assert_eq!(
            place_forks_nothing_pick_line("here", &["a", "b"]),
            "here forks no machines; name a place that runs workspaces with --on: a, b"
        ); // placeForksNothingPickLine("here", ["a", "b"])
        assert_eq!(
            not_a_thread_line("spoo", WordIs::Project, "wsp send", false),
            "spoo is a project; wsp send takes a thread, as wsp threads spoo lists them."
        ); // notAThreadLine("spoo", "project", "wsp send")
        assert_eq!(
            not_a_thread_line("m1", WordIs::Machine, "wsp agents", true),
            "m1 is a machine; wsp agents takes a thread, or a computer with --on, as wsp threads lists them."
        ); // notAThreadLine("m1", "machine", "wsp agents", true)
        assert_eq!(
            not_a_thread_line("spoo", WordIs::Project, "wsp agents", true),
            "spoo is a project; wsp agents takes a thread, or a computer with --on, as wsp threads spoo lists them."
        ); // notAThreadLine("spoo", "project", "wsp agents", true)
        assert_eq!(shared_folder_line(&["a"]), "thread a works in this folder too"); // sharedFolderLine(["a"])
        assert_eq!(shared_folder_line(&["a", "b"]), "threads a and b work in this folder too"); // sharedFolderLine(["a", "b"])
        assert_eq!(shared_folder_line(&["a", "b", "c"]), "threads a, b and c work in this folder too"); // sharedFolderLine(["a", "b", "c"])
        assert_eq!(thread_elsewhere_line("0123456789"), "thread 01234567 does not work in this folder"); // threadElsewhereLine("0123456789")
        assert_eq!(
            child_beside_lead_line("0123456789", "abcdefghij"),
            "thread 01234567 works in the same folder as thread abcdefgh; there is nothing apart to merge."
        ); // childBesideLeadLine("0123456789", "abcdefghij")
        assert_eq!(no_project_line("x"), "no project x"); // noProjectLine("x")
    }
}

mod shared_folder_line_of_none {
    #[test]
    fn prints_what_javascript_prints_for_an_empty_list() {
        // node 22: sharedFolderLine([]) reads others.slice(0, -1).join(", ") as "" and others.at(-1) as undefined.
        let none: [&str; 0] = [];
        assert_eq!(wsp_words::words::thread::shared_folder_line(&none), "threads  and undefined work in this folder too");
    }
}
