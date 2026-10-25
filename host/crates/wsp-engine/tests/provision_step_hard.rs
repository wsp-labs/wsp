// SPDX-License-Identifier: AGPL-3.0-only
//! The tools loop's and the plugins step's rules T1 to T6 in spec/provision_step.md, held to
//! oracle/strengthen.oracle.ts's run of the TS (tests/fixtures/oracle/strengthen_step.json): every stage call in
//! order with the step it says is under way, the rows, and what `run.tools` holds after.
mod common;

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use common::{ok, oracle, res, ExecFn, FakeMachine, PutFn, RunFn};
use wsp_engine::golden_seam::{free_kb_cmd, ToolInstall, BASE_VERSION_LINES, TOOLS_PATH};
use wsp_engine::machine::ExecResult;
use wsp_engine::provision::{new_setup_run, provision_step, EngineStep, PlanPlugin, ProvisionOn, ProvisionPlan, ProvisionStage};
use wsp_wire::views::place::PlaceProvisionRow;

fn floor_read() -> String {
    let mut lines: Vec<String> = Vec::new();
    for e in wsp_catalog::catalog::base_floor() {
        let version = match &e.major {
            None => "9.9.9".to_string(),
            Some(m) => format!("{}.0", m.version),
        };
        lines.push(format!("VERSION {}: {} {version}", e.bin, e.bin));
        for b in e.brings.iter().flatten() {
            lines.push(format!("VERSION {}: 9.9.9", b.bin));
        }
    }
    lines.push("VERSION sshd: OpenSSH_9.6p1".to_string());
    lines.join("\n")
}

fn box_machine(answer: impl Fn(&str) -> Option<ExecResult> + 'static) -> FakeMachine {
    let answer = Arc::new(answer);
    let exec_answer = answer.clone();
    let exec: ExecFn = Arc::new(move |cmd, _| {
        let said = exec_answer(&cmd);
        Box::pin(async move {
            if let Some(said) = said {
                return Ok(said);
            }
            if cmd == free_kb_cmd() {
                return Ok(res(0, "9000000\n", ""));
            }
            if cmd == format!("export PATH={TOOLS_PATH}:$PATH\n{BASE_VERSION_LINES}") {
                return Ok(res(0, &format!("{}\n", floor_read()), ""));
            }
            Ok(ok())
        })
    });
    let run: RunFn = Arc::new(move |script, _| {
        let said = answer(&script);
        Box::pin(async move { Ok(said.unwrap_or_else(ok)) })
    });
    let put: PutFn = Arc::new(|_, _| Box::pin(async { Ok(None) }));
    FakeMachine { id: "spoo".to_string(), exec, run: Some(run), download_url: None, upload_url: None, put_bytes: Some(put) }
}

fn tool(id: &str, label: &str) -> ToolInstall {
    ToolInstall {
        id: id.to_string(),
        label: label.to_string(),
        manager: "script".to_string(),
        cmd: format!("install {id}"),
        ..Default::default()
    }
}

fn plan_of(steps: Vec<ToolInstall>) -> ProvisionPlan {
    ProvisionPlan { recipe_at: "x".to_string(), path: TOOLS_PATH.to_string(), steps, ..Default::default() }
}

fn on() -> ProvisionOn {
    ProvisionOn { home: "/root".to_string(), held: None, stores: None }
}

/// A row as the oracle writes it: id, label, outcome, note and step, `null` where absent; `ms` left out (a clock's).
fn bare(row: &PlaceProvisionRow) -> Value {
    let v = serde_json::to_value(row).unwrap();
    json!({ "id": v["id"], "label": v["label"], "outcome": v["outcome"], "note": v.get("note").cloned().unwrap_or(Value::Null), "step": v.get("step").cloned().unwrap_or(Value::Null) })
}

