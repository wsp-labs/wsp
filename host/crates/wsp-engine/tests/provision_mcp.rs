// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/provision-mcp.test.ts at a4033dc34. The recipe's MCP servers on a computer somebody
//! owns. The guest is a temp directory with real files in it, so what a run ends with is the file on disk: the file an
//! agent keeps its servers in is merged key by key, every other key of its own stands, and a server the agent or the
//! person has under one of the recipe's names is left with its row saying so.
mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};

use common::{box_guest, BoxGuest, FakeMachine, RunFn};
use wsp_catalog::jsonc::parse_jsonc;
use wsp_catalog::mcp::{CODEX_TOML, MCP_SERVERS_JSON, OPENCODE_JSON};
use wsp_engine::golden_seam::{DropName, McpAgentPlan, McpPlan, McpScope, PackFiles, PackedFiles, ProjectMove};
use wsp_engine::golden_seam::{GoldenStage, TOOLS_PATH};
use wsp_engine::machine::RunOptions;
use wsp_engine::provision::{new_setup_run, provision_step, EngineStep, PlanFiles, ProvisionOn, ProvisionPlan, ProvisionStage, StageAt};
use wsp_engine::provision_files::{
    close_agent_files, machine_server_port, once_paths_of, provision_files, servers_out_lines, unland_files, unmerge_servers, OwnedPaths,
    ProvisionFilesAsk, ProvisionLanding, ServersOut, Unlanded,
};
use wsp_engine::provision_mcp::{headless_keys, key_unreached_line, provision_mcp, their_server_line, ProvisionMcpOptions};
use wsp_engine::vault::{tar_of, TarEntry};
use wsp_wire::types::agents_report::{CONFIG_NOT_PLAIN, CONFIG_PAST_CAP, CONFIG_READ_MAX_BYTES};
use wsp_wire::views::golden_image::MCP_ID_PREFIX;
use wsp_wire::views::place::{PlaceProvisionKind, PlaceProvisionRow, PlaceProvisionRowOutcome};
use wsp_words::agents_report::{config_unread_line, servers_unread_line};
use wsp_words::project_path::place_provision_paths;

const HOME: &str = "/Users/dev";

fn box_(present: &[&str]) -> BoxGuest {
    box_guest(present, None)
}

fn npx() -> BoxGuest {
    box_(&["npx"])
}

fn at(root: &str, rel: &str) -> String {
    format!("{root}/{rel}")
}

fn pretty(v: Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(&v).unwrap())
}

fn s(text: &str) -> String {
    text.to_string()
}

fn id(tail: &str) -> String {
    format!("{MCP_ID_PREFIX}{tail}")
}

/// A temp folder outside the guest's home, canonical so a link into it reads as the folder it is.
fn away_dir(prefix: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    let path = std::fs::canonicalize(dir.path()).unwrap().to_str().unwrap().to_string();
    (dir, path)
}

fn is_link(path: &str) -> bool {
    std::fs::symlink_metadata(path).unwrap().file_type().is_symlink()
}

fn word(outcome: PlaceProvisionRowOutcome) -> &'static str {
    match outcome {
        PlaceProvisionRowOutcome::Installed => "installed",
        PlaceProvisionRowOutcome::Present => "present",
        PlaceProvisionRowOutcome::Failed => "failed",
        PlaceProvisionRowOutcome::Skipped => "skipped",
    }
}

/// What Claude Code writes for itself the first time it runs on that computer, with one server the person added
/// there by hand under a name the recipe also carries.
fn claude_on_box() -> String {
    pretty(json!({
        "numStartups": 41,
        "oauthAccount": { "emailAddress": "he@example.com" },
        "mcpServers": { "mine": { "command": "/usr/local/bin/mine", "args": [] } },
        "projects": { "/root/work": { "history": ["his own turn"] } },
    }))
}

/// This computer's own copy of each agent's file, which is what travels: the recipe's servers are taken from it.
fn claude_travelled(gsc: &[&str], far: bool) -> String {
    let mut args = vec![json!("-y"), json!("gsc-mcp")];
    args.extend(gsc.iter().map(|a| json!(a)));
    let mut servers = json!({
        "gsc": { "command": "npx", "args": args },
        "notes": { "command": format!("{HOME}/Library/Notes/mcp"), "args": [] },
        "mine": { "command": "npx", "args": ["theirs-on-the-mac"] },
    });
    if far {
        servers["far"] = json!({ "command": "/usr/bin/far-mcp", "args": [] });
    }
    pretty(json!({ "numStartups": 7, "mcpServers": servers }))
}

fn claude_default() -> String {
    claude_travelled(&["--stdio"], false)
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
    ]
    .join("\n")
}

/// OpenCode's own config as somebody keeps it on that computer: jsonc, with a comment of the person's.
const OPENCODE_ON_BOX: &str = "{\n  // my own servers\n  \"theme\": \"dark\",\n  \"mcp\": {}\n}\n";

fn opencode_travelled() -> String {
    pretty(json!({ "mcp": { "docs": { "type": "local", "command": ["npx", "docs-mcp"], "enabled": true } } }))
}

