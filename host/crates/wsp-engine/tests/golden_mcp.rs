// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/golden-mcp.test.ts at a4033dc34. The MCP stage against a guest that is a temp
//! directory: the configs are read off it and the edited bytes land back on it; the command checks, the uv install and
//! the df reading are canned, so the files it ends with are the proof.
mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};

use common::{box_guest, other, read, res, write, BoxGuest, ExecFn, FakeMachine, RunFn};
use wsp_catalog::jsonc::parse_jsonc;
use wsp_catalog::mcp::{McpFormat, CODEX_TOML, MCP_SERVERS_JSON, OPENCODE_JSON};
use wsp_catalog::roads::{RoadName, UV_INSTALL};
use wsp_engine::errors::Result;
use wsp_engine::golden_seam::GoldenStage;
use wsp_engine::golden_seam::{
    apply_mcp, command_row, free_kb_cmd, guarded_road, mcp_plan_for, mcp_tally, server_needs, AsideRow, DropName, McpAgentPlan,
    McpAgentSource, McpPlan, McpPlanOptions, McpResult, McpScope, McpToolRow, ProjectMove, RecipeEntry, StageListener, ToolInstall,
    ToolResult,
};
use wsp_engine::machine::{ExecOptions, ExecResult, RunOptions};
use wsp_wire::types::init_job::LoginChoice;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::MCP_ID_PREFIX;
use wsp_words::agents_report::config_hard_link_refusal;

const HOME: &str = "/Users/dev";

fn s(text: &str) -> String {
    text.to_string()
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|i| s(i)).collect()
}

fn id(tail: &str) -> String {
    format!("{MCP_ID_PREFIX}{tail}")
}

/// golden-mcp.test.ts:18 `row`: a ticked agents row whose label is the last id segment.
fn row(row_id: &str) -> RecipeEntry {
    RecipeEntry {
        rung: s("agents"),
        id: s(row_id),
        label: s(&row_id[row_id.rfind('/').map(|i| i + 1).unwrap_or(0)..]),
        paths: vec![],
        bytes: 0,
        default: s("bring"),
        bring: Some(true),
        ..RecipeEntry::default()
    }
}

fn row_with(row_id: &str, over: impl FnOnce(&mut RecipeEntry)) -> RecipeEntry {
    let mut r = row(row_id);
    over(&mut r);
    r
}

fn agents() -> IndexMap<String, McpAgentSource> {
    IndexMap::from([
        (
            s("claude"),
            McpAgentSource { label: s("Claude Code"), format: MCP_SERVERS_JSON, files: strings(&["/root/.claude-cfg/.claude.json"]) },
        ),
        (s("codex"), McpAgentSource { label: s("Codex"), format: CODEX_TOML, files: strings(&["/root/.codex/config.toml"]) }),
        (
            s("gemini"),
            McpAgentSource { label: s("Gemini CLI"), format: MCP_SERVERS_JSON, files: strings(&["/root/.gemini/settings.json"]) },
        ),
        (
            s("opencode"),
            McpAgentSource {
                label: s("OpenCode"),
                format: OPENCODE_JSON,
                files: strings(&["/root/.config/opencode/opencode.json", "/root/.config/opencode/opencode.jsonc"]),
            },
        ),
    ])
}

fn scope_in(files: Vec<String>, format: McpFormat, project: Option<ProjectMove>, keep: &[&str], drop: Vec<DropName>) -> McpScope {
    McpScope { files, format, project, keep: strings(keep), drop, travelled: None, own: None, folder: None, trust: None, edit: None }
}

fn scope(files: Vec<String>, format: McpFormat, keep: &[&str], drop: Vec<DropName>) -> McpScope {
    scope_in(files, format, None, keep, drop)
}

fn dn(name: &str, reason: &str) -> DropName {
    DropName { name: s(name), reason: s(reason), keys: None }
}

fn aside(row_id: &str, name: &str, reason: &str) -> AsideRow {
    AsideRow { id: s(row_id), name: s(name), reason: s(reason) }
}

fn agent_plan(agent_id: &str, label: &str, scopes: Vec<McpScope>, aside: Vec<AsideRow>) -> McpAgentPlan {
    McpAgentPlan { id: s(agent_id), label: s(label), scopes, aside }
}

fn scope_view(sc: &McpScope) -> Value {
    json!({
        "files": sc.files,
        "format": format!("{:?}", sc.format),
        "project": sc.project.as_ref().map(|p| json!({ "from": p.from, "to": p.to })),
        "keep": sc.keep,
        "drop": sc.drop.iter().map(|d| json!({ "name": d.name, "reason": d.reason, "keys": d.keys })).collect::<Vec<_>>(),
        "travelled": sc.travelled,
        "own": sc.own.as_ref().map(|o| json!({ "files": o.files, "base": o.base })),
        "folder": sc.folder,
        "trust": sc.trust,
        "edit": sc.edit.is_some(),
    })
}

