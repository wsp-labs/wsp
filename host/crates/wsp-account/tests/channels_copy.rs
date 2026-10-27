// SPDX-License-Identifier: AGPL-3.0-only
//! The served and copy channels, shell tokens, where a child starts, one repository (spec/channels_copy.md). Every
//! expected value is the TS's: oracle/channels_copy.oracle.ts ran `channelsArea` on a fake context over scripted
//! worlds and wrote each world, its script and the trace the TS made of it to tests/fixtures/oracle/channels_copy.json.
//! `Sim` below builds the same context from a world, `play` runs the same script, and a test compares the trace it
//! made, step answers and effects in order, to the TS's as JSON strings (so key order counts).
#![allow(dead_code, unused_imports)]
mod common;
include!("channels_copy/world.rs");

// The tests ----------------------------------------------------------------------------------------------------

// C1 to C5: the served channel.

#[test]
fn c1_served_channel_refusals() {
    check(&["c1 "]);
}

#[test]
fn c2_served_channel_says_hello_with_its_root() {
    check(&["c2 "]);
}

#[test]
fn c3_events_this_channel_hears() {
    check(&["c3 "]);
}

#[test]
fn c4_served_channel_frame_table() {
    check(&["c4 "]);
}

#[test]
fn c5_close_detaches_what_it_attached() {
    check(&["c5 "]);
}

// TS: test/box-panes.test.ts:31
#[test]
fn a_folder_pane_hears_only_its_own_ptys_and_detaches_only_what_stands() {
    check(&["c3 guest and pty events", "c5 close detaches what stands attached", "c4 the panels answer on a folder"]);
}

// TS: test/box-panes.test.ts:94
#[test]
fn a_pane_does_not_hear_the_readings_the_computers_own_page_watches() {
    check(&["c3 the panels hear first on a folder", "c3 guest and pty events"]);
}

// TS: test/box-panes.test.ts:107
#[test]
fn a_command_on_a_panes_channel_is_refused_before_the_link() {
    check(&["c4 frame table"]);
}

// TS: test/box-panes.test.ts:115
#[test]
fn a_served_reply_is_answered_under_a_null_id_and_keeps_its_own() {
    check(&["c4 frame table"]);
}

// C6 and C7: the shell token's environment.

#[test]
fn c6_terminal_token_env_order() {
    check(&["c6 "]);
}

// TS: test/channels.test.ts:263
#[test]
fn a_shells_tabs_open_in_the_threads_folder_each_with_a_token_of_its_own() {
    check(&["c6 terminal token env order", "c12 pty.create mints and keeps"]);
}

// TS: test/channels.test.ts:280
#[test]
fn a_shell_carries_the_git_and_gh_variables_a_turns_launch_does_on_its_own_token() {
    check(&["c6 terminal token env order"]);
}

// TS: test/channels.test.ts:293
#[test]
fn a_shell_carries_the_sign_in_a_claude_turn_is_handed_on_its_own_token() {
    check(&["c6 terminal token env order"]);
}

// TS: test/channels.test.ts:319
#[test]
fn a_codex_threads_shell_is_marked_for_the_profile_that_finds_codexs_program_first() {
    check(&["c6 the harness off the latest row", "c8 the first shell waits"]);
}

// TS: test/channels.test.ts:430
#[test]
fn a_shell_signs_in_only_its_threads_own_agent() {
    check(&["c6 terminal token env order", "c6 the harness off the latest row", "c6 no harness at all", "c6 a harness with no sign-in"]);
}

// TS: test/channels.test.ts:559
#[test]
fn a_thread_of_another_workspace_scopes_the_token_to_this_workspaces_newest_thread() {
    check(&["c6 a thread of another workspace takes the newest here"]);
}

