// SPDX-License-Identifier: AGPL-3.0-only
//! Unit mcp_files's hard tests: every value here is what golden-mcp.ts printed under oracle/golden.oracle.ts
//! (tests/fixtures/oracle/golden_mcp_files.json). spec/golden-mcp_files.md names the rule each pins.
mod golden_common;

use golden_common::{assert_calls, cases, exec_result, fixture, plan_json, plan_of, s, strings, Scripted};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_catalog::mcp::McpFormat;
use wsp_engine::golden_seam::{
    land_configs, mcp_plan_for, mcp_row_id, parse_configs, parse_mcp_id, read_configs_cmd, read_failed, read_reason, read_whole, refused,
    rewrite_string, scope_row_id, McpAgentSource, McpAt, McpPlanOptions, ProjectMove, ReadAsk, RecipeEntry, ScopeFile, ScopeRead,
    MCP_READ_END, MCP_READ_MARK, READ_MS,
};

fn read_json(r: &Option<ScopeRead>) -> Value {
    match r {
        None => Value::Null,
        Some(ScopeRead::File(f)) => json!({ "path": f.path, "text": f.text, "sum": f.sum }),
        Some(ScopeRead::Unread { path, unread }) => json!({ "path": path, "unread": unread }),
    }
}

fn read_of(v: &Value) -> Option<ScopeRead> {
    if v.is_null() {
        return None;
    }
    Some(match v.get("unread") {
        Some(u) => ScopeRead::Unread { path: s(&v["path"]), unread: s(u) },
        None => ScopeRead::File(ScopeFile { path: s(&v["path"]), text: s(&v["text"]), sum: s(&v["sum"]) }),
    })
}

/// A landing's twelve hex digits are random; the oracle pinned them to `ab` bytes, so every landing reads that way.
fn pinned_landings(calls: &Value) -> Value {
    let mut text = serde_json::to_string(calls).unwrap();
    let mark = ".wsp-config-tmp.";
    let mut at = 0;
    while let Some(i) = text[at..].find(mark) {
        let start = at + i + mark.len();
        let end = start + 12;
        if end <= text.len() && text[start..end].bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) {
            text.replace_range(start..end, "abababababab");
        }
        at = start;
    }
    serde_json::from_str(&text).unwrap()
}

#[test]
fn the_read_marks_and_its_time_are_the_ts() {
    let f = fixture("mcp_files");
    assert_eq!(READ_MS, f["READ_MS"].as_u64().unwrap());
    assert_eq!(MCP_READ_MARK, s(&f["MCP_READ_MARK"]));
    assert_eq!(MCP_READ_END, s(&f["MCP_READ_END"]));
}

#[test]
fn row_ids_spell_the_user_scope_the_home_and_an_encoded_folder() {
    let f = fixture("mcp_files");
    for c in cases(&f, "mcpRowId") {
        let at = match &c["at"] {
            Value::Bool(false) => McpAt::User,
            Value::Bool(true) => McpAt::Home,
            v => McpAt::Folder(s(v)),
        };
        assert_eq!(mcp_row_id(&s(&c["agent"]), &at, &s(&c["name"])), s(&c["out"]), "{c}");
    }
    for c in cases(&f, "scopeRowId") {
        let sc = &c["scope"];
        let project = sc.get("project").map(|p| ProjectMove { from: s(&p["from"]), to: s(&p["to"]) });
        assert_eq!(scope_row_id(&s(&c["agent"]), project.as_ref(), sc["folder"].as_str(), &s(&c["name"])), s(&c["out"]), "{c}");
    }
}

#[test]
fn parse_mcp_id_reads_the_agent_scope_and_name_and_nothing_for_a_row_that_is_not_a_server() {
    for c in cases(&fixture("mcp_files"), "parseMcpId") {
        let got = parse_mcp_id(&s(&c["id"])).unwrap().map(|p| {
            let mut v = json!({ "agent": p.agent, "home": p.home, "name": p.name });
            if let Some(f) = p.folder {
                v["folder"] = json!(f);
            }
            v
        });
        assert_eq!(got.unwrap_or(Value::Null), c["out"], "{}", c["id"]);
    }
}

#[test]
fn a_refused_call_its_reason_and_the_failed_read_say_the_ts_words_and_never_a_line_of_output() {
    let f = fixture("mcp_files");
    for c in cases(&f, "refused") {
        let r = refused(&s(&c["message"]));
        assert_eq!(json!({ "exitCode": r.exit_code, "stdout": r.stdout, "stderr": r.stderr }), c["out"]);
    }
    for c in cases(&f, "readReason") {
        assert_eq!(read_reason(&exec_result(&c["res"]), c["timeoutS"].as_f64()), s(&c["out"]), "{c}");
    }
    for c in cases(&f, "readFailed") {
        assert_eq!(read_failed(&exec_result(&c["res"])), s(&c["out"]), "{c}");
    }
}

