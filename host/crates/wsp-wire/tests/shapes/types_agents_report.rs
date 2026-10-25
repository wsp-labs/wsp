// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of agents-report.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::types::agents_report::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_agents_report.json");

/// 5 cases: what zod's parse of AgentRoad wrote, or the first issue it raised.
#[test]
fn agent_road_reads_and_writes_as_zod_does() {
    cases::<AgentRoad>(FIXTURES, "AgentRoad");
}

/// 9 cases: what zod's parse of AgentRow wrote, or the first issue it raised.
#[test]
fn agent_row_reads_and_writes_as_zod_does() {
    cases::<AgentRow>(FIXTURES, "AgentRow");
}

/// 4 cases: what zod's parse of AgentSignInKind wrote, or the first issue it raised.
#[test]
fn agent_sign_in_kind_reads_and_writes_as_zod_does() {
    cases::<AgentSignInKind>(FIXTURES, "AgentSignInKind");
}

/// 4 cases: what zod's parse of AgentSignInState wrote, or the first issue it raised.
#[test]
fn agent_sign_in_state_reads_and_writes_as_zod_does() {
    cases::<AgentSignInState>(FIXTURES, "AgentSignInState");
}

/// 3 cases: what zod's parse of AgentsChangedEvent wrote, or the first issue it raised.
#[test]
fn agents_changed_event_reads_and_writes_as_zod_does() {
    cases_tagged::<AgentsChangedEvent>(FIXTURES, "AgentsChangedEvent", "type", AgentsChangedEvent::TYPE);
}

/// 3 cases: what zod's parse of AgentsProject wrote, or the first issue it raised.
#[test]
fn agents_project_reads_and_writes_as_zod_does() {
    cases::<AgentsProject>(FIXTURES, "AgentsProject");
}

/// 6 cases: what zod's parse of AgentsReport wrote, or the first issue it raised.
#[test]
fn agents_report_reads_and_writes_as_zod_does() {
    cases::<AgentsReport>(FIXTURES, "AgentsReport");
}

/// 6 cases: what zod's parse of AgentsSignInEvent wrote, or the first issue it raised.
#[test]
fn agents_sign_in_event_reads_and_writes_as_zod_does() {
    cases_tagged::<AgentsSignInEvent>(FIXTURES, "AgentsSignInEvent", "type", AgentsSignInEvent::TYPE);
}

/// 5 cases: what zod's parse of AgentsSignInRun wrote, or the first issue it raised.
#[test]
fn agents_sign_in_run_reads_and_writes_as_zod_does() {
    cases::<AgentsSignInRun>(FIXTURES, "AgentsSignInRun");
}

/// 3 cases: what zod's parse of AgentsTarget wrote, or the first issue it raised.
#[test]
fn agents_target_reads_and_writes_as_zod_does() {
    cases::<AgentsTarget>(FIXTURES, "AgentsTarget");
}

/// 8 cases: what zod's parse of McpAuth wrote, or the first issue it raised.
#[test]
fn mcp_auth_reads_and_writes_as_zod_does() {
    cases::<McpAuth>(FIXTURES, "McpAuth");
}

/// 8 cases: what zod's parse of McpRow wrote, or the first issue it raised.
#[test]
fn mcp_row_reads_and_writes_as_zod_does() {
    cases::<McpRow>(FIXTURES, "McpRow");
}

/// 3 cases: what zod's parse of McpRowTransport wrote, or the first issue it raised.
#[test]
fn mcp_row_transport_reads_and_writes_as_zod_does() {
    cases::<McpRowTransport>(FIXTURES, "McpRowTransport");
}

/// 5 cases: what zod's parse of McpScope wrote, or the first issue it raised.
#[test]
fn mcp_scope_reads_and_writes_as_zod_does() {
    cases::<McpScope>(FIXTURES, "McpScope");
}

/// 4 cases: what zod's parse of McpTool wrote, or the first issue it raised.
#[test]
fn mcp_tool_reads_and_writes_as_zod_does() {
    cases::<McpTool>(FIXTURES, "McpTool");
}

/// 5 cases: what zod's parse of McpToolParam wrote, or the first issue it raised.
#[test]
fn mcp_tool_param_reads_and_writes_as_zod_does() {
    cases::<McpToolParam>(FIXTURES, "McpToolParam");
}

