// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/workspace-state.test.ts (:5-138 workspaceState, :140-148 isBilling, :150-179
//! reachShown, :181-325 a workspace's kind, :327-388 the screen-command lines, :390-406 whereWord, :408-419 the state
//! line, :421-439 unpushedLine) and packages/protocol/test/absent-computer.test.ts (:5-47 the absent computer, :48-84
//! this computer's own daemon, :86-148 reaching a computer, :150-180 one dial, :195-201 the image copy line).
//! Skipped: workspace-state.test.ts:243 (every kind has a row in the table: WORKSPACE_KIND_WORDS is not in the
//! stubs), :338 and :349 (screenCommandTyped and screenCommandsOf are not in the stubs); absent-computer.test.ts:183
//! (placeAddSheetWord is not in the stubs), :190 (reads only the constants PLACE_INSTALL.taken and
//! PLACES_WORDS.remove.leaveTakes of words::places, no function of this unit, and would pass against the stubs).

mod support;

use std::collections::HashSet;

use regex::Regex;
use serde_json::{json, Value};
use support::{over, workspace_view};
use wsp_wire::types::changes::Checkout;
use wsp_wire::types::workspace_state::{REPORTED_WORD, START_DAEMON_WORD};
use wsp_wire::views::harness::{HarnessCatalog, ScreenCommand};
use wsp_wire::views::place::PlaceBack;
use wsp_wire::views::workspace::{MachineState, ReachState, WorkspacePhase, WorkspaceStatus, WorkspaceView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::capabilities::PauseMode;
use wsp_words::project_path::place_owned_paths;
use wsp_words::words::computer::{provider_cannot_refusal, relayed_refusal, undriven_refusal, JOINED_COMPUTER, MACHINE_WSP_FORKS, THIS_COMPUTER};
use wsp_words::words::places::{image_copy_line, places_words};
use wsp_words::words::thread::still_working_line;
use wsp_words::words::turn::ROW_LINE_MAX;
use wsp_words::words::units::{CpuWord, NOT_ON_THIS_KIND};
use wsp_words::workspace_state::*;

fn wire<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

fn ws(phase: WorkspacePhase, machine_state: Option<MachineState>, reach: Option<ReachState>) -> WorkspaceStateInput {
    WorkspaceStateInput { phase, machine_state, reach, wake_refused: None }
}

fn phase_view(phase: &str) -> WorkspaceView {
    wire(over(workspace_view(), json!({ "phase": phase })))
}

fn where_record(fields: Value) -> WhereRecord {
    serde_json::from_value(fields).unwrap()
}

fn status(machine_state: &str, reach: &str) -> WorkspaceStatus {
    wire(over(
        workspace_view(),
        json!({ "machineState": machine_state, "reach": { "state": reach }, "size": { "cpu": 1, "memMb": 1024 }, "rateUsdPerHour": 0 }),
    ))
}

fn shown(previous: Option<ReachState>, probed: ReachState) -> ReachState {
    reach_shown(previous.as_ref(), &probed)
}

fn read(fields: Value) -> Checkout {
    wire(over(json!({ "branch": "fix/cart", "ahead": 0, "behind": 0, "changed": 0, "readAt": 1 }), fields))
}

fn screen_commands() -> Vec<ScreenCommand> {
    vec![
        wire(json!({ "name": "login", "control": "sign-in" })),
        wire(json!({ "name": "logout", "control": "sign-in" })),
        wire(json!({ "name": "model", "control": "model" })),
        wire(json!({ "name": "permissions", "control": "access" })),
        wire(json!({ "name": "config", "control": "settings" })),
        wire(json!({ "name": "help", "control": "docs" })),
    ]
}

fn catalog() -> HarnessCatalog {
    wire(json!({
        "harness": "claude", "label": "Claude Code", "source": "harness", "version": null, "models": [], "efforts": [],
        "contextWindows": [], "permissionModes": [], "steers": false, "renames": false, "images": false,
        "screenCommands": [
            { "name": "login", "control": "sign-in" },
            { "name": "logout", "control": "sign-in" },
            { "name": "model", "control": "model" },
            { "name": "permissions", "control": "access" },
            { "name": "config", "control": "settings" },
            { "name": "help", "control": "docs" }
        ]
    }))
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

mod workspace_state_test {
    use super::*;

    mod workspace_state {
        use super::*;

        #[test]
        fn phase_alone_running_pausing_napping_and_waking_each_have_one_word() {
            assert_eq!(workspace_state(&ws(WorkspacePhase::Running, None, None)), WorkspaceState::Running);
            assert_eq!(workspace_state(&ws(WorkspacePhase::Pausing, None, None)), WorkspaceState::Pausing);
            assert_eq!(workspace_state(&ws(WorkspacePhase::Napping, None, None)), WorkspaceState::Paused);
            assert_eq!(workspace_state(&ws(WorkspacePhase::Waking, None, None)), WorkspaceState::Waking);
        }

        #[test]
        fn phase_gone_is_gone_whatever_the_machine_or_the_reach_say_the_record_outlives_the() {
            assert_eq!(workspace_state(&ws(WorkspacePhase::Gone, None, None)), WorkspaceState::Gone);
            assert_eq!(
                workspace_state(&ws(WorkspacePhase::Gone, Some(MachineState::Running), Some(ReachState::Reachable))),
                WorkspaceState::Gone
            );
        }

        #[test]
        fn the_provider_s_word_overrides_the_phase_only_where_it_contradicts_it() {
            assert_eq!(
                workspace_state(&ws(WorkspacePhase::Running, Some(MachineState::Gone), Some(ReachState::Gone))),
                WorkspaceState::Gone
            );
            assert_eq!(
                workspace_state(&ws(WorkspacePhase::Napping, Some(MachineState::Gone), Some(ReachState::Gone))),
                WorkspaceState::Gone
            );
            assert_eq!(
                workspace_state(&ws(WorkspacePhase::Running, Some(MachineState::Paused), Some(ReachState::Napping))),
                WorkspaceState::Paused
            );
            assert_eq!(
                workspace_state(&ws(WorkspacePhase::Running, Some(MachineState::Starting), Some(ReachState::Unreachable))),
                WorkspaceState::Waking
            );
            assert_eq!(
                workspace_state(&ws(WorkspacePhase::Waking, Some(MachineState::Starting), Some(ReachState::Unreachable))),
                WorkspaceState::Waking
            );
            assert_eq!(
                workspace_state(&ws(WorkspacePhase::Pausing, Some(MachineState::Paused), Some(ReachState::Napping))),
                WorkspaceState::Pausing
            );
        }

        #[test]
        fn a_running_machine_whose_daemon_does_not_answer_is_unreachable_a_slow_edge_is_sti() {
            let running = |reach| workspace_state(&ws(WorkspacePhase::Running, Some(MachineState::Running), reach));
            assert_eq!(running(Some(ReachState::Unreachable)), WorkspaceState::Unreachable);
            assert_eq!(running(Some(ReachState::NoDaemon)), WorkspaceState::Unreachable);
            assert_eq!(running(Some(ReachState::Zombie)), WorkspaceState::Unreachable);
            assert_eq!(running(Some(ReachState::Slow)), WorkspaceState::Running);
            assert_eq!(running(Some(ReachState::Unsupported)), WorkspaceState::Running);
            assert_eq!(workspace_state(&ws(WorkspacePhase::Running, None, None)), WorkspaceState::Running);
        }

        #[test]
        fn a_status_carries_all_three_facts_so_every_surface_holding_one_reads_the_state_of() {
            let paused = status("paused", "napping");
            assert_eq!(workspace_state_of(&phase_view("running"), Some(&paused)), WorkspaceState::Paused);
            assert_eq!(
                workspace_state_of(&phase_view("running"), Some(&status("running", "no-daemon"))),
                WorkspaceState::Unreachable
            );
            assert_eq!(
                workspace_state_of(&phase_view("running"), Some(&status("running", "reachable"))),
                WorkspaceState::Running
            );
            assert_eq!(workspace_state_of(&phase_view("running"), None), WorkspaceState::Running);
            assert_eq!(workspace_state_of(&phase_view("napping"), None), WorkspaceState::Paused);
        }

        #[test]
        fn every_state_has_a_capitalised_word_and_a_caller_holding_no_pause_mode_reads_the() {
            let words = [
                (WorkspaceState::Running, "Running"),
                (WorkspaceState::Pausing, "Stopping"),
                (WorkspaceState::Paused, "Stopped"),
                (WorkspaceState::Waking, "Waking"),
                (WorkspaceState::Unreachable, "Unreachable"),
                (WorkspaceState::Gone, "Gone"),
                (WorkspaceState::Waiting, "Waiting"),
                (WorkspaceState::Released, "Deleted"),
            ];
            for (state, word) in words {
                assert_eq!(workspace_word(&state, None), word);
            }
        }

        #[test]
        fn says_paused_only_on_a_computer_whose_pause_keeps_the_machine_s_memory_and_stoppe() {
            assert_eq!(workspace_word(&WorkspaceState::Paused, Some(&PauseMode::Memory)), "Paused");
            assert_eq!(workspace_word(&WorkspaceState::Pausing, Some(&PauseMode::Memory)), "Pausing");
            assert_eq!(workspace_word(&WorkspaceState::Paused, Some(&PauseMode::Disk)), "Stopped");
            assert_eq!(workspace_word(&WorkspaceState::Pausing, Some(&PauseMode::Disk)), "Stopping");
            // Only the two words a nap has differ by mode; the rest are the machine's own state either way.
            for mode in [PauseMode::Memory, PauseMode::Disk] {
                for state in [WorkspaceState::Running, WorkspaceState::Waking, WorkspaceState::Unreachable, WorkspaceState::Gone] {
                    assert_eq!(workspace_word(&state, Some(&mode)), workspace_word(&state, None));
                }
            }
        }

        #[test]
        fn sendrefusal_names_the_state_in_the_sentence_the_composer_and_the_host_both_use_a() {
            let send = |state| send_refusal(&SendRefusalKind::State(state), None, None);
            assert_eq!(send(WorkspaceState::Running), None);
            assert_eq!(send(WorkspaceState::Pausing).as_deref(), Some("Workspace is pausing; wake it to send"));
            assert_eq!(send(WorkspaceState::Paused).as_deref(), Some("Workspace is paused; wake it to send"));
            assert_eq!(send(WorkspaceState::Waking).as_deref(), Some("Workspace is waking; sends open when it is running"));
            assert_eq!(
                send(WorkspaceState::Unreachable).as_deref(),
                Some("Workspace is unreachable; sends open when the machine answers")
            );
            assert_eq!(
                send(WorkspaceState::Gone).as_deref(),
                Some("This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to send, which brings back its home folder from the last saved nap")
            );
            // A send into a sub-thread waiting for its machine waits with it; an export has nothing to read yet.
            assert_eq!(send(WorkspaceState::Waiting), None);
            assert_eq!(
                action_refusal(&WorkspaceState::Waiting, "export", None, None).as_deref(),
                Some("Workspace is waiting for a machine; exports open when it is running")
            );
        }

        #[test]
        fn a_sub_machine_s_record_reads_waiting_while_its_spawn_is_queued_and_released_once() {
            assert_eq!(workspace_state(&ws(WorkspacePhase::Queued, None, None)), WorkspaceState::Waiting);
            assert_eq!(workspace_state(&ws(WorkspacePhase::Released, None, None)), WorkspaceState::Released);
            // The runtime's status says gone for the machine a release deleted, and the record's word still wins.
            assert_eq!(
                workspace_state(&ws(WorkspacePhase::Released, Some(MachineState::Gone), None)),
                WorkspaceState::Released
            );
            assert_eq!(workspace_word(&WorkspaceState::Released, None), "Deleted");
        }

        #[test]
        fn sendrefusal_is_one_table_over_everything_that_refuses_a_send_the_socket_the_look() {
            let blocks = [
                (SendBlock::Connecting, "Connecting to wsp"),
                (SendBlock::Reconnecting, "wsp is not running, reconnecting"),
                (SendBlock::Closed, "wsp is not running"),
                (SendBlock::NotFound, "Task not found"),
                (SendBlock::Loading, "Loading transcript"),
                (SendBlock::NoAgents, "the agents here have not answered yet"),
            ];
            for (kind, words) in blocks {
                assert_eq!(send_refusal(&SendRefusalKind::Block(kind), None, None).as_deref(), Some(words));
            }
            assert_eq!(
                still_working_line(Some("Fix the port list")),
                "Fix the port list replied, still working; the message runs as its next turn once that process exits"
            );
        }

        #[test]
        fn actionrefusal_is_the_same_sentence_for_any_verb_that_needs_the_machine_and_a_sen() {
            assert_eq!(action_refusal(&WorkspaceState::Running, "import", None, None), None);
            assert_eq!(
                action_refusal(&WorkspaceState::Paused, "import", None, None).as_deref(),
                Some("Workspace is paused; wake it to import")
            );
            assert_eq!(
                action_refusal(&WorkspaceState::Pausing, "export", None, None).as_deref(),
                Some("Workspace is pausing; wake it to export")
            );
            assert_eq!(
                action_refusal(&WorkspaceState::Waking, "import", None, None).as_deref(),
                Some("Workspace is waking; imports open when it is running")
            );
            assert_eq!(
                action_refusal(&WorkspaceState::Unreachable, "export", None, None).as_deref(),
                Some("Workspace is unreachable; exports open when the machine answers")
            );
            assert_eq!(
                action_refusal(&WorkspaceState::Gone, "import", Some("404"), None).as_deref(),
                Some("This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to import, which brings back its home folder from the last saved nap (404)")
            );
            for state in [
                WorkspaceState::Running,
                WorkspaceState::Pausing,
                WorkspaceState::Paused,
                WorkspaceState::Waking,
                WorkspaceState::Unreachable,
                WorkspaceState::Gone,
            ] {
                assert_eq!(
                    action_refusal(&state, "send", Some("x"), None),
                    send_refusal(&SendRefusalKind::State(state), Some("x"), None)
                );
            }
        }

        #[test]
        fn goneroadrefusal_one_sentence_for_the_rebuild_wherever_it_is_refused_saying_the_w() {
            // The app's row and the command line both refuse through this, so a workspace cannot read as answering in the
            // terminal while every pane on it reads that it is not.
            assert_eq!(
                gone_road_refusal(&WorkspaceState::Running, GoneRoad::Rebuild, None),
                "This one is running, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"
            );
            assert_eq!(
                gone_road_refusal(&WorkspaceState::Paused, GoneRoad::Rebuild, None),
                "This one is stopped, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"
            );
            assert_eq!(
                gone_road_refusal(&WorkspaceState::Paused, GoneRoad::Rebuild, Some(&PauseMode::Memory)),
                "This one is paused, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"
            );
            assert_eq!(
                gone_road_refusal(&WorkspaceState::Unreachable, GoneRoad::Rebuild, None),
                "This one is not answering yet; the rebuild is offered once it is gone"
            );
            assert_eq!(
                gone_road_refusal(&WorkspaceState::Unreachable, GoneRoad::Forget, None),
                "This one is not answering yet; the forget is offered once it is gone"
            );
            assert_eq!(
                gone_road_refusal(&WorkspaceState::Waking, GoneRoad::Forget, None),
                "Only a workspace whose computer is gone can be forgotten; this one is waking"
            );
        }

        #[test]
        fn needsrebuild_the_rebuild_is_a_road_for_a_gone_machine_a_zombie_or_one_the_provid() {
            assert!(needs_rebuild(&ws(WorkspacePhase::Gone, None, None)));
            assert!(needs_rebuild(&ws(WorkspacePhase::Running, Some(MachineState::Gone), Some(ReachState::Gone))));
            assert!(needs_rebuild(&ws(WorkspacePhase::Running, Some(MachineState::Running), Some(ReachState::Zombie))));
            assert!(!needs_rebuild(&ws(WorkspacePhase::Running, Some(MachineState::Running), Some(ReachState::Unreachable))));
            assert!(!needs_rebuild(&ws(WorkspacePhase::Napping, Some(MachineState::Paused), Some(ReachState::Napping))));
            // A paused machine whose last wake ran its asking out: the words are the record's, and a cleared one is null.
            let napping = |wake_refused: Option<&str>| WorkspaceStateInput {
                phase: WorkspacePhase::Napping,
                machine_state: Some(MachineState::Paused),
                reach: Some(ReachState::Napping),
                wake_refused: wake_refused.map(String::from),
            };
            assert!(needs_rebuild(&napping(Some("the provider answered none of 31 resume requests over 30m"))));
            assert!(!needs_rebuild(&napping(None)));
            // The state words never move for it: the machine is paused, and the wake is offered beside the rebuild.
            assert_eq!(
                workspace_state(&WorkspaceStateInput {
                    phase: WorkspacePhase::Napping,
                    machine_state: Some(MachineState::Paused),
                    reach: None,
                    wake_refused: Some("x".into()),
                }),
                WorkspaceState::Paused
            );
            assert!(!needs_rebuild(&ws(WorkspacePhase::Running, None, None)));
        }

        #[test]
        fn gonerefusal_is_one_sentence_per_verb_quoting_the_provider_when_the_caller_holds() {
            assert_eq!(
                gone_refusal(None, "wake", None),
                "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to wake, which brings back its home folder from the last saved nap"
            );
            assert_eq!(
                gone_refusal(None, "fork", Some("machine m1 is gone at the provider: Not found")),
                "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to fork, which brings back its home folder from the last saved nap (machine m1 is gone at the provider: Not found)"
            );
            assert_eq!(
                gone_refusal(None, "wake", Some("")),
                "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to wake, which brings back its home folder from the last saved nap"
            );
            assert_eq!(
                send_refusal(&SendRefusalKind::State(WorkspaceState::Gone), Some("machine m1 is gone at the provider: Not found"), None),
                Some(gone_refusal(None, "send", Some("machine m1 is gone at the provider: Not found")))
            );
            // The runtime holds the record, so its refusals lead with the workspace's name.
            assert_eq!(
                send_refusal(&SendRefusalKind::State(WorkspaceState::Gone), None, Some("api")).as_deref(),
                Some("api's machine is gone with its disk, so work that was not pushed is lost; rebuild it to send, which brings back its home folder from the last saved nap")
            );
            assert_eq!(
                action_refusal(&WorkspaceState::Gone, "import", Some("404"), Some("api")),
                Some(gone_refusal(Some("api"), "import", Some("404")))
            );
        }
    }

    mod is_billing {
        use super::*;

        #[test]
        fn a_machine_bills_while_it_runs_reachable_or_not_paused_moving_and_gone_ones_bill() {
            assert!(is_billing(&WorkspaceState::Running));
            assert!(is_billing(&WorkspaceState::Unreachable));
            for state in [WorkspaceState::Pausing, WorkspaceState::Paused, WorkspaceState::Waking, WorkspaceState::Gone] {
                assert!(!is_billing(&state));
            }
            // The provider's gone word wins over a record that still says running.
            assert!(!is_billing(&workspace_state(&ws(WorkspacePhase::Running, Some(MachineState::Gone), Some(ReachState::Gone)))));
        }
    }

    mod reach_shown {
        use super::*;

        #[test]
        fn one_silence_after_an_answer_keeps_the_answer_s_word_two_in_a_row_read_unreachabl() {
            assert_eq!(shown(Some(ReachState::Reachable), ReachState::Unreachable), ReachState::Reachable);
            assert_eq!(shown(Some(ReachState::Slow), ReachState::Unreachable), ReachState::Slow);
            assert_eq!(shown(Some(ReachState::Unreachable), ReachState::Unreachable), ReachState::Unreachable);
            assert_eq!(shown(Some(ReachState::Unreachable), ReachState::Reachable), ReachState::Reachable);
            assert_eq!(shown(Some(ReachState::Unreachable), ReachState::Slow), ReachState::Slow);
        }

        #[test]
        fn a_dead_daemon_port_waits_out_the_same_window_a_restart_under_the_unit_takes_abou() {
            assert_eq!(shown(Some(ReachState::Reachable), ReachState::NoDaemon), ReachState::Reachable);
            assert_eq!(shown(Some(ReachState::Slow), ReachState::NoDaemon), ReachState::Slow);
            assert_eq!(shown(Some(ReachState::NoDaemon), ReachState::NoDaemon), ReachState::NoDaemon);
            // The two unanswered words do not cover for each other: either one after the other is the second silence.
            assert_eq!(shown(Some(ReachState::Unreachable), ReachState::NoDaemon), ReachState::NoDaemon);
            assert_eq!(shown(Some(ReachState::NoDaemon), ReachState::Unreachable), ReachState::Unreachable);
        }

        #[test]
        fn a_machine_never_heard_from_or_one_already_failing_reads_its_silence_at_once() {
            assert_eq!(shown(None, ReachState::Unreachable), ReachState::Unreachable);
            assert_eq!(shown(None, ReachState::NoDaemon), ReachState::NoDaemon);
            for previous in [ReachState::NoDaemon, ReachState::Zombie, ReachState::Napping, ReachState::Gone, ReachState::Unsupported] {
                assert_eq!(shown(Some(previous), ReachState::Unreachable), ReachState::Unreachable);
            }
            for previous in [ReachState::Unreachable, ReachState::Zombie, ReachState::Napping, ReachState::Gone, ReachState::Unsupported] {
                assert_eq!(shown(Some(previous), ReachState::NoDaemon), ReachState::NoDaemon);
            }
        }

        #[test]
        fn every_answer_is_shown_as_it_came_whatever_went_before() {
            let answers = [
                ReachState::Reachable,
                ReachState::Slow,
                ReachState::Napping,
                ReachState::Unsupported,
                ReachState::Gone,
                ReachState::Zombie,
            ];
            for probed in answers {
                let mut previous = vec![None];
                previous.extend(answers.iter().map(|a| Some(*a)));
                previous.push(Some(ReachState::Unreachable));
                previous.push(Some(ReachState::NoDaemon));
                for previous in previous {
                    assert_eq!(shown(previous, probed), probed);
                }
            }
        }
    }

    mod what_a_workspace_s_kind_changes_about_its_words {
        use super::*;

        #[test]
        fn a_record_from_before_local_workspaces_existed_carries_no_kind_and_reads_as_a_pro() {
            assert_eq!(workspace_kind(None), WorkspaceKind::Cloud);
            assert_eq!(workspace_kind(Some(&WorkspaceKind::Cloud)), WorkspaceKind::Cloud);
            assert_eq!(workspace_kind(Some(&WorkspaceKind::Local)), WorkspaceKind::Local);
        }

        #[test]
        fn whether_a_workspace_is_this_computer_is_one_predicate_so_the_app_s_rows_and_wsp() {
            assert!(is_local_workspace(Some(&WorkspaceKind::Local)));
            assert!(!is_local_workspace(Some(&WorkspaceKind::Cloud)));
            // A record from before local workspaces existed is a provider fork, here as everywhere.
            assert!(!is_local_workspace(None));
        }

        #[test]
        fn a_machine_wsp_drives_has_a_state_a_bill_and_an_image_this_computer_has_none_of_t() {
            // access is what a thread here runs at when nobody names one: a fork costs a rebuild and nothing else, and this
            // computer is the person's own, so both run every action; the computers somebody owns and works on ask first.
            let sources: &[&str] = &["git", "github", "gitlab", "folder"];
            // `done` is the TS `expect.any(Function)`: it is read through `done_for`, and only its presence is asked here.
            let mut cloud = kind_words(&WorkspaceKind::Cloud);
            assert!(!std::mem::take(&mut cloud.on_delete.done).is_empty());
            assert_eq!(
                cloud,
                WorkspaceKindWords {
                    machine: MACHINE_WSP_FORKS,
                    row_reads_machine: true,
                    cpu: CpuWord::VCPU,
                    where_: Some("a provider"),
                    driven: true,
                    daemon: true,
                    metrics: ReadingRoad::Daemon,
                    processes: ReadingRoad::Daemon,
                    project_sources: sources,
                    copies_folder: false,
                    in_folder: false,
                    agents: true,
                    on_delete: MachineOnDelete { asked: "computer is deleted in the cloud".into(), done: String::new() },
                }
            );
            // This computer's load is read where it runs, in the host's own process, so its Live rows stand whether or not
            // the daemon its terminal and its processes ride ever started. A folder is
            // already on this computer, so an import registers its path and copies nothing, and a repo is cloned into a
            // folder the person names and worked there. Its row's second line is its
            // cores and memory in the size line a fork's row reads, in its own word for a cpu, since its cores are not virtual.
            let mut local = kind_words(&WorkspaceKind::Local);
            assert!(!std::mem::take(&mut local.on_delete.done).is_empty());
            assert_eq!(
                local,
                WorkspaceKindWords {
                    machine: THIS_COMPUTER,
                    row_reads_machine: true,
                    cpu: CpuWord::Cores,
                    where_: Some(THIS_COMPUTER),
                    driven: false,
                    daemon: true,
                    metrics: ReadingRoad::Host,
                    processes: ReadingRoad::Daemon,
                    project_sources: sources,
                    copies_folder: true,
                    in_folder: true,
                    agents: true,
                    on_delete: MachineOnDelete { asked: "computer is left as it is".into(), done: String::new() },
                }
            );
            // A folder on a computer the person joined: its threads run in it as they do here, and nothing is driven, copied
            // or billed; its row reads the computer's own name, since its size is that computer's.
            let mut place = kind_words(&WorkspaceKind::Place);
            assert!(!std::mem::take(&mut place.on_delete.done).is_empty());
            assert_eq!(
                place,
                WorkspaceKindWords {
                    machine: JOINED_COMPUTER,
                    row_reads_machine: false,
                    cpu: CpuWord::Cores,
                    where_: None,
                    driven: false,
                    daemon: true,
                    metrics: ReadingRoad::Daemon,
                    processes: ReadingRoad::Daemon,
                    project_sources: sources,
                    copies_folder: false,
                    in_folder: true,
                    agents: true,
                    on_delete: MachineOnDelete { asked: "computer is left as it is".into(), done: String::new() },
                }
            );
            // The machines wsp forks and this computer run agents that drive this host, each over its own road.
            assert!(agents_may_drive(&WorkspaceKind::Cloud));
            assert!(agents_may_drive(&WorkspaceKind::Local));
            // A thread in a folder on a joined computer reaches the wsp tools through the door that computer's daemon binds.
            assert!(agents_may_drive(&WorkspaceKind::Place));
            // Which kind of source a computer's projects come from: this computer copies a folder of yours beside itself and
            // clones a repo into a folder you name first, and a machine wsp forks takes a repo any of the three ways it can be
            // named and a folder here it clones and seeds from. Whether the folder is copied here is its own word, since a
            // computer that clones takes a folder as a source and still holds a checkout of it.
            assert_eq!(kind_words(&WorkspaceKind::Local).project_sources, ["git", "github", "gitlab", "folder"]);
            assert_eq!(kind_words(&WorkspaceKind::Cloud).project_sources, ["git", "github", "gitlab", "folder"]);
            assert!(kind_words(&WorkspaceKind::Local).copies_folder);
            assert!(!kind_words(&WorkspaceKind::Cloud).copies_folder);
        }

        #[test]
        fn the_two_readings_a_pane_waits_on_are_the_table_s_to_answer_so_nothing_sits_at_pe() {
            // Every kind reads both today: this computer off its own host, a fork off that machine's own /proc through
            // the daemon on it. The words stay so the next kind that reads neither says so.
            for kind in [WorkspaceKind::Local, WorkspaceKind::Cloud] {
                assert!(serves_reading(&kind, &KindReading::Metrics));
                assert!(serves_reading(&kind, &KindReading::Processes));
            }
            assert_eq!(NOT_ON_THIS_KIND, "not on this kind");
        }

        #[test]
        fn says_in_one_place_who_reads_each_of_them_so_a_pane_and_a_host_cannot_each_decide() {
            // The computer the host runs on is read where it runs. Every other machine answers for itself over the link a
            // pane holds, and the processes of every kind, this computer's included, are read on that machine.
            assert_eq!(reading_road(&WorkspaceKind::Local, &KindReading::Metrics), ReadingRoad::Host);
            assert_eq!(reading_road(&WorkspaceKind::Cloud, &KindReading::Metrics), ReadingRoad::Daemon);
            for kind in [WorkspaceKind::Cloud, WorkspaceKind::Local] {
                assert_eq!(reading_road(&kind, &KindReading::Processes), ReadingRoad::Daemon);
            }
            // Whichever road it is read on, a kind that answers a reading is a kind whose slot waits for a figure.
            for kind in [WorkspaceKind::Cloud, WorkspaceKind::Local] {
                assert!(serves_reading(&kind, &KindReading::Metrics));
            }
        }

        #[test]
        fn the_delete_sentence_says_what_the_delete_does_to_this_kind_s_computer() {
            // The words a person reads on the app's Delete row and in the command line's question: computer, never
            // machine, and the cloud, never a provider.
            assert_eq!(
                delete_notice(2.0, &WorkspaceKind::Cloud, None, None, None),
                "Its computer is deleted in the cloud; its record and 2 threads leave this computer."
            );
            assert_eq!(
                delete_notice(1.0, &WorkspaceKind::Local, None, None, None),
                "Its computer is left as it is; its record and 1 thread leave this computer."
            );
            assert_eq!(kind_words(&WorkspaceKind::Cloud).on_delete.done_for("m_1"), "computer m_1 is gone in the cloud");
            assert_eq!(kind_words(&WorkspaceKind::Local).on_delete.done_for("local"), "its computer is left as it is");
            // The sentence names no computer on a kind whose computer stays: naming it would read as the computer going.
            let said = kind_words(&WorkspaceKind::Local).on_delete.done_for("m_2");
            assert!(!said.contains("m_2"), "{said}");
        }

        #[test]
        fn several_copies_on_a_computer_somebody_joined_are_each_deleted_from_it_by_the_com() {
            let on = |name: &str| Some(StandsOn { name: name.into(), computer: "spoo".into() });
            let copies = vec![
                CopyToDelete { name: "fix-login".into(), kind: WorkspaceKind::Cloud, copy: None, machine_id: Some("wsp-workspace-ws_fix".into()), on: on("fix-login") },
                CopyToDelete { name: "fix-login-2".into(), kind: WorkspaceKind::Cloud, copy: None, machine_id: Some("wsp-workspace-ws_fix2".into()), on: on("fix-login-2") },
            ];
            let said = delete_copies_notice(&copies, 2.0);
            assert_eq!(said, "Each one's copy on spoo is deleted; their records and 2 threads leave this computer.");
            assert!(!said.contains("wsp-workspace-"), "{said}");
        }

        #[test]
        fn a_copy_on_a_computer_somebody_joined_is_deleted_from_that_computer_by_the_copy_s() {
            // A box is no cloud, and the machine's own id is wsp's: the person knows the copy and the computer by name.
            let on = StandsOn { name: "fix-login".into(), computer: "spoo".into() };
            assert_eq!(
                delete_notice(1.0, &WorkspaceKind::Cloud, None, Some("wsp-workspace-ws_fix"), Some(&on)),
                "Its copy on spoo is deleted; its record and 1 thread leave this computer."
            );
            assert_eq!(
                on_delete_of(&WorkspaceKind::Cloud, None, Some("wsp-workspace-ws_fix"), Some(&on)).done_for("wsp-workspace-ws_fix"),
                "fix-login is deleted from spoo"
            );
            // A create that made no machine there has none to delete, wherever it stood.
            assert_eq!(
                on_delete_of(&WorkspaceKind::Cloud, None, Some(""), Some(&on)).asked,
                "create failed before any computer was made, so there is none to delete"
            );
        }

        #[test]
        fn the_delete_sentence_for_a_wsp_worktree_says_the_worktree_is_removed_and_the_proj() {
            let copy = CopyAt { path: "/Users/dev/app-fix".into() };
            assert_eq!(
                delete_notice(1.0, &WorkspaceKind::Local, Some(&copy), None, None),
                "Its worktree at /Users/dev/app-fix is removed and the project folder is left as it is; its record and 1 thread leave this computer."
            );
            assert_eq!(
                on_delete_of(&WorkspaceKind::Local, Some(&copy), None, None).done_for("local"),
                "its worktree at /Users/dev/app-fix is removed and the project folder is left as it is"
            );
            assert_eq!(on_delete_of(&WorkspaceKind::Local, None, None, None), kind_words(&WorkspaceKind::Local).on_delete);
        }

        #[test]
        fn the_sentence_keep_this_one_confirms_with_says_what_goes_on_each_copy_s_own_kind() {
            // Forks on a cloud all go the same way, so one sentence covers them.
            let fork = |name: &str, machine_id: &str| CopyToDelete {
                name: name.into(),
                kind: WorkspaceKind::Cloud,
                copy: None,
                machine_id: Some(machine_id.into()),
                on: None,
            };
            let forks = vec![fork("fix (Opus 5.5)", "m_1"), fork("fix (GPT-5.6-Sol)", "m_2")];
            assert_eq!(
                delete_copies_notice(&forks, 2.0),
                "Each one's computer is deleted in the cloud; their records and 2 threads leave this computer."
            );
            // Copies on this Mac each leave their own folder, so each is named with its own.
            let local = |name: &str, path: &str| CopyToDelete {
                name: name.into(),
                kind: WorkspaceKind::Local,
                copy: Some(CopyAt { path: path.into() }),
                machine_id: Some("local".into()),
                on: None,
            };
            let copies = vec![local("fix (Opus 5.5)", "/Users/dev/fix-opus"), local("fix (Sonnet 5)", "/Users/dev/fix-sonnet")];
            assert_eq!(
                delete_copies_notice(&copies, 2.0),
                "fix (Opus 5.5): its worktree at /Users/dev/fix-opus is removed and the project folder is left as it is. fix (Sonnet 5): its worktree at /Users/dev/fix-sonnet is removed and the project folder is left as it is. Their records and 2 threads leave this computer."
            );
            for c in forks.iter().chain(copies.iter()) {
                let said = delete_copies_notice(&[c.clone(), c.clone()], 1.0);
                let asked = on_delete_of(&c.kind, c.copy.as_ref(), c.machine_id.as_deref(), c.on.as_ref()).asked;
                assert!(said.contains(&asked), "{said}");
            }
        }

        #[test]
        fn the_delete_sentence_for_a_create_that_failed_before_any_computer_was_made_names() {
            assert_eq!(
                delete_notice(0.0, &WorkspaceKind::Cloud, None, Some(""), None),
                "Its create failed before any computer was made, so there is none to delete; its record and 0 threads leave this computer."
            );
            assert_eq!(on_delete_of(&WorkspaceKind::Cloud, None, Some(""), None).done_for(""), "its create had made no computer");
        }

        #[test]
        fn the_phrase_for_this_computer_has_one_home_the_refusals_read_it_too() {
            assert_eq!(THIS_COMPUTER, "this computer");
            let said = relayed_refusal("mac");
            assert!(said.contains(THIS_COMPUTER), "{said}");
            assert_eq!(
                undriven_refusal("mac", &machine_word(&WorkspaceKind::Local), "be paused"),
                format!("mac is {THIS_COMPUTER}, which wsp does not run; it cannot be paused")
            );
        }

        #[test]
        fn every_kind_s_own_word_is_in_the_table_so_no_sentence_about_one_kind_s_machine_bo() {
            // The cloud kind's machine is wsp's own, whether the provider runs virtual machines or containers, so a refusal
            // on one names the provider's limit and never tells a person their fork is the computer they are sitting at.
            assert_eq!(machine_word(&WorkspaceKind::Cloud), MACHINE_WSP_FORKS);
            assert_eq!(machine_word(&WorkspaceKind::Local), THIS_COMPUTER);
            // The machine is the subject of every action these sentences take, since each action is the machine's own verb
            // phrase: the provider is where the road is missing, not the thing that would be resized or moved.
            assert_eq!(
                provider_cannot_refusal("api", &machine_word(&WorkspaceKind::Cloud), "be resized"),
                "api is a machine wsp forks, and it cannot be resized on the provider it runs on"
            );
            assert_eq!(
                provider_cannot_refusal("api", &machine_word(&WorkspaceKind::Cloud), "move to a newer image"),
                "api is a machine wsp forks, and it cannot move to a newer image on the provider it runs on"
            );
            for kind in [WorkspaceKind::Cloud, WorkspaceKind::Local] {
                assert_ne!(machine_word(&kind), "");
            }
            let words: HashSet<String> = [WorkspaceKind::Cloud, WorkspaceKind::Local].iter().map(machine_word).collect();
            assert_eq!(words.len(), 2);
        }
    }

    mod a_slash_command_that_works_only_in_the_cli_s_own_terminal {
        use super::*;

        #[test]
        fn the_composer_s_line_says_the_command_is_the_cli_s_own_screen_and_names_wsp_s_con() {
            let screen = screen_commands();
            let catalog = catalog();
            let cloud = Some(WorkspaceKind::Cloud);
            let local = Some(WorkspaceKind::Local);
            assert_eq!(
                screen_command_line(&screen[0], &catalog, cloud.as_ref()),
                "/login works only in Claude Code's own terminal; sign this workspace in from the Workspace panel"
            );
            assert_eq!(
                screen_command_line(&screen[1], &catalog, cloud.as_ref()),
                "/logout works only in Claude Code's own terminal; sign this workspace in from the Workspace panel"
            );
            assert_eq!(
                screen_command_line(&screen[0], &catalog, local.as_ref()),
                format!("/login works only in Claude Code's own terminal; sign in from a terminal on {THIS_COMPUTER}")
            );
            assert_eq!(
                screen_command_line(&screen[2], &catalog, cloud.as_ref()),
                "/model works only in Claude Code's own terminal; pick the model in the row under the box"
            );
            assert_eq!(
                screen_command_line(&screen[3], &catalog, local.as_ref()),
                "/permissions works only in Claude Code's own terminal; pick the access mode in the row under the box"
            );
            assert_eq!(
                screen_command_line(&screen[4], &catalog, cloud.as_ref()),
                "/config works only in Claude Code's own terminal; wsp's settings open from the command palette"
            );
            assert_eq!(
                screen_command_line(&screen[5], &catalog, cloud.as_ref()),
                "/help works only in Claude Code's own terminal; wsp's docs are at usewsp.com/docs"
            );
            // A record from before kinds existed is a provider fork, so it reads the machine's road.
            let said = screen_command_line(&screen[0], &catalog, None);
            assert!(said.contains("Workspace panel"), "{said}");
            // Every line is one sentence for the composer's slot: no line break, sentence case, nothing but words.
            let one_sentence = Regex::new(r"^/[a-z]+ works only in [^\n]+; [a-z][^\n]+[^.]$").unwrap();
            for command in &screen {
                for view in [&cloud, &local] {
                    let line = screen_command_line(command, &catalog, view.as_ref());
                    assert!(one_sentence.is_match(&line), "{line}");
                }
            }
        }

        #[test]
        fn the_composer_s_sign_in_line_and_a_refused_turn_s_half_read_one_road_rule_so_the() {
            let screen = screen_commands();
            let catalog = catalog();
            let cloud = Some(WorkspaceKind::Cloud);
            let local = Some(WorkspaceKind::Local);
            assert_eq!(sign_in_road(local.as_ref()), format!("sign in from a terminal on {THIS_COMPUTER}"));
            assert_eq!(sign_in_road(cloud.as_ref()), "sign this workspace in from the Workspace panel");
            // A thread in a project's folder on a box signs in where it runs, as one on this computer does.
            assert_eq!(sign_in_road(Some(&WorkspaceKind::Place)), "sign in from a terminal on the computer the thread runs on");
            // A record from before kinds existed is a provider fork, so it reads the machine's road here too.
            assert_eq!(sign_in_road(None), sign_in_road(cloud.as_ref()));
            for view in [&cloud, &local, &None] {
                assert_eq!(sign_in_refusal_line(view.as_ref()), format!("{}, then send again", sign_in_road(view.as_ref())));
                let said = screen_command_line(&screen[0], &catalog, view.as_ref());
                assert!(said.contains(&sign_in_road(view.as_ref())), "{said}");
            }
        }
    }

    mod where_a_workspace_runs_in_the_person_s_words {
        use super::*;

        #[test]
        fn names_the_provider_its_own_record_carries_never_the_id_that_provider_minted_for() {
            assert_eq!(where_word(&where_record(json!({ "kind": "cloud", "provider": "solari", "machineId": "fk_slr_1" })), None), "solari");
            assert_eq!(where_word(&where_record(json!({ "kind": "cloud", "provider": "docker", "machineId": "fk_dkr_9" })), None), "docker");
        }

        #[test]
        fn names_the_computer_this_host_holds_a_row_for_by_the_name_it_was_given_above_the() {
            assert_eq!(
                where_word(&where_record(json!({ "kind": "cloud", "provider": "docker", "machineId": "fk_dkr_1" })), Some("vps")),
                "vps"
            );
            assert_eq!(where_word(&where_record(json!({ "kind": "cloud", "machineId": "fk_slr_1" })), None), "a provider");
        }

        #[test]
        fn names_the_computer_the_host_runs_on_the_way_every_other_screen_names_it_off_what() {
            assert_eq!(
                where_word(&where_record(json!({ "kind": "local", "machineId": "local", "facts": { "os": "macOS 26.4" } })), None),
                "this Mac"
            );
            assert_eq!(
                where_word(&where_record(json!({ "kind": "local", "machineId": "local", "facts": { "os": "Ubuntu 24.04.1 LTS" } })), None),
                THIS_COMPUTER
            );
            assert_eq!(where_word(&where_record(json!({ "kind": "local", "machineId": "local" })), None), THIS_COMPUTER);
        }
    }

    mod the_line_a_verb_that_moved_a_workspace_prints {
        use super::*;

        #[test]
        fn prints_the_word_the_next_listing_will_print_in_the_lowercase_a_line_of_work_read() {
            assert_eq!(workspace_state_line("web", &WorkspaceState::Running, None), "web running");
            assert_eq!(workspace_state_line("web", &WorkspaceState::Paused, None), "web stopped");
            assert_eq!(workspace_state_line("web", &WorkspaceState::Waking, None), "web waking");
            assert_eq!(workspace_state_line("web", &WorkspaceState::Gone, None), "web gone");
        }

        #[test]
        fn says_a_machine_that_is_up_and_answering_nothing_is_up_and_answering_nothing_rath() {
            assert_eq!(workspace_state_line("web", &WorkspaceState::Unreachable, None), "web is up and not answering yet");
        }
    }

    mod what_a_copy_holds_that_its_remote_lacks {
        use super::*;

        #[test]
        fn names_its_commits_not_pushed_and_its_uncommitted_files_and_nothing_for_a_copy_th() {
            assert_eq!(unpushed_line("cart", Some(&read(json!({})))), None);
            assert_eq!(unpushed_line("cart", Some(&read(json!({ "ahead": 1 })))).as_deref(), Some("cart holds 1 commit not pushed"));
            assert_eq!(unpushed_line("cart", Some(&read(json!({ "changed": 2 })))).as_deref(), Some("cart holds 2 uncommitted files"));
            assert_eq!(
                unpushed_line("cart", Some(&read(json!({ "ahead": 3, "changed": 1 })))).as_deref(),
                Some("cart holds 3 commits not pushed and 1 uncommitted file")
            );
        }

        #[test]
        fn counts_the_stashes_a_clean_copy_still_holds_beside_its_commits_and_files() {
            assert_eq!(unpushed_line("cart", Some(&read(json!({ "stashes": 1 })))).as_deref(), Some("cart holds 1 stash"));
            assert_eq!(
                unpushed_line("cart", Some(&read(json!({ "ahead": 2, "changed": 1, "stashes": 3 })))).as_deref(),
                Some("cart holds 2 commits not pushed, 1 uncommitted file and 3 stashes")
            );
        }

        #[test]
        fn says_plainly_what_it_could_not_count_and_a_copy_never_read_may_hold_anything() {
            assert_eq!(
                unpushed_line("cart", Some(&read(json!({ "countsUnknown": true, "editsUnread": true })))).as_deref(),
                Some("cart holds commits it could not count and edits it could not read")
            );
            assert_eq!(unpushed_line("cart", None).as_deref(), Some("cart: could not read what is not pushed"));
        }
    }
}

mod absent_computer_test {
    use super::*;

    const NOW: f64 = 1_789_219_800_000.0; // Date.parse("2026-09-12T13:30:00.000Z")

    mod the_one_state_of_a_computer_that_is_not_answering {
        use super::*;

        fn reading() -> AbsentComputer {
            absent_computer("old-laptop", away_ms_of(Some("2026-09-12T12:52:00.000Z"), NOW))
        }

        #[test]
        fn says_the_same_thing_in_every_slot_that_has_to_hold_it() {
            let reading = reading();
            assert_eq!(reading.word, "Unreachable");
            // The table's slot stands beside three fact columns and holds one word; how long it has been is on the row's
            // title and in its detail's Answered row, which is where the figure already was.
            assert_eq!(reading.away, AwayWord::NoAnswer);
            assert_eq!(reading.line, "no answer 38 min, is it on?");
            assert_eq!(reading.said, "old-laptop is not answering");
            assert_eq!(reading.will.as_deref(), Some("it connects on its own when it is on"));
            assert_eq!(reading.sentence, "old-laptop is not answering; it connects on its own when it is on");
        }

        #[test]
        fn writes_the_row_s_line_under_the_cap_the_row_cuts_at_so_the_half_that_says_what_t() {
            let spans: [Option<f64>; 5] = [Some(0.0), Some(59.0 * 60_000.0), Some(23.0 * 3_600_000.0), Some(400.0 * 86_400_000.0), None];
            for ms in spans {
                let line = absent_computer("old-laptop", ms).line;
                assert!(utf16_len(&line) <= ROW_LINE_MAX, "{line}");
            }
        }

        #[test]
        fn says_nothing_about_how_long_it_has_been_when_this_host_never_heard_from_it() {
            assert_eq!(away_ms_of(None, NOW), None);
            assert_eq!(absent_computer("old-laptop", None).away, AwayWord::NoAnswer);
            assert_eq!(absent_computer("old-laptop", None).line, "no answer, is it on?");
        }

        #[test]
        fn states_the_silence_and_asks_and_never_claims_the_computer_is_off_which_this_host() {
            // A computer that is on and simply not answering reads the same line, so the line may not diagnose: the host
            // knows the silence and that the computer dials in by itself, and nothing else.
            let spans: [Option<f64>; 4] = [Some(0.0), Some(38.0 * 60_000.0), Some(23.0 * 3_600_000.0), None];
            for ms in spans {
                let line = absent_computer("hetzner", ms).line;
                assert!(!line.contains("turn it on"), "{line}");
                assert!(!line.contains("switch"), "{line}");
                assert!(line.starts_with("no answer"), "{line}");
            }
        }

        #[test]
        fn folds_a_computer_that_answers_nothing_into_the_state_word_every_surface_reads() {
            assert_eq!(workspace_state(&ws(WorkspacePhase::Running, None, Some(ReachState::Unreachable))), WorkspaceState::Unreachable);
        }
    }

    mod the_one_state_of_this_computer_s_own_daemon_while_it_is_not_running {
        use super::*;

        #[test]
        fn says_the_same_thing_in_every_slot_that_has_to_hold_it_and_the_ticket_s_sentence() {
            let reading = own_daemon_down("this Mac");
            assert_eq!(reading.said, "this Mac's terminals and files stopped");
            assert_eq!(reading.sentence, "this Mac's terminals and files stopped");
            assert_eq!(reading.word, "Stopped");
            assert_eq!(reading.away, AwayWord::Stopped);
            assert_eq!(reading.line, "stopped, start again");
        }

        #[test]
        fn never_says_unreachable_about_the_computer_the_app_is_drawn_on() {
            let reading = own_daemon_down("this Mac");
            for slot in [reading.word.as_str(), reading.away.as_str(), reading.line.as_str(), reading.said.as_str(), reading.sentence.as_str()] {
                assert!(!slot.to_lowercase().contains("unreachable"), "{slot}");
            }
        }

        #[test]
        fn writes_the_row_s_line_under_the_cap_the_row_cuts_at() {
            let reading = own_daemon_down("this Mac");
            assert!(utf16_len(&reading.line) <= ROW_LINE_MAX, "{}", reading.line);
        }

        #[test]
        fn carries_the_button_instead_of_a_second_sentence_since_the_host_holds_the_process() {
            let reading = own_daemon_down("this Mac");
            assert_eq!(reading.start.as_deref(), Some(START_DAEMON_WORD));
            assert_eq!(reading.will, None);
            // A computer this host only waits for has no such road, and says what happens next instead.
            assert_eq!(absent_computer("old-laptop", None).start, None);
            assert_eq!(absent_computer("old-laptop", None).will.as_deref(), Some("it connects on its own when it is on"));
        }

        #[test]
        fn reads_one_daemon_that_is_not_running_off_either_silence_a_probe_of_this_computer() {
            assert!(daemon_silent(Some(&ReachState::Unreachable)));
            assert!(daemon_silent(Some(&ReachState::NoDaemon)));
            for answering in [
                ReachState::Reachable,
                ReachState::Slow,
                ReachState::Napping,
                ReachState::Gone,
                ReachState::Unsupported,
                ReachState::Zombie,
            ] {
                assert!(!daemon_silent(Some(&answering)));
            }
            assert!(!daemon_silent(None));
        }
    }

    mod what_this_host_knows_about_reaching_a_computer_that_is_not_answering {
        use super::*;

        const AWAY_MS: f64 = 32.0 * 60_000.0;

        fn road(name: &str, road: Option<PlaceRoad>, away_ms: Option<f64>, dialled: Option<Dialled>) -> AbsentRoad {
            absent_road(&AbsentRoadInput { name: name.into(), road, away_ms, dialled })
        }

        #[test]
        fn names_the_login_it_dials_on_the_ssh_road_dates_the_silence_and_puts_the_last_ref() {
            let road = road(
                "vps",
                Some(PlaceRoad { ssh: Some("root@65.21.4.12".into()), ..Default::default() }),
                Some(AWAY_MS),
                Some(Dialled {
                    answered: false,
                    said: Some("ssh: connect to host 65.21.4.12 port 22: Connection refused".into()),
                    ..Default::default()
                }),
            );
            // The spec's row detail, one string: the address and the road it is.
            assert_eq!(road.address.as_deref(), Some("root@65.21.4.12 over ssh"));
            assert_eq!(road.answered, "32 min ago");
            assert_eq!(road.refused.as_deref(), Some("ssh: connect to host 65.21.4.12 port 22: Connection refused"));
            assert_eq!(
                road.sentence,
                "wsp logs in to vps at root@65.21.4.12 over ssh; it last answered 32 min ago. The last try said: ssh: connect to host 65.21.4.12 port 22: Connection refused"
            );
        }

        #[test]
        fn names_the_address_a_computer_that_joined_with_a_code_dialled_in_from_since_that() {
            let road = road(
                "old-laptop",
                Some(PlaceRoad { from: Some("192.168.1.34".into()), ..Default::default() }),
                Some(36.0 * 60_000.0),
                None,
            );
            assert_eq!(road.address.as_deref(), Some("192.168.1.34 dials in"));
            assert_eq!(road.refused, None);
            assert_eq!(road.sentence, "wsp waits for old-laptop to dial in, last from 192.168.1.34; it last answered 36 min ago.");
        }

        #[test]
        fn says_a_box_on_the_ssh_dial_back_dials_back_over_ssh_and_never_that_it_dials_in_f() {
            let back = || Some(PlaceBack { box_port: 4640.into() });
            let with_ssh = road(
                "spoo",
                Some(PlaceRoad { ssh: Some("root@spoo".into()), from: Some("127.0.0.1".into()), back: back() }),
                Some(AWAY_MS),
                None,
            );
            assert_eq!(with_ssh.address.as_deref(), Some("root@spoo over ssh"));
            assert_eq!(with_ssh.dials_back.as_deref(), Some("dials back over ssh (127.0.0.1:4640 on spoo)"));
            assert_eq!(
                with_ssh.sentence,
                "wsp logs in to spoo at root@spoo over ssh, and spoo dials back over ssh (127.0.0.1:4640 on spoo); it last answered 32 min ago."
            );
            let bare = road("spoo", Some(PlaceRoad { ssh: None, from: Some("127.0.0.1".into()), back: back() }), Some(AWAY_MS), None);
            assert_eq!(bare.address, None);
            assert_eq!(bare.sentence, "spoo dials back over ssh (127.0.0.1:4640 on spoo); it last answered 32 min ago.");
            for said in [
                with_ssh.sentence.clone(),
                bare.sentence.clone(),
                with_ssh.address.clone().unwrap_or_default(),
                bare.address.clone().unwrap_or_default(),
            ] {
                assert!(!said.contains("127.0.0.1 dials in"), "{said}");
            }
            assert!(!bare.sentence.contains("last from 127.0.0.1"), "{}", bare.sentence);
            assert_eq!(
                road(
                    "vps",
                    Some(PlaceRoad { ssh: Some("root@65.21.4.12".into()), from: Some("65.21.4.12".into()), back: None }),
                    Some(AWAY_MS),
                    None
                )
                .dials_back,
                None
            );
        }

        #[test]
        fn names_the_road_a_link_came_in_on_off_the_address_it_dialled_over_ssh_for_the_for() {
            let handed = ["http://192.168.1.20:4640", "https://h645d7f8a8d48cbd6.example"];
            assert_eq!(back_url(4640.0), "http://127.0.0.1:4640");
            assert_eq!(
                linked_over("http://127.0.0.1:4640", Some(&PlaceBack { box_port: 4640.into() }), &handed).as_deref(),
                Some("over ssh")
            );
            assert_eq!(
                linked_over("http://192.168.1.20:4640", None, &handed).as_deref(),
                Some("at http://192.168.1.20:4640")
            );
            assert_eq!(
                linked_over("https://h645d7f8a8d48cbd6.example", Some(&PlaceBack { box_port: 4640.into() }), &handed).as_deref(),
                Some("at https://h645d7f8a8d48cbd6.example")
            );
            // The box writes the address it dialled, so one this host never handed out is not said in this host's voice.
            assert_eq!(linked_over("http://127.0.0.1:4720", Some(&PlaceBack { box_port: 4640.into() }), &handed), None);
            assert_eq!(linked_over(&format!("https://whatever-it-likes.example/{}", "a".repeat(8000)), None, &handed), None);
        }

        #[test]
        fn says_so_plainly_on_a_computer_this_host_has_no_address_for_and_has_never_heard_f() {
            let road = road("old-laptop", None, None, None);
            assert_eq!(road.address, None);
            assert_eq!(road.answered, "not since it joined");
            assert_eq!(road.sentence, "wsp waits for old-laptop to dial in; it has not answered since it joined.");
        }

        #[test]
        fn keeps_a_refusal_off_the_reading_when_the_last_dial_answered() {
            assert_eq!(
                road("vps", None, Some(AWAY_MS), Some(Dialled { answered: true, round_trip_ms: Some(14.0), said: None })).refused,
                None
            );
        }

        #[test]
        fn marks_a_fact_the_computer_has_stopped_answering_for_as_the_reading_it_is_never_a() {
            assert_eq!(last_known("Debian 12", Some(AWAY_MS), None), "Debian 12, last seen 32 min ago");
            // Nothing to date it against leaves the fact as it stands rather than inventing a span.
            assert_eq!(last_known("Debian 12", None, None), "Debian 12");
            // A figure that grows while the computer is up is dated by the report it was read in, not by the silence: the
            // two spans differ by however long the link was held after that report.
            assert_eq!(last_known("4h 12m", Some(3.0 * 3_600_000.0), Some(REPORTED_WORD)), "4h 12m, reported 3 h ago");
        }
    }

    mod what_one_dial_of_a_computer_answers {
        use super::*;

        #[test]
        fn reads_a_frame_the_link_carried_as_the_computer_itself_answering_with_how_long_it() {
            assert_eq!(
                place_dial_line("vps", None, true, &Dialled { answered: true, round_trip_ms: Some(14.0), said: None }),
                "vps answered in 14 ms."
            );
        }

        #[test]
        fn reads_an_ssh_login_that_answered_while_the_link_is_down_as_the_computer_being_on() {
            let road = PlaceRoad { ssh: Some("root@65.21.4.12".into()), ..Default::default() };
            let line = place_dial_line("vps", Some(&road), false, &Dialled { answered: true, round_trip_ms: Some(412.0), said: None });
            assert_eq!(line, "root@65.21.4.12 answered over ssh in 412 ms, so the computer is on; the agent on it is not dialling this host.");
        }

        #[test]
        fn hands_back_the_road_s_own_sentence_when_nothing_answered_rather_than_a_wsp_shape() {
            let said = "ssh: connect to host 65.21.4.12 port 22: Connection refused";
            let road = PlaceRoad { ssh: Some("root@65.21.4.12".into()), ..Default::default() };
            assert_eq!(
                place_dial_line("vps", Some(&road), false, &Dialled { answered: false, round_trip_ms: None, said: Some(said.into()) }),
                said
            );
        }

        #[test]
        fn says_there_is_no_road_at_all_on_a_computer_that_joined_by_typing_a_code() {
            let said = place_no_dial_line("old-laptop");
            assert!(said.contains("joined by typing a code"), "{said}");
        }

        #[test]
        fn reads_the_road_a_dial_would_take_and_none_at_all_where_the_only_thing_left_is_to() {
            let ssh = |login: &str| PlaceRoad { ssh: Some(login.into()), ..Default::default() };
            // Holding its link: the frame rides the link, whatever else the row carries.
            assert_eq!(place_dial_road(Some(true), None), Some(PlaceDialRoad::Link));
            assert_eq!(place_dial_road(Some(true), Some(&ssh("root@65.21.4.12"))), Some(PlaceDialRoad::Link));
            // Down, and installed over ssh: the login is the road, and the button says so rather than saying try now.
            assert_eq!(place_dial_road(Some(false), Some(&ssh("root@65.21.4.12"))), Some(PlaceDialRoad::Ssh));
            // Joined by typing a code: an address it dialled in from is no road back to it, and neither is an empty login.
            assert_eq!(
                place_dial_road(Some(false), Some(&PlaceRoad { from: Some("192.168.1.34".into()), ..Default::default() })),
                None
            );
            assert_eq!(place_dial_road(Some(false), Some(&ssh(""))), None);
            assert_eq!(place_dial_road(None, None), None);
        }
    }

    mod what_the_two_screens_say_wsp_puts_on_a_computer {
        use super::*;

        #[test]
        fn tells_a_person_the_copy_of_their_image_comes_off_with_wsp_since_it_sits_in_the_r() {
            // The runtime folder is no path of the home's own list; the leave as root takes it on its own road.
            assert!(!place_owned_paths("/home/maya").iter().any(|path| path.contains("var/lib")));
            assert!(places_words::remove::LEAVE_TAKES.contains(&image_copy_line(None)), "{}", places_words::remove::LEAVE_TAKES);
            assert!(!places_words::remove::LEAVE_TAKES.contains("Docker"));
        }
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use wsp_words::workspace_state::*;

    use serde_json::{json, Value};
    use wsp_wire::views::place::PlaceBack;
    use wsp_wire::views::workspace::{ReachState, WorkspaceView};
    use wsp_wire::views::workspace_kind::WorkspaceKind;
    use wsp_wire::wire::capabilities::PauseMode;

    fn from<T: serde::de::DeserializeOwned>(v: Value) -> T {
        serde_json::from_value(v).unwrap()
    }
    fn kind(k: &str) -> WorkspaceKind {
        from(json!(k))
    }
    fn state(s: &str) -> WorkspaceState {
        from(json!(s))
    }
    fn pause(p: &str) -> PauseMode {
        from(json!(p))
    }
    fn block(b: &str) -> SendBlock {
        from(json!(b))
    }
    fn reach(r: &str) -> ReachState {
        from(json!(r))
    }
    fn reach_word(r: ReachState) -> String {
        serde_json::to_value(r).unwrap().as_str().unwrap().to_string()
    }
    fn back(v: Value) -> PlaceBack {
        from(v)
    }
    fn road(v: Value) -> PlaceRoad {
        PlaceRoad { ssh: v["ssh"].as_str().map(Into::into), from: v["from"].as_str().map(Into::into), back: None }
    }
    fn view(v: Value) -> WorkspaceView {
        from(crate::support::over(crate::support::workspace_view(), v))
    }
    fn input(v: Value) -> WorkspaceStateInput {
        WorkspaceStateInput {
            phase: from(v["phase"].clone()),
            machine_state: v.get("machineState").map(|m| from(m.clone())),
            reach: v.get("reach").map(|r| from(r.clone())),
            wake_refused: v["wakeRefused"].as_str().map(Into::into),
        }
    }

    #[test]
    fn the_sign_in_road_and_its_refusal_follow_the_kind() {
        assert_eq!(sign_in_road(None), "sign this workspace in from the Workspace panel"); // signInRoad({})
        assert_eq!(sign_in_refusal_line(None), "sign this workspace in from the Workspace panel, then send again"); // signInRefusalLine({})
        assert_eq!(sign_in_road(Some(&kind("cloud"))), "sign this workspace in from the Workspace panel"); // signInRoad({"kind":"cloud"})
        assert_eq!(sign_in_refusal_line(Some(&kind("cloud"))), "sign this workspace in from the Workspace panel, then send again"); // signInRefusalLine({"kind":"cloud"})
        assert_eq!(sign_in_road(Some(&kind("local"))), "sign in from a terminal on this computer"); // signInRoad({"kind":"local"})
        assert_eq!(sign_in_refusal_line(Some(&kind("local"))), "sign in from a terminal on this computer, then send again"); // signInRefusalLine({"kind":"local"})
        assert_eq!(sign_in_road(Some(&kind("place"))), "sign in from a terminal on the computer the thread runs on"); // signInRoad({"kind":"place"})
        assert_eq!(sign_in_refusal_line(Some(&kind("place"))), "sign in from a terminal on the computer the thread runs on, then send again"); // signInRefusalLine({"kind":"place"})
        assert_eq!(signed_out_line("Codex", "spoo"), "Codex is not signed in on spoo; sign in from a terminal there before you send"); // signedOutLine("Codex", "spoo")
    }

    #[test]
    fn each_kind_says_its_own_machine_and_what_a_delete_takes() {
        assert_eq!(agents_kind_refusal(&kind("cloud")), "agents on a machine wsp forks cannot drive this host, so the spawn switch would hand out a token that opens nothing; it is for the machines wsp forks and this computer"); // agentsKindRefusal("cloud")
        assert_eq!(machine_word(&kind("cloud")), "a machine wsp forks"); // machineWord("cloud")
        assert_eq!(thread_place(&kind("cloud"), "spoo"), "a machine wsp forks"); // threadPlace("cloud", "spoo")
        assert_eq!(delete_notice(1.0, &kind("cloud"), None, None, None), "Its computer is deleted in the cloud; its record and 1 thread leave this computer."); // deleteNotice(1, "cloud")
        assert_eq!(delete_notice(3.0, &kind("cloud"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None), "Its worktree at /r/wt/a is removed and the project folder is left as it is; its record and 3 threads leave this computer."); // deleteNotice(3, "cloud", {path:"/r/wt/a"})
        assert_eq!(delete_notice(0.0, &kind("cloud"), None, Some(""), None), "Its create failed before any computer was made, so there is none to delete; its record and 0 threads leave this computer."); // deleteNotice(0, "cloud", undefined, "")
        assert_eq!(delete_notice(2.0, &kind("cloud"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })), "Its copy on spoo is deleted; its record and 2 threads leave this computer."); // deleteNotice(2, "cloud", undefined, "m1", {name:"web", computer:"spoo"})
        assert_eq!(on_delete_of(&kind("cloud"), None, None, None).done_for("m-7"), "computer m-7 is gone in the cloud"); // onDeleteOf("cloud").done("m-7")
        assert_eq!(on_delete_of(&kind("cloud"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })).done_for("m1"), "web is deleted from spoo"); // onDeleteOf("cloud", undefined, "m1", {name:"web", computer:"spoo"}).done("m1")
        assert_eq!(on_delete_of(&kind("cloud"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None).done_for("m1"), "its worktree at /r/wt/a is removed and the project folder is left as it is"); // onDeleteOf("cloud", {path:"/r/wt/a"}).done("m1")
        assert_eq!(on_delete_of(&kind("cloud"), None, Some(""), None).done_for("x"), "its create had made no computer"); // onDeleteOf("cloud", undefined, "").done("x")
        assert_eq!(agents_kind_refusal(&kind("local")), "agents on this computer cannot drive this host, so the spawn switch would hand out a token that opens nothing; it is for the machines wsp forks and this computer"); // agentsKindRefusal("local")
        assert_eq!(machine_word(&kind("local")), "this computer"); // machineWord("local")
        assert_eq!(thread_place(&kind("local"), "spoo"), "spoo"); // threadPlace("local", "spoo")
        assert_eq!(delete_notice(1.0, &kind("local"), None, None, None), "Its computer is left as it is; its record and 1 thread leave this computer."); // deleteNotice(1, "local")
        assert_eq!(delete_notice(3.0, &kind("local"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None), "Its worktree at /r/wt/a is removed and the project folder is left as it is; its record and 3 threads leave this computer."); // deleteNotice(3, "local", {path:"/r/wt/a"})
        assert_eq!(delete_notice(0.0, &kind("local"), None, Some(""), None), "Its create failed before any computer was made, so there is none to delete; its record and 0 threads leave this computer."); // deleteNotice(0, "local", undefined, "")
        assert_eq!(delete_notice(2.0, &kind("local"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })), "Its copy on spoo is deleted; its record and 2 threads leave this computer."); // deleteNotice(2, "local", undefined, "m1", {name:"web", computer:"spoo"})
        assert_eq!(on_delete_of(&kind("local"), None, None, None).done_for("m-7"), "its computer is left as it is"); // onDeleteOf("local").done("m-7")
        assert_eq!(on_delete_of(&kind("local"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })).done_for("m1"), "web is deleted from spoo"); // onDeleteOf("local", undefined, "m1", {name:"web", computer:"spoo"}).done("m1")
        assert_eq!(on_delete_of(&kind("local"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None).done_for("m1"), "its worktree at /r/wt/a is removed and the project folder is left as it is"); // onDeleteOf("local", {path:"/r/wt/a"}).done("m1")
        assert_eq!(on_delete_of(&kind("local"), None, Some(""), None).done_for("x"), "its create had made no computer"); // onDeleteOf("local", undefined, "").done("x")
        assert_eq!(agents_kind_refusal(&kind("place")), "agents on a computer you joined cannot drive this host, so the spawn switch would hand out a token that opens nothing; it is for the machines wsp forks and this computer"); // agentsKindRefusal("place")
        assert_eq!(machine_word(&kind("place")), "a computer you joined"); // machineWord("place")
        assert_eq!(thread_place(&kind("place"), "spoo"), "spoo"); // threadPlace("place", "spoo")
        assert_eq!(delete_notice(1.0, &kind("place"), None, None, None), "Its computer is left as it is; its record and 1 thread leave this computer."); // deleteNotice(1, "place")
        assert_eq!(delete_notice(3.0, &kind("place"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None), "Its worktree at /r/wt/a is removed and the project folder is left as it is; its record and 3 threads leave this computer."); // deleteNotice(3, "place", {path:"/r/wt/a"})
        assert_eq!(delete_notice(0.0, &kind("place"), None, Some(""), None), "Its create failed before any computer was made, so there is none to delete; its record and 0 threads leave this computer."); // deleteNotice(0, "place", undefined, "")
        assert_eq!(delete_notice(2.0, &kind("place"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })), "Its copy on spoo is deleted; its record and 2 threads leave this computer."); // deleteNotice(2, "place", undefined, "m1", {name:"web", computer:"spoo"})
        assert_eq!(on_delete_of(&kind("place"), None, None, None).done_for("m-7"), "its computer is left as it is"); // onDeleteOf("place").done("m-7")
        assert_eq!(on_delete_of(&kind("place"), None, Some("m1"), Some(&StandsOn { name: "web".into(), computer: "spoo".into() })).done_for("m1"), "web is deleted from spoo"); // onDeleteOf("place", undefined, "m1", {name:"web", computer:"spoo"}).done("m1")
        assert_eq!(on_delete_of(&kind("place"), Some(&CopyAt { path: "/r/wt/a".into() }), None, None).done_for("m1"), "its worktree at /r/wt/a is removed and the project folder is left as it is"); // onDeleteOf("place", {path:"/r/wt/a"}).done("m1")
        assert_eq!(on_delete_of(&kind("place"), None, Some(""), None).done_for("x"), "its create had made no computer"); // onDeleteOf("place", undefined, "").done("x")
    }

    #[test]
    fn each_state_reads_one_word_and_refuses_in_it() {
        assert_eq!(workspace_word(&state("running"), None), "Running"); // workspaceWord("running")
        assert_eq!(workspace_word(&state("running"), Some(&pause("memory"))), "Running"); // workspaceWord("running", "memory")
        assert_eq!(workspace_state_line("web", &state("running"), None), "web running"); // workspaceStateLine("web", "running")
        assert_eq!(paused_or_pausing(&state("running")), false); // pausedOrPausing("running")
        assert_eq!(is_billing(&state("running")), true); // isBilling("running")
        assert_eq!(action_refusal(&state("running"), "open a terminal", None, None).unwrap_or_else(|| "null".into()), "null"); // String(actionRefusal("running", "open a terminal") ?? "null")
        assert_eq!(action_refusal(&state("running"), "open a terminal", Some("its disk went with it"), Some("web")).unwrap_or_else(|| "null".into()), "null"); // String(actionRefusal("running", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("running")), None, None).unwrap_or_else(|| "null".into()), "null"); // String(sendRefusal("running") ?? "null")
        assert_eq!(gone_road_refusal(&state("running"), GoneRoad::Rebuild, None), "This one is running, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"); // goneRoadRefusal("running", "rebuild")
        assert_eq!(gone_road_refusal(&state("running"), GoneRoad::Forget, Some(&pause("disk"))), "Only a workspace whose computer is gone can be forgotten; this one is running"); // goneRoadRefusal("running", "forget", "disk")
        assert_eq!(workspace_word(&state("pausing"), None), "Stopping"); // workspaceWord("pausing")
        assert_eq!(workspace_word(&state("pausing"), Some(&pause("memory"))), "Pausing"); // workspaceWord("pausing", "memory")
        assert_eq!(workspace_state_line("web", &state("pausing"), None), "web stopping"); // workspaceStateLine("web", "pausing")
        assert_eq!(paused_or_pausing(&state("pausing")), true); // pausedOrPausing("pausing")
        assert_eq!(is_billing(&state("pausing")), false); // isBilling("pausing")
        assert_eq!(action_refusal(&state("pausing"), "open a terminal", None, None).unwrap_or_else(|| "null".into()), "Workspace is pausing; wake it to open a terminal"); // String(actionRefusal("pausing", "open a terminal") ?? "null")
        assert_eq!(action_refusal(&state("pausing"), "open a terminal", Some("its disk went with it"), Some("web")).unwrap_or_else(|| "null".into()), "Workspace is pausing; wake it to open a terminal"); // String(actionRefusal("pausing", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("pausing")), None, None).unwrap_or_else(|| "null".into()), "Workspace is pausing; wake it to send"); // String(sendRefusal("pausing") ?? "null")
        assert_eq!(gone_road_refusal(&state("pausing"), GoneRoad::Rebuild, None), "This one is stopping, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"); // goneRoadRefusal("pausing", "rebuild")
        assert_eq!(gone_road_refusal(&state("pausing"), GoneRoad::Forget, Some(&pause("disk"))), "Only a workspace whose computer is gone can be forgotten; this one is stopping"); // goneRoadRefusal("pausing", "forget", "disk")
        assert_eq!(workspace_word(&state("paused"), None), "Stopped"); // workspaceWord("paused")
        assert_eq!(workspace_word(&state("paused"), Some(&pause("memory"))), "Paused"); // workspaceWord("paused", "memory")
        assert_eq!(workspace_state_line("web", &state("paused"), None), "web stopped"); // workspaceStateLine("web", "paused")
        assert_eq!(paused_or_pausing(&state("paused")), true); // pausedOrPausing("paused")
        assert_eq!(is_billing(&state("paused")), false); // isBilling("paused")
        assert_eq!(action_refusal(&state("paused"), "open a terminal", None, None).unwrap_or_else(|| "null".into()), "Workspace is paused; wake it to open a terminal"); // String(actionRefusal("paused", "open a terminal") ?? "null")
        assert_eq!(action_refusal(&state("paused"), "open a terminal", Some("its disk went with it"), Some("web")).unwrap_or_else(|| "null".into()), "Workspace is paused; wake it to open a terminal"); // String(actionRefusal("paused", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("paused")), None, None).unwrap_or_else(|| "null".into()), "Workspace is paused; wake it to send"); // String(sendRefusal("paused") ?? "null")
        assert_eq!(gone_road_refusal(&state("paused"), GoneRoad::Rebuild, None), "This one is stopped, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"); // goneRoadRefusal("paused", "rebuild")
        assert_eq!(gone_road_refusal(&state("paused"), GoneRoad::Forget, Some(&pause("disk"))), "Only a workspace whose computer is gone can be forgotten; this one is stopped"); // goneRoadRefusal("paused", "forget", "disk")
        assert_eq!(workspace_word(&state("waking"), None), "Waking"); // workspaceWord("waking")
        assert_eq!(workspace_word(&state("waking"), Some(&pause("memory"))), "Waking"); // workspaceWord("waking", "memory")
        assert_eq!(workspace_state_line("web", &state("waking"), None), "web waking"); // workspaceStateLine("web", "waking")
        assert_eq!(paused_or_pausing(&state("waking")), false); // pausedOrPausing("waking")
        assert_eq!(is_billing(&state("waking")), false); // isBilling("waking")
        assert_eq!(action_refusal(&state("waking"), "open a terminal", None, None).unwrap_or_else(|| "null".into()), "Workspace is waking; open a terminals open when it is running"); // String(actionRefusal("waking", "open a terminal") ?? "null")
        assert_eq!(action_refusal(&state("waking"), "open a terminal", Some("its disk went with it"), Some("web")).unwrap_or_else(|| "null".into()), "Workspace is waking; open a terminals open when it is running"); // String(actionRefusal("waking", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("waking")), None, None).unwrap_or_else(|| "null".into()), "Workspace is waking; sends open when it is running"); // String(sendRefusal("waking") ?? "null")
        assert_eq!(gone_road_refusal(&state("waking"), GoneRoad::Rebuild, None), "This one is waking, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"); // goneRoadRefusal("waking", "rebuild")
        assert_eq!(gone_road_refusal(&state("waking"), GoneRoad::Forget, Some(&pause("disk"))), "Only a workspace whose computer is gone can be forgotten; this one is waking"); // goneRoadRefusal("waking", "forget", "disk")
        assert_eq!(workspace_word(&state("unreachable"), None), "Unreachable"); // workspaceWord("unreachable")
        assert_eq!(workspace_word(&state("unreachable"), Some(&pause("memory"))), "Unreachable"); // workspaceWord("unreachable", "memory")
        assert_eq!(workspace_state_line("web", &state("unreachable"), None), "web is up and not answering yet"); // workspaceStateLine("web", "unreachable")
        assert_eq!(paused_or_pausing(&state("unreachable")), false); // pausedOrPausing("unreachable")
        assert_eq!(is_billing(&state("unreachable")), true); // isBilling("unreachable")
        assert_eq!(action_refusal(&state("unreachable"), "open a terminal", None, None).unwrap_or_else(|| "null".into()), "Workspace is unreachable; open a terminals open when the machine answers"); // String(actionRefusal("unreachable", "open a terminal") ?? "null")
        assert_eq!(action_refusal(&state("unreachable"), "open a terminal", Some("its disk went with it"), Some("web")).unwrap_or_else(|| "null".into()), "Workspace is unreachable; open a terminals open when the machine answers"); // String(actionRefusal("unreachable", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("unreachable")), None, None).unwrap_or_else(|| "null".into()), "Workspace is unreachable; sends open when the machine answers"); // String(sendRefusal("unreachable") ?? "null")
        assert_eq!(gone_road_refusal(&state("unreachable"), GoneRoad::Rebuild, None), "This one is not answering yet; the rebuild is offered once it is gone"); // goneRoadRefusal("unreachable", "rebuild")
        assert_eq!(gone_road_refusal(&state("unreachable"), GoneRoad::Forget, Some(&pause("disk"))), "This one is not answering yet; the forget is offered once it is gone"); // goneRoadRefusal("unreachable", "forget", "disk")
        assert_eq!(workspace_word(&state("gone"), None), "Gone"); // workspaceWord("gone")
        assert_eq!(workspace_word(&state("gone"), Some(&pause("memory"))), "Gone"); // workspaceWord("gone", "memory")
        assert_eq!(workspace_state_line("web", &state("gone"), None), "web gone"); // workspaceStateLine("web", "gone")
        assert_eq!(paused_or_pausing(&state("gone")), false); // pausedOrPausing("gone")
        assert_eq!(is_billing(&state("gone")), false); // isBilling("gone")
        assert_eq!(action_refusal(&state("gone"), "open a terminal", None, None).unwrap_or_else(|| "null".into()), "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to open a terminal, which brings back its home folder from the last saved nap"); // String(actionRefusal("gone", "open a terminal") ?? "null")
        assert_eq!(action_refusal(&state("gone"), "open a terminal", Some("its disk went with it"), Some("web")).unwrap_or_else(|| "null".into()), "web's machine is gone with its disk, so work that was not pushed is lost; rebuild it to open a terminal, which brings back its home folder from the last saved nap (its disk went with it)"); // String(actionRefusal("gone", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("gone")), None, None).unwrap_or_else(|| "null".into()), "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to send, which brings back its home folder from the last saved nap"); // String(sendRefusal("gone") ?? "null")
        assert_eq!(gone_road_refusal(&state("gone"), GoneRoad::Rebuild, None), "This one is gone, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"); // goneRoadRefusal("gone", "rebuild")
        assert_eq!(gone_road_refusal(&state("gone"), GoneRoad::Forget, Some(&pause("disk"))), "Only a workspace whose computer is gone can be forgotten; this one is gone"); // goneRoadRefusal("gone", "forget", "disk")
        assert_eq!(workspace_word(&state("waiting"), None), "Waiting"); // workspaceWord("waiting")
        assert_eq!(workspace_word(&state("waiting"), Some(&pause("memory"))), "Waiting"); // workspaceWord("waiting", "memory")
        assert_eq!(workspace_state_line("web", &state("waiting"), None), "web waiting"); // workspaceStateLine("web", "waiting")
        assert_eq!(paused_or_pausing(&state("waiting")), false); // pausedOrPausing("waiting")
        assert_eq!(is_billing(&state("waiting")), false); // isBilling("waiting")
        assert_eq!(action_refusal(&state("waiting"), "open a terminal", None, None).unwrap_or_else(|| "null".into()), "Workspace is waiting for a machine; open a terminals open when it is running"); // String(actionRefusal("waiting", "open a terminal") ?? "null")
        assert_eq!(action_refusal(&state("waiting"), "open a terminal", Some("its disk went with it"), Some("web")).unwrap_or_else(|| "null".into()), "Workspace is waiting for a machine; open a terminals open when it is running"); // String(actionRefusal("waiting", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("waiting")), None, None).unwrap_or_else(|| "null".into()), "null"); // String(sendRefusal("waiting") ?? "null")
        assert_eq!(gone_road_refusal(&state("waiting"), GoneRoad::Rebuild, None), "This one is waiting, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"); // goneRoadRefusal("waiting", "rebuild")
        assert_eq!(gone_road_refusal(&state("waiting"), GoneRoad::Forget, Some(&pause("disk"))), "Only a workspace whose computer is gone can be forgotten; this one is waiting"); // goneRoadRefusal("waiting", "forget", "disk")
        assert_eq!(workspace_word(&state("released"), None), "Deleted"); // workspaceWord("released")
        assert_eq!(workspace_word(&state("released"), Some(&pause("memory"))), "Deleted"); // workspaceWord("released", "memory")
        assert_eq!(workspace_state_line("web", &state("released"), None), "web deleted"); // workspaceStateLine("web", "released")
        assert_eq!(paused_or_pausing(&state("released")), false); // pausedOrPausing("released")
        assert_eq!(is_billing(&state("released")), false); // isBilling("released")
        assert_eq!(action_refusal(&state("released"), "open a terminal", None, None).unwrap_or_else(|| "null".into()), "This workspace's machine was deleted once its sub-thread was done, so there is none to open a terminal on"); // String(actionRefusal("released", "open a terminal") ?? "null")
        assert_eq!(action_refusal(&state("released"), "open a terminal", Some("its disk went with it"), Some("web")).unwrap_or_else(|| "null".into()), "web's machine was deleted once its sub-thread was done, so there is none to open a terminal on"); // String(actionRefusal("released", "open a terminal", "its disk went with it", "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::State(state("released")), None, None).unwrap_or_else(|| "null".into()), "This workspace's machine was deleted once its sub-thread was done, so there is none to send on"); // String(sendRefusal("released") ?? "null")
        assert_eq!(gone_road_refusal(&state("released"), GoneRoad::Rebuild, None), "This one is deleted, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"); // goneRoadRefusal("released", "rebuild")
        assert_eq!(gone_road_refusal(&state("released"), GoneRoad::Forget, Some(&pause("disk"))), "Only a workspace whose computer is gone can be forgotten; this one is deleted"); // goneRoadRefusal("released", "forget", "disk")
        assert_eq!(send_refusal(&SendRefusalKind::Block(block("connecting")), None, Some("web")).unwrap_or_else(|| "null".into()), "Connecting to wsp"); // String(sendRefusal("connecting", undefined, "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::Block(block("reconnecting")), None, Some("web")).unwrap_or_else(|| "null".into()), "wsp is not running, reconnecting"); // String(sendRefusal("reconnecting", undefined, "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::Block(block("closed")), None, Some("web")).unwrap_or_else(|| "null".into()), "wsp is not running"); // String(sendRefusal("closed", undefined, "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::Block(block("not-found")), None, Some("web")).unwrap_or_else(|| "null".into()), "Task not found"); // String(sendRefusal("not-found", undefined, "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::Block(block("loading")), None, Some("web")).unwrap_or_else(|| "null".into()), "Loading transcript"); // String(sendRefusal("loading", undefined, "web") ?? "null")
        assert_eq!(send_refusal(&SendRefusalKind::Block(block("no-agents")), None, Some("web")).unwrap_or_else(|| "null".into()), "the agents here have not answered yet"); // String(sendRefusal("no-agents", undefined, "web") ?? "null")
        assert_eq!(not_answering_yet(GoneRoad::Rebuild), "This one is not answering yet; the rebuild is offered once it is gone"); // notAnsweringYet("rebuild")
        assert_eq!(not_answering_yet(GoneRoad::Forget), "This one is not answering yet; the forget is offered once it is gone"); // notAnsweringYet("forget")
        assert_eq!(gone_refusal(None, "open it", None), "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to open it, which brings back its home folder from the last saved nap"); // goneRefusal(undefined, "open it", undefined)
        assert_eq!(gone_refusal(Some("web"), "open it", Some("")), "web's machine is gone with its disk, so work that was not pushed is lost; rebuild it to open it, which brings back its home folder from the last saved nap"); // goneRefusal("web", "open it", "")
        assert_eq!(gone_refusal(Some("web"), "open it", Some("disk lost")), "web's machine is gone with its disk, so work that was not pushed is lost; rebuild it to open it, which brings back its home folder from the last saved nap (disk lost)"); // goneRefusal("web", "open it", "disk lost")
    }

    #[test]
    fn the_state_reads_off_phase_machine_and_reach() {
        assert_eq!(daemon_silent(None), false); // daemonSilent(undefined)
        assert_eq!(daemon_silent(Some(&reach("reachable"))), false); // daemonSilent("reachable")
        assert_eq!(daemon_silent(Some(&reach("no-daemon"))), true); // daemonSilent("no-daemon")
        assert_eq!(daemon_silent(Some(&reach("unreachable"))), true); // daemonSilent("unreachable")
        assert_eq!(daemon_silent(Some(&reach("napping"))), false); // daemonSilent("napping")
        assert_eq!(daemon_silent(Some(&reach("unsupported"))), false); // daemonSilent("unsupported")
        assert_eq!(daemon_silent(Some(&reach("gone"))), false); // daemonSilent("gone")
        assert_eq!(daemon_silent(Some(&reach("slow"))), false); // daemonSilent("slow")
        assert_eq!(daemon_silent(Some(&reach("zombie"))), false); // daemonSilent("zombie")
        assert_eq!(reach_word(reach_shown(None, &reach("reachable"))), "reachable"); // reachShown(undefined, "reachable")
        assert_eq!(reach_word(reach_shown(Some(&reach("unreachable")), &reach("slow"))), "slow"); // reachShown("unreachable", "slow")
        assert_eq!(reach_word(reach_shown(Some(&reach("reachable")), &reach("slow"))), "slow"); // reachShown("reachable", "slow")
        assert_eq!(reach_word(reach_shown(Some(&reach("slow")), &reach("unreachable"))), "slow"); // reachShown("slow", "unreachable")
        assert_eq!(reach_word(reach_shown(Some(&reach("no-daemon")), &reach("slow"))), "slow"); // reachShown("no-daemon", "slow")
        assert_eq!(start_failed(&view(json!({"phase":"gone","machineId":""}))), true); // startFailed({"phase":"gone","machineId":""})
        assert_eq!(start_failed(&view(json!({"phase":"gone","machineId":"m1"}))), false); // startFailed({"phase":"gone","machineId":"m1"})
        assert_eq!(start_failed(&view(json!({"phase":"running","machineId":""}))), false); // startFailed({"phase":"running","machineId":""})
        assert_eq!(needs_rebuild(&input(json!({"phase":"running"}))), false); // needsRebuild({"phase":"running"})
        assert_eq!(needs_rebuild(&input(json!({"phase":"gone"}))), true); // needsRebuild({"phase":"gone"})
        assert_eq!(needs_rebuild(&input(json!({"phase":"running","reach":"zombie"}))), true); // needsRebuild({"phase":"running","reach":"zombie"})
        assert_eq!(needs_rebuild(&input(json!({"phase":"running","wakeRefused":"no room"}))), true); // needsRebuild({"phase":"running","wakeRefused":"no room"})
        assert_eq!(needs_rebuild(&input(json!({"phase":"running","wakeRefused":null}))), false); // needsRebuild({"phase":"running","wakeRefused":null})
        assert_eq!(workspace_state(&input(json!({"phase":"running"}))).as_str(), "running"); // workspaceState({"phase":"running"})
        assert_eq!(workspace_state(&input(json!({"phase":"running","machineState":"paused"}))).as_str(), "paused"); // workspaceState({"phase":"running","machineState":"paused"})
        assert_eq!(workspace_state(&input(json!({"phase":"running","machineState":"running","reach":"unreachable"}))).as_str(), "unreachable"); // workspaceState({"phase":"running","machineState":"running","reach":"unreachable"})
        assert_eq!(workspace_state(&input(json!({"phase":"running","reach":"gone"}))).as_str(), "running"); // workspaceState({"phase":"running","reach":"gone"})
        assert_eq!(workspace_state(&input(json!({"phase":"waking"}))).as_str(), "waking"); // workspaceState({"phase":"waking"})
        assert_eq!(workspace_state(&input(json!({"phase":"gone"}))).as_str(), "gone"); // workspaceState({"phase":"gone"})
        assert_eq!(workspace_state(&input(json!({"phase":"running","wakeRefused":"no room"}))).as_str(), "running"); // workspaceState({"phase":"running","wakeRefused":"no room"})
    }

    #[test]
    fn an_absent_computer_reads_its_road_and_its_last_word() {
        assert_eq!(back_url(2222.0), "http://127.0.0.1:2222"); // backUrl(2222)
        assert_eq!(dials_back_word(2222.0, "spoo"), "dials back over ssh (127.0.0.1:2222 on spoo)"); // dialsBackWord(2222, "spoo")
        assert_eq!(back_url(0.0), "http://127.0.0.1:0"); // backUrl(0)
        assert_eq!(dials_back_word(0.0, "spoo"), "dials back over ssh (127.0.0.1:0 on spoo)"); // dialsBackWord(0, "spoo")
        assert_eq!(linked_over("10.0.0.2", None, &[] as &[&str]).unwrap_or_else(|| "none".into()), "none"); // String(linkedOver("10.0.0.2", undefined, []) ?? "none")
        assert_eq!(linked_over("10.0.0.2", Some(&back(json!({"boxPort":2222}))), &["10.0.0.2"] as &[&str]).unwrap_or_else(|| "none".into()), "at 10.0.0.2"); // String(linkedOver("10.0.0.2", {"boxPort":2222}, ["10.0.0.2"]) ?? "none")
        assert_eq!(linked_over("10.0.0.2", Some(&back(json!({"boxPort":2222}))), &["10.0.0.9"] as &[&str]).unwrap_or_else(|| "none".into()), "none"); // String(linkedOver("10.0.0.2", {"boxPort":2222}, ["10.0.0.9"]) ?? "none")
        assert_eq!(linked_over(&back_url(2222.0), Some(&back(json!({"boxPort":2222}))), &[] as &[&str]).unwrap_or_else(|| "none".into()), "over ssh"); // String(linkedOver(backUrl(2222), {boxPort: 2222}, []) ?? "none")
        assert_eq!(last_known("2.1 GB", None, None), "2.1 GB"); // lastKnown("2.1 GB", null)
        assert_eq!(last_known("2.1 GB", Some(90000.0), None), "2.1 GB, last seen 1 min ago"); // lastKnown("2.1 GB", 90000)
        assert_eq!(last_known("2.1 GB", Some(7200000.0), Some("read")), "2.1 GB, read 2 h ago"); // lastKnown("2.1 GB", 7200000, "read")
        assert_eq!(last_known("x", Some(0.0), None), "x, last seen 0 min ago"); // lastKnown("x", 0)
        assert_eq!(waited_line(0.0), "Waited 0 s for your computer"); // waitedLine(0)
        assert_eq!(waited_line(4999.0), "Waited 5 s for your computer"); // waitedLine(4999)
        assert_eq!(waited_line(5000.0), "Waited 5 s for your computer"); // waitedLine(5000)
        assert_eq!(waited_line(65000.0), "Waited 1 min for your computer"); // waitedLine(65000)
        assert_eq!(waited_line(3700000.0), "Waited 62 min for your computer"); // waitedLine(3700000)
        assert_eq!(composer_held_line("spoo"), "held until spoo answers"); // composerHeldLine("spoo")
        assert_eq!(place_no_dial_line("spoo"), "wsp has no road to dial spoo: it joined by typing a code, so it connects on its own when it is on."); // placeNoDialLine("spoo")
        assert_eq!(away_ms_of(None, 1792920600000.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "null"); // String(awayMsOf({}, 1792920600000) ?? "null")
        assert_eq!(away_ms_of(Some("2026-10-25T09:00:00.000Z"), 1792920600000.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "1800000"); // String(awayMsOf({"lastSeenAt":"2026-10-25T09:00:00.000Z"}, 1792920600000) ?? "null")
        assert_eq!(away_ms_of(Some("not a date"), 1792920600000.0).map(wsp_words::js::number_string).unwrap_or_else(|| "null".into()), "null"); // String(awayMsOf({"lastSeenAt":"not a date"}, 1792920600000) ?? "null")
        assert_eq!(ssh_road_of(None).unwrap_or_else(|| "none".into()), "none"); // String(sshRoadOf(undefined) ?? "none")
        assert_eq!(ssh_road_of(Some(&road(json!({})))).unwrap_or_else(|| "none".into()), "none"); // String(sshRoadOf({}) ?? "none")
        assert_eq!(ssh_road_of(Some(&road(json!({"ssh":""})))).unwrap_or_else(|| "none".into()), "none"); // String(sshRoadOf({"ssh":""}) ?? "none")
        assert_eq!(ssh_road_of(Some(&road(json!({"ssh":"root@10.0.0.2"})))).unwrap_or_else(|| "none".into()), "root@10.0.0.2"); // String(sshRoadOf({"ssh":"root@10.0.0.2"}) ?? "none")
    }
}