/// The whole plan as a JSON value, every field, so two plans compare as `toEqual` compares them.
fn plan_view(plan: &McpPlan) -> Value {
    json!({
        "agents": plan.agents.iter().map(|a| json!({
            "id": a.id,
            "label": a.label,
            "scopes": a.scopes.iter().map(scope_view).collect::<Vec<_>>(),
            "aside": a.aside.iter().map(|x| json!({ "id": x.id, "name": x.name, "reason": x.reason })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "guestHome": plan.guest_home,
        "rewrites": plan.rewrites,
        "binDirs": plan.bin_dirs,
        "tools": plan.tools.iter().map(|t| json!({ "id": t.id, "ticked": t.ticked, "reason": t.reason })).collect::<Vec<_>>(),
    })
}

fn mr(row_id: &str, agent: &str, name: &str, outcome: &str, note: Option<&str>) -> McpResult {
    McpResult { id: s(row_id), agent: s(agent), name: s(name), outcome: s(outcome), note: note.map(s), keys: None }
}

// TS: packages/engine/test/golden-mcp.test.ts:28
// "keeps the ticked servers, drops the unticked with their reason and a ticked server whose secret was not answered copy, sets aside an agent that is not ticked, and moves the home scope"
#[test]
fn keeps_ticked_drops_unticked_sets_aside_unticked_agent_and_moves_the_home_scope() {
    let rows = vec![
        row("agents/claude"),
        row(&id("claude/github")),
        row_with(&id("claude/notes"), |r| {
            r.bring = Some(false);
            r.default = s("skip");
            r.reason = Some(s("command ~/Library/x is macOS-only, will not run"));
        }),
        row_with(&id("claude/old"), |r| r.bring = Some(false)),
        row(&id("claude/home/zomato")),
        row_with(&id("claude/gsc"), |r| {
            r.consent = Some(true);
            r.choice = Some(LoginChoice::Skip);
        }),
        row_with(&id("claude/home/drive"), |r| {
            r.consent = Some(true);
            r.choice = Some(LoginChoice::Copy);
        }),
        row_with("agents/codex", |r| r.bring = Some(false)),
        row(&id("codex/grafana")),
        row_with(&id("mcp-remote"), |r| r.paths = strings(&["~/.mcp-auth"])),
        row("agents/gemini"),
        row_with("tools/go/codebase-memory-mcp", |r| r.rung = s("tools")),
        row_with("tools/brew/jq", |r| {
            r.rung = s("tools");
            r.bring = Some(false);
            r.default = s("skip");
            r.reason = Some(s("no Linux bottle"));
        }),
        row_with("tools/brew-cask/iterm2", |r| {
            r.rung = s("tools");
            r.bring = Some(false);
        }),
        row_with("tools/hand/omp", |r| {
            r.rung = s("tools");
            r.bring = Some(false);
            r.default = s("skip");
            r.reason = Some(s("installed by hand; no Linux build known"));
        }),
    ];
    let actual =
        mcp_plan_for(&rows, &McpPlanOptions { home: s(HOME), guest_home: None, agents: agents(), bin_dirs: None }).expect("a plan");
    let claude_files = strings(&["/root/.claude-cfg/.claude.json"]);
    let expected = McpPlan {
        agents: vec![
            agent_plan(
                "claude",
                "Claude Code",
                vec![
                    scope(
                        claude_files.clone(),
                        MCP_SERVERS_JSON,
                        &["github"],
                        vec![
                            dn("notes", "command ~/Library/x is macOS-only, will not run"),
                            dn("old", "unticked"),
                            dn("gsc", "credential-shaped; not copied without your answer on its row"),
                        ],
                    ),
                    scope_in(
                        claude_files,
                        MCP_SERVERS_JSON,
                        Some(ProjectMove { from: s(HOME), to: s("/root") }),
                        &["zomato", "drive"],
                        vec![],
                    ),
                ],
                vec![],
            ),
            agent_plan(
                "codex",
                "Codex",
                vec![],
                vec![aside(&id("codex/grafana"), "grafana", "Codex is not ticked, so its config did not travel")],
            ),
        ],
        guest_home: s("/root"),
        rewrites: vec![(format!("{HOME}/"), s("/root/")), (s("/opt/homebrew/"), s("/home/linuxbrew/.linuxbrew/"))],
        bin_dirs: vec![format!("{HOME}/.local/bin/"), s("~/.local/bin/"), s("/opt/homebrew/bin/"), s("/opt/homebrew/sbin/")],
        tools: vec![
            McpToolRow { id: s("tools/go/codebase-memory-mcp"), ticked: true, reason: None },
            McpToolRow { id: s("tools/brew/jq"), ticked: false, reason: Some(s("no Linux bottle")) },
            McpToolRow { id: s("tools/hand/omp"), ticked: false, reason: Some(s("installed by hand; no Linux build known")) },
        ],
    };
    assert_eq!(plan_view(&actual), plan_view(&expected));
}

// TS: packages/engine/test/golden-mcp.test.ts:65
// "is nothing when the recipe has no MCP rows"
#[test]
fn is_nothing_when_the_recipe_has_no_mcp_rows() {
    let plan = mcp_plan_for(&[row("agents/claude")], &McpPlanOptions { home: s(HOME), guest_home: None, agents: agents(), bin_dirs: None });
    assert!(plan.is_none());
}

// ---- the guest and its fixtures -------------------------------------------------------------------------------

fn guest(present: &[&str]) -> BoxGuest {
    box_guest(present, Some(free_kb_cmd()))
}

/// The guest with its uv install answering `uv` instead of exit 0 (box-guest.ts `canned.uv`); the command is still
/// logged by the guest.
fn guest_with_uv(present: &[&str], uv: ExecResult) -> (BoxGuest, FakeMachine) {
    let g = guest(present);
    let inner_exec = g.machine.exec.clone();
    let inner_run = g.machine.run.clone().unwrap();
    let uv_exec = uv.clone();
    let exec: ExecFn = Arc::new(move |cmd: String, o: ExecOptions| -> BoxFuture<'static, Result<ExecResult>> {
        let uv = uv_exec.clone();
        let inner = inner_exec.clone();
        Box::pin(async move {
            let ran = inner(cmd.clone(), o).await;
            if cmd.contains("astral-sh/uv/releases") {
                Ok(uv)
            } else {
                ran
            }
        })
    });
    let uv_run = uv;
    let run: RunFn = Arc::new(move |script: String, o: RunOptions| -> BoxFuture<'static, Result<ExecResult>> {
        let uv = uv_run.clone();
        let inner = inner_run.clone();
        Box::pin(async move {
            let ran = inner(script.clone(), o).await;
            if script.contains("astral-sh/uv/releases") {
                Ok(uv)
            } else {
                ran
            }
        })
    });
    let machine = FakeMachine { exec, run: Some(run), ..g.machine.clone() };
    (g, machine)
}

fn claude() -> Value {
    json!({
        "numStartups": 3,
        "mcpServers": {
            "github": { "type": "stdio", "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"], "env": { "GITHUB_TOKEN": "ghp_secret" } },
            "gsc": { "command": "uvx", "args": ["mcp-search-console"], "env": { "GSC_CREDENTIALS_PATH": format!("{HOME}/.config/gsc/creds.json") } },
            "notes": { "command": format!("{HOME}/Library/Application Support/Notes/mcp"), "args": [] },
            "survivor": { "command": "survivor-bin", "args": [] },
        },
        "projects": {
            HOME: {
                "allowedTools": ["Bash"],
                "mcpServers": {
                    "zomato": { "type": "stdio", "command": "npx", "args": ["mcp-remote", "https://mcp.zomato.com/mcp"] },
                    "whatsapp": { "command": format!("{HOME}/.local/bin/uv"), "args": ["--directory", format!("{HOME}/code/wa"), "run", "main.py"] },
                },
            },
        },
    })
}

fn pretty(v: &Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(v).unwrap())
}