/// `terminal_token` and `shell_sign_ins` called straight, against the environment the TS's pty.create carried.
#[test]
fn c6_terminal_token_and_sign_ins_called_directly() {
    let fixture = oracle("channels_copy");
    let s = &fixture["scenarios"]["c6 terminal token env order"];
    let frame = s["trace"]["fx"].as_array().unwrap().iter().find(|e| e[0] == json!("frame")).unwrap()[1].clone();
    let thread_env_keys = s["world"]["threadEnv"].as_object().unwrap().len();
    let minted: IndexMap<String, String> =
        frame["env"].as_object().unwrap().iter().skip(thread_env_keys).map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
    let signed: IndexMap<String, String> =
        [("SIGN_IN".to_string(), "claude:tok1".to_string()), ("WSP_TERMINAL".to_string(), "claude".to_string())].into_iter().collect();
    assert_eq!(frame["env"]["SIGN_IN"], json!("claude:tok1"));
    assert_eq!(frame["env"]["WSP_TERMINAL"], json!("claude"));
    let world = s["world"].clone();
    run(async move {
        let b = build(&world);
        let got = b.area.terminal_token(&b.entry, Some(&json!("t1"))).await.map_err(|e| thrown(&e)).unwrap();
        assert_eq!(got, Some(ShellToken { device_id: "d1".into(), env: minted.clone() }));
        assert_eq!(serde_json::to_string(&got.unwrap().env).unwrap(), serde_json::to_string(&minted).unwrap());
        let b = build(&world);
        let signs = b.area.shell_sign_ins(&b.entry, "t1", "tok1").await.expect("sign-ins");
        assert_eq!(serde_json::to_string(&signs).unwrap(), serde_json::to_string(&signed).unwrap());
        let none = b.area.shell_sign_ins(&b.entry, "t-with-no-harness", "tok1").await.expect("sign-ins");
        assert!(none.is_empty());
        let wrote: Vec<Value> = b.sim.fx.all().into_iter().filter(|e| e[0] == json!("exec")).collect();
        assert_eq!(
            serde_json::to_string(&wrote).unwrap(),
            serde_json::to_string(&json!([["exec", "write-terminal claude claude-bin", 20000]])).unwrap()
        );
    });
}

// C8: the terminal programs.

#[test]
fn c8_first_shell_waits_later_ones_do_not() {
    check(&["c8 the first shell waits", "c8 a write that never answers", "c8 each agent on a machine", "c8 a deleted workspace forgets"]);
}

// TS: test/channels.test.ts:337
#[test]
fn a_machines_first_codex_shell_opens_once_the_program_is_written_and_a_later_one_does_not_wait() {
    check(&["c8 the first shell waits"]);
}

// TS: test/channels.test.ts:356
#[test]
fn a_machines_first_shell_opens_after_an_inline_execs_span_where_the_write_never_answers() {
    check(&["c8 a write that never answers"]);
}

// TS: test/channels.test.ts:375
#[test]
fn a_refused_write_warns_why_and_the_shell_opens_all_the_same() {
    check(&["c8 a refused write warns", "c8 a failed exec counts", "c8 a refused write with no stderr"]);
}

// TS: test/channels.test.ts:449
#[test]
fn each_threads_agent_is_signed_in_in_its_own_terminal_where_both_share_a_machine() {
    check(&["c8 each agent on a machine"]);
}

// C9: the tokens the host keeps.

#[test]
fn c9_shell_tokens_kept_and_revoked() {
    check(&[
        "c9 tokens kept and revoked",
        "c9 a kill the daemon refused",
        "c9 a list ends the exited",
        "c9 a list that names no ptys",
        "c9 a failed revoke warns",
        "c9 a failed store write warns",
    ]);
}

// TS: test/channels.test.ts:465
#[test]
fn a_shells_token_dies_at_its_exit_its_kill_and_a_list_that_no_longer_holds_it() {
    check(&["c9 tokens kept and revoked"]);
}

#[test]
fn c9_shell_held_devices_skip_dead_workspaces() {
    check(&["c9 held devices skip dead workspaces"]);
}

// TS: test/channels.test.ts:506
#[test]
fn a_shells_token_is_held_off_the_documents_with_no_turn_running() {
    check(&["c9 held devices skip dead workspaces"]);
}

