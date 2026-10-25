// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/provision-files.test.ts at a4033dc34, the ledger half: the ownership read as the daemon
//! renders it, taking a row's files off a computer, and taking wsp's servers back out of the agents' own files.
mod common;

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;

use common::{box_guest, digest, read, write};
use wsp_catalog::mcp::{McpFormat, CODEX_TOML, MCP_SERVERS_JSON, OPENCODE_JSON};
use wsp_engine::errors::Result;
use wsp_engine::golden_seam::{ScopeFile, ScopeRead};
use wsp_engine::provision_files::{
    landed_files_script, server_digest, servers_out_lines, unland_files, unmerge_servers, ServerPort, ServersOut, Unlanded, SERVER_MARK,
};
use wsp_wire::views::golden_image::MCP_ID_PREFIX;
use wsp_words::project_path::place_provision_paths;

const CONTRACT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../daemon/fixtures/contract/");

// TS: packages/engine/test/provision-files.test.ts:641
// landed-files.sh equals its regeneration, so the twin the daemon carries is this script
#[test]
fn landed_files_sh_equals_its_regeneration_so_the_twin_the_daemon_carries_is_this_script() {
    let text = format!("{}\n", landed_files_script("{home}"));
    let path = format!("{CONTRACT}landed-files.sh");
    let on_disk = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("daemon/fixtures/contract/landed-files.sh is missing ({e}); regenerate it from the engine and commit it")
    });
    assert_eq!(on_disk, text, "daemon/fixtures/contract/landed-files.sh is behind the engine");
}

// TS: packages/engine/test/provision-files.test.ts:654
// takes off what wsp left as it left it, keeps a file the person has written since, never follows a link or a climb out,
// and drops every line for the row
#[tokio::test(flavor = "multi_thread")]
async fn takes_off_what_wsp_left_as_it_left_it_keeps_a_file_the_person_has_written_since_never_follows_a_link_or_a_climb_out() {
    let g = box_guest(&[], None);
    let root = g.root_str();
    write(&g.root, ".claude/skills/a/SKILL.md", "a\n");
    write(&g.root, ".claude/skills/a/notes.md", "edited there\n");
    write(&g.root, ".claude/skills/b/SKILL.md", "b\n");
    let outside = tempfile::Builder::new().prefix("wsp-unland-outside-").tempdir().unwrap();
    fs::write(outside.path().join("victim"), "v\n").unwrap();
    symlink(outside.path(), g.root.join(".claude/skills/a/link")).unwrap();
    fs::write(outside.path().join("climbed"), "c\n").unwrap();
    let ledger = place_provision_paths(&root).landed;
    let line = |rel: &str, at: &str| format!("{rel}\t{at}\t{at}\n");
    let outside_name = outside.path().file_name().unwrap().to_str().unwrap().to_string();
    write(
        &g.root,
        &ledger[root.len() + 1..],
        &[
            line(".claude/skills/a/SKILL.md", &digest("a\n")),
            line(".claude/skills/a/notes.md", &digest("as wsp left it\n")),
            line(".claude/skills/a/link/victim", &digest("v\n")),
            line(&format!(".claude/skills/a/../../../{outside_name}/climbed"), &digest("c\n")),
            line(".claude/skills/b/SKILL.md", &digest("b\n")),
        ]
        .concat(),
    );

    let out = unland_files(&g.machine, &root, &[".claude/skills/a".to_string()], &IndexMap::new()).await;
    assert_eq!(
        out,
        Some(Unlanded {
            gone: vec![".claude/skills/a/SKILL.md".to_string()],
            kept: vec![".claude/skills/a/link/victim".to_string(), ".claude/skills/a/notes.md".to_string()],
        })
    );
    assert!(!g.root.join(".claude/skills/a/SKILL.md").exists());
    assert_eq!(read(&g.root, ".claude/skills/a/notes.md"), "edited there\n");
    assert_eq!(fs::read_to_string(outside.path().join("victim")).unwrap(), "v\n");
    assert_eq!(fs::read_to_string(outside.path().join("climbed")).unwrap(), "c\n");
    assert_eq!(read(&g.root, ".claude/skills/b/SKILL.md"), "b\n");
    assert_eq!(fs::read_to_string(&ledger).unwrap(), line(".claude/skills/b/SKILL.md", &digest("b\n")));
}