fn codex_text() -> String {
    [
        r#"model = "gpt-5""#,
        "",
        "[mcp_servers.grafana]",
        r#"command = "/opt/homebrew/bin/uvx""#,
        r#"args = ["mcp-grafana", "--config=/Users/dev/.config/grafana.toml", "/Users/dev/tab\there"]"#,
        r#"note = "keep é \"quoted\" and\ttab""#,
        "",
        "[mcp_servers.grafana.env]",
        r#"GRAFANA_SERVICE_ACCOUNT_TOKEN = "glsa_x" # keep the comment"#,
        "",
        "[mcp_servers.sentry]",
        r#"url = "https://mcp.sentry.dev/mcp""#,
        r#"bearer_token_env_var = "SENTRY_TOKEN""#,
        "",
        "[mcp_servers.keeper]",
        r#"command = "keeper""#,
        "",
        "[[hooks.SessionStart]]",
        r#"matcher = "x""#,
        "",
    ]
    .join("\n")
}

const GEMINI: &str =
    "{\n  // servers\n  \"mcpServers\": { \"memory\": { \"command\": \"~/.local/bin/codebase-memory-mcp\" }, \"gone\": { \"command\": \"/Applications/X.app/x\" } },\n  \"theme\": \"dark\"\n}\n";

fn at(root: &str, rel: &str) -> String {
    format!("{root}/{rel}")
}

fn plan_on(root: &str) -> McpPlan {
    McpPlan {
        agents: vec![
            agent_plan(
                "claude",
                "Claude Code",
                vec![
                    scope(
                        vec![at(root, ".claude-cfg/.claude.json")],
                        MCP_SERVERS_JSON,
                        &["github", "gsc"],
                        vec![dn("notes", "command ~/Library/Application Support/Notes/mcp is macOS-only, will not run")],
                    ),
                    scope_in(
                        vec![at(root, ".claude-cfg/.claude.json")],
                        MCP_SERVERS_JSON,
                        Some(ProjectMove { from: s(HOME), to: s(root) }),
                        &["zomato", "whatsapp"],
                        vec![],
                    ),
                ],
                vec![],
            ),
            agent_plan(
                "codex",
                "Codex",
                vec![scope(vec![at(root, ".codex/config.toml")], CODEX_TOML, &["grafana"], vec![dn("sentry", "unticked")])],
                vec![],
            ),
            agent_plan(
                "gemini",
                "Gemini CLI",
                vec![scope(
                    vec![at(root, ".gemini/settings.json")],
                    MCP_SERVERS_JSON,
                    &["memory"],
                    vec![dn("gone", "path /Applications/X.app/x is macOS-only, will not run")],
                )],
                vec![],
            ),
            agent_plan(
                "opencode",
                "OpenCode",
                vec![scope(vec![at(root, ".config/opencode/opencode.json")], OPENCODE_JSON, &["ctx"], vec![])],
                vec![aside(&id("opencode/late"), "late", "OpenCode is not ticked, so its config did not travel")],
            ),
        ],
        guest_home: s(root),
        rewrites: vec![(format!("{HOME}/"), format!("{root}/")), (s("/opt/homebrew/"), s("/home/linuxbrew/.linuxbrew/"))],
        bin_dirs: vec![format!("{HOME}/.local/bin/"), s("~/.local/bin/"), s("/opt/homebrew/bin/"), s("/opt/homebrew/sbin/")],
        tools: vec![],
    }
}

fn seed(root: &str) {
    let r = Path::new(root);
    write(r, ".claude-cfg/.claude.json", &pretty(&claude()));
    write(r, ".codex/config.toml", &codex_text());
    write(r, ".gemini/settings.json", GEMINI);
}

fn word_of(stage: GoldenStage) -> String {
    serde_json::to_value(stage).unwrap().as_str().unwrap().to_string()
}

/// A stage listener and what it heard: `<stage>:<detail>` when `with_stage`, else the detail, else the stage word.
fn recorder(with_stage: bool) -> (StageListener, Arc<Mutex<Vec<String>>>) {
    let heard: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = heard.clone();
    let listener: StageListener = Arc::new(move |stage: GoldenStage, detail: Option<&str>| {
        let line = if with_stage {
            format!("{}:{}", word_of(stage), detail.unwrap_or(""))
        } else {
            detail.map(str::to_string).unwrap_or_else(|| word_of(stage))
        };
        sink.lock().unwrap().push(line);
    });
    (listener, heard)
}

fn quiet() -> StageListener {
    Arc::new(|_: GoldenStage, _: Option<&str>| {})
}

fn gemini_only(root: &str, tools: Vec<McpToolRow>) -> McpPlan {
    let mut plan = plan_on(root);
    plan.agents = vec![agent_plan(
        "gemini",
        "Gemini CLI",
        vec![scope(vec![at(root, ".gemini/settings.json")], MCP_SERVERS_JSON, &["memory", "vanished"], vec![])],
        vec![],
    )];
    plan.tools = tools;
    plan
}

fn gemini_servers(root: &str) -> Vec<String> {
    let text = read(Path::new(root), ".gemini/settings.json");
    let parsed = parse_jsonc(&text).unwrap();
    parsed["mcpServers"].as_object().unwrap().keys().cloned().collect()
}

fn keys_of(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

fn names_in_dir_starting_wsp(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".wsp-"))
        .collect()
}

fn note_of<'a>(results: &'a [McpResult], name: &str) -> Option<&'a str> {
    results.iter().find(|r| r.name == name).unwrap_or_else(|| panic!("no row {name}")).note.as_deref()
}

/// `/a.*b.*c/s`: each needle found after the one before it.
fn in_order(text: &str, needles: &[&str]) -> bool {
    let mut at = 0;
    for n in needles {
        match text[at..].find(n) {
            Some(i) => at += i + n.len(),
            None => return false,
        }
    }
    true
}

