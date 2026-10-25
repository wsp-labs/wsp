// SPDX-License-Identifier: AGPL-3.0-only
//! Review tests for the provision area that fail on the current code. Each carries the TS answer inline, read off
//! the TS at a4033dc34 with node v22.23.4; nothing is read from outside the crate.
mod common;

use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
use serde_json::{json, Value};

use common::{box_guest, BoxGuest, ExecFn, FakeMachine, PutFn};
use wsp_catalog::mcp::MCP_SERVERS_JSON;
use wsp_engine::golden_seam::{GoldenStage, McpAgentPlan, McpPlan, McpScope, TOOLS_PATH};
use wsp_engine::provision_files::{close_agent_files, machine_server_port, unmerge_servers, ServersOut};
use wsp_engine::provision_mcp::{provision_mcp, ProvisionMcpOptions};

fn s(text: &str) -> String {
    text.to_string()
}

// ---------------------------------------------------------------- finding 1: configs JSON.parse reads and serde_json refuses

const GSC_COPY: &str = "{\"mcpServers\":{\"gsc\":{\"command\":\"npx\",\"args\":[\"a\"]}}}\n";

fn claude_scope(path: &str, travelled: &str) -> McpScope {
    McpScope {
        files: vec![s(path)],
        format: MCP_SERVERS_JSON,
        project: None,
        keep: vec![s("gsc")],
        drop: vec![],
        travelled: Some(s(travelled)),
        own: None,
        folder: None,
        trust: None,
        edit: None,
    }
}

fn claude_plan(root: &str, path: &str, travelled: &str) -> McpPlan {
    McpPlan {
        agents: vec![McpAgentPlan { id: s("claude"), label: s("Claude Code"), scopes: vec![claude_scope(path, travelled)], aside: vec![] }],
        guest_home: s(root),
        rewrites: vec![],
        bin_dirs: vec![],
        tools: vec![],
    }
}

fn mcp_options(root: &str) -> ProvisionMcpOptions {
    ProvisionMcpOptions {
        home: s(root),
        landed: Default::default(),
        tools: vec![],
        stage: Arc::new(|_: GoldenStage, _: Option<&str>| {}),
        path: s(TOOLS_PATH),
        held: None,
        stores: None,
        into: None,
    }
}

/// Claude's own file holding `own` and a copy that carries `gsc`, through `provision_mcp`: the rows' (outcome, note)
/// and the file's text after.
async fn merge_claude(own: &str, travelled: &str) -> (Vec<(String, String)>, String) {
    let g: BoxGuest = box_guest(&["npx"], None);
    let root = g.root_str();
    let path = format!("{root}/.claude-cfg/.claude.json");
    std::fs::create_dir_all(format!("{root}/.claude-cfg")).unwrap();
    std::fs::write(&path, own).unwrap();
    let rows = provision_mcp(&g.machine, &claude_plan(&root, &path, travelled), mcp_options(&root)).await;
    let rows = rows.iter().map(|r| (format!("{:?}", r.outcome), r.note.clone().unwrap_or_default())).collect();
    (rows, std::fs::read_to_string(&path).unwrap())
}

// TS packages/catalog/src/jsonc.ts:41 (JSON.parse) and provision-mcp.ts:266 (merge splices text): installed, the
// escape untouched; probe printed `surrogate ROWS installed:npx fetches the package on first use` and
// `surrogate FILE "{\"note\":\"\\ud83d\",\"other\":\"\\ude00x\",\"mcpServers\":{\"gsc\": {\"command\":\"npx\",\"args\":[\"a\"]}}}\n"`.
#[tokio::test(flavor = "current_thread")]
async fn a_claude_file_with_a_lone_surrogate_escape_gets_the_server_and_keeps_its_bytes() {
    let own = "{\"note\":\"\\ud83d\",\"other\":\"\\ude00x\",\"mcpServers\":{}}\n";
    let (rows, file) = merge_claude(own, GSC_COPY).await;
    assert_eq!(rows, vec![(s("Installed"), s("npx fetches the package on first use"))], "rows");
    assert_eq!(
        file,
        "{\"note\":\"\\ud83d\",\"other\":\"\\ude00x\",\"mcpServers\":{\"gsc\": {\"command\":\"npx\",\"args\":[\"a\"]}}}\n"
    );
}

// TS packages/catalog/src/jsonc.ts:41 (JSON.parse of the copy that travelled): the same, installed.
#[tokio::test(flavor = "current_thread")]
async fn a_copy_that_travelled_with_a_lone_surrogate_escape_still_gives_its_servers() {
    let copy = "{\"history\":\"\\ud83d\",\"mcpServers\":{\"gsc\":{\"command\":\"npx\",\"args\":[\"a\"]}}}\n";
    let (rows, _) = merge_claude("{\"numStartups\":3}\n", copy).await;
    assert_eq!(rows, vec![(s("Installed"), s("npx fetches the package on first use"))]);
}

