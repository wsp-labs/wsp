// SPDX-License-Identifier: AGPL-3.0-only
//! provision_plan's R-rules (spec/provision_plan.md R1 to R8): the plan's files, the need rows' place, skipped and
//! extras, the presence read's exact text, its pages and its markers, the waiters' single pass and row_of. Values from
//! running provision.ts at a4033dc34 (scratch oracle, 2026-10-25).
mod common;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;

use common::{res, ExecFn, FakeMachine};
use wsp_catalog::mcp::MCP_SERVERS_JSON;
use wsp_engine::golden_seam::{agent_installs_for, GoldenFiles, GoldenImport, McpAgentPlan, McpPlan, McpScope, PackFiles, PackedFiles, PinRead, SkippedRow, ToolInstall, ToolResult};
use wsp_engine::machine::ExecOptions;
use wsp_engine::provision::*;
use wsp_engine::provision_files::ProvisionLanding;
use wsp_wire::views::place::PlaceProvisionRowOutcome;

fn s(t: &str) -> String {
    t.to_string()
}

fn landing(id: &str, dest: &str, once: bool) -> ProvisionLanding {
    ProvisionLanding { id: s(id), label: s(id), dest: s(dest), once }
}

fn pack() -> PackFiles {
    Arc::new(|_| Box::pin(async { Ok(PackedFiles::default()) }))
}

fn files_of(lands: Vec<ProvisionLanding>) -> Option<GoldenFiles> {
    Some(GoldenFiles { lands, pack: pack() })
}

fn tool(id: &str) -> ToolInstall {
    ToolInstall { id: s(id), label: s(id), manager: s("script"), cmd: s("true"), ..Default::default() }
}

/// R1: the plan's files are the first landing per dest in first-seen order; a dest becomes once by exact equality with
/// an agent's server file under the guest home (a folder above it does not), a landing that came once stays once, and
/// an empty list of landings is no files at all.
#[test]
fn the_plans_files_are_one_per_dest_once_by_exact_dest_and_none_when_empty() {
    let mcp = McpPlan {
        agents: vec![McpAgentPlan {
            id: s("claude"),
            label: s("Claude Code"),
            scopes: vec![McpScope {
                files: vec![s("/root/.claude/settings.json")],
                format: MCP_SERVERS_JSON,
                project: None,
                keep: vec![],
                drop: vec![],
                travelled: None,
                own: None,
                folder: None,
                trust: None,
                edit: None,
            }],
            aside: vec![],
        }],
        guest_home: s("/root"),
        rewrites: vec![],
        bin_dirs: vec![],
        tools: vec![],
    };
    let imp = GoldenImport {
        files: files_of(vec![landing("a", ".x/y", true), landing("b", ".claude", false), landing("c", ".claude/settings.json", false)]),
        mcp: Some(mcp),
        ..Default::default()
    };
    let plan = provision_plan_of(&imp, "x", "/p", None, None);
    let lands: Vec<(String, bool)> = plan.files.unwrap().lands.into_iter().map(|l| (l.dest, l.once)).collect();
    assert_eq!(lands, vec![(s(".x/y"), true), (s(".claude"), false), (s(".claude/settings.json"), true)]);
    let imp = GoldenImport {
        files: files_of(vec![landing("1", "b", false), landing("2", "a", false), landing("3", "b", false), landing("4", "a", false), landing("5", "c", false)]),
        ..Default::default()
    };
    let ids: Vec<String> = provision_plan_of(&imp, "x", "/p", None, None).files.unwrap().lands.into_iter().map(|l| l.id).collect();
    assert_eq!(ids, vec![s("1"), s("2"), s("5")]);
    let imp = GoldenImport { files: files_of(vec![]), ..Default::default() };
    assert!(provision_plan_of(&imp, "x", "/p", None, None).files.is_none());
}