fn scope(files: Vec<String>, format: wsp_catalog::mcp::McpFormat, keep: &[&str], drop: Vec<DropName>) -> McpScope {
    McpScope {
        files,
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

fn dropped(name: &str, reason: &str) -> DropName {
    DropName { name: s(name), reason: s(reason), keys: None }
}

fn opencode_plan_on(root: &str) -> McpPlan {
    McpPlan {
        agents: vec![McpAgentPlan {
            id: s("opencode"),
            label: s("OpenCode"),
            scopes: vec![scope(vec![at(root, ".config/opencode/opencode.json")], OPENCODE_JSON, &["docs"], vec![])],
            aside: vec![],
        }],
        guest_home: s(root),
        rewrites: vec![(format!("{HOME}/"), format!("{root}/"))],
        bin_dirs: vec![],
        tools: vec![],
    }
}

fn lands() -> Vec<ProvisionLanding> {
    vec![
        ProvisionLanding { id: s("agents/claude"), label: s("Claude Code"), dest: s(".claude-cfg/.claude.json"), once: true },
        ProvisionLanding { id: s("agents/codex"), label: s("Codex"), dest: s(".codex/config.toml"), once: true },
    ]
}

fn plan_on(root: &str, far: bool, unticked: &[&str]) -> McpPlan {
    let mut asked = vec!["gsc", "mine"];
    if far {
        asked.push("far");
    }
    let keep: Vec<&str> = asked.into_iter().filter(|name| !unticked.contains(name)).collect();
    let mut drop = vec![dropped("notes", "command is macOS-only, will not run")];
    drop.extend(unticked.iter().map(|name| dropped(name, "unticked")));
    McpPlan {
        agents: vec![
            McpAgentPlan {
                id: s("claude"),
                label: s("Claude Code"),
                scopes: vec![scope(vec![at(root, ".claude-cfg/.claude.json")], MCP_SERVERS_JSON, &keep, drop)],
                aside: vec![],
            },
            McpAgentPlan {
                id: s("codex"),
                label: s("Codex"),
                scopes: vec![scope(
                    vec![at(root, ".codex/config.toml")],
                    wsp_catalog::mcp::CODEX_TOML,
                    &["context7"],
                    vec![dropped("grafana", "unticked")],
                )],
                aside: vec![],
            },
        ],
        guest_home: s(root),
        rewrites: vec![(format!("{HOME}/"), format!("{root}/"))],
        bin_dirs: vec![format!("{HOME}/.local/bin/")],
        tools: vec![],
    }
}

fn packed(tar: Vec<u8>) -> PackedFiles {
    let len = tar.len() as u64;
    PackedFiles { tar, bytes: len, unpacked: len, ..PackedFiles::default() }
}

fn file(path: &str, mode: u32, content: &str) -> TarEntry {
    TarEntry::File { path: s(path), mode, content: content.as_bytes().to_vec() }
}

fn pack_of(entries: Vec<TarEntry>) -> PackFiles {
    Arc::new(move |_leave| {
        let entries = entries.clone();
        Box::pin(async move { Ok(packed(tar_of(&entries)?)) })
    })
}

fn pack_configs(claude: &str, codex: &str) -> PackFiles {
    pack_of(vec![file(".claude-cfg/.claude.json", 0o600, claude), file(".codex/config.toml", 0o600, codex)])
}

fn no_stage() -> wsp_engine::golden_seam::StageListener {
    Arc::new(|_: GoldenStage, _: Option<&str>| {})
}

fn no_provision_stage() -> ProvisionStage {
    Arc::new(|_: &str, _: Option<&StageAt>, _: Option<&PlaceProvisionRow>| {})
}

fn files_ask(home: &str, lands: Vec<ProvisionLanding>, pack: PackFiles, stores: IndexMap<String, String>) -> ProvisionFilesAsk {
    ProvisionFilesAsk { home: s(home), lands, pack, say: None, stores }
}

fn mcp_options(home: &str, landed: OwnedPaths) -> ProvisionMcpOptions {
    ProvisionMcpOptions {
        home: s(home),
        landed,
        tools: vec![],
        stage: no_stage(),
        path: s(TOOLS_PATH),
        held: None,
        stores: None,
        into: None,
    }
}

fn stores_of(codex: &str) -> IndexMap<String, String> {
    IndexMap::from([(s("codex"), s(codex))])
}

type ServerRow = (String, String, Option<String>);

fn srv(row_id: &str, outcome: &str, note: Option<&str>) -> ServerRow {
    (s(row_id), s(outcome), note.map(s))
}

fn pairs(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter().map(|(a, b)| (s(a), s(b))).collect()
}

fn ids_and_outcomes(rows: &[ServerRow]) -> Vec<(String, String)> {
    rows.iter().map(|r| (r.0.clone(), r.1.clone())).collect()
}

fn found<'a>(rows: &'a [ServerRow], row_id: &str) -> &'a ServerRow {
    rows.iter().find(|r| r.0 == row_id).unwrap_or_else(|| panic!("no row {row_id}"))
}

#[derive(Default)]
struct Opts {
    claude: Option<String>,
    codex: Option<String>,
    far: bool,
    untick: Vec<&'static str>,
    held: Option<IndexSet<String>>,
}

struct Ran {
    files: Vec<(String, String)>,
    servers: Vec<ServerRow>,
    closing: String,
}

/// One run of the job's files and servers rounds on that computer, in the order the job runs them, with the close
/// that folds what it wrote into the list beside the job.
async fn run(g: &BoxGuest, o: Opts) -> Ran {
    let said: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let claude = o.claude.unwrap_or_else(|| claude_travelled(&["--stdio"], o.far));
    let codex = o.codex.unwrap_or_else(codex_travelled);
    let root = g.root_str();
    let landed =
        provision_files(&common::handle(&g.machine), files_ask(&root, lands(), pack_configs(&claude, &codex), IndexMap::new())).await;
    let heard = said.clone();
    let options = ProvisionMcpOptions {
        held: o.held,
        stage: Arc::new(move |_: GoldenStage, detail: Option<&str>| {
            if let Some(d) = detail {
                heard.lock().unwrap().push(d.to_string());
            }
        }),
        ..mcp_options(&root, landed.owned.clone())
    };
    let servers = provision_mcp(&g.machine, &plan_on(&root, o.far, &o.untick), options).await;
    close_agent_files(&g.machine, &root, &once_paths_of(&lands()), &IndexMap::new()).await;
    let closing = said.lock().unwrap().last().cloned().unwrap_or_default();
    Ran {
        files: landed.rows.iter().map(|r| (r.id.clone(), s(word(r.outcome)))).collect(),
        servers: servers.iter().map(|r| (r.id.clone(), s(word(r.outcome)), r.note.clone())).collect(),
        closing,
    }
}

const CODEX_AGENTS_MD: &str = "Answer in one line.\n";
const CODEX_PROMPT: &str = "Review the diff.\n";

/// The servers step of a box job carrying Codex alone: its config, its standing instructions and a prompt, with the
/// one server the person ticked.
fn codex_step_plan(root: &str) -> ProvisionPlan {
    let mut mcp = plan_on(root, false, &[]);
    mcp.agents.retain(|a| a.id == "codex");
    ProvisionPlan {
        recipe_at: s("picks"),
        path: s(TOOLS_PATH),
        files: Some(PlanFiles {
            lands: vec![
                ProvisionLanding { id: s("agents/codex"), label: s("Codex"), dest: s(".codex/config.toml"), once: true },
                ProvisionLanding { id: s("agents/codex"), label: s("Codex"), dest: s(".codex/AGENTS.md"), once: false },
                ProvisionLanding { id: s("agents/codex"), label: s("Codex"), dest: s(".codex/prompts"), once: false },
            ],
            pack: pack_of(vec![
                file(".codex/config.toml", 0o600, &codex_travelled()),
                file(".codex/AGENTS.md", 0o644, CODEX_AGENTS_MD),
                file(".codex/prompts/review.md", 0o644, CODEX_PROMPT),
            ]),
        }),
        mcp: Some(mcp),
        ..ProvisionPlan::default()
    }
}

const CODEX_OWN: [(&str, &str); 2] = [(".codex/AGENTS.md", CODEX_AGENTS_MD), (".codex/prompts/review.md", CODEX_PROMPT)];

/// One files round and its close with no server in it, as the skills step lands a skill, given the stores its step
/// is handed.
async fn land_only(g: &BoxGuest, files: &[(&str, &str)], stores: Option<IndexMap<String, String>>) {
    let root = g.root_str();
    let landings = files
        .iter()
        .map(|(path, _)| ProvisionLanding { id: format!("files/{path}"), label: s(path), dest: s(path), once: false })
        .collect();
    let pack = pack_of(files.iter().map(|(path, content)| file(path, 0o644, content)).collect());
    let stores = stores.unwrap_or_default();
    provision_files(&common::handle(&g.machine), files_ask(&root, landings, pack, stores.clone())).await;
    close_agent_files(&g.machine, &root, &[], &stores).await;
}