/// 4 cases: what zod's parse of PageReach wrote, or the first issue it raised.
#[test]
fn page_reach_reads_and_writes_as_zod_does() {
    cases::<PageReach>(FIXTURES, "PageReach");
}

/// 3 cases: what zod's parse of PluginAsk wrote, or the first issue it raised.
#[test]
fn plugin_ask_reads_and_writes_as_zod_does() {
    cases::<PluginAsk>(FIXTURES, "PluginAsk");
}

/// 3 cases: what zod's parse of PluginBrings wrote, or the first issue it raised.
#[test]
fn plugin_brings_reads_and_writes_as_zod_does() {
    cases::<PluginBrings>(FIXTURES, "PluginBrings");
}

/// 5 cases: what zod's parse of PluginMissing wrote, or the first issue it raised.
#[test]
fn plugin_missing_reads_and_writes_as_zod_does() {
    cases::<PluginMissing>(FIXTURES, "PluginMissing");
}

/// 7 cases: what zod's parse of PluginRow wrote, or the first issue it raised.
#[test]
fn plugin_row_reads_and_writes_as_zod_does() {
    cases::<PluginRow>(FIXTURES, "PluginRow");
}

/// 4 cases: what zod's parse of PluginScope wrote, or the first issue it raised.
#[test]
fn plugin_scope_reads_and_writes_as_zod_does() {
    cases::<PluginScope>(FIXTURES, "PluginScope");
}

/// 5 cases: what zod's parse of ServerAdd wrote, or the first issue it raised.
#[test]
fn server_add_reads_and_writes_as_zod_does() {
    cases::<ServerAdd>(FIXTURES, "ServerAdd");
}

/// 5 cases: what zod's parse of ServerAsk wrote, or the first issue it raised.
#[test]
fn server_ask_reads_and_writes_as_zod_does() {
    cases::<ServerAsk>(FIXTURES, "ServerAsk");
}

/// 5 cases: what zod's parse of ServerToolsAnswer wrote, or the first issue it raised.
#[test]
fn server_tools_answer_reads_and_writes_as_zod_does() {
    cases::<ServerToolsAnswer>(FIXTURES, "ServerToolsAnswer");
}

/// 4 cases: what zod's parse of SignInLine wrote, or the first issue it raised.
#[test]
fn sign_in_line_reads_and_writes_as_zod_does() {
    cases::<SignInLine>(FIXTURES, "SignInLine");
}

/// 7 cases: what zod's parse of SignInRoad wrote, or the first issue it raised.
#[test]
fn sign_in_road_reads_and_writes_as_zod_does() {
    cases::<SignInRoad>(FIXTURES, "SignInRoad");
}

/// 3 cases: what zod's parse of SkillAdded wrote, or the first issue it raised.
#[test]
fn skill_added_reads_and_writes_as_zod_does() {
    cases::<SkillAdded>(FIXTURES, "SkillAdded");
}

/// 5 cases: what zod's parse of SkillHit wrote, or the first issue it raised.
#[test]
fn skill_hit_reads_and_writes_as_zod_does() {
    cases::<SkillHit>(FIXTURES, "SkillHit");
}

/// 4 cases: what zod's parse of SkillPath wrote, or the first issue it raised.
#[test]
fn skill_path_reads_and_writes_as_zod_does() {
    cases::<SkillPath>(FIXTURES, "SkillPath");
}

/// 5 cases: what zod's parse of SkillPreview wrote, or the first issue it raised.
#[test]
fn skill_preview_reads_and_writes_as_zod_does() {
    cases::<SkillPreview>(FIXTURES, "SkillPreview");
}

/// 6 cases: what zod's parse of SkillRow wrote, or the first issue it raised.
#[test]
fn skill_row_reads_and_writes_as_zod_does() {
    cases::<SkillRow>(FIXTURES, "SkillRow");
}

/// 4 cases: what zod's parse of SkillScope wrote, or the first issue it raised.
#[test]
fn skill_scope_reads_and_writes_as_zod_does() {
    cases::<SkillScope>(FIXTURES, "SkillScope");
}