// TS packages/catalog/src/jsonc.ts:41 (JSON.parse reads 1e400 as Infinity) and provision-mcp.ts:266; probe printed
// `exp FILE "{\"y\":1e400,\"z\":-1e400,\"mcpServers\":{\"gsc\": {\"command\":\"npx\",\"args\":[\"a\"]}}}\n"`.
#[tokio::test(flavor = "current_thread")]
async fn a_claude_file_with_a_number_past_f64_is_merged_and_the_number_text_kept() {
    let own = "{\"y\":1e400,\"z\":-1e400,\"mcpServers\":{}}\n";
    let (rows, file) = merge_claude(own, GSC_COPY).await;
    assert_eq!(rows, vec![(s("Installed"), s("npx fetches the package on first use"))], "rows");
    assert_eq!(file, "{\"y\":1e400,\"z\":-1e400,\"mcpServers\":{\"gsc\": {\"command\":\"npx\",\"args\":[\"a\"]}}}\n");
}

// TS provision-files.ts:621-623 (entryOf, then remove, on JSON.parse-able text); probe printed
// `OUT [{"path":"<ROOT>/.claude-cfg/.claude.json","names":["gsc"]}] FILE "{\"note\":\"\\ud83d\",\"numStartups\":3,\"mcpServers\":{}}\n"`.
#[tokio::test(flavor = "current_thread")]
async fn a_leave_takes_wsps_server_out_of_a_claude_file_that_holds_a_lone_surrogate_escape() {
    let g: BoxGuest = box_guest(&["npx"], None);
    let root = g.root_str();
    let path = format!("{root}/.claude-cfg/.claude.json");
    std::fs::create_dir_all(format!("{root}/.claude-cfg")).unwrap();
    std::fs::write(&path, "{\"numStartups\":3,\"mcpServers\":{}}\n").unwrap();
    provision_mcp(&g.machine, &claude_plan(&root, &path, GSC_COPY), mcp_options(&root)).await;
    close_agent_files(&g.machine, &root, &[], &IndexMap::new()).await;
    let text = std::fs::read_to_string(&path).unwrap().replacen("{\"numStartups\":3,", "{\"note\":\"\\ud83d\",\"numStartups\":3,", 1);
    std::fs::write(&path, text).unwrap();
    let stores = IndexMap::from([(s("claude"), format!("{root}/.claude-cfg"))]);
    let out = unmerge_servers(&machine_server_port(Arc::new(g.machine.clone())), &root, &stores).await;
    let got: Vec<(String, Vec<String>)> = out
        .into_iter()
        .map(|o| match o {
            ServersOut::Took { path, names } => (path.replace(&root, "<ROOT>"), names),
            ServersOut::Unread { path, unread } => (path.replace(&root, "<ROOT>"), vec![unread]),
        })
        .collect();
    assert_eq!(got, vec![(s("<ROOT>/.claude-cfg/.claude.json"), vec![s("gsc")])]);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"note\":\"\\ud83d\",\"numStartups\":3,\"mcpServers\":{}}\n");
}

// ---------------------------------------------------------------- findings 3 and the partial items: the facts file

fn stdout_of(facts: &str) -> String {
    use base64::Engine as _;
    format!(
        "WSP_CTX\nKERNEL 6.1.0\nDISK 1000 500\nOVERLAY yes\nAGENT claude\nFACTS {}\nWSP_CTX_END\n",
        base64::engine::general_purpose::STANDARD.encode(facts.as_bytes())
    )
}

/// The files of a gzipped ustar archive, by path.
fn untar(gz: &[u8]) -> IndexMap<String, String> {
    use std::io::Read;
    let mut tar = Vec::new();
    flate2::read::GzDecoder::new(gz).read_to_end(&mut tar).unwrap();
    let mut out = IndexMap::new();
    let mut at = 0;
    while at + 512 <= tar.len() {
        let h = &tar[at..at + 512];
        if h.iter().all(|b| *b == 0) {
            break;
        }
        let field = |a: usize, n: usize| String::from_utf8_lossy(&h[a..a + n]).split('\0').next().unwrap_or("").to_string();
        let (name, prefix) = (field(0, 100), field(345, 155));
        let size = usize::from_str_radix(field(124, 12).trim(), 8).unwrap();
        let key = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        out.insert(key, String::from_utf8_lossy(&tar[at + 512..at + 512 + size]).into_owned());
        at += 512 + size.div_ceil(512) * 512;
    }
    out
}