/// Every answer the guest's runs come back with from here on, as the host holds each one.
fn answered(g: &mut BoxGuest) -> Arc<Mutex<Vec<String>>> {
    let answers: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let old: RunFn = g.machine.run.clone().expect("the guest answers runs");
    let seen = answers.clone();
    let run: RunFn = Arc::new(move |script: String, opts: RunOptions| {
        let fut = old(script, opts);
        let seen = seen.clone();
        Box::pin(async move {
            let r = fut.await?;
            seen.lock().unwrap().push(r.stdout.clone());
            Ok(r)
        })
    });
    g.machine = FakeMachine { run: Some(run), ..g.machine.clone() };
    answers
}

fn longest(answers: &Arc<Mutex<Vec<String>>>) -> usize {
    answers.lock().unwrap().iter().map(|a| a.len()).max().unwrap_or(0)
}

fn list(root: &str) -> Vec<String> {
    std::fs::read_to_string(place_provision_paths(root).landed).unwrap().split('\n').filter(|l| !l.is_empty()).map(s).collect()
}

fn list_ids(root: &str) -> Vec<String> {
    list(root).iter().map(|l| l.split('\t').next().unwrap().to_string()).collect()
}

fn claude_of(root: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(at(root, ".claude-cfg/.claude.json")).unwrap()).unwrap()
}

fn claude_server_names(root: &str) -> Vec<String> {
    claude_of(root)["mcpServers"].as_object().unwrap().keys().cloned().collect()
}

fn read_file(path: &str) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn write_file(path: &str, text: &str) {
    std::fs::write(path, text).unwrap();
}

fn mkdirs(path: &str) {
    std::fs::create_dir_all(path).unwrap();
}

fn server_names(text: &str, root: &str) -> Vec<String> {
    CODEX_TOML.read(text, root).iter().map(|x| x.name.clone()).collect()
}

fn file_rows(rows: &[PlaceProvisionRow]) -> Vec<&PlaceProvisionRow> {
    rows.iter().filter(|r| r.kind == Some(PlaceProvisionKind::File)).collect()
}

fn unlanded(gone: &[&str]) -> Option<Unlanded> {
    Some(Unlanded { gone: gone.iter().map(|g| s(g)).collect(), kept: vec![] })
}

// TS: packages/engine/test/provision-mcp.test.ts:195
// lands each server as the copy carries it, every key the person set kept and every value still a name
#[tokio::test(flavor = "multi_thread")]
async fn lands_each_server_as_the_copy_carries_it_every_key_kept_and_every_value_still_a_name() {
    let g = npx();
    let gsc = json!({ "type": "http", "url": "https://gsc.example/mcp", "headers": { "Authorization": "Bearer ${GSC_TOKEN}" }, "oauth": { "clientId": "abc", "callbackPort": 8080 } });
    let claude = pretty(json!({ "mcpServers": { "gsc": gsc, "mine": { "command": "npx", "args": [] } } }));
    let codex = [
        "[mcp_servers.context7]",
        "command = \"npx\"",
        "args = [\"-y\", \"context7\"]",
        "env_vars = [\"GITHUB_PERSONAL_ACCESS_TOKEN\", \"HTTPS_PROXY\"]",
        "disabled_tools = [\"delete_repository\"]",
        "startup_timeout_sec = 40",
        "",
    ]
    .join("\n");
    run(&g, Opts { claude: Some(claude), codex: Some(codex), ..Opts::default() }).await;
    assert_eq!(claude_of(&g.root_str())["mcpServers"]["gsc"], gsc);
    let own = read_file(&at(&g.root_str(), ".codex/config.toml"));
    for line in [
        "env_vars = [\"GITHUB_PERSONAL_ACCESS_TOKEN\", \"HTTPS_PROXY\"]",
        "disabled_tools = [\"delete_repository\"]",
        "startup_timeout_sec = 40",
    ] {
        assert!(own.contains(line), "{line} in {own}");
    }
}

// TS: packages/engine/test/provision-mcp.test.ts:206
// merges them into the file its agent keeps, reads them present on the next run, and writes its own copy again only where this computer's changed
#[tokio::test(flavor = "multi_thread")]
async fn merges_into_the_agents_file_reads_present_next_run_and_rewrites_only_where_the_copy_changed() {
    let g = npx();
    let root = g.root_str();
    mkdirs(&at(&root, ".claude-cfg"));
    write_file(&at(&root, ".claude-cfg/.claude.json"), &claude_on_box());

    // Claude Code has already run on that computer, so its file stands and its servers are merged into it; Codex has
    // not, so its file lands whole and the servers that came in it arrived with this run.
    let first = run(&g, Opts::default()).await;
    assert_eq!(first.files, pairs(&[("files/.claude-cfg/.claude.json", "present"), ("files/.codex/config.toml", "installed")]));
    assert_eq!(
        first.servers,
        vec![
            srv(&id("claude/gsc"), "installed", Some("npx fetches the package on first use")),
            srv(&id("claude/notes"), "skipped", Some("command is macOS-only, will not run")),
            srv(&id("claude/mine"), "skipped", Some(&their_server_line("Claude Code", "mine", &at(&root, ".claude-cfg/.claude.json")))),
            srv(&id("codex/context7"), "installed", Some("npx fetches the package on first use")),
            srv(&id("codex/grafana"), "skipped", Some("unticked")),
        ]
    );
    // A file that arrived whole is wsp's own hand for that run, so the server the person unticked comes out of it
    // again rather than standing there because nobody owns it.
    assert!(!read_file(&at(&root, ".codex/config.toml")).contains("grafana"));
    let after = claude_of(&root);
    assert_eq!(after["numStartups"], json!(41));
    assert_eq!(after["oauthAccount"], json!({ "emailAddress": "he@example.com" }));
    assert_eq!(after["projects"], json!({ "/root/work": { "history": ["his own turn"] } }));
    assert_eq!(after["mcpServers"]["mine"], json!({ "command": "/usr/local/bin/mine", "args": [] }));
    assert_eq!(after["mcpServers"]["gsc"], json!({ "command": "npx", "args": ["-y", "gsc-mcp", "--stdio"] }));
    // What wsp owns in a file its agent keeps is the keys, never the bytes.
    assert_eq!(list(&root).len(), 2);
    let mut ids = list_ids(&root);
    ids.sort();
    assert_eq!(ids, vec![id("claude/gsc"), id("codex/context7")]);

    // Both agents write their own files again, as they do at every launch and at the first turn in a folder.
    let mut rewritten = claude_of(&root);
    rewritten["numStartups"] = json!(42);
    rewritten["projects"]["/root/again"] = json!({ "history": [] });
    write_file(&at(&root, ".claude-cfg/.claude.json"), &pretty(rewritten));
    let codex_now = read_file(&at(&root, ".codex/config.toml"));
    write_file(&at(&root, ".codex/config.toml"), &format!("{codex_now}\n[projects.\"/root/repo\"]\ntrust_level = \"trusted\"\n"));
    let mut listed = list(&root);
    listed.sort();

    let second = run(&g, Opts::default()).await;
    assert_eq!(second.files, pairs(&[("files/.claude-cfg/.claude.json", "present"), ("files/.codex/config.toml", "present")]));
    assert_eq!(
        ids_and_outcomes(&second.servers),
        pairs(&[
            (&id("claude/gsc"), "present"),
            (&id("claude/notes"), "skipped"),
            (&id("claude/mine"), "skipped"),
            (&id("codex/context7"), "present"),
            (&id("codex/grafana"), "skipped"),
        ])
    );
    let mut again = list(&root);
    again.sort();
    assert_eq!(again, listed);
    assert!(read_file(&at(&root, ".codex/config.toml")).contains("[projects.\"/root/repo\"]"));
    // The words the round closes with say what its rows say: nothing on this run was installed.
    assert!(second.closing.contains("gsc, context7 already there"), "{}", second.closing);
    assert!(!second.closing.contains("installed"));
    assert!(!second.closing.contains("fetched"));

    // This computer's copy of one server changed: that key is wsp's own by the list, so it is written again.
    let third = run(&g, Opts { claude: Some(claude_travelled(&["--stdio", "--verbose"], false)), ..Opts::default() }).await;
    assert_eq!(
        ids_and_outcomes(&third.servers),
        pairs(&[
            (&id("claude/gsc"), "installed"),
            (&id("claude/notes"), "skipped"),
            (&id("claude/mine"), "skipped"),
            (&id("codex/context7"), "present"),
            (&id("codex/grafana"), "skipped"),
        ])
    );
    let held = claude_of(&root);
    assert_eq!(held["mcpServers"]["gsc"]["args"], json!(["-y", "gsc-mcp", "--stdio", "--verbose"]));
    assert_eq!(held["numStartups"], json!(42));
    assert_eq!(held["projects"]["/root/again"], json!({ "history": [] }));
    assert_eq!(held["mcpServers"]["mine"], json!({ "command": "/usr/local/bin/mine", "args": [] }));
    // On a run where one server did arrive, the closing words name that one and say the other is already there.
    assert!(third.closing.contains("context7 already there"), "{}", third.closing);
    assert!(third.closing.contains("gsc: package fetched on first use by npx"), "{}", third.closing);
    assert!(!third.closing.contains("context7 installed"));
    let gsc_line = |lines: &[String]| lines.iter().find(|l| l.starts_with(&format!("{}\t", id("claude/gsc")))).cloned();
    let gsc = gsc_line(&list(&root)).expect("the list holds gsc");
    assert_ne!(Some(gsc), gsc_line(&listed));
    assert_eq!(list(&root).len(), 2);
}

