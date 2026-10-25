// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/workspace.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::views::workspace::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_workspace.json");

/// 10 cases: what zod's parse of BranchOfRecord wrote, or the first issue it raised.
#[test]
fn branch_of_record_reads_and_writes_as_zod_does() {
    cases::<BranchOfRecord>(FIXTURES, "BranchOfRecord");
}

/// 4 cases: what zod's parse of Carried wrote, or the first issue it raised.
#[test]
fn carried_reads_and_writes_as_zod_does() {
    cases::<Carried>(FIXTURES, "Carried");
}

/// 4 cases: what zod's parse of CarryModule wrote, or the first issue it raised.
#[test]
fn carry_module_reads_and_writes_as_zod_does() {
    cases::<CarryModule>(FIXTURES, "CarryModule");
}

/// 7 cases: what zod's parse of CopyAsk wrote, or the first issue it raised.
#[test]
fn copy_ask_reads_and_writes_as_zod_does() {
    cases::<CopyAsk>(FIXTURES, "CopyAsk");
}

/// 11 cases: what zod's parse of CopyReport wrote, or the first issue it raised.
#[test]
fn copy_report_reads_and_writes_as_zod_does() {
    cases::<CopyReport>(FIXTURES, "CopyReport");
}

/// 3 cases: what zod's parse of CopyRoad wrote, or the first issue it raised.
#[test]
fn copy_road_reads_and_writes_as_zod_does() {
    cases::<CopyRoad>(FIXTURES, "CopyRoad");
}

/// 4 cases: what zod's parse of DaemonKind wrote, or the first issue it raised.
#[test]
fn daemon_kind_reads_and_writes_as_zod_does() {
    cases::<DaemonKind>(FIXTURES, "DaemonKind");
}

/// 4 cases: what zod's parse of DaemonReachView wrote, or the first issue it raised.
#[test]
fn daemon_reach_view_reads_and_writes_as_zod_does() {
    cases::<DaemonReachView>(FIXTURES, "DaemonReachView");
}

/// 3 cases: what zod's parse of EventAsker wrote, or the first issue it raised.
#[test]
fn event_asker_reads_and_writes_as_zod_does() {
    cases::<EventAsker>(FIXTURES, "EventAsker");
}

/// 4 cases: what zod's parse of FoundModule wrote, or the first issue it raised.
#[test]
fn found_module_reads_and_writes_as_zod_does() {
    cases::<FoundModule>(FIXTURES, "FoundModule");
}

/// 3 cases: what zod's parse of MachineFacts wrote, or the first issue it raised.
#[test]
fn machine_facts_reads_and_writes_as_zod_does() {
    cases::<MachineFacts>(FIXTURES, "MachineFacts");
}

/// 5 cases: what zod's parse of MachineState wrote, or the first issue it raised.
#[test]
fn machine_state_reads_and_writes_as_zod_does() {
    cases::<MachineState>(FIXTURES, "MachineState");
}

/// 4 cases: what zod's parse of PortProbeView wrote, or the first issue it raised.
#[test]
fn port_probe_view_reads_and_writes_as_zod_does() {
    cases::<PortProbeView>(FIXTURES, "PortProbeView");
}

/// 3 cases: what zod's parse of PortReachView wrote, or the first issue it raised.
#[test]
fn port_reach_view_reads_and_writes_as_zod_does() {
    cases::<PortReachView>(FIXTURES, "PortReachView");
}

/// 8 cases: what zod's parse of ProjectAddStage wrote, or the first issue it raised.
#[test]
fn project_add_stage_reads_and_writes_as_zod_does() {
    cases::<ProjectAddStage>(FIXTURES, "ProjectAddStage");
}

/// 6 cases: what zod's parse of ProjectBranch wrote, or the first issue it raised.
#[test]
fn project_branch_reads_and_writes_as_zod_does() {
    cases::<ProjectBranch>(FIXTURES, "ProjectBranch");
}

/// 6 cases: what zod's parse of ProjectCopy wrote, or the first issue it raised.
#[test]
fn project_copy_reads_and_writes_as_zod_does() {
    cases::<ProjectCopy>(FIXTURES, "ProjectCopy");
}

/// 3 cases: what zod's parse of ProjectRef wrote, or the first issue it raised.
#[test]
fn project_ref_reads_and_writes_as_zod_does() {
    cases::<ProjectRef>(FIXTURES, "ProjectRef");
}

/// 4 cases: what zod's parse of ProjectSeed wrote, or the first issue it raised.
#[test]
fn project_seed_reads_and_writes_as_zod_does() {
    cases::<ProjectSeed>(FIXTURES, "ProjectSeed");
}

/// 5 cases: what zod's parse of ProjectSource wrote, or the first issue it raised.
#[test]
fn project_source_reads_and_writes_as_zod_does() {
    cases::<ProjectSource>(FIXTURES, "ProjectSource");
}

/// 4 cases: what zod's parse of ProjectView wrote, or the first issue it raised.
#[test]
fn project_view_reads_and_writes_as_zod_does() {
    cases::<ProjectView>(FIXTURES, "ProjectView");
}

/// 9 cases: what zod's parse of ReachState wrote, or the first issue it raised.
#[test]
fn reach_state_reads_and_writes_as_zod_does() {
    cases::<ReachState>(FIXTURES, "ReachState");
}

/// 6 cases: what zod's parse of ReachStatus wrote, or the first issue it raised.
#[test]
fn reach_status_reads_and_writes_as_zod_does() {
    cases::<ReachStatus>(FIXTURES, "ReachStatus");
}

