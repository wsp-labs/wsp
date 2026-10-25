// SPDX-License-Identifier: AGPL-3.0-only
//! The two-pass merge's rules S1 to S6 in spec/provision_mcp.md: what each round appends to the list beside the job,
//! what `into` is told, and the rows, held to oracle/strengthen.oracle.ts's run of the TS on the same guest
//! (tests/fixtures/oracle/strengthen_mcp.json, every temp root written `{root}`).
mod common;

use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
use serde_json::{json, Value};

use common::{box_guest, oracle, BoxGuest, RunFn};
use wsp_catalog::mcp::{CODEX_TOML, MCP_SERVERS_JSON};
use wsp_engine::golden_seam::{DropName, GoldenStage, McpAgentPlan, McpPlan, McpScope, PackFiles, PackedFiles, TOOLS_PATH};
use wsp_engine::provision_files::{close_agent_files, once_paths_of, provision_files, OwnedPaths, ProvisionFilesAsk, ProvisionLanding};
use wsp_engine::provision_mcp::{provision_mcp, ProvisionMcpOptions};
use wsp_engine::vault::{tar_of, TarEntry};
use wsp_wire::views::place::PlaceProvisionRowOutcome;
use wsp_words::project_path::place_provision_paths;

const HOME: &str = "/Users/dev";

fn s(t: &str) -> String {
    t.to_string()
}

fn pretty(v: Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(&v).unwrap())
}

fn fixture(key: &str, root: &str) -> Value {
    let text = serde_json::to_string(&oracle("strengthen_mcp.json")[key]).unwrap();
    serde_json::from_str(&text.replace("{root}", root)).unwrap()
}

fn claude_travelled() -> String {
    pretty(json!({
        "numStartups": 7,
        "mcpServers": {
            "gsc": { "command": "npx", "args": ["-y", "gsc-mcp", "--stdio"] },
            "notes": { "command": format!("{HOME}/Library/Notes/mcp"), "args": [] },
            "mine": { "command": "npx", "args": ["theirs-on-the-mac"] },
        },
    }))
}

fn codex_travelled() -> String {
    [
        "model = \"gpt-5\"",
        "",
        "[mcp_servers.context7]",
        "command = \"npx\"",
        "args = [\"-y\", \"context7\"]",
        "",
        "[mcp_servers.grafana]",
        "command = \"npx\"",
        "",
    ]
    .join("\n")
}

fn lands() -> Vec<ProvisionLanding> {
    vec![
        ProvisionLanding { id: s("agents/claude"), label: s("Claude Code"), dest: s(".claude-cfg/.claude.json"), once: true },
        ProvisionLanding { id: s("agents/codex"), label: s("Codex"), dest: s(".codex/config.toml"), once: true },
    ]
}

fn dropped(name: &str, reason: &str) -> DropName {
    DropName { name: s(name), reason: s(reason), keys: None }
}

fn scope(file: String, format: wsp_catalog::mcp::McpFormat, keep: &[&str], drop: Vec<DropName>) -> McpScope {
    McpScope {
        files: vec![file],
        format,
        project: None,
        keep: keep.iter().map(|k| s(k)).collect(),
        drop,
        travelled: None,
        own: None,
        folder: None,
        trust: None,
        edit: None,
    }
}

fn plan_on(root: &str) -> McpPlan {
    McpPlan {
        agents: vec![
            McpAgentPlan {
                id: s("claude"),
                label: s("Claude Code"),
                scopes: vec![scope(
                    format!("{root}/.claude-cfg/.claude.json"),
                    MCP_SERVERS_JSON,
                    &["gsc", "mine"],
                    vec![dropped("notes", "command is macOS-only, will not run")],
                )],
                aside: vec![],
            },
            McpAgentPlan {
                id: s("codex"),
                label: s("Codex"),
                scopes: vec![scope(format!("{root}/.codex/config.toml"), CODEX_TOML, &["context7"], vec![dropped("grafana", "unticked")])],
                aside: vec![],
            },
        ],
        guest_home: s(root),
        rewrites: vec![(format!("{HOME}/"), format!("{root}/"))],
        bin_dirs: vec![],
        tools: vec![],
    }
}