// TS: packages/engine/test/provision-mcp.test.ts:290
// leaves a server the agent itself rewrote since, names whose it is, and skips one whose command that computer does not have
#[tokio::test(flavor = "multi_thread")]
async fn leaves_a_server_the_agent_rewrote_since_and_skips_one_whose_command_is_not_there() {
    let g = npx();
    let root = g.root_str();
    mkdirs(&at(&root, ".claude-cfg"));
    write_file(&at(&root, ".claude-cfg/.claude.json"), &claude_on_box());
    let first = run(&g, Opts { far: true, ..Opts::default() }).await;
    assert_eq!(found(&first.servers, &id("claude/far")), &srv(&id("claude/far"), "skipped", Some("command not on the machine")));
    assert!(claude_of(&root)["mcpServers"].get("far").is_none());

    // The agent adds a field of its own to the entry wsp wrote: the entry is no longer the one the list holds, so
    // it is the agent's from here on and the next run leaves it exactly as it is.
    let mut held = claude_of(&root);
    held["mcpServers"]["gsc"]["args"] = json!(["-y", "gsc-mcp", "--stdio", "--codex-added-this"]);
    let theirs = pretty(held);
    write_file(&at(&root, ".claude-cfg/.claude.json"), &theirs);
    let again = run(&g, Opts { claude: Some(claude_travelled(&["--stdio", "--verbose"], false)), ..Opts::default() }).await;
    assert_eq!(
        found(&again.servers, &id("claude/gsc")),
        &srv(&id("claude/gsc"), "skipped", Some(&their_server_line("Claude Code", "gsc", &at(&root, ".claude-cfg/.claude.json"))))
    );
    assert_eq!(read_file(&at(&root, ".claude-cfg/.claude.json")), theirs);
}

// TS: packages/engine/test/provision-mcp.test.ts:314
// reads no config through a link or past the cap, names the file on each of its servers, and writes neither
#[tokio::test(flavor = "multi_thread")]
async fn reads_no_config_through_a_link_or_past_the_cap_and_writes_neither() {
    let mut g = npx();
    let root = g.root_str();
    let (_away_dir, away) = away_dir("wsp-box-away-");
    let outside = format!("{}\n", serde_json::to_string(&json!({ "mcpServers": {}, "note": "a file out of the home" })).unwrap());
    write_file(&at(&away, "claude.json"), &outside);
    let claude = at(&root, ".claude-cfg/.claude.json");
    let codex = at(&root, ".codex/config.toml");
    mkdirs(&at(&root, ".claude-cfg"));
    mkdirs(&at(&root, ".codex"));
    std::os::unix::fs::symlink(at(&away, "claude.json"), &claude).unwrap();
    let big = format!("{}#{}\n", codex_travelled(), "x".repeat(CONFIG_READ_MAX_BYTES as usize));
    write_file(&codex, &big);
    let answers = answered(&mut g);

    let r = run(&g, Opts::default()).await;
    let prefix = base64::engine::general_purpose::STANDARD.encode(outside.as_bytes())[..24].to_string();
    assert!(!answers.lock().unwrap().join("\n").contains(&prefix));
    assert!(longest(&answers) < CONFIG_READ_MAX_BYTES as usize);
    assert_eq!(
        r.servers,
        vec![
            srv(&id("claude/gsc"), "skipped", Some(&config_unread_line(&claude, CONFIG_NOT_PLAIN))),
            srv(&id("claude/mine"), "skipped", Some(&config_unread_line(&claude, CONFIG_NOT_PLAIN))),
            srv(&id("claude/notes"), "skipped", Some("command is macOS-only, will not run")),
            srv(&id("codex/context7"), "skipped", Some(&config_unread_line(&codex, CONFIG_PAST_CAP))),
            srv(&id("codex/grafana"), "skipped", Some("unticked")),
        ]
    );
    assert!(r.closing.contains(&config_unread_line(&claude, CONFIG_NOT_PLAIN)), "{}", r.closing);
    assert!(is_link(&claude));
    assert_eq!(read_file(&at(&away, "claude.json")), outside);
    assert_eq!(read_file(&codex), big);
}

// TS: packages/engine/test/provision-mcp.test.ts:346
// merges into a config linked inside the home, the link standing, and the leave over the link takes its servers back out
#[tokio::test(flavor = "multi_thread")]
async fn merges_into_a_config_linked_inside_the_home_and_the_leave_takes_its_servers_back_out() {
    let g = npx();
    let root = g.root_str();
    let claude = at(&root, ".claude-cfg/.claude.json");
    mkdirs(&at(&root, ".claude-cfg"));
    mkdirs(&at(&root, "dotfiles"));
    write_file(&at(&root, "dotfiles/claude.json"), &claude_on_box());
    std::os::unix::fs::symlink("../dotfiles/claude.json", &claude).unwrap();

    let r = run(&g, Opts::default()).await;
    assert_eq!(r.servers[0], srv(&id("claude/gsc"), "installed", Some("npx fetches the package on first use")));
    assert!(is_link(&claude));
    assert!(claude_server_names(&root).contains(&s("gsc")));

    let port = machine_server_port(Arc::new(g.machine.clone()));
    let out = unmerge_servers(&port, &root, &IndexMap::from([(s("claude"), at(&root, ".claude-cfg"))])).await.unwrap();
    assert!(out.contains(&ServersOut::Took { path: claude.clone(), names: vec![s("gsc")] }), "{out:?}");
    assert!(is_link(&claude));
    assert_eq!(claude_server_names(&root), vec![s("mine")]);
}