/// 6 cases: what zod's parse of ReachView wrote, or the first issue it raised.
#[test]
fn reach_view_reads_and_writes_as_zod_does() {
    cases::<ReachView>(FIXTURES, "ReachView");
}

/// 7 cases: what zod's parse of SeedChoice wrote, or the first issue it raised.
#[test]
fn seed_choice_reads_and_writes_as_zod_does() {
    cases::<SeedChoice>(FIXTURES, "SeedChoice");
}

/// 9 cases: what zod's parse of SeedFile wrote, or the first issue it raised.
#[test]
fn seed_file_reads_and_writes_as_zod_does() {
    cases::<SeedFile>(FIXTURES, "SeedFile");
}

/// 7 cases: what zod's parse of SeedKind wrote, or the first issue it raised.
#[test]
fn seed_kind_reads_and_writes_as_zod_does() {
    cases::<SeedKind>(FIXTURES, "SeedKind");
}

/// 7 cases: what zod's parse of SeedPickRow wrote, or the first issue it raised.
#[test]
fn seed_pick_row_reads_and_writes_as_zod_does() {
    cases::<SeedPickRow>(FIXTURES, "SeedPickRow");
}

/// 12 cases: what zod's parse of SeedPlan wrote, or the first issue it raised.
#[test]
fn seed_plan_reads_and_writes_as_zod_does() {
    cases::<SeedPlan>(FIXTURES, "SeedPlan");
}

/// 3 cases: what zod's parse of ThreadScope wrote, or the first issue it raised.
#[test]
fn thread_scope_reads_and_writes_as_zod_does() {
    cases::<ThreadScope>(FIXTURES, "ThreadScope");
}

/// 8 cases: what zod's parse of WorkspaceAgents wrote, or the first issue it raised.
#[test]
fn workspace_agents_reads_and_writes_as_zod_does() {
    cases::<WorkspaceAgents>(FIXTURES, "WorkspaceAgents");
}

/// 9 cases: what zod's parse of WorkspaceListing wrote, or the first issue it raised.
#[test]
fn workspace_listing_reads_and_writes_as_zod_does() {
    cases::<WorkspaceListing>(FIXTURES, "WorkspaceListing");
}

/// 4 cases: what zod's parse of WorkspaceOrigin wrote, or the first issue it raised.
#[test]
fn workspace_origin_reads_and_writes_as_zod_does() {
    cases::<WorkspaceOrigin>(FIXTURES, "WorkspaceOrigin");
}

/// 8 cases: what zod's parse of WorkspaceOut wrote, or the first issue it raised.
#[test]
fn workspace_out_reads_and_writes_as_zod_does() {
    cases::<WorkspaceOut>(FIXTURES, "WorkspaceOut");
}

/// 8 cases: what zod's parse of WorkspacePhase wrote, or the first issue it raised.
#[test]
fn workspace_phase_reads_and_writes_as_zod_does() {
    cases::<WorkspacePhase>(FIXTURES, "WorkspacePhase");
}

/// 6 cases: what zod's parse of WorkspaceProject wrote, or the first issue it raised.
#[test]
fn workspace_project_reads_and_writes_as_zod_does() {
    cases::<WorkspaceProject>(FIXTURES, "WorkspaceProject");
}

/// 9 cases: what zod's parse of WorkspaceStatus wrote, or the first issue it raised.
#[test]
fn workspace_status_reads_and_writes_as_zod_does() {
    cases::<WorkspaceStatus>(FIXTURES, "WorkspaceStatus");
}

/// 6 cases: what zod's parse of WorkspaceTalked wrote, or the first issue it raised.
#[test]
fn workspace_talked_reads_and_writes_as_zod_does() {
    cases::<WorkspaceTalked>(FIXTURES, "WorkspaceTalked");
}

/// 8 cases: what zod's parse of WorkspaceView wrote, or the first issue it raised.
#[test]
fn workspace_view_reads_and_writes_as_zod_does() {
    cases::<WorkspaceView>(FIXTURES, "WorkspaceView");
}

/// 4 cases: what zod's parse of WorktreeAsk wrote, or the first issue it raised.
#[test]
fn worktree_ask_reads_and_writes_as_zod_does() {
    cases::<WorktreeAsk>(FIXTURES, "WorktreeAsk");
}

/// 5 cases: what zod's parse of WorktreeFolder wrote, or the first issue it raised.
#[test]
fn worktree_folder_reads_and_writes_as_zod_does() {
    cases::<WorktreeFolder>(FIXTURES, "WorktreeFolder");
}

/// 4 cases: what zod's parse of WorktreeMade wrote, or the first issue it raised.
#[test]
fn worktree_made_reads_and_writes_as_zod_does() {
    cases::<WorktreeMade>(FIXTURES, "WorktreeMade");
}

/// 4 cases: what zod's parse of WorktreeRemoval wrote, or the first issue it raised.
#[test]
fn worktree_removal_reads_and_writes_as_zod_does() {
    cases::<WorktreeRemoval>(FIXTURES, "WorktreeRemoval");
}

/// 8 cases: what zod's parse of WorktreeReport wrote, or the first issue it raised.
#[test]
fn worktree_report_reads_and_writes_as_zod_does() {
    cases::<WorktreeReport>(FIXTURES, "WorktreeReport");
}

/// 4 cases: what zod's parse of WorktreeSettled wrote, or the first issue it raised.
#[test]
fn worktree_settled_reads_and_writes_as_zod_does() {
    cases::<WorktreeSettled>(FIXTURES, "WorktreeSettled");
}