fn is_config_landing(name: &str) -> bool {
    name.strip_prefix(".wsp-config-tmp.").is_some_and(|h| h.len() == 12 && h.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')))
}

fn config_files(root: &str) -> Vec<String> {
    [".claude-cfg/.claude.json", ".codex/config.toml", ".gemini/settings.json"].iter().map(|f| at(root, f)).collect()
}

// TS: packages/engine/test/golden-mcp.test.ts:154
// "writes the kept definitions with their paths rewritten, drops the unticked, leaves a server it never heard of alone, and names each on the stage"
#[tokio::test]
async fn writes_kept_definitions_rewritten_drops_unticked_leaves_unknown_and_names_each_on_stage() {
    let g = guest(&["npx", "codebase-memory-mcp"]);
    let root = g.root_str();
    seed(&root);
    let (stage, stages) = recorder(true);
    let results = apply_mcp(&g.machine, &plan_on(&root), stage, &[]).await;
    let cmds = g.cmds();
    let runs = g.runs();
    let landed = g.landed();
    // One detached run reads every config; the edit itself is this computer's, so the machine runs no node of its own.
    assert_eq!(runs.len(), 2);
    assert!(runs[0].contains("wsp_mcp_read 0 "));
    assert!(runs[1].contains("astral-sh/uv/releases"));
    assert!(runs[1].contains("\nsetsid bash -c '"));
    assert!(!cmds.iter().any(|c| c.contains("node -e")));
    // The read's answer is every config whole, the servers' secrets with it, so it says its output is not a log's.
    let unlogged = g.log.lock().unwrap().run_unlogged.clone();
    assert!(unlogged[0]);
    assert!(!unlogged[1]);
    // Each edited config lands beside itself under the writer's own prefix and goes over the file by the one config
    // write, so its mode and owner stand and an agent launching in that moment reads one whole copy or the other.
    let configs = config_files(&root);
    let landed_dirs: Vec<String> = landed.iter().map(|l| Path::new(l).parent().unwrap().to_str().unwrap().to_string()).collect();
    let config_dirs: Vec<String> = configs.iter().map(|c| Path::new(c).parent().unwrap().to_str().unwrap().to_string()).collect();
    assert_eq!(landed_dirs, config_dirs);
    for l in &landed {
        let base = Path::new(l).file_name().unwrap().to_str().unwrap();
        assert!(is_config_landing(base), "{base}");
    }
    let write_cmd = cmds.iter().find(|c| c.contains(r#"mv -f "$n" "$r""#)).expect("the config write");
    for (i, c) in configs.iter().enumerate() {
        assert!(write_cmd.contains(&format!("f={}\n", shell_quote(c))));
        assert!(write_cmd.contains(&shell_quote(&landed[i])));
    }
    for c in &configs {
        assert_eq!(names_in_dir_starting_wsp(Path::new(c).parent().unwrap()), Vec::<String>::new());
    }
    let check = cmds.iter().find(|c| c.split('\n').nth(1).is_some_and(|l| l.starts_with("if command -v"))).expect("the PATH check");
    assert!(!runs.contains(check));

    let claude_after: Value = serde_json::from_str(&read(Path::new(&root), ".claude-cfg/.claude.json")).unwrap();
    assert_eq!(claude_after["numStartups"], json!(3));
    assert_eq!(keys_of(&claude_after["mcpServers"]), strings(&["github", "gsc", "survivor"]));
    assert_eq!(claude_after["mcpServers"]["github"], claude()["mcpServers"]["github"]);
    assert_eq!(claude_after["mcpServers"]["gsc"]["env"]["GSC_CREDENTIALS_PATH"], json!(format!("{root}/.config/gsc/creds.json")));
    // Servers local to the laptop's home folder now belong to the machine's; the rest of the laptop project entry stays.
    assert_eq!(claude_after["projects"][HOME], json!({ "allowedTools": ["Bash"], "mcpServers": {} }));
    assert_eq!(
        claude_after["projects"][&root]["mcpServers"],
        json!({
            "zomato": claude()["projects"][HOME]["mcpServers"]["zomato"],
            "whatsapp": { "command": "uv", "args": ["--directory", format!("{root}/code/wa"), "run", "main.py"] },
        })
    );

    let codex_after = read(Path::new(&root), ".codex/config.toml");
    let expected_codex = [
        r#"model = "gpt-5""#.to_string(),
        String::new(),
        "[mcp_servers.grafana]".to_string(),
        r#"command = "uvx""#.to_string(),
        // A rewritten string keeps its escapes; one the rewrite never touched is left byte for byte.
        format!(r#"args = ["mcp-grafana", "--config={root}/.config/grafana.toml", "{root}/tab\there"]"#),
        r#"note = "keep é \"quoted\" and\ttab""#.to_string(),
        String::new(),
        "[mcp_servers.grafana.env]".to_string(),
        r#"GRAFANA_SERVICE_ACCOUNT_TOKEN = "glsa_x" # keep the comment"#.to_string(),
        String::new(),
        "[mcp_servers.keeper]".to_string(),
        r#"command = "keeper""#.to_string(),
        String::new(),
        "[[hooks.SessionStart]]".to_string(),
        r#"matcher = "x""#.to_string(),
        String::new(),
    ]
    .join("\n");
    assert_eq!(codex_after, expected_codex);

    let gemini_after = read(Path::new(&root), ".gemini/settings.json");
    assert_eq!(
        parse_jsonc(&gemini_after).unwrap(),
        json!({ "mcpServers": { "memory": { "command": "codebase-memory-mcp" } }, "theme": "dark" })
    );
    assert!(gemini_after.contains("  // servers\n"));

    // A definition whose package npx or uv pulls down when the agent first starts it is in place, not installed.
    let npx = Some("npx fetches the package on first use");
    let uv = Some("uv installed for it; uv fetches the package on first use");
    let gone = "path /Applications/X.app/x is macOS-only, will not run";
    let notes = "command ~/Library/Application Support/Notes/mcp is macOS-only, will not run";
    assert_eq!(
        results,
        vec![
            mr(&id("claude/github"), "Claude Code", "github", "fetched-on-first-use", npx),
            mr(&id("claude/gsc"), "Claude Code", "gsc", "fetched-on-first-use", uv),
            mr(&id("claude/notes"), "Claude Code", "notes", "skipped", Some(notes)),
            mr(&id("claude/home/zomato"), "Claude Code", "zomato", "fetched-on-first-use", npx),
            mr(&id("claude/home/whatsapp"), "Claude Code", "whatsapp", "fetched-on-first-use", uv),
            mr(&id("codex/grafana"), "Codex", "grafana", "fetched-on-first-use", uv),
            mr(&id("codex/sentry"), "Codex", "sentry", "skipped", Some("unticked")),
            mr(&id("gemini/memory"), "Gemini CLI", "memory", "installed", None),
            mr(&id("gemini/gone"), "Gemini CLI", "gone", "skipped", Some(gone)),
            mr(&id("opencode/ctx"), "OpenCode", "ctx", "skipped", Some("OpenCode's config is not on the machine")),
            mr(&id("opencode/late"), "OpenCode", "late", "skipped", Some("OpenCode is not ticked, so its config did not travel")),
        ]
    );
    assert_eq!(
        *stages.lock().unwrap(),
        vec![
            s("installing-mcp:Claude Code 5, Codex 2, Gemini CLI 2, OpenCode 2"),
            s("installing-mcp:uv for gsc, whatsapp, grafana"),
            format!(
                "installing-mcp:memory installed; github, zomato: package fetched on first use by npx; gsc, whatsapp, grafana: uv installed, package fetched on first use by uv; notes skipped ({notes}); sentry skipped (unticked); gone skipped ({gone}); ctx skipped (OpenCode's config is not on the machine); late skipped (OpenCode is not ticked, so its config did not travel); 2.9 GB free"
            ),
        ]
    );
    // Every command is looked for once on the machine's PATH; uv, found missing, is installed by its checksummed release.
    let checks: Vec<&String> = cmds.iter().filter(|c| c.contains("command -v '")).collect();
    assert_eq!(checks.len(), 1);
    assert!(in_order(checks[0], &["command -v 'npx'", "command -v 'uvx'", "command -v 'uv'", "command -v 'codebase-memory-mcp'"]));
    assert_eq!(cmds.iter().filter(|c| c.contains("astral-sh/uv/releases")).count(), 1);
    let check_at = cmds.iter().position(|c| c == checks[0]).unwrap();
    let uv_at = cmds.iter().position(|c| c.contains("astral-sh/uv/releases")).unwrap();
    assert!(check_at < uv_at);
    assert!(!cmds.iter().any(|c| c.contains("ghp_secret")));
    // uv is a script road row in the catalog, so the stage walks that road and takes set -euo pipefail from it rather than writing its own.
    assert_eq!(cmds.iter().find(|c| c.contains("astral-sh/uv/releases")).unwrap(), &guarded_road(RoadName::Script, &UV_INSTALL));
}

// TS: packages/engine/test/golden-mcp.test.ts:250
// "the tally for the stage's end line counts the servers by outcome, zero counts left out, and says nothing for an empty plan"
#[test]
fn the_tally_counts_servers_by_outcome_zero_counts_left_out_and_nothing_for_an_empty_plan() {
    let r = |name: &str, outcome: &str| mr(&id(&format!("claude/{name}")), "Claude Code", name, outcome, None);
    assert_eq!(
        mcp_tally(&[r("a", "installed"), r("b", "installed"), r("c", "skipped"), r("d", "fetched-on-first-use")]),
        Some(s("2 installed, 1 skipped, 1 on first use"))
    );
    assert_eq!(mcp_tally(&[r("a", "installed")]), Some(s("1 installed")));
    assert_eq!(mcp_tally(&[]), None);
}

// TS: packages/engine/test/golden-mcp.test.ts:261
// "a server whose command is not on the machine is taken out of the config and skipped with the reason; a kept server the config no longer holds is named"
#[tokio::test]
async fn a_server_whose_command_is_not_on_the_machine_is_taken_out_and_skipped_with_the_reason() {
    let g = guest(&["npx"]);
    let root = g.root_str();
    seed(&root);
    let (stage, stages) = recorder(false);
    let results = apply_mcp(&g.machine, &gemini_only(&root, vec![]), stage, &[]).await;
    assert_eq!(
        results,
        vec![
            mr(&id("gemini/vanished"), "Gemini CLI", "vanished", "skipped", Some("not in the config that travelled")),
            mr(&id("gemini/memory"), "Gemini CLI", "memory", "skipped", Some("command not on the machine")),
        ]
    );
    assert_eq!(gemini_servers(&root), strings(&["gone"]));
    assert_eq!(
        stages.lock().unwrap().last().unwrap(),
        "vanished skipped (not in the config that travelled); memory skipped (command not on the machine); 2.9 GB free"
    );
}

// TS: packages/engine/test/golden-mcp.test.ts:274
// "a server whose command is on the machine is written and installed"
#[tokio::test]
async fn a_server_whose_command_is_on_the_machine_is_written_and_installed() {
    let g = guest(&["codebase-memory-mcp"]);
    let root = g.root_str();
    seed(&root);
    let results = apply_mcp(&g.machine, &gemini_only(&root, vec![]), quiet(), &[]).await;
    assert_eq!(results[0], mr(&id("gemini/memory"), "Gemini CLI", "memory", "installed", None));
    assert_eq!(gemini_servers(&root), strings(&["memory", "gone"]));
}

// TS: packages/engine/test/golden-mcp.test.ts:282
// "a server that runs through npx is written even when npx is not on the machine yet"
#[tokio::test]
async fn a_server_that_runs_through_npx_is_written_even_when_npx_is_not_on_the_machine_yet() {
    let g = guest(&[]);
    let root = g.root_str();
    seed(&root);
    let mut plan = plan_on(&root);
    plan.agents = vec![agent_plan(
        "claude",
        "Claude Code",
        vec![scope(vec![at(&root, ".claude-cfg/.claude.json")], MCP_SERVERS_JSON, &["github"], vec![])],
        vec![],
    )];
    let results = apply_mcp(&g.machine, &plan, quiet(), &[]).await;
    assert_eq!(
        results,
        vec![mr(
            &id("claude/github"),
            "Claude Code",
            "github",
            "fetched-on-first-use",
            Some("npx fetches the package on first use; npx is not on the machine; the server starts once it is installed there")
        )]
    );
    let after: Value = serde_json::from_str(&read(Path::new(&root), ".claude-cfg/.claude.json")).unwrap();
    assert!(keys_of(&after["mcpServers"]).contains(&s("github")));
}

// TS: packages/engine/test/golden-mcp.test.ts:293
// "the reason names the recipe's tool row that would have brought the command, and what became of it"
#[tokio::test]
async fn the_reason_names_the_tool_row_that_would_have_brought_the_command_and_what_became_of_it() {
    let tool_row = |row_id: &str, ticked: bool, reason: Option<&str>| McpToolRow { id: s(row_id), ticked, reason: reason.map(s) };
    let result = |row_id: &str, outcome: &str, note: &str| ToolResult {
        id: s(row_id),
        label: s(&row_id[row_id.rfind('/').unwrap() + 1..]),
        outcome: s(outcome),
        note: Some(s(note)),
        ..ToolResult::default()
    };
    let cases: Vec<(McpToolRow, Option<ToolResult>, String)> = vec![
        (
            tool_row("tools/go/codebase-memory-mcp", true, None),
            Some(result("tools/go/codebase-memory-mcp", "skipped", "1.8 GB free, keeping 2 GB free")),
            s("command not on the machine; tools/go/codebase-memory-mcp was skipped (1.8 GB free, keeping 2 GB free)"),
        ),
        (
            tool_row("tools/go/codebase-memory-mcp", true, None),
            Some(result("tools/go/codebase-memory-mcp", "failed", "exit 1: go: module not found")),
            s("command not on the machine; tools/go/codebase-memory-mcp failed (exit 1: go: module not found)"),
        ),
        (
            tool_row("tools/brew/codebase-memory-mcp", false, Some("no Linux bottle")),
            None,
            s("command not on the machine; tools/brew/codebase-memory-mcp was not ticked (no Linux bottle)"),
        ),
        (
            tool_row("tools/cargo/codebase-memory-mcp", false, None),
            None,
            s("command not on the machine; tools/cargo/codebase-memory-mcp was not ticked"),
        ),
        (
            tool_row("tools/npm/codebase-memory-mcp", true, None),
            None,
            s("command not on the machine; tools/npm/codebase-memory-mcp was ticked, but nothing by that name is on PATH"),
        ),
        (
            tool_row("tools/go/other-bin", true, None),
            Some(result("tools/go/other-bin", "skipped", "1.8 GB free, keeping 2 GB free")),
            s("command not on the machine"),
        ),
    ];
    for (tool, outcome, note) in cases {
        let g = guest(&["npx"]);
        let root = g.root_str();
        seed(&root);
        let tools: Vec<ToolResult> = outcome.into_iter().collect();
        let results = apply_mcp(&g.machine, &gemini_only(&root, vec![tool]), quiet(), &tools).await;
        assert_eq!(note_of(&results, "memory"), Some(note.as_str()));
    }
}

// TS: packages/engine/test/golden-mcp.test.ts:310
// "names the row that puts a command on, and when none does, has a server wait on every row putting on a command nothing names"
#[test]
fn names_the_row_that_puts_a_command_on_and_otherwise_a_server_waits_on_every_unnamed_row() {
    let r = |row_id: &str, bin: Option<&str>| ToolInstall { id: s(row_id), bin: bin.map(s), ..ToolInstall::default() };
    let rows = vec![
        r("tools/brew-toolchain/mcp", None),
        r("tools/brew/git-delta", None),
        r("tools/brew/yq", None),
        r("tools/npm/@playwright/mcp", None),
        r("tools/custom/mine", None),
        r("tools/custom/named", Some("named")),
    ];
    let none: IndexSet<String> = IndexSet::new();
    assert_eq!(command_row("delta", &rows).map(|x| x.id.as_str()), Some("tools/brew/git-delta"));
    assert_eq!(command_row("/opt/homebrew/bin/yq", &rows).map(|x| x.id.as_str()), Some("tools/brew/yq"));
    assert_eq!(command_row("named", &rows).map(|x| x.id.as_str()), Some("tools/custom/named"));
    // The toolchain's rows put no command on, whatever their last word.
    assert!(command_row("mcp", &rows[..1]).is_none());
    let ids = |found: Vec<&ToolInstall>| found.iter().map(|x| x.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(server_needs("delta", &rows, &none)), strings(&["tools/brew/git-delta"]));
    assert_eq!(ids(server_needs("playwright-mcp", &rows, &none)), strings(&["tools/npm/@playwright/mcp", "tools/custom/mine"]));
    for command in ["uvx", "npx", "python3", "git"] {
        assert_eq!(ids(server_needs(command, &rows, &none)), Vec::<String>::new());
    }
    assert_eq!(ids(server_needs("node", &rows, &IndexSet::from([s("node")]))), Vec::<String>::new());
}

// TS: packages/engine/test/golden-mcp.test.ts:323
// "running twice leaves the files as they were after the first run and reports the same, the moved home servers included"
#[tokio::test]
async fn running_twice_leaves_the_files_as_after_the_first_run_and_reports_the_same() {
    let g = guest(&["npx", "uv", "codebase-memory-mcp"]);
    let root = g.root_str();
    seed(&root);
    let first = apply_mcp(&g.machine, &plan_on(&root), quiet(), &[]).await;
    let files = |root: &str| -> Vec<String> {
        [".claude-cfg/.claude.json", ".codex/config.toml", ".gemini/settings.json"].iter().map(|f| read(Path::new(root), f)).collect()
    };
    let after = files(&root);
    let second = apply_mcp(&g.machine, &plan_on(&root), quiet(), &[]).await;
    assert_eq!(files(&root), after);
    assert_eq!(second, first);
    let moved: Vec<String> = second.iter().filter(|r| r.name == "zomato" || r.name == "whatsapp").map(|r| r.outcome.clone()).collect();
    assert_eq!(moved, strings(&["fetched-on-first-use", "fetched-on-first-use"]));
}

// TS: packages/engine/test/golden-mcp.test.ts:334
// "OpenCode's config: the kept local server's command and paths are rewritten under the mcp key, the dropped one comes out, the rest of the file stays"
#[tokio::test]
async fn opencode_config_rewrites_the_kept_local_server_drops_the_dropped_one_and_keeps_the_rest() {
    let g = guest(&["ctx-bin"]);
    let root = g.root_str();
    std::fs::create_dir_all(Path::new(&root).join(".config").join("opencode")).unwrap();
    let before = json!({
        "$schema": "https://opencode.ai/config.json",
        "theme": "dark",
        "mcp": {
            "ctx": { "type": "local", "command": ["/opt/homebrew/bin/ctx-bin", format!("{HOME}/notes")], "enabled": true },
            "late": { "type": "remote", "url": "https://late.example/mcp" },
            "other": { "type": "remote", "url": "https://o.example" },
        },
    });
    write(Path::new(&root), ".config/opencode/opencode.json", &pretty(&before));
    let mut plan = plan_on(&root);
    plan.agents = vec![agent_plan(
        "opencode",
        "OpenCode",
        vec![scope(vec![at(&root, ".config/opencode/opencode.json")], OPENCODE_JSON, &["ctx"], vec![dn("late", "unticked")])],
        vec![],
    )];
    let results = apply_mcp(&g.machine, &plan, quiet(), &[]).await;
    assert_eq!(
        results,
        vec![
            mr(&id("opencode/ctx"), "OpenCode", "ctx", "installed", None),
            mr(&id("opencode/late"), "OpenCode", "late", "skipped", Some("unticked")),
        ]
    );
    let after: Value = serde_json::from_str(&read(Path::new(&root), ".config/opencode/opencode.json")).unwrap();
    assert_eq!(
        after,
        json!({
            "$schema": "https://opencode.ai/config.json",
            "theme": "dark",
            "mcp": {
                "ctx": { "type": "local", "command": ["ctx-bin", format!("{root}/notes")], "enabled": true },
                "other": { "type": "remote", "url": "https://o.example" },
            },
        })
    );
}

// TS: packages/engine/test/golden-mcp.test.ts:394
// "a format the catalog gains is one module: the stage runs the module's editor over the text it read and reports through it, with no format of its own"
// Adapted: McpFormat is a closed enum, so the TS's custom one-line-per-server format cannot be built here; the same
// round runs through the Codex format on a small file and asserts the same results, the file bytes, and that nothing
// of the format reaches the machine's commands.
#[tokio::test]
async fn a_format_the_catalog_gains_is_one_module_the_stage_runs_its_editor_with_no_format_of_its_own() {
    let g = guest(&["alpha"]);
    let root = g.root_str();
    write(
        Path::new(&root),
        ".lines/servers.toml",
        &format!(
            "[mcp_servers.alpha]\ncommand = \"/opt/homebrew/bin/alpha\"\nargs = [\"--dir={HOME}/x\"]\n\n[mcp_servers.beta]\ncommand = \"beta-bin\"\n\n[mcp_servers.other]\ncommand = \"keeper\"\n"
        ),
    );
    let mut plan = plan_on(&root);
    plan.agents = vec![agent_plan(
        "lines",
        "Lines",
        vec![scope(vec![at(&root, ".lines/servers.toml")], CODEX_TOML, &["alpha", "gone"], vec![dn("beta", "unticked")])],
        vec![],
    )];
    let results = apply_mcp(&g.machine, &plan, quiet(), &[]).await;
    assert_eq!(
        results,
        vec![
            mr(&id("lines/alpha"), "Lines", "alpha", "installed", None),
            mr(&id("lines/gone"), "Lines", "gone", "skipped", Some("not in the config that travelled")),
            mr(&id("lines/beta"), "Lines", "beta", "skipped", Some("unticked")),
        ]
    );
    assert_eq!(
        read(Path::new(&root), ".lines/servers.toml"),
        format!("[mcp_servers.alpha]\ncommand = \"alpha\"\nargs = [\"--dir={root}/x\"]\n\n[mcp_servers.other]\ncommand = \"keeper\"\n")
    );
    // Nothing of the format reaches the machine: it reads the file out and takes the bytes back.
    let cmds = g.cmds();
    assert!(!cmds.iter().any(|c| c.contains("node -e")));
    assert!(!cmds.iter().any(|c| c.contains("mcp_servers")));
}

// TS: packages/engine/test/golden-mcp.test.ts:411
// "a read whose leader was killed says so by its exit code: its output is every config whole, so no reason is built from it"
#[tokio::test]
async fn a_read_whose_leader_was_killed_says_so_by_its_exit_code_and_no_reason_is_built_from_its_output() {
    use base64::Engine as _;
    let g = guest(&["npx"]);
    let root = g.root_str();
    seed(&root);
    // What the wrapper leaves when the leader dies by a signal: the exit file holds 137, the err file is empty, and
    // the out file holds the lines printed so far, which here is a config of the person's.
    let printed = base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&claude()).unwrap().as_bytes());
    let inner = g.machine.run.clone().unwrap();
    let out = format!("{printed}\n");
    let run: RunFn = Arc::new(move |script: String, o: RunOptions| -> BoxFuture<'static, Result<ExecResult>> {
        if script.contains("wsp_mcp_read ") {
            let out = out.clone();
            Box::pin(async move { Ok(res(137, &out, "")) })
        } else {
            inner(script, o)
        }
    });
    let killed = FakeMachine { run: Some(run), ..g.machine.clone() };
    let (stage, stages) = recorder(false);
    let results = apply_mcp(&killed, &plan_on(&root), stage, &[]).await;
    assert_eq!(note_of(&results, "github"), Some("the config edit did not run (exit 137)"));
    assert!(printed.len() > 160);
    let heard = stages.lock().unwrap().clone();
    for said in results.iter().map(|r| r.note.clone().unwrap_or_default()).chain(heard) {
        assert!(!said.contains(&printed));
        assert!(!said.contains(&printed[..60]));
        assert!(!said.contains("ghp_secret"));
    }
}

// TS: packages/engine/test/golden-mcp.test.ts:429
// "a machine that refuses the script, as the provider does over its body cap, skips every server with the words, touches no config, and does not throw"
#[tokio::test]
async fn a_machine_that_refuses_the_script_skips_every_server_touches_no_config_and_does_not_throw() {
    let g = guest(&["npx"]);
    let root = g.root_str();
    seed(&root);
    let before = read(Path::new(&root), ".claude-cfg/.claude.json");
    let run: RunFn = Arc::new(|_: String, _: RunOptions| -> BoxFuture<'static, Result<ExecResult>> {
        Box::pin(async { Err(other("Payload Too Large")) })
    });
    let refusing = FakeMachine { run: Some(run), ..g.machine.clone() };
    let (stage, stages) = recorder(false);
    let results = apply_mcp(&refusing, &plan_on(&root), stage, &[]).await;
    assert_eq!(results.iter().map(|r| r.outcome.clone()).collect::<Vec<_>>(), vec![s("skipped"); 11]);
    assert_eq!(
        results.iter().find(|r| r.name == "github").unwrap(),
        &mr(&id("claude/github"), "Claude Code", "github", "skipped", Some("the config edit did not run (Payload Too Large)"))
    );
    assert_eq!(note_of(&results, "notes"), Some("command ~/Library/Application Support/Notes/mcp is macOS-only, will not run"));
    assert!(stages.lock().unwrap().last().unwrap().contains("github skipped (the config edit did not run (Payload Too Large))"));
    assert_eq!(read(Path::new(&root), ".claude-cfg/.claude.json"), before);
}