// TS: packages/engine/test/provision-mcp.test.ts:366
// the leave over the link takes nothing out of a config that became a link or grew past the cap, and names each
#[tokio::test(flavor = "multi_thread")]
async fn the_leave_takes_nothing_out_of_a_config_that_became_a_link_or_grew_past_the_cap() {
    let mut g = npx();
    let root = g.root_str();
    mkdirs(&at(&root, ".claude-cfg"));
    write_file(&at(&root, ".claude-cfg/.claude.json"), &claude_on_box());
    run(&g, Opts::default()).await;
    let claude = at(&root, ".claude-cfg/.claude.json");
    let codex = at(&root, ".codex/config.toml");
    assert!(claude_server_names(&root).contains(&s("gsc")));
    let (_away_dir, away) = away_dir("wsp-box-away-");
    std::fs::rename(&claude, at(&away, "claude.json")).unwrap();
    std::os::unix::fs::symlink(at(&away, "claude.json"), &claude).unwrap();
    let big = format!("{}#{}\n", read_file(&codex), "x".repeat(CONFIG_READ_MAX_BYTES as usize));
    write_file(&codex, &big);
    let answers = answered(&mut g);

    let port = machine_server_port(Arc::new(g.machine.clone()));
    let out = unmerge_servers(&port, &root, &IndexMap::from([(s("claude"), at(&root, ".claude-cfg"))])).await.unwrap();
    assert!(longest(&answers) < CONFIG_READ_MAX_BYTES as usize);
    assert_eq!(
        out,
        vec![
            ServersOut::Unread { path: claude.clone(), unread: s(CONFIG_NOT_PLAIN) },
            ServersOut::Unread { path: codex.clone(), unread: s(CONFIG_PAST_CAP) },
        ]
    );
    let lines: Vec<String> = out.iter().flat_map(servers_out_lines).collect();
    assert_eq!(lines, vec![servers_unread_line(&claude, CONFIG_NOT_PLAIN), servers_unread_line(&codex, CONFIG_PAST_CAP)]);
    let away_servers: Value = serde_json::from_str(&read_file(&at(&away, "claude.json"))).unwrap();
    assert!(away_servers["mcpServers"].as_object().unwrap().contains_key("gsc"));
    assert_eq!(read_file(&codex), big);
}

// TS: packages/engine/test/provision-mcp.test.ts:394
// writes down the keys it no longer owns, so the list stops naming a server that was unticked on this computer
#[tokio::test(flavor = "multi_thread")]
async fn writes_down_the_keys_it_no_longer_owns_so_the_list_stops_naming_an_unticked_server() {
    let g = npx();
    let root = g.root_str();
    mkdirs(&at(&root, ".claude-cfg"));
    write_file(&at(&root, ".claude-cfg/.claude.json"), &claude_on_box());
    run(&g, Opts::default()).await;
    let mut ids = list_ids(&root);
    ids.sort();
    assert_eq!(ids, vec![id("claude/gsc"), id("codex/context7")]);

    // The person unticks that server on this computer: the merge takes wsp's own entry back out of the file its
    // agent keeps, and the list stops saying wsp owns the name.
    let second = run(&g, Opts { untick: vec!["gsc"], ..Opts::default() }).await;
    assert_eq!(found(&second.servers, &id("claude/gsc")).1, "skipped");
    assert!(claude_of(&root)["mcpServers"].get("gsc").is_none());
    assert_eq!(list_ids(&root), vec![id("codex/context7")]);
    // The close reads what the round wrote down for a name it no longer owns once, so what stands is what wsp owns.
    assert!(!read_file(&place_provision_paths(&root).landed).contains("\t-\t-"));
    assert_eq!(claude_of(&root)["numStartups"], json!(41));
    assert_eq!(claude_of(&root)["mcpServers"]["mine"], json!({ "command": "/usr/local/bin/mine", "args": [] }));
}

// TS: packages/engine/test/provision-mcp.test.ts:416
// reads a home folder's server in a copy whose folder keys the pack already moved to that computer's home, and says it landed
#[tokio::test(flavor = "multi_thread")]
async fn reads_a_home_folders_server_in_a_copy_whose_folder_keys_the_pack_already_moved() {
    let g = npx();
    let root = g.root_str();
    let notion = json!({ "type": "http", "url": "https://notion.example/mcp" });
    let claude = pretty(json!({ "mcpServers": {}, "projects": { root.clone(): { "mcpServers": { "notion": notion } } } }));
    let mut plan = plan_on(&root, false, &[]);
    let mut claude_scope = scope(vec![at(&root, ".claude-cfg/.claude.json")], MCP_SERVERS_JSON, &["notion"], vec![]);
    claude_scope.project = Some(ProjectMove { from: s(HOME), to: root.clone() });
    plan.agents = vec![McpAgentPlan { id: s("claude"), label: s("Claude Code"), scopes: vec![claude_scope], aside: vec![] }];
    let landed = provision_files(&common::handle(&g.machine), files_ask(&root, lands(), pack_configs(&claude, ""), IndexMap::new())).await;
    let rows = provision_mcp(&g.machine, &plan, mcp_options(&root, landed.owned.clone())).await;
    assert_eq!(
        rows.iter().map(|r| (r.id.clone(), s(word(r.outcome)), r.note.clone())).collect::<Vec<_>>(),
        vec![srv(&id("claude/home/notion"), "installed", None)]
    );
    close_agent_files(&g.machine, &root, &once_paths_of(&lands()), &IndexMap::new()).await;
    assert_eq!(claude_of(&root)["projects"][root.as_str()], json!({ "mcpServers": { "notion": notion } }));

    // A later run finds the file standing, its entry the one the copy carries: there as the recipe asks.
    let again = provision_files(&common::handle(&g.machine), files_ask(&root, lands(), pack_configs(&claude, ""), IndexMap::new())).await;
    let rows = provision_mcp(&g.machine, &plan, mcp_options(&root, again.owned.clone())).await;
    assert_eq!(
        rows.iter().map(|r| (r.id.clone(), s(word(r.outcome)))).collect::<Vec<_>>(),
        pairs(&[(&id("claude/home/notion"), "present")])
    );
    close_agent_files(&g.machine, &root, &once_paths_of(&lands()), &IndexMap::new()).await;
}