#[test]
fn read_configs_cmd_is_the_ts_script_byte_for_byte() {
    for c in cases(&fixture("mcp_files"), "readConfigsCmd") {
        let asks: Vec<ReadAsk> =
            c["scopes"].as_array().unwrap().iter().map(|a| ReadAsk { files: strings(&a["files"]), base: s(&a["base"]) }).collect();
        assert_eq!(read_configs_cmd(&asks), s(&c["out"]), "{}", c["scopes"]);
    }
}

#[test]
fn read_whole_finds_the_last_line_trimmed() {
    for c in cases(&fixture("mcp_files"), "readWhole") {
        assert_eq!(read_whole(&s(&c["stdout"])), c["out"].as_bool().unwrap(), "{c}");
    }
}

#[test]
fn parse_configs_reads_each_scope_and_nothing_when_no_line_carried_the_mark() {
    for c in cases(&fixture("mcp_files"), "parseConfigs") {
        let asks: Vec<ReadAsk> =
            c["scopes"].as_array().unwrap().iter().map(|a| ReadAsk { files: strings(&a["files"]), base: String::new() }).collect();
        let got = parse_configs(&s(&c["stdout"]), &asks).map(|v| Value::Array(v.iter().map(read_json).collect())).unwrap_or(Value::Null);
        assert_eq!(got, c["out"], "{:?}", c["stdout"]);
    }
}

#[test]
fn rewrite_string_names_a_bin_dir_command_and_moves_a_laptop_prefix_past_a_flag() {
    let f = fixture("mcp_files");
    let plan = plan_of(&f["plan"]);
    for c in cases(&f, "rewriteString") {
        assert_eq!(rewrite_string(&plan, &s(&c["s"]), c["command"].as_bool().unwrap()), s(&c["out"]), "{c}");
    }
}

#[test]
fn mcp_plan_for_keeps_drops_and_sets_aside_as_the_ts_and_is_nothing_with_no_server_row() {
    let f = fixture("mcp_files");
    let mut sources = IndexMap::new();
    sources.insert(
        "claude".to_string(),
        McpAgentSource {
            label: "Claude Code".into(),
            format: McpFormat::McpServersJson,
            files: vec!["/root/.claude-cfg/.claude.json".into()],
        },
    );
    sources.insert(
        "codex".to_string(),
        McpAgentSource { label: "Codex".into(), format: McpFormat::CodexToml, files: vec!["/root/.codex/config.toml".into()] },
    );
    sources.insert(
        "gemini".to_string(),
        McpAgentSource { label: "Gemini CLI".into(), format: McpFormat::McpServersJson, files: vec!["/root/.gemini/settings.json".into()] },
    );
    for c in cases(&f, "mcpPlanFor") {
        let rows: Vec<RecipeEntry> = serde_json::from_value(c["rows"].clone()).unwrap();
        let opts = McpPlanOptions {
            home: s(&c["home"]),
            guest_home: c["guestHome"].as_str().map(str::to_string),
            agents: sources.clone(),
            bin_dirs: c.get("binDirs").map(strings),
        };
        let got = mcp_plan_for(&rows, &opts).unwrap().map(|p| plan_json(&p)).unwrap_or(Value::Null);
        assert_eq!(got, c["out"], "{c}");
    }
}

#[tokio::test]
async fn land_configs_lands_what_changed_then_writes_each_and_sweeps_every_landing_on_a_failure() {
    for c in cases(&fixture("mcp_files"), "landConfigs") {
        let m = Scripted::new(&c["rules"], true);
        let read: Vec<Option<ScopeRead>> = c["read"].as_array().unwrap().iter().map(read_of).collect();
        let texts: IndexMap<String, String> = c["texts"].as_array().unwrap().iter().map(|p| (s(&p[0]), s(&p[1]))).collect();
        let base = |p: &str| if p.contains(".toml") { "/root/.codex".to_string() } else { "/root".to_string() };
        let got = land_configs(&m, &base, &read, &texts).await;
        assert_eq!(got.as_deref(), c["out"].as_str(), "{}", c["name"]);
        assert_calls(&s(&c["name"]), &pinned_landings(&m.calls()), &c["calls"]);
    }
}