// TS: packages/engine/test/golden-mcp.test.ts:443
// "a write the machine refuses leaves no landing beside the config: those bytes are the whole file at the upload road's own mode"
#[tokio::test]
async fn a_write_the_machine_refuses_leaves_no_landing_beside_the_config() {
    let g = guest(&["codebase-memory-mcp"]);
    let root = g.root_str();
    seed(&root);
    let inner = g.machine.exec.clone();
    let exec: ExecFn = Arc::new(move |cmd: String, o: ExecOptions| -> BoxFuture<'static, Result<ExecResult>> {
        if cmd.contains(r#"mv -f "$n" "$r""#) {
            Box::pin(async { Ok(res(1, "", "cat: write error: No space left on device")) })
        } else {
            inner(cmd, o)
        }
    });
    let refusing = FakeMachine { exec, ..g.machine.clone() };
    let results = apply_mcp(&refusing, &gemini_only(&root, vec![]), quiet(), &[]).await;
    assert_eq!(note_of(&results, "memory"), Some("the edited config did not land (cat: write error: No space left on device)"));
    let landed = g.landed();
    assert_eq!(landed.len(), 1);
    assert!(!Path::new(&landed[0]).exists());
    assert!(g.cmds().contains(&format!("rm -f {}", shell_quote(&landed[0]))));
    // The config itself is as it was: the write never ran.
    assert_eq!(read(Path::new(&root), ".gemini/settings.json"), GEMINI);
}