// TS: packages/engine/test/provision-mcp.test.ts:435
// says on a server's row where its key does not reach that computer's threads, since that agent's launch hands it no value
#[tokio::test(flavor = "multi_thread")]
async fn says_on_a_servers_row_where_its_key_does_not_reach_that_computers_threads() {
    let g = npx();
    let held: IndexSet<String> = IndexSet::from([s("LINEAR_TOKEN")]);
    let claude = pretty(json!({
        "mcpServers": {
            "gsc": { "type": "http", "url": "https://gsc.example/mcp", "headers": { "Authorization": "Bearer ${LINEAR_TOKEN}" } },
            "mine": { "command": "npx", "args": [] },
        }
    }));
    let codex = ["[mcp_servers.context7]", "url = \"https://c7.example/mcp\"", "bearer_token_env_var = \"LINEAR_TOKEN\"", ""].join("\n");
    let ran = run(&g, Opts { claude: Some(claude), codex: Some(codex), held: Some(held.clone()), ..Opts::default() }).await;
    let servers = ran.servers;
    assert_eq!(
        found(&servers, &id("codex/context7")),
        &srv(&id("codex/context7"), "installed", Some(&key_unreached_line("Codex", &[s("LINEAR_TOKEN")])))
    );
    assert_eq!(found(&servers, &id("claude/gsc")).2, None);

    let o = npx();
    let oroot = o.root_str();
    let config = at(&oroot, ".config/opencode/opencode.json");
    mkdirs(&at(&oroot, ".config/opencode"));
    write_file(&config, OPENCODE_ON_BOX);
    let travelled = pretty(
        json!({ "mcp": { "docs": { "type": "local", "command": ["npx", "docs-mcp"], "enabled": true, "environment": { "DOCS_KEY": "{env:LINEAR_TOKEN}" } } } }),
    );
    let opencode_lands =
        vec![ProvisionLanding { id: s("agents/opencode"), label: s("OpenCode"), dest: s(".config/opencode/opencode.json"), once: true }];
    let pack = pack_of(vec![file(".config/opencode/opencode.json", 0o600, &travelled)]);
    let landed = provision_files(&common::handle(&o.machine), files_ask(&oroot, opencode_lands, pack, IndexMap::new())).await;
    let options = ProvisionMcpOptions { held: Some(held.clone()), ..mcp_options(&oroot, landed.owned.clone()) };
    let rows = provision_mcp(&o.machine, &opencode_plan_on(&oroot), options).await;
    assert_eq!(
        rows.iter().map(|r| r.note.clone()).collect::<Vec<_>>(),
        vec![Some(format!("npx fetches the package on first use; {}", key_unreached_line("OpenCode", &[s("LINEAR_TOKEN")])))]
    );
    // Read again with the server already as it travelled, the row reads present and still says so.
    let options = ProvisionMcpOptions { held: Some(held), ..mcp_options(&oroot, OwnedPaths::new()) };
    let again = provision_mcp(&o.machine, &opencode_plan_on(&oroot), options).await;
    assert_eq!(
        again.iter().map(|r| (s(word(r.outcome)), r.note.clone())).collect::<Vec<_>>(),
        vec![(s("present"), Some(key_unreached_line("OpenCode", &[s("LINEAR_TOKEN")])))]
    );
}

// TS: packages/engine/test/provision-mcp.test.ts:458
// merges into a jsonc config in place, so the person's comments stand and no row says anything of them
#[tokio::test(flavor = "multi_thread")]
async fn merges_into_a_jsonc_config_in_place_so_the_persons_comments_stand() {
    let g = npx();
    let root = g.root_str();
    let config = at(&root, ".config/opencode/opencode.json");
    mkdirs(&at(&root, ".config/opencode"));
    write_file(&config, OPENCODE_ON_BOX);
    let opencode_lands =
        vec![ProvisionLanding { id: s("agents/opencode"), label: s("OpenCode"), dest: s(".config/opencode/opencode.json"), once: true }];
    let pack = pack_of(vec![file(".config/opencode/opencode.json", 0o600, &opencode_travelled())]);
    let landed = provision_files(&common::handle(&g.machine), files_ask(&root, opencode_lands, pack, IndexMap::new())).await;
    let rows = provision_mcp(&g.machine, &opencode_plan_on(&root), mcp_options(&root, landed.owned.clone())).await;
    assert_eq!(
        rows.iter().map(|r| (r.id.clone(), s(word(r.outcome)), r.note.clone())).collect::<Vec<_>>(),
        vec![srv(&id("opencode/docs"), "installed", Some("npx fetches the package on first use"))]
    );
    let held = read_file(&config);
    assert_eq!(
        parse_jsonc(&held).unwrap(),
        json!({ "theme": "dark", "mcp": { "docs": { "type": "local", "command": ["npx", "docs-mcp"], "enabled": true } } })
    );
    assert!(held.contains("  // my own servers\n"), "{held}");
}

// TS: packages/engine/test/provision-mcp.test.ts:473
// merges an agent's servers into the config under the store its threads there read, outside the home, and takes them back out of it on a leave
#[tokio::test(flavor = "multi_thread")]
async fn merges_an_agents_servers_into_the_store_config_outside_the_home_and_takes_them_out_on_a_leave() {
    let g = npx();
    let root = g.root_str();
    let (_logins_dir, logins) = away_dir("wsp-box-logins-");
    write_file(&at(&logins, "config.toml"), "model = \"o4\"\n");
    let landed = provision_files(
        &common::handle(&g.machine),
        files_ask(&root, lands(), pack_configs(&claude_default(), &codex_travelled()), IndexMap::new()),
    )
    .await;
    let o = ProvisionMcpOptions { stores: Some(stores_of(&logins)), ..mcp_options(&root, landed.owned.clone()) };
    let rows = provision_mcp(&g.machine, &plan_on(&root, false, &[]), o.clone()).await;
    close_agent_files(&g.machine, &root, &once_paths_of(&lands()), &IndexMap::new()).await;
    assert_eq!(s(word(rows.iter().find(|r| r.id == id("codex/context7")).unwrap().outcome)), "installed");
    let own = read_file(&at(&logins, "config.toml"));
    assert!(own.starts_with("model = \"o4\"\n"));
    assert_eq!(server_names(&own, &root), vec![s("context7")]);
    // Claude Code names no store here, so its servers go where the catalog keeps them, as before.
    assert!(claude_server_names(&root).contains(&s("gsc")));

    let again = provision_mcp(&g.machine, &plan_on(&root, false, &[]), ProvisionMcpOptions { landed: OwnedPaths::new(), ..o }).await;
    assert_eq!(s(word(again.iter().find(|r| r.id == id("codex/context7")).unwrap().outcome)), "present");

    let port = machine_server_port(Arc::new(g.machine.clone()));
    let out = unmerge_servers(&port, &root, &stores_of(&logins)).await.unwrap();
    let config = at(&logins, "config.toml");
    assert_eq!(
        out.iter().find(|x| matches!(x, ServersOut::Took { path, .. } | ServersOut::Unread { path, .. } if *path == config)),
        Some(&ServersOut::Took { path: config.clone(), names: vec![s("context7")] })
    );
    assert_eq!(read_file(&config), "model = \"o4\"\n");
}