#[test]
fn c9_deleted_workspace_ends_its_shells() {
    check(&["c9 deleted workspace ends its shells", "c9 delete of a workspace with no shells"]);
}

// TS: test/channels.test.ts:575
#[test]
fn a_shells_token_goes_with_its_workspace() {
    check(&["c9 deleted workspace ends its shells", "c8 a deleted workspace forgets"]);
}

#[test]
fn c9_shell_tokens_survive_a_restart() {
    check(&["c9 shell tokens survive a restart"]);
}

// TS: test/channels.test.ts:484
#[test]
fn a_shells_token_outlives_a_host_restart_while_its_shell_stands() {
    check(&["c9 shell tokens survive a restart"]);
}

// C10: a computer that cannot run workspaces.

#[test]
fn c10_a_blocked_computer_carries_status_and_ping() {
    check(&["c10 "]);
}

// C11: who mints.

#[test]
fn c11_only_a_machines_own_daemon_mints() {
    check(&["c11 "]);
}

// C12: pty.create.

#[test]
fn c12_pty_create_mints_and_keeps() {
    check(&["c12 pty.create mints and keeps", "c12 cwd is the frame's or the thread's folder", "c12 a pty.create with no token"]);
}

// TS: test/channels.test.ts:535
#[test]
fn a_shell_is_not_minted_a_token_while_the_broker_is_not_ready() {
    check(&["c12 a pty.create with no token", "c11 a machine's own daemon mints, broker not ready"]);
}

// TS: test/terminal-folder.test.ts:32
#[test]
fn a_tab_that_names_no_folder_opens_in_the_threads_folder_and_one_that_names_a_folder_keeps_it() {
    check(&["c12 cwd is the frame's or the thread's folder"]);
}

// TS: test/terminal-folder.test.ts:45
#[test]
fn a_cloud_forks_first_tab_opens_in_the_folder_the_thread_works_in() {
    check(&["c12 cwd is the frame's or the thread's folder"]);
}

// TS: test/serve-daemon.test.ts:294
#[test]
fn a_thread_terminal_with_no_folder_starts_where_the_thread_works() {
    check(&["c12 cwd is the frame's or the thread's folder"]);
}

#[test]
fn c12_a_late_reply_revokes() {
    check(&[
        "c12 a late reply revokes",
        "c12 a reply while the workspace is deleting",
        "c12 a reply while the workspace is gone",
        "c12 a refused reply revokes",
        "c12 a reply with no ptyId",
    ]);
}

// TS: test/channels.test.ts:543
#[test]
fn a_shells_token_is_taken_away_when_its_open_answers_after_its_workspace_was_deleted() {
    check(&["c12 a late reply revokes"]);
}

#[test]
fn c12_a_failed_send_revokes() {
    check(&["c12 a failed send revokes"]);
}

// C13: where a child starts. The table runs every row the TS's tests reach, 11 and the rows around them.

#[test]
fn c13_lead_start_table() {
    check_where("the lead start rows", |name| {
        name.starts_with("c13 ") && !name.starts_with("c13 a blocked") && !name.starts_with("c13 a lead on")
    });
}

#[test]
fn c13_a_blocked_lead_reads_nothing_and_a_folder_lead_is_never_blocked() {
    check(&["c13 a blocked lead", "c13 a lead on a folder"]);
}

#[test]
fn c13_a_lead_on_its_own_daemon_reads_over_a_channel_it_closes() {
    check(&["c13 a lead on its own daemon"]);
}

// TS: test/spawn-guard.test.ts:186
#[test]
fn a_child_starts_on_the_branch_its_lead_is_on() {
    check(&["c13 an upstream of another name", "c13 ahead pushes with changes staying"]);
}

// TS: test/spawn-guard.test.ts:274
#[test]
fn a_lead_with_commits_on_the_default_branch_is_refused_before_anything_is_pushed() {
    check(&["c13 default branch ahead, a fork", "c13 default branch master ahead", "c13 default branch counts unknown"]);
}

// TS: test/spawn-guard.test.ts:288
#[test]
fn the_default_branch_is_the_one_the_leads_remote_starts_on() {
    check(&["c13 default branch master ahead", "c13 a branch called main where the default is master"]);
}