fn pack() -> PackFiles {
    Arc::new(move |_leave| {
        Box::pin(async move {
            let tar = tar_of(&[
                TarEntry::File { path: s(".claude-cfg/.claude.json"), mode: 0o600, content: claude_travelled().into_bytes() },
                TarEntry::File { path: s(".codex/config.toml"), mode: 0o600, content: codex_travelled().into_bytes() },
            ])?;
            let len = tar.len() as u64;
            Ok(PackedFiles { tar, bytes: len, unpacked: len, ..PackedFiles::default() })
        })
    })
}

fn lines_of(path: &str) -> Vec<String> {
    std::fs::read_to_string(path).map(|t| t.split('\n').filter(|l| !l.is_empty()).map(s).collect()).unwrap_or_default()
}

fn word(o: PlaceProvisionRowOutcome) -> &'static str {
    match o {
        PlaceProvisionRowOutcome::Installed => "installed",
        PlaceProvisionRowOutcome::Present => "present",
        PlaceProvisionRowOutcome::Failed => "failed",
        PlaceProvisionRowOutcome::Skipped => "skipped",
    }
}

type Into = Arc<Mutex<Vec<(String, Vec<String>)>>>;

/// One round as the oracle's `round`: the files round unless `files` is false, the servers round, what it appended
/// to the landing, then the close; answers `{ rows, appended, listed }` in the oracle's shape.
async fn round(g: &BoxGuest, plan: &McpPlan, files: bool, into: Option<Into>) -> Value {
    let root = g.root_str();
    let owned: OwnedPaths = if files {
        provision_files(
            &g.machine,
            ProvisionFilesAsk { home: root.clone(), lands: lands(), pack: pack(), say: None, stores: IndexMap::new() },
        )
        .await
        .owned
    } else {
        OwnedPaths::default()
    };
    let at = place_provision_paths(&root);
    let before = lines_of(&at.landing).len();
    let into_fn: Option<Arc<dyn Fn(&str, &[String])>> = into.map(|seen| {
        let f: Arc<dyn Fn(&str, &[String])> =
            Arc::new(move |file: &str, names: &[String]| seen.lock().unwrap().push((s(file), names.to_vec())));
        f
    });
    let rows = provision_mcp(
        &g.machine,
        plan,
        ProvisionMcpOptions {
            home: root.clone(),
            landed: owned,
            tools: vec![],
            stage: Arc::new(|_: GoldenStage, _: Option<&str>| {}),
            path: s(TOOLS_PATH),
            held: None,
            stores: None,
            into: into_fn,
        },
    )
    .await;
    let appended: Vec<String> = lines_of(&at.landing).into_iter().skip(before).collect();
    close_agent_files(&g.machine, &root, &once_paths_of(&lands()), &IndexMap::new()).await;
    json!({
        "rows": rows.iter().map(|r| json!([r.id, word(r.outcome), r.note])).collect::<Vec<_>>(),
        "appended": appended,
        "listed": lines_of(&at.landed),
    })
}

fn into_json(seen: &Into) -> Value {
    json!(seen.lock().unwrap().iter().map(|(f, n)| json!([f, n])).collect::<Vec<_>>())
}

/// S1, S2: records in the order the merge wrote them, then a tombstone for every planned name it did not write (the
/// drop, the name set aside as the person's, the unticked one), every round; the close keeps the records alone.
#[tokio::test]
async fn every_round_appends_its_records_then_a_tombstone_for_each_planned_name_it_did_not_write() {
    let g = box_guest(&["npx"], None);
    let root = g.root_str();
    common::write(
        &g.root,
        ".claude-cfg/.claude.json",
        &pretty(json!({ "numStartups": 41, "mcpServers": { "mine": { "command": "/usr/local/bin/mine", "args": [] } } })),
    );
    let into: Into = Arc::default();
    let first = round(&g, &plan_on(&root), true, Some(into.clone())).await;
    let second = round(&g, &plan_on(&root), true, None).await;
    let want = fixture("theirsTwoRounds", &root);
    assert_eq!(first, want["first"]);
    assert_eq!(into_json(&into), want["firstInto"]);
    assert_eq!(second, want["second"]);
    assert_eq!(common::read(&g.root, ".claude-cfg/.claude.json"), want["claude"].as_str().unwrap());
    assert_eq!(common::read(&g.root, ".codex/config.toml"), want["codex"].as_str().unwrap());
}