// TS: packages/engine/test/provision-mcp.test.ts:499
// writes the key Codex keeps a server's sign-in by on a box into its store's config, once, and never over the person's own
#[tokio::test(flavor = "multi_thread")]
async fn writes_the_key_codex_keeps_a_servers_sign_in_by_into_the_store_config_once() {
    let g = npx();
    let root = g.root_str();
    let (_logins_dir, logins) = away_dir("wsp-box-logins-");
    let on = ProvisionOn { home: root.clone(), held: None, stores: Some(stores_of(&logins)) };
    let linux = cfg!(target_os = "linux");
    let key = "mcp_oauth_credentials_store = \"file\"\n";
    provision_step(&common::handle(&g.machine), &codex_step_plan(&root), EngineStep::Mcp, new_setup_run(), no_provision_stage(), &on)
        .await
        .unwrap();
    let own = read_file(&at(&logins, "config.toml"));
    assert_eq!(own.starts_with(key), linux, "the key stands first, before any table");
    assert!(own.contains("model = \"gpt-5\""));
    assert_eq!(server_names(&own, &root), vec![s("context7")]);
    let again =
        provision_step(&common::handle(&g.machine), &codex_step_plan(&root), EngineStep::Mcp, new_setup_run(), no_provision_stage(), &on)
            .await
            .unwrap();
    assert_eq!(read_file(&at(&logins, "config.toml")), own);
    assert_eq!(file_rows(&again).iter().map(|r| word(r.outcome)).collect::<Vec<_>>(), vec!["present", "present", "present"]);

    // The person's own value stands, whatever it is.
    let (_theirs_dir, theirs) = away_dir("wsp-box-logins-");
    let theirs_config = at(&theirs, "config.toml");
    let none_agents: BTreeSet<String> = BTreeSet::new();
    write_file(&theirs_config, "model = \"o4\"\n  mcp_oauth_credentials_store = \"keyring\"\n");
    assert_eq!(headless_keys(&g.machine, &root, &stores_of(&theirs), &none_agents).await, Vec::<String>::new());
    assert_eq!(read_file(&theirs_config), "model = \"o4\"\n  mcp_oauth_credentials_store = \"keyring\"\n");

    // A quoted spelling of the key is the same key: a second one above it makes Codex refuse its whole config.
    let quoted = "\"mcp_oauth_credentials_store\" = \"keyring\"\n[mcp_servers.docs]\nurl = \"https://docs.example.test/mcp\"\n";
    write_file(&theirs_config, quoted);
    assert_eq!(headless_keys(&g.machine, &root, &stores_of(&theirs), &none_agents).await, Vec::<String>::new());
    assert_eq!(read_file(&theirs_config), quoted);
    // A key of that name under a table is not the top-level one Codex reads.
    write_file(&theirs_config, "[profiles.work]\nmcp_oauth_credentials_store = \"keyring\"\n");
    headless_keys(&g.machine, &root, &stores_of(&theirs), &none_agents).await;
    let under_table = "[profiles.work]\nmcp_oauth_credentials_store = \"keyring\"\n";
    assert_eq!(read_file(&theirs_config), if linux { format!("{key}{under_table}") } else { under_table.to_string() });

    // A store that is not there is made only where the setup puts Codex on.
    let none = at(&theirs, "not-yet");
    headless_keys(&g.machine, &root, &stores_of(&none), &none_agents).await;
    assert!(!Path::new(&none).exists());
    headless_keys(&g.machine, &root, &stores_of(&none), &BTreeSet::from([s("codex")])).await;
    let none_config = at(&none, "config.toml");
    let made = if Path::new(&none_config).exists() { Some(read_file(&none_config)) } else { None };
    assert_eq!(made, if linux { Some(s(key)) } else { None });
}

// TS: packages/engine/test/provision-mcp.test.ts:539
// says on the row of a store config that stood from an earlier run that it did, and which of this computer's servers merged into it
#[tokio::test(flavor = "multi_thread")]
async fn says_on_the_row_of_a_store_config_that_stood_which_of_this_computers_servers_merged_into_it() {
    let g = npx();
    let root = g.root_str();
    let (_logins_dir, logins) = away_dir("wsp-box-logins-");
    let on = ProvisionOn { home: root.clone(), held: None, stores: Some(stores_of(&logins)) };
    let label = format!("Codex {logins}/config.toml");
    let config_row =
        |rows: &[PlaceProvisionRow]| rows.iter().find(|r| r.label == label).cloned().unwrap_or_else(|| panic!("no row {label}"));
    let config = at(&logins, "config.toml");

    // An earlier run's config, with a server of its own and its own context7: nothing of this computer's goes in.
    write_file(
        &config,
        &[
            "[mcp_servers.axiom]",
            "url = \"https://axiom.example/mcp\"",
            "",
            "[mcp_servers.context7]",
            "command = \"/usr/local/bin/their-c7\"",
            "",
        ]
        .join("\n"),
    );
    let none =
        provision_step(&common::handle(&g.machine), &codex_step_plan(&root), EngineStep::Mcp, new_setup_run(), no_provision_stage(), &on)
            .await
            .unwrap();
    let row = config_row(&none);
    assert_eq!(word(row.outcome), "present");
    assert_eq!(
        row.note.as_deref(),
        Some("already there before this run, so it stands as it was; none of this computer's servers merged into it")
    );

    // Without its own context7, this computer's merges into it, and the row names it.
    write_file(&config, &["[mcp_servers.axiom]", "url = \"https://axiom.example/mcp\"", ""].join("\n"));
    let merged =
        provision_step(&common::handle(&g.machine), &codex_step_plan(&root), EngineStep::Mcp, new_setup_run(), no_provision_stage(), &on)
            .await
            .unwrap();
    let row = config_row(&merged);
    assert_eq!(word(row.outcome), "present");
    assert_eq!(row.note.as_deref(), Some("already there before this run, so it stands as it was; this computer's context7 merged into it"));
    assert_eq!(server_names(&read_file(&config), &root), vec![s("axiom"), s("context7")]);
}

// TS: packages/engine/test/provision-mcp.test.ts:558
// lands the rest of an agent's own files under the store its threads there read, and leaves none of them under the home
#[tokio::test(flavor = "multi_thread")]
async fn lands_the_rest_of_an_agents_own_files_under_the_store_and_none_under_the_home() {
    let g = npx();
    let root = g.root_str();
    let (_logins_dir, logins) = away_dir("wsp-box-logins-");
    let plan = codex_step_plan(&root);
    let on = ProvisionOn { home: root.clone(), held: None, stores: Some(stores_of(&logins)) };
    let rows =
        provision_step(&common::handle(&g.machine), &plan, EngineStep::Mcp, new_setup_run(), no_provision_stage(), &on).await.unwrap();
    let own = read_file(&at(&logins, "config.toml"));
    assert!(own.contains("model = \"gpt-5\""));
    assert_eq!(server_names(&own, &root), vec![s("context7")]);
    assert_eq!(read_file(&at(&logins, "AGENTS.md")), CODEX_AGENTS_MD);
    assert_eq!(read_file(&at(&logins, "prompts/review.md")), CODEX_PROMPT);
    assert!(!Path::new(&at(&root, ".codex")).exists());
    assert_eq!(
        file_rows(&rows).iter().map(|r| (r.label.clone(), s(word(r.outcome)))).collect::<Vec<_>>(),
        vec![
            (format!("Codex {logins}/config.toml"), s("installed")),
            (format!("Codex {logins}/AGENTS.md"), s("installed")),
            (format!("Codex {logins}/prompts"), s("installed")),
        ]
    );

    // The next run reads them as wsp's own copies, and a sync that takes the prompts off takes them from the store.
    let again =
        provision_step(&common::handle(&g.machine), &plan, EngineStep::Mcp, new_setup_run(), no_provision_stage(), &on).await.unwrap();
    assert_eq!(file_rows(&again).iter().map(|r| word(r.outcome)).collect::<Vec<_>>(), vec!["present", "present", "present"]);
    assert_eq!(
        unland_files(&g.machine, &root, &[s(".codex/prompts")], on.stores.as_ref().unwrap()).await,
        unlanded(&[".codex/prompts/review.md"])
    );
    assert!(!Path::new(&at(&logins, "prompts")).exists());
    assert!(Path::new(&at(&logins, "AGENTS.md")).exists());
}