/// R2: every agent install comes first, then every need row, then the import's tools; `agents` counts installs and need
/// rows, so the split point the steps read is exact.
#[test]
fn need_rows_follow_every_agent_install_not_their_own_agent() {
    let mut imp = GoldenImport { tools: vec![tool("tools/z")], ..Default::default() };
    let agents = agent_installs_for(&["agents/codex", "agents/claude"]);
    imp.agents = agents.installs;
    imp.node = agents.node;
    let plan = provision_plan_of(&imp, "x", "/p", None, None);
    let ids: Vec<&str> = plan.steps.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["agents/node", "agents/codex", "agents/claude", "agents/codex/bubblewrap", "agents/codex/bwrap-apparmor", "tools/z"]);
    assert_eq!(plan.agents, 5);
}

/// R3: skipped is the skipped agents then the skipped tools; no extras is `compiler: true` and no optional field; given
/// extras carry their own compiler and only the fields they set, an empty list staying `Some`.
#[test]
fn skipped_rows_then_extras_overlay_only_what_they_set() {
    let imp = GoldenImport {
        skipped_agents: vec![SkippedRow { id: s("agents/zed"), label: s("Zed"), note: s("no installer known") }],
        skipped_tools: vec![SkippedRow { id: s("tools/brew-cask/raycast"), label: s("Raycast"), note: s("macOS app") }],
        ..Default::default()
    };
    let plan = provision_plan_of(&imp, "x", "/p", None, None);
    assert_eq!(
        plan.skipped,
        vec![
            PlanSkipped { id: s("agents/zed"), label: s("Zed"), note: s("no installer known") },
            PlanSkipped { id: s("tools/brew-cask/raycast"), label: s("Raycast"), note: s("macOS app") },
        ]
    );
    assert!(plan.compiler);
    assert!(plan.files.is_none() && plan.mcp.is_none() && plan.skills.is_none() && plan.configs.is_none() && plan.github.is_none() && plan.plugins.is_none());
    let plan = provision_plan_of(&imp, "x", "/p", None, Some(ProvisionExtras { compiler: false, github: Some(vec![]), plugins: Some(vec![]), ..Default::default() }));
    assert!(!plan.compiler);
    assert_eq!(plan.github, Some(vec![]));
    assert_eq!(plan.plugins.map(|p| p.len()), Some(0));
    assert!(plan.skills.is_none());
}

type Seen = Arc<Mutex<Vec<(String, Option<u64>)>>>;

fn answering(answers: Vec<wsp_engine::errors::Result<wsp_engine::machine::ExecResult>>) -> (FakeMachine, Seen) {
    let seen: Seen = Arc::default();
    let log = seen.clone();
    let answers = Arc::new(Mutex::new(answers.into_iter()));
    let exec: ExecFn = Arc::new(move |cmd: String, o: ExecOptions| {
        log.lock().unwrap().push((cmd, o.timeout_ms));
        let next = answers.lock().unwrap().next().unwrap_or_else(|| Ok(res(0, "", "")));
        Box::pin(async move { next })
    });
    (FakeMachine::new(exec), seen)
}

fn bin(id: &str, b: &str) -> ToolInstall {
    ToolInstall { bin: Some(s(b)), ..tool(id) }
}

