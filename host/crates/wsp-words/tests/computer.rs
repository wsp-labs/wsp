// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/copy.test.ts:62-79 (portsWord, madeOfWord), format.test.ts:477-489
//! (spawnDepthRefusal), :1342-1385 (moveTimedOutLine, RESUME_UNANSWERED, wakeGaveUpLine, wakeAskingAgainLine, wakeAsksIn,
//! WAKE_STOPPED, workspaceAwakeLine), :1547 (thisComputer), place-state.test.ts:243 (spendCapRefusal assertion only),
//! projects.test.ts:199 (isMacMachine) and workspace-state.test.ts:307 and :313 (relayedRefusal, undrivenRefusal,
//! providerCannotRefusal, with machineWord's value written out).
//! Skipped: workspace-state.test.ts:195 (kindWords, workspace words), :358 and :374 (screenCommandLine, signInRoad,
//! workspace words), :401 (whereWord, workspace words); THIS_COMPUTER is incidental in all four.

mod support;

use wsp_wire::wire::capabilities::Capabilities;
use wsp_words::words::computer::*;

fn caps(v: serde_json::Value) -> Capabilities {
    serde_json::from_value(support::over(support::capabilities(), v)).unwrap()
}

mod what_a_row_says_about_a_copys_network {
    use super::*;

    #[test]
    fn is_its_own_network_the_computers_ports_with_the_copys_port_base_or_those_ports_alo() {
        let here = caps(serde_json::json!({ "copies": true, "ownNetwork": false }));
        for computer in ["zingzy's MacBook Pro", "spoo"] {
            assert_eq!(ports_word(&caps(serde_json::json!({ "copies": false, "ownNetwork": true })), computer), "own network");
            assert_eq!(ports_word(&here, computer), format!("shares {computer}'s ports"));
            assert_eq!(ports_word(&caps(serde_json::json!({ "copies": false, "ownNetwork": false })), computer), "");
        }
    }

    #[test]
    fn gives_a_copy_with_its_own_network_that_word_whatever_road_made_it_since_the_ports_() {
        assert_eq!(ports_word(&caps(serde_json::json!({ "copies": true, "ownNetwork": true })), "spoo"), "own network");
    }
}

mod what_a_row_says_a_workspace_is_made_of {
    use super::*;

    #[test]
    fn is_a_copy_for_both_roads_the_one_word_the_first_run_screen_reads_before_any_road_() {
        assert_eq!(made_of_word("clonefile"), "a copy");
        assert_eq!(made_of_word("worktree"), "a copy");
        assert_eq!(COPY_WORD, "a copy");
    }
}

mod the_depth_refusal {
    use super::*;

    #[test]
    fn names_the_computers_cap_and_both_ways_to_raise_it_quoting_a_name_the_shell_would_s() {
        assert_eq!(
            spawn_depth_refusal("th_0123456789", 2.0, 2.0, &DepthHeld::Computer("spoo".into())),
            "thread th_01234 is 2 deep under its root and this computer allows 2, so a thread this deep may not spawn; raise Levels deep in Settings > Computers or run wsp computers set spoo --max-depth 3"
        );
        let quoted = spawn_depth_refusal("th_0123456789", 2.0, 2.0, &DepthHeld::Computer("my box".into()));
        assert!(quoted.contains("wsp computers set 'my box' --max-depth 3"), "{quoted}");
    }

    #[test]
    fn names_the_machine_whose_own_cap_holds_which_stands_for_its_life() {
        assert_eq!(
            spawn_depth_refusal("th_0123456789", 1.0, 1.0, &DepthHeld::Workspace("lead".into())),
            "thread th_01234 is 1 deep under its root and lead allows 1, so a thread this deep may not spawn; lead was made with that cap; ask the person"
        );
    }
}

mod a_pause_or_a_wake_the_provider_never_answered {
    use super::*;

    #[test]
    fn names_the_move_how_long_it_was_given_in_all_and_what_the_provider_reads_about_the_() {
        assert_eq!(
            move_timed_out_line(Move::Wake, 361_000.0, Some("paused")),
            "wake did not complete in 6m 1s; the provider did not answer and reads the machine paused; try again"
        );
        assert_eq!(
            move_timed_out_line(Move::Pause, 480_000.0, Some("running")),
            "pause did not complete in 8m; the provider did not answer and reads the machine running; try again"
        );
    }

    #[test]
    fn says_when_the_provider_could_not_be_read_about_the_machine_either() {
        assert_eq!(
            move_timed_out_line(Move::Pause, 240_000.0, None),
            "pause did not complete in 4m; the provider did not answer and could not be read about the machine; try again"
        );
    }
}

mod a_resume_the_provider_does_not_take {
    use super::*;