// TS: packages/engine/test/provision-mcp.test.ts:586
// moves the copies an earlier run left under the home into the store, a file the person wrote there since staying
#[tokio::test(flavor = "multi_thread")]
async fn moves_the_copies_an_earlier_run_left_under_the_home_into_the_store() {
    let g = npx();
    let root = g.root_str();
    let (_logins_dir, logins) = away_dir("wsp-box-logins-");
    let plan = codex_step_plan(&root);
    let home_only = ProvisionOn { home: root.clone(), held: None, stores: None };
    provision_step(&common::handle(&g.machine), &plan, EngineStep::Mcp, new_setup_run(), no_provision_stage(), &home_only).await.unwrap();
    write_file(&at(&root, ".codex/prompts/mine.md"), "his own\n");
    let with_store = ProvisionOn { home: root.clone(), held: None, stores: Some(stores_of(&logins)) };
    provision_step(&common::handle(&g.machine), &plan, EngineStep::Mcp, new_setup_run(), no_provision_stage(), &with_store).await.unwrap();
    assert_eq!(read_file(&at(&logins, "AGENTS.md")), CODEX_AGENTS_MD);
    assert!(!Path::new(&at(&root, ".codex/AGENTS.md")).exists());
    assert!(!Path::new(&at(&root, ".codex/prompts/review.md")).exists());
    assert_eq!(read_file(&at(&root, ".codex/prompts/mine.md")), "his own\n");
}

// TS: packages/engine/test/provision-mcp.test.ts:600
// takes a dropped skill off under the home where the skills step landed it, on a box whose Claude reads a store of its own
#[tokio::test(flavor = "multi_thread")]
async fn takes_a_dropped_skill_off_under_the_home_on_a_box_whose_claude_reads_a_store() {
    let g = npx();
    let root = g.root_str();
    let (_logins_dir, logins) = away_dir("wsp-box-logins-");
    let skill = ".claude/skills/why/SKILL.md";
    land_only(&g, &[(skill, "# why\n")], None).await;
    assert!(list(&root).iter().any(|l| l.starts_with(&format!("{skill}\t"))));
    let stores = IndexMap::from([(s("claude"), at(&root, ".claude-cfg")), (s("codex"), logins)]);
    assert_eq!(unland_files(&g.machine, &root, &[s(".claude/skills/why")], &stores).await, unlanded(&[skill]));
    assert!(!Path::new(&at(&root, ".claude/skills/why")).exists());
    assert!(!list(&root).iter().any(|l| l.starts_with(&format!("{skill}\t"))));
}

// TS: packages/engine/test/provision-mcp.test.ts:613
// takes the copies an earlier run left under the home off with a Codex dropped before any run moved them
#[tokio::test(flavor = "multi_thread")]
async fn takes_the_copies_left_under_the_home_off_with_a_codex_dropped_before_any_run_moved_them() {
    let g = npx();
    let root = g.root_str();
    let (_logins_dir, logins) = away_dir("wsp-box-logins-");
    land_only(&g, &CODEX_OWN, None).await;
    write_file(&at(&root, ".codex/prompts/mine.md"), "his own\n");
    let out = unland_files(&g.machine, &root, &[s(".codex/AGENTS.md"), s(".codex/prompts")], &stores_of(&logins)).await;
    assert_eq!(out, unlanded(&[".codex/AGENTS.md", ".codex/prompts/review.md"]));
    assert!(!Path::new(&at(&root, ".codex/AGENTS.md")).exists());
    assert!(!Path::new(&at(&root, ".codex/prompts/review.md")).exists());
    assert_eq!(read_file(&at(&root, ".codex/prompts/mine.md")), "his own\n");
    assert_eq!(list(&root).into_iter().filter(|l| l.starts_with(".codex/")).collect::<Vec<_>>(), Vec::<String>::new());
}

// TS: packages/engine/test/provision-mcp.test.ts:627
// moves an earlier copy into the store without taking the agent's own folder under the home
#[tokio::test(flavor = "multi_thread")]
async fn moves_an_earlier_copy_into_the_store_without_taking_the_agents_own_folder() {
    let g = npx();
    let root = g.root_str();
    let (_logins_dir, logins) = away_dir("wsp-box-logins-");
    land_only(&g, &CODEX_OWN, None).await;
    land_only(&g, &CODEX_OWN, Some(stores_of(&logins))).await;
    assert_eq!(read_file(&at(&logins, "prompts/review.md")), CODEX_PROMPT);
    assert!(!Path::new(&at(&root, ".codex/prompts")).exists());
    assert!(Path::new(&at(&root, ".codex")).exists());
}

// TS: packages/engine/test/provision-mcp.test.ts:638
// takes every file wsp landed in a store off by the agent's folder alone, and leaves the store standing
#[tokio::test(flavor = "multi_thread")]
async fn takes_every_file_landed_in_a_store_off_by_the_agents_folder_and_leaves_the_store() {
    let g = npx();
    let root = g.root_str();
    let (_logins_dir, logins) = away_dir("wsp-box-logins-");
    land_only(&g, &CODEX_OWN, Some(stores_of(&logins))).await;
    assert_eq!(
        unland_files(&g.machine, &root, &[s(".codex")], &stores_of(&logins)).await,
        unlanded(&[".codex/AGENTS.md", ".codex/prompts/review.md"])
    );
    assert!(!Path::new(&at(&logins, "AGENTS.md")).exists());
    assert!(Path::new(&logins).exists());
    assert_eq!(list(&root).into_iter().filter(|l| l.starts_with(".codex/")).collect::<Vec<_>>(), Vec::<String>::new());
}

// TS: packages/engine/test/provision-mcp.test.ts:649
// leaves a config that is on no computer to the merge itself, which skips its servers rather than writing a file nobody has
#[tokio::test(flavor = "multi_thread")]
async fn leaves_a_config_on_no_computer_to_the_merge_which_skips_its_servers() {
    let g = npx();
    let root = g.root_str();
    let rows = provision_mcp(&g.machine, &plan_on(&root, false, &[]), mcp_options(&root, OwnedPaths::new())).await;
    assert!(rows.iter().all(|r| r.outcome == PlaceProvisionRowOutcome::Skipped));
    assert!(rows[0].note.as_deref().unwrap_or("").contains("Claude Code's config is not on the machine"), "{:?}", rows[0].note);
}