/// R4: one exec under 20 s per page of eight reads: the path line, then one `if` per step that has a test, the index
/// counting only those steps, the printf's `\n` the two characters.
#[tokio::test]
async fn the_presence_read_is_one_exact_exec_whose_index_counts_only_steps_with_a_test() {
    let (m, seen) = answering(vec![]);
    let steps = vec![
        ToolInstall { bin: Some(s("it's")), check: Some(s("a --v")), ..tool("a") },
        ToolInstall { check: Some(s("b ok")), ..tool("b") },
        tool("c"),
        ToolInstall { bin: Some(s("d")), asks: Some(s("1")), pin: Some(PinRead { read: Some(s("d -v")), fixed: true, words: s("w") }), ..tool("d") },
    ];
    let got = present_steps(&m, &steps, "/p/bin:/bin", None, Versions::Asked).await.unwrap();
    assert!(got.is_empty());
    let want = [
        "export PATH=/p/bin:/bin PNPM_HOME=/root/.local/share/pnpm",
        "if ( a --v ) >/dev/null 2>&1 && command -v 'it'\\''s' >/dev/null 2>&1; then printf 'wsp-present %s %s\\n' 0 \"$(command -v 'it'\\''s' 2>/dev/null)\"; fi",
        "if ( b ok ) >/dev/null 2>&1; then printf 'wsp-present %s %s\\n' 1 \"\"; fi",
        "if command -v 'd' >/dev/null 2>&1 && [ \"$( ( d -v ) 2>/dev/null | head -n 1 | tr -d '[:space:]' )\" = '1' ]; then printf 'wsp-present %s %s\\n' 2 \"$(command -v 'd' 2>/dev/null)\"; fi",
    ]
    .join("\n");
    assert_eq!(*seen.lock().unwrap(), vec![(want, Some(20_000))]);
    let (m, seen) = answering(vec![]);
    assert!(present_steps(&m, &[], "/p", None, Versions::Asked).await.unwrap().is_empty());
    assert!(seen.lock().unwrap().is_empty());
}

/// R5: a page that exits non-zero says nothing is present in it, markers and all; the next page is still read with
/// its index from 0; an exec that rejects rejects the call with nothing kept.
#[tokio::test]
async fn a_failed_page_reads_nothing_the_next_page_counts_from_zero_and_a_rejection_rejects() {
    let steps: Vec<ToolInstall> = (0..10).map(|i| bin(&format!("s{i}"), &format!("b{i}"))).collect();
    let (m, seen) = answering(vec![Ok(res(1, "wsp-present 0 /x\nwsp-present 1 /y\n", "")), Ok(res(0, "wsp-present 0 /a\nwsp-present 1\n", ""))]);
    let got = present_steps(&m, &steps, "/bin", None, Versions::Asked).await.unwrap();
    assert_eq!(got.into_iter().collect::<Vec<_>>(), vec![(s("s8"), PresentRead { path: Some(s("/a")) }), (s("s9"), PresentRead { path: None })]);
    let second = seen.lock().unwrap()[1].0.clone();
    assert_eq!(
        second,
        [
            "export PATH=/bin PNPM_HOME=/root/.local/share/pnpm",
            "if command -v 'b8' >/dev/null 2>&1; then printf 'wsp-present %s %s\\n' 0 \"$(command -v 'b8' 2>/dev/null)\"; fi",
            "if command -v 'b9' >/dev/null 2>&1; then printf 'wsp-present %s %s\\n' 1 \"$(command -v 'b9' 2>/dev/null)\"; fi",
        ]
        .join("\n")
    );
    let (m, _) = answering(vec![Ok(res(0, "wsp-present 0 /x\n", "")), Err(common::other("link down"))]);
    assert_eq!(present_steps(&m, &steps, "/bin", None, Versions::Asked).await.unwrap_err().to_string(), "link down");
}