/// `apply_machine_context` over a machine whose probe answers with `facts` as its FACTS file: the skill's
/// "Tools that did not install" line and the machine-context.json text written back.
async fn apply_facts(facts: &str) -> Value {
    let landed: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let stdout = stdout_of(facts);
    let exec: ExecFn = Arc::new(move |cmd, _| {
        let stdout = stdout.clone();
        Box::pin(async move { Ok(if cmd.contains("echo WSP_CTX") { common::res(0, &stdout, "") } else { common::ok() }) })
    });
    let seen = landed.clone();
    let put: PutFn = Arc::new(move |_, bytes| {
        *seen.lock().unwrap() = Some(bytes);
        Box::pin(async { Ok(None) })
    });
    let machine = FakeMachine { id: s("m"), exec, run: None, download_url: None, upload_url: None, put_bytes: Some(put) };
    wsp_engine::machine_context::apply_machine_context(&machine, Default::default()).await;
    let files = landed.lock().unwrap().clone().map(|gz| untar(&gz)).unwrap_or_default();
    let skill = files.get("etc/wsp/skills/wsp-machine/SKILL.md").cloned().unwrap_or_default();
    json!({
        "factsJson": files.get("etc/wsp/machine-context.json"),
        "tools": skill.lines().find(|l| l.starts_with("- Tools that did not install")),
    })
}

// TS machine-context.ts:95 (JSON.parse keeps the escape) and :529 (written back as `\ud83d`); the skill read
// `- Tools that did not install: A (U+FFFD).` The TS host writes such a file: golden-tools.ts:96 slice(0, 160).
#[tokio::test(flavor = "current_thread")]
async fn a_facts_file_a_js_host_wrote_with_a_lone_surrogate_escape_is_still_read() {
    let got = apply_facts("{\"tools\":[{\"id\":\"a\",\"label\":\"A\",\"note\":\"\\ud83d\"}]}").await;
    assert_eq!(got["tools"], json!("- Tools that did not install: A (\u{FFFD})."));
}

// TS machine-context.ts:96 (the element is kept as it is) and :336 (`${m.label} (${m.note})`); probe printed
// `id only | none ... "tools":"- Tools that did not install: undefined (undefined)."`.
#[tokio::test(flavor = "current_thread")]
async fn a_facts_item_missing_its_label_and_note_renders_as_the_ts_prints_it() {
    let got = apply_facts("{\"tools\":[{\"id\":\"a\"}]}").await;
    assert_eq!(got["tools"], json!("- Tools that did not install: undefined (undefined)."));
}

// TS machine-context.ts:96 and :529 (JSON.stringify of the kept elements); probe printed
// `string element | none ... "factsJson":"{\n  \"tools\": [\n    \"foo\"\n  ],\n  \"agents\": [],\n  \"files\": []\n}\n"`.
#[tokio::test(flavor = "current_thread")]
async fn a_facts_element_that_is_not_an_object_is_kept_and_written_back_as_it_was() {
    let got = apply_facts("{\"tools\":[\"foo\"]}").await;
    assert_eq!(got["factsJson"], json!("{\n  \"tools\": [\n    \"foo\"\n  ],\n  \"agents\": [],\n  \"files\": []\n}\n"));
}

// TS machine-context.ts:96 and :529; probe printed `note empty | none ... "factsJson":"{\n  \"tools\": [\n    {\n      \"id\": \"a\",\n      \"label\": \"A\",\n      \"note\": \"\"\n    }\n  ], ...`.
#[tokio::test(flavor = "current_thread")]
async fn a_facts_item_with_an_explicit_empty_note_is_written_back_with_the_empty_note() {
    let got = apply_facts("{\"tools\":[{\"id\":\"a\",\"label\":\"A\",\"note\":\"\"}]}").await;
    assert_eq!(
        got["factsJson"],
        json!("{\n  \"tools\": [\n    {\n      \"id\": \"a\",\n      \"label\": \"A\",\n      \"note\": \"\"\n    }\n  ],\n  \"agents\": [],\n  \"files\": []\n}\n")
    );
}

// TS machine-context.ts:96 and :529; probe printed `extra field | none ... \"note\": \"n\",\n      \"extra\": 1\n    }`.
#[tokio::test(flavor = "current_thread")]
async fn a_facts_item_with_a_field_the_ts_does_not_know_keeps_it_when_written_back() {
    let got = apply_facts("{\"tools\":[{\"id\":\"a\",\"label\":\"A\",\"note\":\"n\",\"extra\":1}]}").await;
    assert!(got["factsJson"].as_str().unwrap().contains("\"extra\": 1"), "{}", got["factsJson"]);
}