// TS: packages/engine/test/provision-files.test.ts:685
// takes the emptied folder with the last file in it
#[tokio::test(flavor = "multi_thread")]
async fn takes_the_emptied_folder_with_the_last_file_in_it() {
    let g = box_guest(&[], None);
    let root = g.root_str();
    write(&g.root, ".claude/skills/a/refs/one.md", "1\n");
    let ledger = place_provision_paths(&root).landed;
    write(&g.root, &ledger[root.len() + 1..], &format!(".claude/skills/a/refs/one.md\t{d}\t{d}\n", d = digest("1\n")));
    let out = unland_files(&g.machine, &root, &[".claude/skills/a".to_string()], &IndexMap::new()).await;
    assert_eq!(out, Some(Unlanded { gone: vec![".claude/skills/a/refs/one.md".to_string()], kept: vec![] }));
    assert!(!g.root.join(".claude/skills/a").exists());
    assert!(Path::new(&g.root).join(".claude/skills").exists());
}

const CLAUDE: &str = "/root/.claude.json";
const CODEX: &str = "/root/.codex/config.toml";

/// Claude Code's own file on that computer: one server wsp merged in, one of the person's own, one whose entry the agent
/// has rewritten since, and one under the machine's home folder.
fn claude_text() -> String {
    let value = serde_json::json!({
        "numStartups": 41,
        "mcpServers": {
            "gsc": { "command": "npx", "args": ["gsc-mcp"] },
            "mine": { "command": "/usr/local/bin/mine" },
            "notes": { "command": "notes", "args": ["--rewritten-by-the-agent"] },
        },
        "projects": {
            "/root": { "mcpServers": { "zed": { "command": "zed" } } },
            "/root/work": { "history": ["his own turn"] },
        },
    });
    format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
}

fn codex_text() -> String {
    [
        "[projects.\"/root/repo\"]",
        "trust_level = \"trusted\"",
        "",
        "[mcp_servers.mine]",
        "command = \"/usr/local/bin/mine\"",
        "",
        "[mcp_servers.context7]",
        "command = \"npx\"",
        "args = [\"-y\", \"context7\"]",
        "",
    ]
    .join("\n")
}

type Files = Arc<Mutex<IndexMap<String, String>>>;

/// That computer as the three calls the unmerge makes of it, over files held here rather than on a disk.
struct FakePort {
    files: Files,
    keys: IndexMap<String, String>,
    wrote: Arc<Mutex<Vec<String>>>,
}

#[async_trait(?Send)]
impl ServerPort for FakePort {
    async fn run(&self, script: &str) -> String {
        if script.contains(MCP_ID_PREFIX) {
            self.keys.iter().map(|(id, digest)| format!("{SERVER_MARK}\t{digest}\t{id}")).collect::<Vec<_>>().join("\n")
        } else {
            String::new()
        }
    }
    async fn read(&self, candidates: &[String], _base: &str) -> Result<Option<ScopeRead>> {
        let files = self.files.lock().unwrap();
        Ok(candidates.iter().find_map(|path| {
            files.get(path).map(|text| ScopeRead::File(ScopeFile { path: path.clone(), text: text.clone(), sum: String::new() }))
        }))
    }
    async fn write(&self, file: &ScopeFile, text: &str, _base: &str) -> Result<()> {
        self.files.lock().unwrap().insert(file.path.clone(), text.to_string());
        self.wrote.lock().unwrap().push(file.path.clone());
        Ok(())
    }
}

struct Computer {
    port: FakePort,
    files: Files,
    wrote: Arc<Mutex<Vec<String>>>,
}

fn fake_port(files: &[(&str, String)], keys: &[(String, String)]) -> Computer {
    let files: Files = Arc::new(Mutex::new(files.iter().map(|(p, t)| (p.to_string(), t.clone())).collect()));
    let wrote = Arc::new(Mutex::new(Vec::new()));
    let port = FakePort { files: files.clone(), keys: keys.iter().cloned().collect(), wrote: wrote.clone() };
    Computer { port, files, wrote }
}

impl Computer {
    fn text(&self, path: &str) -> String {
        self.files.lock().unwrap().get(path).unwrap().clone()
    }
    fn wrote(&self) -> Vec<String> {
        self.wrote.lock().unwrap().clone()
    }
}

fn digest_in(text: &str, format: McpFormat, name: &str, project: Option<&str>) -> String {
    server_digest(format.entry_of(text, name, project).as_deref()).unwrap()
}