// TS: test/spawn-guard.test.ts:307
#[test]
fn a_lead_whose_own_base_is_the_default_branch_and_holds_commits_on_it_is_refused() {
    check(&["c13 default branch ahead, a fork"]);
}

// TS: test/spawn-guard.test.ts:319
#[test]
fn the_default_branch_refusal_speaks_in_the_words_of_the_road_that_met_it() {
    check(&["c13 default branch ahead, a fork", "c13 default branch ahead, a run", "c13 push refused on the default branch"]);
}

// TS: test/spawn-guard.test.ts:334
#[test]
fn a_lead_ahead_of_its_upstream_pushes_its_branch_first() {
    check(&["c13 an upstream of another name", "c13 ahead pushes with changes staying", "c13 counts unknown pushes"]);
}

// TS: test/tree.test.ts:208
#[test]
fn a_childs_base_is_the_leads_pushed_branch_and_the_fork_says_what_stayed() {
    check(&["c13 ahead pushes with changes staying", "c13 push answers another branch name"]);
}

// TS: test/tree.test.ts:225
#[test]
fn a_lead_on_the_branch_its_work_started_from_pushes_nothing() {
    check(&["c13 on the start branch with changes"]);
}

// TS: test/tree.test.ts:234
#[test]
fn a_lead_whose_branch_the_remote_holds_whole_pushes_nothing_and_children_start_on_it() {
    check(&["c13 tracks itself with nothing new"]);
}

// TS: test/tree.test.ts:242
#[test]
fn a_lead_on_an_upstream_of_another_name_is_pushed() {
    check(&["c13 an upstream of another name"]);
}

// TS: test/tree.test.ts:250
#[test]
fn a_lead_cut_from_the_bases_remote_branch_with_no_commits_pushes_nothing() {
    check(&["c13 tracks the base with nothing new"]);
}

// TS: test/tree.test.ts:262
#[test]
fn a_lead_with_no_commits_over_the_base_pushes_nothing() {
    check(&["c13 no upstream and nothing new"]);
}

// TS: test/tree.test.ts:270
#[test]
fn a_lead_with_no_credential_refuses_the_fork_in_the_pushs_own_sentence() {
    check(&["c13 push refused with no credential", "c13 push refused with no code"]);
}

// TS: test/tree.test.ts:279
#[test]
fn a_lead_that_did_not_say_its_branch_refuses_the_fork() {
    check(&["c13 status refused"]);
}

// TS: test/bring-back.test.ts:133
#[test]
fn a_parent_that_did_not_say_which_branch_it_is_on_stops_the_fork() {
    check(&["c13 status refused with a code"]);
}

// TS: test/bring-back.test.ts:115
#[test]
fn a_branch_only_the_parent_holds_is_pushed_first() {
    check(&["c13 ahead pushes with changes staying"]);
}

// TS: test/fork-start-events.test.ts:146
#[test]
fn a_leads_default_branch_refusal_is_read_before_any_stage() {
    check(&["c13 default branch ahead, a fork"]);
}

// C14: a child put on its lead's branch.

#[test]
fn c14_start_child_on() {
    check(&["c14 a worktree takes the branch put", "c14 a copy without a worktree"]);
}

#[test]
fn c14_a_failed_start_deletes_the_child() {
    check(&["c14 a refused start deletes the child", "c14 a refused start whose delete fails", "c14 a failed persist deletes the child"]);
}

// TS: test/tree.test.ts:286
#[test]
fn a_child_that_would_not_go_onto_the_leads_branch_is_deleted_and_the_fork_refused() {
    check(&["c14 a refused start deletes the child"]);
}

// TS: test/bring-back.test.ts:173
#[test]
fn a_child_is_put_on_the_branch_its_lead_was_on_at_the_fork() {
    check(&["c14 a worktree takes the branch put", "c14 a copy without a worktree"]);
}

// C15: one repository.