    // Constants only: no stub function is called, so this passes against the stubs.
    #[test]
    fn says_the_provider_has_not_answered_and_that_the_machine_is_being_read_since_a_hung() {
        assert_eq!(RESUME_UNANSWERED, "the provider has not answered the resume request; reading the machine");
        // Neither a refusal nor a retry of the person's: the machine's own state settles it and the host asks again.
        for word in ["try again", "failed", "refused"] {
            assert!(!RESUME_UNANSWERED.contains(word), "{RESUME_UNANSWERED}");
        }
    }

    #[test]
    fn ends_the_asking_with_what_the_provider_did_where_the_work_is_and_the_one_road_to_a() {
        assert_eq!(
            wake_gave_up_line(30.0, 30.0 * 60_000.0),
            "the provider answered none of 30 resume requests over 30m; the work on this machine's disk stays with the provider, and a rebuild starts a new machine from the image"
        );
        let one = wake_gave_up_line(1.0, 30_000.0);
        assert!(one.contains("none of 1 resume request over 30s"), "{one}");
    }

    #[test]
    fn counts_the_hosts_own_asks_on_the_row_of_the_number_it_will_make() {
        assert_eq!(wake_asking_again_line(3.0, wake_asks_in(30.0 * 60_000.0, 60_000.0), None), "waking, asking again (3 of 30)");
        assert_eq!(wake_asking_again_line(1.0, 2.0, None), "waking, asking again (1 of 2)");
    }

    #[test]
    fn fits_as_many_asks_in_the_asking_as_the_cadence_leaves_room_for_since_the_window_is() {
        assert_eq!(wake_asks_in(30.0 * 60_000.0, 60_000.0), 30.0);
        // The last ask starts inside the window, so a window of one cadence holds one ask and half a cadence more holds two.
        assert_eq!(wake_asks_in(60_000.0, 60_000.0), 1.0);
        assert_eq!(wake_asks_in(90_000.0, 60_000.0), 2.0);
        assert_eq!(wake_asks_in(0.0, 60_000.0), 0.0);
    }

    // Constants only: no stub function is called, so this passes against the stubs.
    #[test]
    fn says_the_machine_is_still_paused_when_the_person_stopped_the_asking() {
        assert_eq!(WAKE_STOPPED, "waking stopped; the machine is still paused");
    }

    #[test]
    fn names_the_workspace_in_what_the_needs_you_road_says_outside_the_app() {
        assert_eq!(workspace_awake_line("b1"), "b1 is awake");
    }
}

mod the_computer_being_read {
    use super::*;

    #[test]
    fn is_a_mac_by_name_on_a_mac_and_the_plain_word_anywhere_else() {
        assert_eq!(this_computer(Platform::Darwin), "this Mac");
        assert_eq!(this_computer(Platform::Linux), "this computer");
    }
}

mod the_word_a_places_row_says {
    use super::*;

    // Only the refusal's assertion: the placeSpendLimit and placeStateOf ones belong to the places unit.
    #[test]
    fn reads_a_spend_limit_only_off_a_clouds_cap_and_the_refusal_opens_with_the_rows_own_() {
        assert_eq!(
            spend_cap_refusal("solari", 10.02, 10.0),
            format!("{SPEND_LIMIT_LINE} on solari ($10.02/$10); raise its spend per day or start the machine after midnight")
        );
    }
}

mod getting_a_project_onto_a_machine {
    use super::*;

    #[test]
    fn a_machine_says_it_is_a_mac_by_the_name_its_maker_gives_it_or_by_the_kernel_where_i() {
        assert!(is_mac_machine(Some("macOS 15.5")));
        assert!(is_mac_machine(Some("Darwin 25.4.0")));
        assert!(!is_mac_machine(Some("Ubuntu 24.04.1 LTS")));
        assert!(!is_mac_machine(Some("Debian GNU/Linux 12 (bookworm)")));
        // A machine that has answered nothing yet has said nothing to hide.
        assert!(!is_mac_machine(None));
        assert!(!is_mac_machine(Some("")));
    }
}

mod what_a_workspaces_kind_changes_about_its_words {
    use super::*;

    // machineWord is not in the stubs: machineWord("local") is written as THIS_COMPUTER, the value the TS asserts it has.
    #[test]
    fn the_phrase_for_this_computer_has_one_home_the_refusals_read_it_too() {
        assert_eq!(THIS_COMPUTER, "this computer");
        let relayed = relayed_refusal("mac");
        assert!(relayed.contains(THIS_COMPUTER), "{relayed}");
        assert_eq!(undriven_refusal("mac", THIS_COMPUTER, "be paused"), format!("mac is {THIS_COMPUTER}, which wsp does not run; it cannot be paused"));
    }

