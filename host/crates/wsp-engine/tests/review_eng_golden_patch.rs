// SPDX-License-Identifier: AGPL-3.0-only
//! Review tests for the golden seam that fail on the current code. Each holds the TS's answer inline, as node 22
//! printed it from the TS at the cited file:line, and reads no file outside the crate.
#![allow(clippy::too_many_lines)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine as _;
use indexmap::IndexMap;
use serde_json::json;
use wsp_catalog::mcp::McpFormat;
use wsp_engine::cancel::Cancel;
use wsp_engine::errors::{EngineError, Result};
use wsp_engine::golden_seam::{
    apply_mcp, install_tools_at, land_configs, reason_of, GoldenStage, InstallToolsOptions, McpAgentPlan, McpPlan, McpScope, ScopeFile,
    ScopeRead, StageListener, ToolInstall,
};
use wsp_engine::machine::{
    BytesLanded, ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineOffers, MachineState, RunOptions, SnapshotOptions,
};

/// A machine that answers by the first rule whose text is in the call (and whose kind is the call's): its next answer,
/// the last one again once they run out; a call no rule matches answers exit 0 with nothing. It takes bytes by
/// `put_bytes` and answers nothing for them.
struct Scripted {
    rules: Vec<(&'static str, &'static str, Vec<ExecResult>)>,
    used: Mutex<Vec<usize>>,
}

impl Scripted {
    fn new(rules: Vec<(&'static str, &'static str, Vec<ExecResult>)>) -> Self {
        let used = Mutex::new(vec![0; rules.len()]);
        Scripted { rules, used }
    }

    fn answer(&self, kind: &str, text: &str) -> Result<ExecResult> {
        let Some(at) = self.rules.iter().position(|(has, k, _)| text.contains(has) && (k.is_empty() || *k == kind)) else {
            return Ok(res(0, "", ""));
        };
        let mut used = self.used.lock().unwrap();
        let seq = &self.rules[at].2;
        let a = seq[used[at].min(seq.len() - 1)].clone();
        used[at] += 1;
        Ok(a)
    }
}

fn res(exit_code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code: exit_code.into(), stdout: stdout.to_string(), stderr: stderr.to_string() }
}