/// T1 to T4: a row is staged with the step *after* it as the one under way (`None` after the last), a detail line
/// with the step it is on; the loop's own check lines come after the last row; a row the check corrected is said
/// again as a line alone; a failed hook fails its row and is not said again; `run.tools` keeps the loop's result, so
/// the row whose hook failed still reads installed there.
#[tokio::test(flavor = "multi_thread")]
async fn the_tools_loop_says_each_row_with_the_next_step_and_keeps_the_loops_result_on_the_run() {
    let machine = box_machine(|cmd| {
        if cmd.contains("wsp-check") {
            return Some(res(0, "wsp-check 0 Error: brew cannot read the folder.\n", ""));
        }
        if cmd.contains("git lfs install") {
            return Some(res(2, "", "warming up\ngit: 'lfs' is not a git command\n"));
        }
        None
    });
    let steps = vec![
        ToolInstall { manager: "brew".to_string(), check: Some("brew list --versions bat".to_string()), ..tool("tools/brew/bat", "bat") },
        ToolInstall { bin: Some("git-lfs".to_string()), ..tool("tools/brew/git-lfs", "Git LFS") },
        ToolInstall { bin: Some("jq".to_string()), ..tool("tools/brew/jq", "jq") },
    ];
    let run = new_setup_run();
    let staged = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen = staged.clone();
    let stage: ProvisionStage = Arc::new(move |detail, at, row| {
        seen.lock().unwrap().push(json!([detail, at.map(|a| format!("{}/{}: {}", a.index, a.of, a.label)), row.map(bare)]));
    });
    let rows = provision_step(&machine, &plan_of(steps), EngineStep::Clis, run.clone(), stage, &on()).await.unwrap();
    let want = &oracle("strengthen_step.json")["toolsLoop"];
    assert_eq!(json!(*staged.lock().unwrap()), want["staged"]);
    assert_eq!(json!(rows.iter().map(bare).collect::<Vec<_>>()), want["rows"]);
    let tools: Vec<Value> = run.lock().unwrap().tools.iter().map(|t| json!([t.id, t.outcome, t.note])).collect();
    assert_eq!(json!(tools), want["runTools"]);
}

/// T5, T6: a plugin's note is the last line of its trimmed stderr, else of its stdout, cut to 300 UTF-16 units, else
/// `exit <n>`; a run that rejects is exit -1 with the error's message as stderr; `asked` is read only on a non-zero
/// exit and only a `Some` skips; each row is staged with its line as it lands.
#[tokio::test(flavor = "multi_thread")]
async fn a_plugins_note_is_the_last_trimmed_line_of_what_it_said_cut_at_300_else_its_exit() {
    let long = "y".repeat(400);
    let exec: ExecFn = Arc::new(|_, _| Box::pin(async { Ok(ok()) }));
    let run: RunFn = Arc::new(move |script: String, _| {
        let long = long.clone();
        Box::pin(async move {
            if script.ends_with("\nadd blank") {
                return Ok(res(3, "", ""));
            }
            if script.ends_with("\nadd gone") {
                return Err(common::other("the machine went away\nconnection reset"));
            }
            if script.ends_with("\nadd quiet") {
                return Ok(res(1, "  first\nlast of stdout  \n", ""));
            }
            if script.ends_with("\nadd long") {
                return Ok(res(1, "", &format!("{long}\n\n")));
            }
            Ok(ok())
        })
    });
    let machine = FakeMachine { run: Some(run), ..FakeMachine::new(exec) };
    let plugin = |name: &str| PlanPlugin {
        id: format!("plugins/{name}"),
        label: name.to_string(),
        cmd: format!("add {name}"),
        asked: Some(Arc::new(|out: &str| out.contains("never").then(|| "asked".to_string()))),
    };
    let plan =
        ProvisionPlan { plugins: Some(["blank", "gone", "quiet", "long", "fine"].into_iter().map(plugin).collect()), ..plan_of(vec![]) };
    let staged = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen = staged.clone();
    let stage: ProvisionStage = Arc::new(move |detail, _, row| seen.lock().unwrap().push(json!([detail, row.map(bare)])));
    let rows = provision_step(&machine, &plan, EngineStep::Plugins, new_setup_run(), stage, &on()).await.unwrap();
    let want = &oracle("strengthen_step.json")["plugins"];
    assert_eq!(json!(rows.iter().map(bare).collect::<Vec<_>>()), want["rows"]);
    assert_eq!(json!(*staged.lock().unwrap()), want["staged"]);
}
