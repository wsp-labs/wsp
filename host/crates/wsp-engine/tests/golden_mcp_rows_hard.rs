// SPDX-License-Identifier: AGPL-3.0-only
//! Unit mcp_rows's hard tests: every value here is what golden-mcp.ts printed under oracle/golden.oracle.ts
//! (tests/fixtures/oracle/golden_mcp_rows.json). spec/golden-mcp_rows.md names the rule each pins.
mod golden_common;

use golden_common::{agents_json, agents_of, assert_calls, cases, fixture, plan_of, s, strings, tool_result, Scripted, Stages};
use indexmap::IndexSet;
use serde_json::{json, Value};
use wsp_engine::golden_seam::{
    absent_commands, absent_reason, apply_mcp, as_run, command_row, fetchers, mcp_opening, mcp_rows, mcp_tally, server_needs,
    without_absent, EditedName, EditedOutcome, McpResult, McpRowsOptions, ScopeOutcome, ToolInstall, ToolResult, TOOLS_PATH,
};

fn outcome_of(v: &Value) -> ScopeOutcome {
    ScopeOutcome {
        file: v["file"].as_str().map(str::to_string),
        error: v["error"].as_str().map(str::to_string),
        unread: v["unread"].as_str().map(str::to_string),
        results: v["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| EditedName {
                name: s(&r["name"]),
                outcome: serde_json::from_value::<EditedOutcome>(r["outcome"].clone()).unwrap(),
                command: r["command"].as_str().map(str::to_string),
            })
            .collect(),
    }
}

fn report_of(v: &Value) -> Vec<ScopeOutcome> {
    v.as_array().unwrap().iter().map(outcome_of).collect()
}

fn tools_of(v: &Value) -> Vec<ToolResult> {
    v.as_array().unwrap().iter().map(tool_result).collect()
}

fn set(v: &Value) -> IndexSet<String> {
    strings(v).into_iter().collect()
}

fn rows_json(rows: &[McpResult]) -> Value {
    serde_json::to_value(rows).unwrap()
}

/// The rows `command_row` and `server_needs` are asked about: `{ id, bin? }` as ToolInstall carries them.
fn need_rows(v: &Value) -> Vec<ToolInstall> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| ToolInstall { id: s(&r["id"]), bin: r["bin"].as_str().map(str::to_string), ..ToolInstall::default() })
        .collect()
}

#[test]
fn the_fetchers_are_npx_uvx_and_uv_in_the_ts_order() {
    let f = fixture("mcp_rows");
    let want: Vec<(String, String)> = f["FETCHERS"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), s(v))).collect();
    let got: Vec<(String, String)> = fetchers().into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    assert_eq!(got, want);
}

#[tokio::test]
async fn absent_commands_asks_once_for_every_distinct_command_as_run_and_reads_the_no_lines() {
    let f = fixture("mcp_rows");
    let plan = plan_of(&f["plan"]);
    for c in cases(&f, "absentCommands") {
        let m = Scripted::new(&c["rules"], false);
        let got = absent_commands(&m, &plan, &report_of(&c["report"]), c["path"].as_str().unwrap_or(TOOLS_PATH)).await;
        assert_eq!(got, strings(&c["out"]), "{}", c["name"]);
        assert_calls(&s(&c["name"]), &m.calls(), &c["calls"]);
    }
}

#[test]
fn as_run_and_absent_reason_say_what_the_ts_said() {
    let f = fixture("mcp_rows");
    let plan = plan_of(&f["plan"]);
    for c in cases(&f, "asRun") {
        assert_eq!(as_run(&plan, &s(&c["command"])), s(&c["out"]), "{c}");
    }
    for c in cases(&f, "absentReason") {
        assert_eq!(absent_reason(&s(&c["command"]), &plan, &tools_of(&c["tools"])), s(&c["out"]), "{c}");
    }
}

#[test]
fn without_absent_moves_each_missing_kept_server_to_the_drops_with_its_reason_in_plan_order() {
    let f = fixture("mcp_rows");
    let plan = plan_of(&f["plan"]);
    let report = report_of(&f["report"]);
    for c in cases(&f, "withoutAbsent") {
        let got = without_absent(&plan, &plan.agents, &report, &set(&c["missing"]), &tools_of(&c["tools"]));
        assert_eq!(agents_json(&got), c["out"]["agents"], "missing {}", c["missing"]);
    }
}