#[async_trait(?Send)]
impl Machine for Scripted {
    fn id(&self) -> &str {
        "m1"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { put_bytes: true, ..MachineOffers::default() }
    }
    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> Result<ExecResult> {
        self.answer("exec", cmd)
    }
    async fn run(&self, script: &str, _opts: RunOptions) -> Result<ExecResult> {
        self.answer("run", script)
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Ok("snap".to_string())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<()> {
        Ok(())
    }
    async fn kill(&self) -> Result<()> {
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        Err(EngineError::Other { message: "no state".into(), code: None })
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Ok("https://signed.example/dl".to_string())
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Ok("https://signed.example/ul".to_string())
    }
    async fn put_bytes(&self, _path: &str, _bytes: &[u8], _timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        Ok(None)
    }
}

// --- finding 1: JSON.parse reads a Claude config that serde_json refuses -----------------------------------------------

/// One Claude scope keeping `github`, run through the builder's whole round on a machine that holds `config` as
/// `/root/.claude.json`, `npx` on its PATH and 9 GiB free.
async fn github_row(config: &str) -> (String, Option<String>) {
    let read = format!("wsp-mcp 0 0 {}\nwsp-mcp end\n", base64::engine::general_purpose::STANDARD.encode(config));
    let m = Scripted::new(vec![
        ("wsp_mcp_read() {", "run", vec![res(0, &read, "")]),
        ("if command -v", "exec", vec![res(0, "ok npx\n", "")]),
        ("df -Pk", "", vec![res(0, "9437184\n", "")]),
    ]);
    let scope = McpScope {
        files: vec!["/root/.claude.json".into()],
        format: McpFormat::McpServersJson,
        project: None,
        keep: vec!["github".into()],
        drop: vec![],
        travelled: None,
        own: None,
        folder: None,
        trust: None,
        edit: None,
    };
    let plan = McpPlan {
        agents: vec![McpAgentPlan { id: "claude".into(), label: "Claude Code".into(), scopes: vec![scope], aside: vec![] }],
        guest_home: "/root".into(),
        rewrites: vec![],
        bin_dirs: vec![],
        tools: vec![],
    };
    let stage: StageListener = Arc::new(|_: GoldenStage, _: Option<&str>| {});
    let rows = apply_mcp(&m, &plan, stage, &[]).await;
    let row = rows.iter().find(|r| r.name == "github").expect("a row for github");
    (row.outcome.clone(), row.note.clone())
}

// TS golden-mcp.ts:303 `scope.format.edit` over packages/catalog/src/jsonc.ts:41 `JSON.parse`, node 22, for each input:
//   PATCH1 [["github","fetched-on-first-use","npx fetches the package on first use"]]
const GITHUB_ON_FIRST_USE: (&str, &str) = ("fetched-on-first-use", "npx fetches the package on first use");

#[tokio::test(flavor = "current_thread")]
async fn a_claude_config_holding_a_lone_surrogate_escape_still_has_its_github_server_edited() {
    // A history string cut inside a pair, written as \ud83d (the raw string keeps the backslash for the JSON text).
    let config = r#"{"history": ["cut \ud83d here"], "mcpServers": {"github": {"command": "npx"}}}"#;
    let (outcome, note) = github_row(config).await;
    assert_eq!((outcome.as_str(), note.as_deref()), (GITHUB_ON_FIRST_USE.0, Some(GITHUB_ON_FIRST_USE.1)));
}

#[tokio::test(flavor = "current_thread")]
async fn a_claude_config_holding_1e999_still_has_its_github_server_edited() {
    // JSON.parse reads 1e999 as Infinity.
    let config = r#"{"numStartups": 1e999, "mcpServers": {"github": {"command": "npx"}}}"#;
    let (outcome, note) = github_row(config).await;
    assert_eq!((outcome.as_str(), note.as_deref()), (GITHUB_ON_FIRST_USE.0, Some(GITHUB_ON_FIRST_USE.1)));
}

#[tokio::test(flavor = "current_thread")]
async fn a_claude_config_nested_200_deep_still_has_its_github_server_edited() {
    // serde_json stops at 128 levels of nesting, JSON.parse does not.
    let config = format!(r#"{{"deep": {}{}, "mcpServers": {{"github": {{"command": "npx"}}}}}}"#, "[".repeat(200), "]".repeat(200));
    let (outcome, note) = github_row(&config).await;
    assert_eq!((outcome.as_str(), note.as_deref()), (GITHUB_ON_FIRST_USE.0, Some(GITHUB_ON_FIRST_USE.1)));
}

// --- finding 3: the exit code of a config write is read whole ----------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn a_config_write_that_exits_2_pow_32_plus_5_is_worded_by_its_exit_and_not_as_exit_5() {
    // TS golden-mcp.ts:334 `configRefusal(res, ...) ?? reasonOf(res, ...)`, config-write.ts:98 compares exitCode to 5 whole:
    //   NIT land 4294967301 {"ok":"the edited config did not land (exit 4294967301)"}
    // (exit 5 itself is "... /root/a.json changed while wsp was writing it, so nothing was written; try again.").
    let m = Scripted::new(vec![("printf", "exec", vec![res(4294967301, "", "")])]);
    let read = vec![Some(ScopeRead::File(ScopeFile { path: "/root/a.json".into(), text: "{}\n".into(), sum: "s1".into() }))];
    let texts: IndexMap<String, String> = [("/root/a.json".to_string(), "{\"y\":2}\n".to_string())].into_iter().collect();
    let out = land_configs(&m, &|_: &str| "/root".to_string(), &read, &texts).await;
    assert_eq!(out.as_deref(), Some("the edited config did not land (exit 4294967301)"));
}

// --- finding 4: what an install moved on the disk keeps its fraction ---------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn an_install_that_moved_half_a_byte_by_df_says_so_in_its_bytes() {
    // TS golden-tools.ts:479 `Math.max(0, free.bytes - left.bytes)` with df printing 5000000.0005 then 5000000 kB:
    //   PATCH4 [0.5120000839233398]
    let m = Scripted::new(vec![(
        "df -Pk",
        "",
        vec![res(0, "5000000.0005\n", ""), res(0, "5000000\n", ""), res(0, "5000000\n", ""), res(0, "5000000\n", "")],
    )]);
    let step =
        ToolInstall { id: "tools/x".into(), label: "x".into(), manager: "script".into(), cmd: "echo x".into(), ..Default::default() };
    let stage = Arc::new(|_: GoldenStage, _: Option<&str>| {});
    let opts = InstallToolsOptions { keep_caches: true, ..Default::default() };
    let out = install_tools_at(&m, &[step], stage, GoldenStage::InstallingTools, opts).await.unwrap();
    assert_eq!(serde_json::to_value(&out.tools[0]).unwrap()["bytes"], json!(0.5120000839233398));
}

// --- finding 5: an exit code past the i64 range is worded as JavaScript writes it --------------------------------------

#[test]
fn a_run_that_exits_1e21_is_worded_exit_1e_plus_21() {
    // TS golden-tools.ts:96 `exit ${res.exitCode}`:
    //   PATCH5 "exit 1e+21"
    let r: ExecResult = serde_json::from_value(json!({ "exitCode": 1e21, "stdout": "", "stderr": "" })).unwrap();
    assert_eq!(reason_of(&r, 20.0), "exit 1e+21");
}