    // Only the providerCannotRefusal assertions: machineWord is not in the stubs, so machineWord("cloud") is written
    // as MACHINE_WSP_FORKS, the value the TS asserts it has.
    #[test]
    fn every_kinds_own_word_is_in_the_table_so_no_sentence_about_one_kinds_machine_borrow() {
        assert_eq!(provider_cannot_refusal("api", MACHINE_WSP_FORKS, "be resized"), "api is a machine wsp forks, and it cannot be resized on the provider it runs on");
        assert_eq!(
            provider_cannot_refusal("api", MACHINE_WSP_FORKS, "move to a newer image"),
            "api is a machine wsp forks, and it cannot move to a newer image on the provider it runs on"
        );
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::words::computer::*;

    use serde_json::json;
    use wsp_wire::wire::capabilities::Capabilities;

    fn caps(copies: bool, own_network: bool) -> Capabilities {
        super::caps(json!({ "copies": copies, "ownNetwork": own_network }))
    }

    #[test]
    fn this_computer_is_named_off_what_it_reported() {
        assert_eq!(this_computer_line("spoo", "/w"), "Threads on spoo run in /w on this computer, under your own sign-ins."); // thisComputerLine("spoo", "/w")
        assert_eq!(is_mac_machine(None), false); // isMacMachine(undefined)
        assert_eq!(is_mac_machine(None), false); // isMacMachine(null)
        assert_eq!(is_mac_machine(Some("")), false); // isMacMachine("")
        assert_eq!(is_mac_machine(Some("macOS 15")), true); // isMacMachine("macOS 15")
        assert_eq!(is_mac_machine(Some("Darwin")), true); // isMacMachine("Darwin")
        assert_eq!(is_mac_machine(Some("darwin")), false); // isMacMachine("darwin")
        assert_eq!(is_mac_machine(Some("macOSX")), false); // isMacMachine("macOSX")
        assert_eq!(is_mac_machine(Some("Darwin Kernel")), true); // isMacMachine("Darwin Kernel")
        assert_eq!(is_mac_machine(Some("Linux")), false); // isMacMachine("Linux")
        assert_eq!(computer_word(None), "this computer"); // computerWord(undefined)
        assert_eq!(computer_word(Some("macOS 15")), "this Mac"); // computerWord("macOS 15")
        assert_eq!(computer_word(Some("Ubuntu")), "this computer"); // computerWord("Ubuntu")
        assert_eq!(ports_word(&caps(false, true), "spoo"), "own network"); // portsWord({copies:false,ownNetwork:true}, "spoo")
        assert_eq!(ports_word(&caps(false, false), "spoo"), ""); // portsWord({copies:false,ownNetwork:false}, "spoo")
        assert_eq!(ports_word(&caps(true, false), "spoo"), "shares spoo's ports"); // portsWord({copies:true,ownNetwork:false}, "spoo")
        assert_eq!(no_preview_route_line(3000.0, "spoo"), "spoo opens no address this pane can reach for port 3000; it answers as localhost:3000 on that computer."); // noPreviewRouteLine(3000, "spoo")
        assert_eq!(also_title(Platform::Darwin), "Also on this Mac"); // alsoTitle("darwin")
        assert_eq!(also_title(Platform::Linux), "Also on this computer"); // alsoTitle("linux")
    }

    #[test]
    fn a_thread_is_named_by_its_first_eight_units() {
        assert_eq!(thread_word(""), ""); // threadWord("")
        assert_eq!(thread_word("\u{1f600}\u{1f600}\u{1f600}\u{1f600}x"), "\u{1f600}\u{1f600}\u{1f600}\u{1f600}"); // threadWord("😀😀😀😀x")
        assert_eq!(thread_word("abc\u{1f600}defgh"), "abc\u{1f600}def"); // threadWord("abc😀defgh")
        assert_eq!(thread_word("abcdefg\u{1f600}"), "abcdefg"); // rule 8.5: node gives "abcdefg\\ud83d", the lone half is dropped // threadWord("abcdefg😀")
        assert_eq!(thread_at("0123456789", None), "thread 01234567"); // threadAt("0123456789", undefined)
        assert_eq!(thread_at("01", Some("")), "thread 01 on "); // threadAt("01", "")
    }

    #[test]
    fn every_spawn_act_reads_its_own_words() {
        assert_eq!(agents_off_refusal("w", SpawnAct::ThreadNew), "agents on w may not open a thread, since it was made with them off; ask the person"); // agentsOffRefusal("w", "thread_new")
        assert_eq!(agents_off_refusal("w", SpawnAct::Fork), "agents on w may not fork a machine, since it was made with them off; ask the person"); // agentsOffRefusal("w", "fork")
        assert_eq!(agents_off_refusal("w", SpawnAct::Send), "agents on w may not send into a thread, since it was made with them off; ask the person"); // agentsOffRefusal("w", "send")
        assert_eq!(agents_off_refusal("w", SpawnAct::BringBack), "agents on w may not bring its work back, since it was made with them off; ask the person"); // agentsOffRefusal("w", "bring_back")
        assert_eq!(agents_off_refusal("w", SpawnAct::Commit), "agents on w may not commit its work, since it was made with them off; ask the person"); // agentsOffRefusal("w", "commit")
        assert_eq!(agents_off_refusal("w", SpawnAct::Fix), "agents on w may not ask its agent to fix, since it was made with them off; ask the person"); // agentsOffRefusal("w", "fix")
        assert_eq!(agents_off_refusal("w", SpawnAct::Update), "agents on w may not update its branch from the base, since it was made with them off; ask the person"); // agentsOffRefusal("w", "update")
        assert_eq!(agents_off_refusal("w", SpawnAct::Merge), "agents on w may not merge its pull request, since it was made with them off; ask the person"); // agentsOffRefusal("w", "merge")
        assert_eq!(agents_off_refusal("w", SpawnAct::MergeIn), "agents on w may not merge a child's branch into its own, since it was made with them off; ask the person"); // agentsOffRefusal("w", "merge_in")
        assert_eq!(agents_off_refusal("w", SpawnAct::Start), "agents on w may not start work from a link, since it was made with them off; ask the person"); // agentsOffRefusal("w", "start")
        assert_eq!(agents_off_refusal("w", SpawnAct::Review), "agents on w may not start a review, since it was made with them off; ask the person"); // agentsOffRefusal("w", "review")
        assert_eq!(agents_off_refusal("w", SpawnAct::ReviewPost), "agents on w may not post a review, since it was made with them off; ask the person"); // agentsOffRefusal("w", "review_post")
        assert_eq!(agents_off_refusal("w", SpawnAct::Delete), "agents on w may not delete anything, since it was made with them off; ask the person"); // agentsOffRefusal("w", "delete")
        assert_eq!(agents_off_refusal("w", SpawnAct::DeleteSub), "agents on w may not delete a sub-thread it started, since it was made with them off; ask the person"); // agentsOffRefusal("w", "delete_sub")
        assert_eq!(agents_off_refusal("w", SpawnAct::Pause), "agents on w may not pause a machine, since it was made with them off; ask the person"); // agentsOffRefusal("w", "pause")
        assert_eq!(agents_off_refusal("w", SpawnAct::Resize), "agents on w may not move a machine to another size, since it was made with them off; ask the person"); // agentsOffRefusal("w", "resize")
        assert_eq!(agents_off_refusal("w", SpawnAct::Import), "agents on w may not import a folder, since it was made with them off; ask the person"); // agentsOffRefusal("w", "import")
        assert_eq!(agents_off_refusal("w", SpawnAct::Export), "agents on w may not export a folder, since it was made with them off; ask the person"); // agentsOffRefusal("w", "export")
        assert_eq!(agents_off_refusal("w", SpawnAct::Agents), "agents on w may not change what agents may do, since it was made with them off; ask the person"); // agentsOffRefusal("w", "agents")
        assert_eq!(agents_off_refusal("w", SpawnAct::Size), "agents on w may not pick a machine's size, since it was made with them off; ask the person"); // agentsOffRefusal("w", "size")
        assert_eq!(spawn_act_refusal("0123456789", SpawnAct::ThreadNew, None), "this request came out of thread 01234567, and a thread may only open a thread, fork a machine, send into a thread, bring its work back, commit its work, ask its agent to fix, update its branch from the base, merge a child's branch into its own, delete a sub-thread it started, never open a thread"); // spawnActRefusal("0123456789", "thread_new", undefined)
        assert_eq!(spawn_act_refusal("0123456789", SpawnAct::Fork, None), "this request came out of thread 01234567, and a thread may only open a thread, fork a machine, send into a thread, bring its work back, commit its work, ask its agent to fix, update its branch from the base, merge a child's branch into its own, delete a sub-thread it started, never fork a machine"); // spawnActRefusal("0123456789", "fork", undefined)
        assert_eq!(spawn_act_refusal("t", SpawnAct::Delete, Some("spoo")), "this request came out of thread t on spoo, and a thread may only open a thread, fork a machine, send into a thread, bring its work back, commit its work, ask its agent to fix, update its branch from the base, merge a child's branch into its own, delete a sub-thread it started, never delete anything"); // spawnActRefusal("t", "delete", "spoo")
        assert_eq!(agents_off_computer_refusal("t1", "my box", SpawnAct::Fork), "thread t1 may not fork a machine, since agents on my box are off; turn them on with wsp computers set 'my box' --spawn on"); // agentsOffComputerRefusal("t1", "my box", "fork")
        assert_eq!(agents_off_computer_refusal("t1", "o'brien", SpawnAct::Fork), "thread t1 may not fork a machine, since agents on o'brien are off; turn them on with wsp computers set 'o'\\''brien' --spawn on"); // agentsOffComputerRefusal("t1", "o'brien", "fork")
        assert_eq!(folder_gone_refusal("t1", SpawnAct::Send), "thread t1 may not send into a thread, since this host no longer holds the folder it works in; ask the person"); // folderGoneRefusal("t1", "send")
        assert_eq!(root_gone_refusal("t1", SpawnAct::Commit), "thread t1 may not commit its work, since the thread that started its tree was deleted; finish the work in this thread, or ask the person to start the one you need"); // rootGoneRefusal("t1", "commit")
    }

    #[test]
    fn only_a_json_object_is_a_frame() {
        assert_eq!(is_object_frame(&json!(null)), false); // isObjectFrame(null)
        assert_eq!(is_object_frame(&json!(1)), false); // isObjectFrame(1)
        assert_eq!(is_object_frame(&json!("s")), false); // isObjectFrame("s")
        assert_eq!(is_object_frame(&json!(true)), false); // isObjectFrame(true)
        assert_eq!(is_object_frame(&json!([])), false); // isObjectFrame([])
        assert_eq!(is_object_frame(&json!({})), true); // isObjectFrame({})
        assert_eq!(is_object_frame(&json!({"op":1})), true); // isObjectFrame({"op":1})
    }

    #[test]
    fn forks_depth_and_reach_refusals() {
        assert_eq!(default_branch_fix(DefaultBranchRoad::Fork, "w"), "Move the commits in w onto a branch of their own, then fork it again."); // defaultBranchFix("fork", "w")
        assert_eq!(default_branch_fix(DefaultBranchRoad::Run, "w"), "Move the commits in w onto a branch of their own, then start the thread again."); // defaultBranchFix("run", "w")
        assert_eq!(default_branch_fix(DefaultBranchRoad::BringBack, "w"), "Move the commits in w onto a branch of their own, then bring it back again."); // defaultBranchFix("bring back", "w")
        assert_eq!(folder_fork_fix("my project"), "Open a thread in it with wsp run 'my project' \"<task>\"."); // folderForkFix("my project")
        assert_eq!(spawn_cap_refusal("root", 3.0, 3.0), "thread root already holds 3 of its 3 machines; delete one before forking another"); // spawnCapRefusal("root", 3, 3)
        assert_eq!(spend_cap_refusal("boat", 5.5, 5.0), "spend limit reached today on boat ($5.50/$5); raise its spend per day or start the machine after midnight"); // spendCapRefusal("boat", 5.5, 5)
        assert_eq!(spawn_depth_refusal("t1", 2.0, 2.0, &DepthHeld::Computer("my box".into())), "thread t1 is 2 deep under its root and this computer allows 2, so a thread this deep may not spawn; raise Levels deep in Settings > Computers or run wsp computers set 'my box' --max-depth 3"); // spawnDepthRefusal("t1", 2, 2, {computer: "my box"})
        assert_eq!(spawn_depth_refusal("t1", 0.0, 0.0, &DepthHeld::Workspace("ws".into())), "thread t1 is 0 deep under its root and ws allows 0, so a thread this deep may not spawn; ws was made with that cap; ask the person"); // spawnDepthRefusal("t1", 0, 0, {workspace: "ws"})
        assert_eq!(elsewhere_workspace_line("w", "on", "from", AcrossAct::Start, None), "w is on on, and a thread on from cannot start a thread there yet; ask the person"); // elsewhereWorkspaceLine("w", "on", "from", "start")
        assert_eq!(elsewhere_workspace_line("w", "on", "from", AcrossAct::Exec, None), "w is on on, and a thread on from cannot run commands there yet; ask the person"); // elsewhereWorkspaceLine("w", "on", "from", "exec")
        assert_eq!(elsewhere_workspace_line("w", "on", "from", AcrossAct::Commit, None), "w is on on, and a thread on from cannot commit there yet; ask the person"); // elsewhereWorkspaceLine("w", "on", "from", "commit")
        assert_eq!(elsewhere_workspace_line("w", "on", "from", AcrossAct::Update, None), "w is on on, and a thread on from cannot update a copy there yet; ask the person"); // elsewhereWorkspaceLine("w", "on", "from", "update")
        assert_eq!(elsewhere_workspace_line("w", "on", "from", AcrossAct::Fix, None), "w is on on, and a thread on from cannot ask for a fix there yet; ask the person"); // elsewhereWorkspaceLine("w", "on", "from", "fix")
        assert_eq!(elsewhere_workspace_line("w", "on", "from", AcrossAct::Wake, None), "w is on on, and a thread on from cannot wake it there yet; ask the person"); // elsewhereWorkspaceLine("w", "on", "from", "wake")
        assert_eq!(elsewhere_workspace_line("w", "on", "from", AcrossAct::Work, None), "w is on on, and a thread on from cannot work there yet; ask the person"); // elsewhereWorkspaceLine("w", "on", "from", "work")
        assert_eq!(elsewhere_workspace_line("w", "on", "from", AcrossAct::Exec, Some("lead123456")), "w is on on, and a thread on from cannot run commands there yet; ask thread lead1234, your lead, with wsp send lead1234 \"<message>\""); // elsewhereWorkspaceLine("w", "on", "from", "exec", "lead123456")
        assert_eq!(child_to_leads_computer_line("a", "b", "lead123456"), "a thread on a cannot start one on b yet; ask thread lead1234, your lead, with wsp send lead1234 \"<message>\", or name a project on a"); // childToLeadsComputerLine("a", "b", "lead123456")
        assert_eq!(tree_thread_out_of_reach_line("t1", "t2"), "thread t1 works in a folder thread t2 cannot act on; ask it with wsp send t1 \"<message>\""); // treeThreadOutOfReachLine("t1", "t2")
        assert_eq!(spawn_reach_fix("proj", true), "Start a child on proj instead with wsp run proj \"<message>\", which forks a machine in your own tree."); // spawnReachFix("proj", true)
        assert_eq!(spawn_reach_fix("proj", false), "Start a child in proj's folder instead with wsp run proj \"<message>\", which opens a thread there in your own tree."); // spawnReachFix("proj", false)
        assert_eq!(no_workspace_refusal(None), "no such workspace"); // noWorkspaceRefusal()
        assert_eq!(no_workspace_refusal(Some("")), "no workspace "); // noWorkspaceRefusal("")
        assert_eq!(id_prefix_refusal("ab", &[] as &[&str]), "0 workspaces start with ab (); give more of the id"); // idPrefixRefusal("ab", [])
        assert_eq!(id_prefix_refusal("ab", &["abc", "abd"]), "2 workspaces start with ab (abc, abd); give more of the id"); // idPrefixRefusal("ab", ["abc", "abd"])
        assert_eq!(agents_word(None), ""); // agentsWord(undefined)
        assert_eq!(agents_word(Some(&AgentsSwitch { spawn: false, max_machines: 3.0, max_depth: 1.0 })), ""); // agentsWord({spawn:false,maxMachines:3})
        assert_eq!(agents_word(Some(&AgentsSwitch { spawn: true, max_machines: 1.0, max_depth: 1.0 })), "1 machine"); // agentsWord({spawn:true,maxMachines:1})
        assert_eq!(agents_word(Some(&AgentsSwitch { spawn: true, max_machines: 0.0, max_depth: 1.0 })), "0 machines"); // agentsWord({spawn:true,maxMachines:0})
        assert_eq!(agents_line(None), "agents may not spawn"); // agentsLine(undefined)
        assert_eq!(agents_line(Some(&AgentsSwitch { spawn: true, max_machines: 4.0, max_depth: 2.0 })), "agents may spawn: up to 4 machines"); // agentsLine({spawn:true,maxMachines:4,maxDepth:2})
    }

    #[test]
    fn host_keys_and_root_shells() {
        assert_eq!(host_key_kept_note("SHA256:abc", None), "SHA256:abc"); // hostKeyKeptNote("SHA256:abc", undefined)
        assert_eq!(host_key_kept_note("SHA256:abc", Some("/home/u/.ssh/known_hosts")), "SHA256:abc"); // hostKeyKeptNote("SHA256:abc", "/home/u/.ssh/known_hosts")
        assert_eq!(host_key_kept_note("SHA256:abc", Some("/etc/other")), "SHA256:abc in /etc/other"); // hostKeyKeptNote("SHA256:abc", "/etc/other")
        assert_eq!(host_key_kept_note("SHA256:abc", Some(".ssh/known_hosts")), "SHA256:abc in .ssh/known_hosts"); // hostKeyKeptNote("SHA256:abc", ".ssh/known_hosts")
        assert_eq!(host_key_kept_note("SHA256:abc", Some("known_hosts")), "SHA256:abc in known_hosts"); // hostKeyKeptNote("SHA256:abc", "known_hosts")
        assert_eq!(place_root_shell_refusal("u@h", "bash"), "u@h runs bash as its root's login shell, and sshd hands every command wsp sends to that shell: bash is not a shell wsp has read as opening no file of that computer's, and root's home there is the one every workspace on it writes, so a file read under it would run as root; run chsh -s /bin/bash root on it and add it again"); // placeRootShellRefusal("u@h", "bash")
        assert_eq!(place_root_shell_refusal("u@h", "zsh"), "u@h runs zsh as its root's login shell, and sshd hands every command wsp sends to that shell: zsh reads ~/.zshenv before running anything, and root's home there is the one every workspace on it writes, so a dial would run a workspace's file as root; run chsh -s /bin/bash root on it and add it again"); // placeRootShellRefusal("u@h", "zsh")
        assert_eq!(place_root_shell_refusal("u@h", "fish"), "u@h runs fish as its root's login shell, and sshd hands every command wsp sends to that shell: fish reads config.fish before running anything, and root's home there is the one every workspace on it writes, so a dial would run a workspace's file as root; run chsh -s /bin/bash root on it and add it again"); // placeRootShellRefusal("u@h", "fish")
        assert_eq!(place_root_shell_refusal("u@h", "dash"), "u@h runs dash as its root's login shell, and sshd hands every command wsp sends to that shell: dash is not a shell wsp has read as opening no file of that computer's, and root's home there is the one every workspace on it writes, so a file read under it would run as root; run chsh -s /bin/bash root on it and add it again"); // placeRootShellRefusal("u@h", "dash")
        assert_eq!(host_key_matches("ssh-ed25519 SHA256:abc", "SHA256:abc"), true); // hostKeyMatches("ssh-ed25519 SHA256:abc", "SHA256:abc")
        assert_eq!(host_key_matches("SHA256:abc", "256 SHA256:abc u@h (ED25519)"), true); // hostKeyMatches("SHA256:abc", "256 SHA256:abc u@h (ED25519)")
        assert_eq!(host_key_matches("", ""), false); // hostKeyMatches("", "")
        assert_eq!(host_key_matches("  ", "  "), false); // hostKeyMatches("  ", "  ")
        assert_eq!(host_key_matches("ssh-ed25519 AAAA", "ssh-ed25519 AAAA"), true); // hostKeyMatches("ssh-ed25519 AAAA", "ssh-ed25519 AAAA")
        assert_eq!(host_key_matches("ssh-ed25519 AAAA", "AAAA"), false); // hostKeyMatches("ssh-ed25519 AAAA", "AAAA")
        assert_eq!(host_key_matches("SHA256:x", "SHA256:y"), false); // hostKeyMatches("SHA256:x", "SHA256:y")
        assert_eq!(host_key_ask("h", "k"), "h answers with the key k; trust it and continue?"); // hostKeyAsk("h", "k")
        assert_eq!(host_key_unconfirmed_refusal("h", "ssh-ed25519 SHA256:k"), "h has never been reached from this computer and answers with the key ssh-ed25519 SHA256:k; check it against the computer itself, then run wsp add h --host-key 'ssh-ed25519 SHA256:k'"); // hostKeyUnconfirmedRefusal("h", "ssh-ed25519 SHA256:k")
        assert_eq!(host_key_unscannable_refusal("h", None), "h has never been reached from this computer and answered no key to a scan; read its host key on the computer itself and run wsp add h --host-key '<type> <fingerprint>'"); // hostKeyUnscannableRefusal("h")
        assert_eq!(host_key_unscannable_refusal("h", Some("ProxyJump")), "h has never been reached from this computer and is reached through ProxyJump in your ssh config, which a key scan cannot follow; read its host key on the computer itself and run wsp add h --host-key '<type> <fingerprint>'"); // hostKeyUnscannableRefusal("h", "ProxyJump")
        assert_eq!(host_key_mismatch_refusal(&HostKeyMismatch { address: "h".into(), pinned: "P".into(), wrote: None, target: "[h]:22".into(), file: "/k".into() }), "h answered with a key this computer could not read, not the P you pinned; ssh wrote it into /k, and ssh-keygen -R [h]:22 -f /k takes it out"); // hostKeyMismatchRefusal({address:"h",pinned:"P",target:"[h]:22",file:"/k"})
        assert_eq!(host_key_mismatch_refusal(&HostKeyMismatch { address: "h".into(), pinned: "P".into(), wrote: Some("W".into()), target: "h".into(), file: "/k".into() }), "h answered with W, not the P you pinned; ssh wrote it into /k, and ssh-keygen -R h -f /k takes it out"); // hostKeyMismatchRefusal({address:"h",pinned:"P",wrote:"W",target:"h",file:"/k"})
    }

    #[test]
    fn a_machines_lines_hold_their_edges() {
        assert_eq!(machine_lacks_short("a, b, c"), "a"); // machineLacksShort("a, b, c")
        assert_eq!(machine_lacks_short("no comma"), "no comma"); // machineLacksShort("no comma")
        assert_eq!(machine_lacks_short("  lead , rest"), "lead"); // machineLacksShort("  lead , rest")
        assert_eq!(machine_lacks_short(""), ""); // machineLacksShort("")
        assert_eq!(machine_lacks_short(",x"), ""); // machineLacksShort(",x")
        assert_eq!(move_timed_out_line(Move::Wake, 361000.0, None), "wake did not complete in 6m 1s; the provider did not answer and could not be read about the machine; try again"); // moveTimedOutLine("wake", 361000, undefined)
        assert_eq!(wake_asks_in(1.0, 1000.0), 1.0); // wakeAsksIn(1, 1000)
        assert_eq!(wake_asks_in(1000.0, 1000.0), 1.0); // wakeAsksIn(1000, 1000)
        assert_eq!(wake_asks_in(1001.0, 1000.0), 2.0); // wakeAsksIn(1001, 1000)
        assert_eq!(wake_asks_in(0.0, 1000.0), 0.0); // wakeAsksIn(0, 1000)
        assert_eq!(wake_asks_in(60000.0, 7000.0), 9.0); // wakeAsksIn(60000, 7000)
        assert_eq!(wake_asking_again_line(2.0, 9.0, None), "waking, asking again (2 of 9)"); // wakeAskingAgainLine(2, 9)
        assert_eq!(wake_asking_again_line(2.0, 9.0, Some(LineStyle::Short)), "asking 2/9"); // wakeAskingAgainLine(2, 9, "short")
        assert_eq!(wake_gave_up_line(1.0, 30000.0), "the provider answered none of 1 resume request over 30s; the work on this machine's disk stays with the provider, and a rebuild starts a new machine from the image"); // wakeGaveUpLine(1, 30000)
        assert_eq!(thread_left_line("box", &[]), "box did not say that what this thread started there has ended; look in Processes and end what still runs there"); // threadLeftLine("box", [])
        assert_eq!(thread_left_line("box", &[42]), "a process this thread started on box still runs after Stop (42); end it in Processes, or restart box"); // threadLeftLine("box", [42])
        assert_eq!(thread_left_line("box", &[1, 2, 3]), "3 processes this thread started on box still run after Stop (1, 2, 3); end them in Processes, or restart box"); // threadLeftLine("box", [1, 2, 3])
        assert_eq!(thread_end_late_line("box", 2.5), "box did not answer within 2.5 s, so the turn ends there, with what this thread started, once box answers"); // threadEndLateLine("box", 2.5)
    }

    #[test]
    fn a_machine_check_marks_its_error_and_nothing_else_reads_as_marked() {
        let lacks = machine_lacking(NO_NODE_LINE);
        assert_eq!(lacks.message, NO_NODE_LINE);
        assert_eq!((lacks.fix.clone(), lacks.kind.clone()), (None, None));
        assert_eq!(machine_lacks_line(&lacks), Some(NO_NODE_LINE.to_string()));
        assert!(!machine_never_answered(&lacks));
        let unanswered = machine_unanswered("ssh: connect timed out");
        assert_eq!(machine_lacks_line(&unanswered), None);
        assert!(machine_never_answered(&unanswered));
        let plain = wsp_words::exit::HostError::new(NO_NODE_LINE);
        assert_eq!(machine_lacks_line(&plain), None);
        assert!(!machine_never_answered(&plain));
        assert_eq!(MACHINE_LACKS_LINES.map(machine_lacks_short), ["this machine has no Node 22", "this login does not linger", "this machine runs no systemd"]);
    }

    #[test]
    fn spawn_acts_keep_the_tables_order_keys_and_words() {
        let keys: Vec<&str> = SpawnAct::ALL.iter().map(|a| a.key()).collect();
        assert_eq!(keys, ["thread_new", "fork", "send", "bring_back", "commit", "fix", "update", "merge", "merge_in", "start", "review", "review_post", "delete", "delete_sub", "pause", "resize", "import", "export", "agents", "size"]);
        for a in SpawnAct::ALL {
            assert_eq!(serde_json::to_value(a).unwrap(), serde_json::json!(a.key()));
        }
        let allowed: Vec<&str> = SPAWN_ACTS_ALLOWED.iter().map(|a| a.words()).collect();
        assert_eq!(allowed, ["open a thread", "fork a machine", "send into a thread", "bring its work back", "commit its work", "ask its agent to fix", "update its branch from the base", "merge a child's branch into its own", "delete a sub-thread it started"]);
    }
}