#[test]
fn mcp_opening_counts_every_server_of_each_agent() {
    for c in cases(&fixture("mcp_rows"), "mcpOpening") {
        let agents = if c["agents"].is_array() { agents_of(&c["agents"]) } else { agents_of(&c["agents"]["agents"]) };
        assert_eq!(mcp_opening(&agents), s(&c["out"]), "{c}");
    }
}

#[tokio::test]
async fn mcp_rows_writes_every_row_and_the_closing_words_as_the_ts_in_every_scenario() {
    let f = fixture("mcp_rows");
    let plan = plan_of(&f["plan"]);
    for c in cases(&f, "mcpRows") {
        let name = s(&c["name"]);
        let m = Scripted::new(&c["rules"], false);
        let stages = Stages::default();
        let o = McpRowsOptions {
            agents: plan.agents.clone(),
            report: if c["report"].is_null() { None } else { Some(report_of(&c["report"])) },
            failure: c["failure"].as_str().map(str::to_string),
            missing: set(&c["missing"]),
            present: set(&c["present"]),
            stage: stages.listener(),
        };
        let got = mcp_rows(&m, &plan, o).await;
        assert_eq!(rows_json(&got), c["out"], "{name}: the rows");
        assert_eq!(stages.value(), c["stages"], "{name}: the stage lines");
        assert_calls(&name, &m.calls(), &c["calls"]);
    }
}

#[tokio::test]
async fn apply_mcp_runs_the_whole_round_and_every_failure_road_as_the_ts() {
    let f = fixture("mcp_rows");
    let plan = plan_of(&f["plan"]);
    let tools = vec![
        ToolResult {
            id: "tools/npm/keeper".into(),
            label: "keeper".into(),
            outcome: "failed".into(),
            note: Some("npm broke".into()),
            ..ToolResult::default()
        },
        ToolResult { id: "tools/cargo/local-mcp".into(), label: "local-mcp".into(), outcome: "installed".into(), ..ToolResult::default() },
    ];
    for c in cases(&f, "applyMcp") {
        let name = s(&c["name"]);
        let m = Scripted::new(&c["rules"], true);
        let stages = Stages::default();
        let got = apply_mcp(&m, &plan, stages.listener(), &tools).await;
        assert_eq!(rows_json(&got), c["out"], "{name}: the rows");
        assert_eq!(stages.value(), c["stages"], "{name}: the stage lines");
        let calls = m.calls();
        assert_calls(&name, &calls, &c["calls"]);
        assert_eq!(calls[0]["unlogged"], json!(true), "{name}: the read's output is the person's own");
    }
}

#[test]
fn mcp_tally_counts_by_outcome_and_leaves_out_zeros() {
    for c in cases(&fixture("mcp_rows"), "mcpTally") {
        let rows: Vec<McpResult> = c["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| McpResult { id: "x".into(), agent: "a".into(), name: "n".into(), outcome: s(&r["outcome"]), note: None, keys: None })
            .collect();
        assert_eq!(mcp_tally(&rows).map(Value::from).unwrap_or(Value::Null), c["out"], "{c}");
    }
}

#[test]
fn command_row_and_server_needs_name_the_row_that_puts_a_command_on_as_the_ts() {
    let f = fixture("mcp_rows");
    for c in cases(&f, "commandRow") {
        let rows = need_rows(&c["rows"]);
        let got = command_row(&s(&c["command"]), &rows).map(|r| json!(r.id));
        assert_eq!(got, c["out"].get("id").cloned(), "{}", c["command"]);
    }
    for c in cases(&f, "serverNeeds") {
        let rows = need_rows(&c["rows"]);
        let got: Vec<String> = server_needs(&s(&c["command"]), &rows, &set(&c["ready"])).into_iter().map(|r| r.id.clone()).collect();
        let want: Vec<String> = c["out"].as_array().unwrap().iter().map(|r| s(&r["id"])).collect();
        assert_eq!(got, want, "{}", c["command"]);
    }
}