#[test]
fn c15_repository_of_names_a_remote_by_host_and_lowercased_owner_and_repo() {
    let fixture = oracle("channels_copy");
    for row in fixture["repositories"].as_array().unwrap() {
        let remote = row["remote"].as_str().unwrap();
        let got = ChannelsArea::repository_of(remote);
        assert_eq!(json!(got), row["repo"], "repository_of({remote:?})");
    }
}

#[test]
fn c15_same_repository_table() {
    check(&["c15 saved remotes", "c15 a folder here's origin now"]);
}

// TS: test/spawn-guard.test.ts:415
#[test]
fn one_repository_is_one_whatever_its_remotes_spelling() {
    check(&["c15 saved remotes"]);
}

// TS: test/box-repository.test.ts:31
#[test]
fn a_folders_origin_is_read_now_and_matches_either_spelling() {
    check(&["c15 a folder here's origin now"]);
}

// TS: test/guest-spawn.test.ts:520
#[test]
fn a_leads_folder_whose_origin_moved_matches_the_projects_recorded_under_the_old_name_and_the_new() {
    check(&["c15 a folder here's origin now"]);
}

// TS: test/guest-spawn.test.ts:534
#[test]
fn a_leads_folder_whose_origin_points_at_a_fork_matches_no_other_projects_name() {
    check(&["c15 a folder here's origin now"]);
}

// TS: test/tree.test.ts:301
#[test]
fn a_childs_project_may_be_another_computers_project_of_the_same_repository_and_never_another() {
    check(&["c15 saved remotes", "c15 a folder here's origin now"]);
}

#[test]
fn c15_origin_read_only_for_a_folder_here() {
    check(&["c15 an origin is read only for a folder here"]);
}

/// `origin_now` called straight: the trimmed stdout where git exits 0, nothing otherwise, and the read it makes.
#[test]
fn c15_origin_now_answers_the_trimmed_origin_or_nothing() {
    let fixture = oracle("channels_copy");
    let world = fixture["scenarios"]["c15 a folder here's origin now"]["world"].clone();
    run(async move {
        let b = build(&world);
        let home = b.sim.projects["home"].clone();
        let git_here = b.sim.projects["git-here"].clone();
        let folder_box = b.sim.projects["folder-box"].clone();
        let boat = b.sim.projects["boat"].clone();
        *b.sim.origins.borrow_mut() = [
            ("/home/u/wsp".to_string(), Some("  git@github.com:me/lab.git \n\n".to_string())),
            ("/srv/folder".to_string(), Some("git@github.com:me/lab.git".to_string())),
        ]
        .into_iter()
        .collect();
        assert_eq!(b.area.origin_now(&home, None), "git@github.com:me/lab.git");
        assert_eq!(b.area.origin_now(&home, Some(&Caller::Road(WorkspaceOrigin::Here))), "git@github.com:me/lab.git");
        assert_eq!(b.area.origin_now(&git_here, None), "");
        assert_eq!(b.area.origin_now(&folder_box, None), "");
        assert_eq!(b.area.origin_now(&boat, None), "");
        *b.sim.origins.borrow_mut() = IndexMap::new();
        assert_eq!(b.area.origin_now(&home, None), "");
        let reads: Vec<Value> = b.sim.fx.all();
        assert_eq!(
            serde_json::to_string(&reads).unwrap(),
            serde_json::to_string(&json!([
                ["origin-read", "/home/u/wsp", 2000],
                ["origin-read", "/home/u/wsp", 2000],
                ["origin-read", "/home/u/wsp", 2000]
            ]))
            .unwrap()
        );
    });
}

// TS: test/channels.test.ts:226
#[test]
fn a_shell_on_a_folder_of_a_joined_computer_carries_no_token_while_the_broker_is_ready() {
    check(&["c11 a folder with a ready broker mints nothing", "c11 a folder on a computer never mints"]);
}

// TS: test/guest-spawn.test.ts:363
#[test]
fn a_child_that_leads_reads_its_own_branch_as_the_new_base() {
    check(&["c13 tracks itself with nothing new", "c13 an upstream of another name"]);
}
