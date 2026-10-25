// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/project-bundle.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::views::project_bundle::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_project_bundle.json");

/// 10 cases: what zod's parse of CredentialSignal wrote, or the first issue it raised.
#[test]
fn credential_signal_reads_and_writes_as_zod_does() {
    cases::<CredentialSignal>(FIXTURES, "CredentialSignal");
}

/// 5 cases: what zod's parse of HostFolder wrote, or the first issue it raised.
#[test]
fn host_folder_reads_and_writes_as_zod_does() {
    cases::<HostFolder>(FIXTURES, "HostFolder");
}

/// 4 cases: what zod's parse of HostFolderListing wrote, or the first issue it raised.
#[test]
fn host_folder_listing_reads_and_writes_as_zod_does() {
    cases::<HostFolderListing>(FIXTURES, "HostFolderListing");
}

/// 5 cases: what zod's parse of ProjectAgent wrote, or the first issue it raised.
#[test]
fn project_agent_reads_and_writes_as_zod_does() {
    cases::<ProjectAgent>(FIXTURES, "ProjectAgent");
}

/// 6 cases: what zod's parse of ProjectAgentOutcome wrote, or the first issue it raised.
#[test]
fn project_agent_outcome_reads_and_writes_as_zod_does() {
    cases::<ProjectAgentOutcome>(FIXTURES, "ProjectAgentOutcome");
}

/// 5 cases: what zod's parse of ProjectAgentResult wrote, or the first issue it raised.
#[test]
fn project_agent_result_reads_and_writes_as_zod_does() {
    cases::<ProjectAgentResult>(FIXTURES, "ProjectAgentResult");
}

/// 3 cases: what zod's parse of ProjectCarry wrote, or the first issue it raised.
#[test]
fn project_carry_reads_and_writes_as_zod_does() {
    cases::<ProjectCarry>(FIXTURES, "ProjectCarry");
}

/// 5 cases: what zod's parse of ProjectExportEvent wrote, or the first issue it raised.
#[test]
fn project_export_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ProjectExportEvent>(FIXTURES, "ProjectExportEvent", "type", ProjectExportEvent::TYPE);
}

/// 3 cases: what zod's parse of ProjectExportResult wrote, or the first issue it raised.
#[test]
fn project_export_result_reads_and_writes_as_zod_does() {
    cases::<ProjectExportResult>(FIXTURES, "ProjectExportResult");
}

/// 6 cases: what zod's parse of ProjectExportStage wrote, or the first issue it raised.
#[test]
fn project_export_stage_reads_and_writes_as_zod_does() {
    cases::<ProjectExportStage>(FIXTURES, "ProjectExportStage");
}

/// 5 cases: what zod's parse of ProjectImportEvent wrote, or the first issue it raised.
#[test]
fn project_import_event_reads_and_writes_as_zod_does() {
    cases_tagged::<ProjectImportEvent>(FIXTURES, "ProjectImportEvent", "type", ProjectImportEvent::TYPE);
}

/// 3 cases: what zod's parse of ProjectImportResult wrote, or the first issue it raised.
#[test]
fn project_import_result_reads_and_writes_as_zod_does() {
    cases::<ProjectImportResult>(FIXTURES, "ProjectImportResult");
}

/// 8 cases: what zod's parse of ProjectImportStage wrote, or the first issue it raised.
#[test]
fn project_import_stage_reads_and_writes_as_zod_does() {
    cases::<ProjectImportStage>(FIXTURES, "ProjectImportStage");
}

/// 5 cases: what zod's parse of ProjectPlan wrote, or the first issue it raised.
#[test]
fn project_plan_reads_and_writes_as_zod_does() {
    cases::<ProjectPlan>(FIXTURES, "ProjectPlan");
}

/// 3 cases: what zod's parse of ProjectRewrite wrote, or the first issue it raised.
#[test]
fn project_rewrite_reads_and_writes_as_zod_does() {
    cases::<ProjectRewrite>(FIXTURES, "ProjectRewrite");
}

/// 4 cases: what zod's parse of ProjectSecret wrote, or the first issue it raised.
#[test]
fn project_secret_reads_and_writes_as_zod_does() {
    cases::<ProjectSecret>(FIXTURES, "ProjectSecret");
}

/// 12 cases: what zod's parse of TerminalConfig wrote, or the first issue it raised.
#[test]
fn terminal_config_reads_and_writes_as_zod_does() {
    cases::<TerminalConfig>(FIXTURES, "TerminalConfig");
}

/// 5 cases: what zod's parse of TerminalCursorStyle wrote, or the first issue it raised.
#[test]
fn terminal_cursor_style_reads_and_writes_as_zod_does() {
    cases::<TerminalCursorStyle>(FIXTURES, "TerminalCursorStyle");
}

/// 11 cases: what zod's parse of TerminalRgb wrote, or the first issue it raised.
#[test]
fn terminal_rgb_reads_and_writes_as_zod_does() {
    cases::<TerminalRgb>(FIXTURES, "TerminalRgb");
}

/// 3 cases: what zod's parse of TerminalScheme wrote, or the first issue it raised.
#[test]
fn terminal_scheme_reads_and_writes_as_zod_does() {
    cases::<TerminalScheme>(FIXTURES, "TerminalScheme");
}