/// S3: a folder its agent's file cannot be made to trust sets every kept name of that folder's scopes aside with the
/// untrusted line, the second pass fails the same way, nothing is planned or appended for the scope, the file stands
/// byte for byte, and `into` still hears the file, with no names.
#[tokio::test]
async fn a_folder_that_cannot_be_trusted_sets_every_kept_name_aside_and_appends_nothing() {
    let g = box_guest(&["npx"], None);
    let root = g.root_str();
    let on_box = "model = \"o3\"\n\nprojects = { \"/root/repo\" = { trust_level = \"untrusted\" } }\n";
    common::write(&g.root, ".codex/config.toml", on_box);
    let mut codex = scope(format!("{root}/.codex/config.toml"), CODEX_TOML, &["context7"], vec![dropped("grafana", "unticked")]);
    codex.travelled = Some(codex_travelled());
    codex.folder = Some(s("/root/repo"));
    codex.trust = Some(s("/root/repo"));
    let plan =
        McpPlan { agents: vec![McpAgentPlan { id: s("codex"), label: s("Codex"), scopes: vec![codex], aside: vec![] }], ..plan_on(&root) };
    let into: Into = Arc::default();
    let got = round(&g, &plan, false, Some(into.clone())).await;
    let want = fixture("untrusted", &root);
    assert_eq!(got["rows"], want["rows"]);
    assert_eq!(got["appended"], json!([]));
    assert_eq!(got["listed"], json!([]));
    assert_eq!(into_json(&into), want["into"]);
    assert_eq!(common::read(&g.root, ".codex/config.toml"), on_box);
}

/// S4: with no copy of this computer's beside the job, a kept name the file defines reads present, one it does not is
/// set aside with the no-copy line, the file is not written, nothing is appended (so the list's old lines for those
/// names stand), and `into` hears the file with no names.
#[tokio::test]
async fn a_round_with_no_copy_reads_the_file_writes_nothing_and_appends_nothing() {
    let g = box_guest(&["npx"], None);
    let root = g.root_str();
    let on_box = pretty(json!({ "mcpServers": { "gsc": { "command": "npx", "args": ["-y", "gsc-mcp", "--stdio"] } } }));
    common::write(&g.root, ".claude-cfg/.claude.json", &on_box);
    let mut plan = plan_on(&root);
    plan.agents.truncate(1);
    let into: Into = Arc::default();
    let got = round(&g, &plan, false, Some(into.clone())).await;
    let want = fixture("noCopy", &root);
    assert_eq!(got, json!({ "rows": want["rows"], "appended": want["appended"], "listed": want["listed"] }));
    assert_eq!(into_json(&into), want["into"]);
    assert_eq!(common::read(&g.root, ".claude-cfg/.claude.json"), on_box);
}

/// S5: a read that exits non-zero merges nothing: every kept server is skipped with the read's failure line, the
/// drops keep their own reasons, `into` is never called and nothing is appended.
#[tokio::test]
async fn a_read_that_exits_non_zero_skips_every_kept_server_and_never_calls_into() {
    let mut g = box_guest(&["npx"], None);
    let root = g.root_str();
    let inner = g.machine.run.clone().unwrap();
    let run: RunFn = Arc::new(move |script: String, opts| {
        if script.starts_with("wsp_mcp_read() {") {
            return Box::pin(async { Ok(common::res(3, "", "read: permission denied")) });
        }
        inner(script, opts)
    });
    g.machine.run = Some(run);
    let into: Into = Arc::default();
    let got = round(&g, &plan_on(&root), true, Some(into.clone())).await;
    let want = fixture("readFailed", &root);
    assert_eq!(got["rows"], want["rows"]);
    assert_eq!(got["appended"], json!([]));
    assert_eq!(into_json(&into), json!([]));
}