/// R6: each marker line is trimmed, its rest rejoined on single spaces, the last marker of an index wins, a line not
/// starting with the mark or with an index outside the page is ignored, and the answer is in step order; a step id
/// twice keeps its first place and its last read.
#[tokio::test]
async fn markers_are_trimmed_the_last_wins_and_the_answer_is_in_step_order() {
    let steps: Vec<ToolInstall> = (0..6).map(|i| bin(&format!("s{i}"), &format!("b{i}"))).collect();
    let said = "  wsp-present 0   /a b  \nwsp-present 1\nwsp-present 2 /c\nwsp-present 2 /d\nxwsp-present 3 /e\nwsp-present x /f\nwsp-present 9 /g\nwsp-present 4 /h\t\nwsp-present 5 /i  j";
    let (m, _) = answering(vec![Ok(res(0, said, ""))]);
    let got: Vec<(String, Option<String>)> = present_steps(&m, &steps, "/bin", None, Versions::Asked).await.unwrap().into_iter().map(|(k, v)| (k, v.path)).collect();
    assert_eq!(got, vec![(s("s0"), Some(s("/a b"))), (s("s1"), None), (s("s2"), Some(s("/d"))), (s("s4"), Some(s("/h"))), (s("s5"), Some(s("/i  j")))]);
    let steps = vec![bin("k", "a"), bin("m", "m"), bin("k", "k2")];
    let (m, _) = answering(vec![Ok(res(0, "wsp-present 0 /a\nwsp-present 1 /b\nwsp-present 2 /c", ""))]);
    let got: Vec<(String, Option<String>)> = present_steps(&m, &steps, "/bin", None, Versions::Asked).await.unwrap().into_iter().map(|(k, v)| (k, v.path)).collect();
    assert_eq!(got, vec![(s("k"), Some(s("/c"))), (s("m"), Some(s("/b")))]);
}

/// R7: one pass over the ids given: a step with no test is present only when every step waiting on it is in that set,
/// whatever this pass already found; each id once.
#[test]
fn the_waiters_rule_is_one_pass_over_the_set_given() {
    let after = |id: &str, on: &str, b: Option<&str>| ToolInstall { after: Some(s(on)), bin: b.map(s), ..tool(id) };
    let steps = vec![tool("A"), after("B", "A", None), after("C", "B", Some("c")), after("D", "D", None), bin("E", "e")];
    let set = |ids: &[&str]| ids.iter().map(|i| s(i)).collect::<BTreeSet<String>>();
    assert_eq!(present_by_what_waits(&steps, &set(&["C"])), vec![s("B")]);
    assert_eq!(present_by_what_waits(&steps, &set(&["B", "C"])), vec![s("A"), s("B")]);
    let twice = vec![tool("Z"), tool("Z"), after("W", "Z", Some("w"))];
    assert_eq!(present_by_what_waits(&twice, &set(&["W"])), vec![s("Z")]);
}

fn result(outcome: &str, note: Option<&str>, ms: Option<i64>) -> ToolResult {
    ToolResult { id: s("tools/node"), label: s("Node"), outcome: s(outcome), note: note.map(s), ms, bytes: None }
}

/// R8: row_of: an installed result read present is `present` and loses its own note (the elsewhere note or nothing);
/// a read never turns a failure or a skip into present; `ms: Some(0)` and `note: Some("")` are kept. row_line prints
/// `()` for an empty note.
#[test]
fn row_of_reads_present_only_over_an_install_and_row_line_prints_an_empty_note() {
    let read = |p: Option<&str>| IndexMap::from([(s("tools/node"), PresentRead { path: p.map(s) })]);
    let none = IndexMap::new();
    let row = row_of(&result("installed", Some("already on the machine"), Some(7)), &read(Some("/usr/bin/node")), None);
    assert_eq!((row.outcome, row.note.clone(), row.ms), (PlaceProvisionRowOutcome::Present, None, Some(7)));
    let row = row_of(&result("failed", Some("boom"), None), &read(Some("/usr/bin/node")), None);
    assert_eq!((row.outcome, row.note.clone()), (PlaceProvisionRowOutcome::Failed, Some(s("boom"))));
    let row = row_of(&result("installed", Some(""), Some(0)), &none, None);
    assert_eq!((row.outcome, row.note.clone(), row.ms), (PlaceProvisionRowOutcome::Installed, Some(s("")), Some(0)));
    assert!(row.kind.is_none() && row.step.is_none());
    assert_eq!(row_line(&row), "Node: installed ()");
    let row = row_of(&result("installed", None, None), &read(None), None);
    assert_eq!(row_line(&row), "Node: present");
}