// TS: packages/engine/test/golden-mcp.test.ts:456
// "a config with a second hard link is left as it is with the words, and one linked inside the home is written through and stays a link"
#[tokio::test]
async fn a_config_with_a_second_hard_link_is_left_as_it_is_and_one_linked_inside_the_home_stays_a_link() {
    let g = guest(&["codebase-memory-mcp"]);
    let root = g.root_str();
    let r = Path::new(&root);
    seed(&root);
    std::fs::hard_link(r.join(".gemini").join("settings.json"), r.join("twin.json")).unwrap();
    let refused = apply_mcp(&g.machine, &gemini_only(&root, vec![]), quiet(), &[]).await;
    assert_eq!(
        note_of(&refused, "memory").map(str::to_string),
        Some(format!(
            "the edited config did not land ({})",
            config_hard_link_refusal(r.join(".gemini").join("settings.json").to_str().unwrap())
        ))
    );
    assert_eq!(read(r, ".gemini/settings.json"), GEMINI);
    assert_eq!(names_in_dir_starting_wsp(&r.join(".gemini")), Vec::<String>::new());

    std::fs::remove_file(r.join("twin.json")).unwrap();
    std::fs::create_dir(r.join("dotfiles")).unwrap();
    std::fs::rename(r.join(".gemini").join("settings.json"), r.join("dotfiles").join("settings.json")).unwrap();
    std::os::unix::fs::symlink(r.join("dotfiles").join("settings.json"), r.join(".gemini").join("settings.json")).unwrap();
    apply_mcp(&g.machine, &gemini_only(&root, vec![]), quiet(), &[]).await;
    assert!(std::fs::symlink_metadata(r.join(".gemini").join("settings.json")).unwrap().file_type().is_symlink());
    assert!(read(r, "dotfiles/settings.json").contains(r#""memory": { "command": "codebase-memory-mcp" }"#));
}

// TS: packages/engine/test/golden-mcp.test.ts:474
// "a config that is on the machine and empty did not parse; it is not a config that is not there"
#[tokio::test]
async fn a_config_that_is_on_the_machine_and_empty_did_not_parse_it_is_not_one_that_is_not_there() {
    let g = guest(&["codebase-memory-mcp"]);
    let root = g.root_str();
    write(Path::new(&root), ".gemini/settings.json", "");
    let results = apply_mcp(&g.machine, &gemini_only(&root, vec![]), quiet(), &[]).await;
    assert_eq!(results.len(), 2);
    for r in &results {
        assert!(r.note.as_deref().is_some_and(|n| n.contains("did not parse")), "{:?}", r.note);
    }
    assert!(!results.iter().any(|r| r.note.as_deref().is_some_and(|n| n.contains("is not on the machine"))));
}

// TS: packages/engine/test/golden-mcp.test.ts:483
// "a PATH check the machine refuses drops no server: the definitions land and the stage still closes"
#[tokio::test]
async fn a_path_check_the_machine_refuses_drops_no_server_the_definitions_land_and_the_stage_closes() {
    let g = guest(&["npx"]);
    let root = g.root_str();
    seed(&root);
    let inner = g.machine.exec.clone();
    let exec: ExecFn = Arc::new(move |cmd: String, o: ExecOptions| -> BoxFuture<'static, Result<ExecResult>> {
        if cmd.contains("command -v") {
            Box::pin(async { Err(other("Bad Gateway")) })
        } else {
            inner(cmd, o)
        }
    });
    let refusing = FakeMachine { exec, ..g.machine.clone() };
    let results = apply_mcp(&refusing, &gemini_only(&root, vec![]), quiet(), &[]).await;
    assert_eq!(gemini_servers(&root), strings(&["memory", "gone"]));
    assert_eq!(results.iter().find(|r| r.name == "memory").unwrap().outcome, "installed");
    assert_eq!(g.cmds().iter().filter(|c| c.contains("command -v")).count(), 0);
}

// TS: packages/engine/test/golden-mcp.test.ts:493
// "a failed uv install is named on every server that needed it, and the definitions still land"
#[tokio::test]
async fn a_failed_uv_install_is_named_on_every_server_that_needed_it_and_the_definitions_still_land() {
    let (g, machine) = guest_with_uv(&["npx"], res(1, "", "curl: (6) Could not resolve host"));
    let root = g.root_str();
    seed(&root);
    let results = apply_mcp(&machine, &plan_on(&root), quiet(), &[]).await;
    assert_eq!(
        results.iter().find(|r| r.name == "gsc").unwrap(),
        &mr(
            &id("claude/gsc"),
            "Claude Code",
            "gsc",
            "fetched-on-first-use",
            Some("uv did not install (curl: (6) Could not resolve host); the server starts once it is installed there; uv fetches the package on first use")
        )
    );
    let after: Value = serde_json::from_str(&read(Path::new(&root), ".claude-cfg/.claude.json")).unwrap();
    assert_eq!(keys_of(&after["mcpServers"]), strings(&["github", "gsc", "survivor"]));
}