// TS: packages/engine/test/provision-files.test.ts:734
// takes out the names the list holds whose entry there is still wsp's own, under both scopes and in both formats
#[tokio::test]
async fn takes_out_the_names_the_list_holds_whose_entry_there_is_still_wsps_own_under_both_scopes_and_in_both_formats() {
    let (claude, codex) = (claude_text(), codex_text());
    let keys = vec![
        (format!("{MCP_ID_PREFIX}claude/gsc"), digest_in(&claude, MCP_SERVERS_JSON, "gsc", None)),
        (format!("{MCP_ID_PREFIX}claude/home/zed"), digest_in(&claude, MCP_SERVERS_JSON, "zed", Some("/root"))),
        (format!("{MCP_ID_PREFIX}claude/notes"), "a digest from the round that landed it".to_string()),
        (format!("{MCP_ID_PREFIX}codex/context7"), digest_in(&codex, CODEX_TOML, "context7", None)),
    ];
    let computer = fake_port(&[(CLAUDE, claude), (CODEX, codex)], &keys);

    let took = unmerge_servers(&computer.port, "/root", &IndexMap::new()).await.unwrap();
    assert_eq!(
        took,
        vec![
            ServersOut::Took { path: CLAUDE.to_string(), names: vec!["gsc".to_string(), "zed".to_string()] },
            ServersOut::Took { path: CODEX.to_string(), names: vec!["context7".to_string()] },
        ]
    );
    let mut wrote = computer.wrote();
    wrote.sort();
    let mut both = vec![CODEX.to_string(), CLAUDE.to_string()];
    both.sort();
    assert_eq!(wrote, both);
    assert_eq!(servers_out_lines(&took[1]), vec![format!("context7 (out of {CODEX})")]);

    let claude: serde_json::Value = serde_json::from_str(&computer.text(CLAUDE)).unwrap();
    let names: Vec<&String> = claude["mcpServers"].as_object().unwrap().keys().collect();
    assert_eq!(names, vec!["mine", "notes"]);
    assert_eq!(claude["projects"]["/root"]["mcpServers"], serde_json::json!({}));
    assert_eq!(claude["projects"]["/root/work"], serde_json::json!({ "history": ["his own turn"] }));
    assert_eq!(claude["numStartups"], serde_json::json!(41));
    assert_eq!(
        computer.text(CODEX),
        ["[projects.\"/root/repo\"]", "trust_level = \"trusted\"", "", "[mcp_servers.mine]", "command = \"/usr/local/bin/mine\"", ""]
            .join("\n")
    );
}

// TS: packages/engine/test/provision-files.test.ts:762
// keeps the comments of a jsonc file it takes a key out of
#[tokio::test]
async fn keeps_the_comments_of_a_jsonc_file_it_takes_a_key_out_of() {
    let opencode = "/root/.config/opencode/opencode.json";
    let commented = "{\n  // my own servers\n  \"theme\": \"dark\",\n  \"mcp\": { \"docs\": { \"type\": \"local\", \"command\": [\"npx\", \"docs-mcp\"] } }\n}\n";
    let claude = claude_text();
    let keys = vec![
        (format!("{MCP_ID_PREFIX}opencode/docs"), digest_in(commented, OPENCODE_JSON, "docs", None)),
        (format!("{MCP_ID_PREFIX}claude/gsc"), digest_in(&claude, MCP_SERVERS_JSON, "gsc", None)),
    ];
    let computer = fake_port(&[(opencode, commented.to_string()), (CLAUDE, claude)], &keys);

    let took = unmerge_servers(&computer.port, "/root", &IndexMap::new()).await.unwrap();
    assert_eq!(
        took,
        vec![
            ServersOut::Took { path: CLAUDE.to_string(), names: vec!["gsc".to_string()] },
            ServersOut::Took { path: opencode.to_string(), names: vec!["docs".to_string()] },
        ]
    );
    assert_eq!(servers_out_lines(&took[1]), vec![format!("docs (out of {opencode})")]);
    assert!(computer.text(opencode).contains("// my own servers\n"));
}

// TS: packages/engine/test/provision-files.test.ts:781
// writes nothing where the list holds no key of that agent's, where the file is not there, and where every entry has
// changed
#[tokio::test]
async fn writes_nothing_where_the_list_holds_no_key_where_the_file_is_not_there_and_where_every_entry_has_changed() {
    let empty = fake_port(&[(CLAUDE, claude_text())], &[]);
    assert_eq!(unmerge_servers(&empty.port, "/root", &IndexMap::new()).await.unwrap(), vec![]);
    assert_eq!(empty.wrote(), Vec::<String>::new());

    let gone = fake_port(&[], &[(format!("{MCP_ID_PREFIX}claude/gsc"), "d1".to_string())]);
    assert_eq!(unmerge_servers(&gone.port, "/root", &IndexMap::new()).await.unwrap(), vec![]);
    let theirs = fake_port(&[(CLAUDE, claude_text())], &[(format!("{MCP_ID_PREFIX}claude/gsc"), "d1".to_string())]);
    assert_eq!(unmerge_servers(&theirs.port, "/root", &IndexMap::new()).await.unwrap(), vec![]);
    assert_eq!(theirs.wrote(), Vec::<String>::new());
}
