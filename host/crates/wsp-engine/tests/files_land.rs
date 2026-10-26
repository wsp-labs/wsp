// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/provision-files.test.ts at a4033dc34, the landing half: which files may land, the rows
//! a landing answers with, the landing on the computer itself and what already stands there. The landing is run by a
//! real shell over a real directory standing in for that computer's home.
mod common;

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use common::{box_guest, digest, other, read, res, write, BoxGuest, FakeMachine};
use wsp_catalog::mcp::MCP_SERVERS_JSON;
use wsp_engine::errors::Result;
use wsp_engine::golden_seam::{LeaveOut, McpAgentPlan, McpPlan, McpScope, PackFiles, PackedFiles, SkippedPath, StagedFile};
use wsp_engine::machine::ExecResult;
use wsp_engine::provision_files::{
    agent_state_file, append_landing, close_agent_files, files_rows, land_agent_files, landed_servers, once_paths_of, parse_landed,
    provision_files, standing_digests, FilesSay, LandAgentFiles, LandFilesResult, OwnedHow, OwnedPaths, ProvisionFilesAsk,
    ProvisionLanding, LEDGER_MARK, STAND_MARK,
};
use wsp_engine::provision_mcp::{no_copy_line, provision_mcp, ProvisionMcpOptions};
use wsp_engine::vault::{tar_of, TarEntry};
use wsp_wire::types::daemon_contract::TOOLS_PATH;
use wsp_wire::views::golden_image::MCP_ID_PREFIX;
use wsp_wire::views::place::{PlaceProvisionKind, PlaceProvisionRowOutcome as Outcome};
use wsp_wire::wire::daemon::EXEC_DEADLINE_EXIT;
use wsp_words::project_path::place_provision_paths;
use wsp_words::views::place_setup_words::{
    provision_landed_line, provision_list_read_line, provision_packed_line, provision_shipped_line, Packed, Shipped,
};

const HOME: &str = "/Users/dev";

fn may(id: &str, path: &str) -> bool {
    agent_state_file(id, &format!("{HOME}/{path}"), HOME).unwrap()
}

// TS: packages/engine/test/provision-files.test.ts:50
// is a file on an agent's own row at a path the catalog names for that agent
#[test]
fn is_a_file_on_an_agents_own_row_at_a_path_the_catalog_names_for_that_agent() {
    assert!(may("agents/claude", ".claude/skills/why/SKILL.md"));
    assert!(may("agents/claude", ".claude/settings.json"));
    assert!(may("agents/claude", ".claude.json"));
    assert!(may("agents/codex", ".codex/AGENTS.md"));
    assert!(may("agents/codex", ".codex/prompts/plan.md"));
    assert!(may("agents/opencode", ".config/opencode/skills/a/SKILL.md"));
    assert!(may("agents/mcp/claude/github", ".claude.json"));
}

// TS: packages/engine/test/provision-files.test.ts:61
// is never a dotfile, a login's store, another agent's path or an agent the catalog does not carry
#[test]
fn is_never_a_dotfile_a_logins_store_another_agents_path_or_an_agent_the_catalog_does_not_carry() {
    assert!(!may("dotfiles/zshrc", ".zshrc"));
    assert!(!may("dotfiles/ssh", ".ssh/config"));
    assert!(!may("agents/mcp/mcp-remote", ".mcp-auth/mcp-remote-0.1.29/tokens.json"));
    assert!(!may("agents/aider", ".aider.conf.yml"));
    assert!(!may("agents/claude", ".codex/AGENTS.md"));
    assert!(!may("agents/codex", ".codex.bak/AGENTS.md"));
    assert!(!may("logins/claude", ".claude/.credentials.json"));
}

fn landing(id: &str, label: &str, dest: &str) -> ProvisionLanding {
    ProvisionLanding { id: id.to_string(), label: label.to_string(), dest: dest.to_string(), once: false }
}

fn landing_once(id: &str, label: &str, dest: &str) -> ProvisionLanding {
    ProvisionLanding { once: true, ..landing(id, label, dest) }
}

fn row_lands() -> Vec<ProvisionLanding> {
    vec![
        landing("agents/claude", "Claude Code", ".claude-cfg/skills"),
        landing("agents/codex", "Codex", ".codex/AGENTS.md"),
        landing("agents/gemini", "Gemini CLI", ".gemini/GEMINI.md"),
    ]
}

// TS: packages/engine/test/provision-files.test.ts:79
// is installed where anything landed, present where every path was there already, skipped where the person keeps their
// own, and names what it did not write over
#[test]
fn is_installed_where_anything_landed_present_where_every_path_was_there_already_skipped_where_the_person_keeps_their_own() {
    let lands = row_lands();
    let printed = [
        "wsp-land\tinstalled\td1\t.claude-cfg/skills/why/SKILL.md",
        "wsp-land\tkept\td2\t.claude-cfg/skills/mine/SKILL.md",
        "wsp-land\tpresent\td3\t.codex/AGENTS.md",
        "wsp-land\tkept\td4\t.gemini/GEMINI.md",
        "wsp-land\tinstalled\td5\t.claude-cfg/settings.json",
        "not a line of the landing's",
    ]
    .join("\n");
    let rows = files_rows(&lands, &parse_landed(&printed), "/root", &vec![]);
    let got: Vec<(&str, Outcome, Option<&str>)> = rows.iter().map(|r| (r.id.as_str(), r.outcome, r.note.as_deref())).collect();
    assert_eq!(
        got,
        vec![
            (
                "files/.claude-cfg/skills",
                Outcome::Installed,
                Some("1 of 2 files; 1 already there with other content: .claude-cfg/skills/mine/SKILL.md")
            ),
            ("files/.codex/AGENTS.md", Outcome::Present, None),
            ("files/.gemini/GEMINI.md", Outcome::Skipped, Some("already there with other content: .gemini/GEMINI.md")),
            ("files/.claude-cfg/settings.json", Outcome::Installed, None),
        ]
    );
    assert!(rows.iter().all(|r| r.kind == Some(PlaceProvisionKind::File)));
    assert_eq!(rows[0].label, "Claude Code /root/.claude-cfg/skills");
    assert_eq!(rows[3].label, "/root/.claude-cfg/settings.json");
}

// TS: packages/engine/test/provision-files.test.ts:107
// is failed where a path could not be written, and skipped where nothing of a planned path travelled
#[test]
fn is_failed_where_a_path_could_not_be_written_and_skipped_where_nothing_of_a_planned_path_travelled() {
    let rows = files_rows(&row_lands(), &parse_landed("wsp-land\tfailed\td1\t.claude-cfg/skills/why/SKILL.md"), "/root", &vec![]);
    assert_eq!(rows[0].outcome, Outcome::Failed);
    assert!(rows[0].note.as_deref().unwrap().contains(".claude-cfg/skills/why/SKILL.md"));
    assert_eq!(rows[1].outcome, Outcome::Skipped);
    assert_eq!(rows[1].note.as_deref(), Some("nothing of it travelled"));
}

fn box_() -> BoxGuest {
    box_guest(&[], None)
}

fn quiet() -> FilesSay {
    Arc::new(|_| {})
}

fn no_stores() -> IndexMap<String, String> {
    IndexMap::new()
}

fn file(path: &str, content: &str) -> TarEntry {
    TarEntry::File { path: path.to_string(), mode: 0o644, content: content.as_bytes().to_vec() }
}

fn tar(entries: &[TarEntry]) -> Vec<u8> {
    tar_of(entries).unwrap()
}

/// A packed archive as the host's own pack answers with one, with the sizes nothing here reads.
fn packed(tar: Vec<u8>) -> PackedFiles {
    PackedFiles { bytes: tar.len() as u64, unpacked: tar.len() as u64, tar, ..Default::default() }
}

fn pack_of(tar: Vec<u8>) -> PackFiles {
    Arc::new(move |_| {
        let tar = tar.clone();
        Box::pin(async move { Ok(packed(tar)) })
    })
}

fn pack_that_fails(message: &'static str) -> PackFiles {
    Arc::new(move |_| Box::pin(async move { Err(other(message)) }))
}

type Sent = Arc<Mutex<Vec<Vec<String>>>>;
type Asked = Arc<Mutex<Vec<Vec<StagedFile>>>>;

struct Packing {
    pack: PackFiles,
    sent: Sent,
    #[allow(dead_code)]
    asked: Asked,
}

impl Packing {
    fn sent(&self) -> Vec<Vec<String>> {
        self.sent.lock().unwrap().clone()
    }
}

/// The pack on this computer as the host's own answers one: the staged files with the digest each would land at, every
/// path the answer names left out of the archive, and what it left out said beside what it packed. `sent` is the
/// archive's own paths, one entry per pack.
fn packing(files: &[(&str, &str)]) -> Packing {
    let files: Vec<(String, String)> = files.iter().map(|(p, c)| (p.to_string(), c.to_string())).collect();
    let sent: Sent = Arc::new(Mutex::new(Vec::new()));
    let asked: Asked = Arc::new(Mutex::new(Vec::new()));
    let (sent_in, asked_in) = (sent.clone(), asked.clone());
    let pack: PackFiles = Arc::new(move |leave_out: Option<LeaveOut>| {
        let files = files.clone();
        let (sent, asked) = (sent_in.clone(), asked_in.clone());
        let fut: LocalBoxFuture<'static, Result<PackedFiles>> = Box::pin(async move {
            let staged: Vec<StagedFile> =
                files.iter().map(|(path, content)| StagedFile { dest: path.clone(), digest: digest(content) }).collect();
            let answered = match leave_out {
                None => Vec::new(),
                Some(leave_out) => {
                    asked.lock().unwrap().push(staged.clone());
                    leave_out(staged).await?
                }
            };
            let stood: Vec<String> = files.iter().map(|(p, _)| p.clone()).filter(|p| answered.contains(p)).collect();
            let travels: Vec<&(String, String)> = files.iter().filter(|(p, _)| !stood.contains(p)).collect();
            sent.lock().unwrap().push(travels.iter().map(|(p, _)| p.clone()).collect());
            let entries: Vec<TarEntry> = travels.iter().map(|(p, c)| file(p, c)).collect();
            let tar = tar_of(&entries)?;
            Ok(PackedFiles {
                bytes: tar.len() as u64,
                unpacked: tar.len() as u64,
                tar,
                skipped: vec![],
                files: Some(travels.len() as u64),
                stood: Some(stood),
            })
        });
        fut
    });
    Packing { pack, sent, asked }
}

/// The claude scope over the config the landing puts there, for the run that reads whose that config is. One server of
/// it is in the copy that travels and one is in neither that copy nor the file there.
fn mcp_plan_on(root: &str) -> McpPlan {
    McpPlan {
        agents: vec![McpAgentPlan {
            id: "claude".to_string(),
            label: "Claude Code".to_string(),
            scopes: vec![McpScope {
                files: vec![format!("{root}/.claude-cfg/.claude.json")],
                format: MCP_SERVERS_JSON,
                project: None,
                keep: vec!["github".to_string(), "gsc".to_string()],
                drop: vec![],
                travelled: None,
                own: None,
                folder: None,
                trust: None,
                edit: None,
            }],
            aside: vec![],
        }],
        guest_home: root.to_string(),
        rewrites: vec![],
        bin_dirs: vec![],
        tools: vec![],
    }
}

fn mcp_options(root: &str, landed: OwnedPaths) -> ProvisionMcpOptions {
    ProvisionMcpOptions {
        home: root.to_string(),
        landed,
        tools: vec![],
        stage: Arc::new(|_, _| {}),
        path: TOOLS_PATH.to_string(),
        held: None,
        stores: None,
        into: None,
    }
}

fn lands_all() -> Vec<ProvisionLanding> {
    vec![
        landing("agents/claude", "Claude Code", ".claude-cfg/skills"),
        landing("agents/claude", "Claude Code", ".claude-cfg/CLAUDE.md"),
        landing("agents/codex", "Codex", ".codex/AGENTS.md"),
    ]
}

/// A skill folder whose name holds a backslash: the list beside the job is read by a shell, and a path handed to awk as a
/// variable would have that backslash read as an escape.
const ODD: &str = ".claude-cfg/skills/back\\slash/SKILL.md";

#[derive(Default)]
struct Over<'a> {
    skill: Option<&'a str>,
    odd: Option<&'a str>,
    agents: Option<&'a str>,
}

fn tar_all(over: Over) -> Vec<u8> {
    tar(&[
        file(".claude-cfg/skills/why/SKILL.md", over.skill.unwrap_or("the why skill\n")),
        file(ODD, over.odd.unwrap_or("a skill with a backslash in its folder\n")),
        file(".claude-cfg/CLAUDE.md", "his standing rules\n"),
        file(".codex/AGENTS.md", over.agents.unwrap_or("the same rules for codex\n")),
    ])
}

async fn land(g: &BoxGuest, tar: Vec<u8>, lands: &[ProvisionLanding]) -> LandFilesResult {
    land_agent_files(
        &g.machine,
        LandAgentFiles { home: g.root_str(), tar, lands: lands.to_vec(), stood: vec![], say: quiet(), roots: vec![] },
    )
    .await
    .unwrap()
}

async fn close(g: &BoxGuest, once: &[String]) {
    close_agent_files(&g.machine, &g.root_str(), once, &no_stores()).await;
}

fn lines_of(path: &str) -> Vec<String> {
    fs::read_to_string(path).unwrap().split('\n').filter(|l| !l.is_empty()).map(str::to_string).collect()
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn outcomes(rows: &[wsp_wire::views::place::PlaceProvisionRow]) -> Vec<Outcome> {
    rows.iter().map(|r| r.outcome).collect()
}

fn ids_and_outcomes(rows: &[wsp_wire::views::place::PlaceProvisionRow]) -> Vec<(String, Outcome)> {
    rows.iter().map(|r| (r.id.clone(), r.outcome)).collect()
}

// TS: packages/engine/test/provision-files.test.ts:191
// lands what is missing, leaves what is already the same, never writes over the person's own file, and says which is
// which
#[tokio::test(flavor = "multi_thread")]
async fn lands_what_is_missing_leaves_what_is_already_the_same_never_writes_over_the_persons_own_file() {
    let g = box_();
    let root = g.root_str();
    write(&g.root, ".codex/AGENTS.md", "what he wrote on the box\n");
    write(&g.root, ".claude-cfg/CLAUDE.md", "his standing rules\n");
    let landed = land(&g, tar_all(Over::default()), &lands_all()).await;
    assert_eq!(read(&g.root, ".claude-cfg/skills/why/SKILL.md"), "the why skill\n");
    assert_eq!(read(&g.root, ".codex/AGENTS.md"), "what he wrote on the box\n");
    assert_eq!(
        ids_and_outcomes(&landed.rows),
        vec![
            ("files/.claude-cfg/skills".to_string(), Outcome::Installed),
            ("files/.claude-cfg/CLAUDE.md".to_string(), Outcome::Present),
            ("files/.codex/AGENTS.md".to_string(), Outcome::Skipped),
        ]
    );
    assert!(landed.rows[2].note.as_deref().unwrap().contains("already there with other content"));
    let mut owned: Vec<(String, OwnedHow)> = landed.owned.iter().map(|(k, v)| (k.clone(), *v)).collect();
    owned.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        owned,
        vec![
            (format!("{root}/.claude-cfg/CLAUDE.md"), OwnedHow::Present),
            (format!("{root}/{ODD}"), OwnedHow::Installed),
            (format!("{root}/.claude-cfg/skills/why/SKILL.md"), OwnedHow::Installed),
        ]
    );
    assert_eq!(
        landed.skipped,
        vec![SkippedPath {
            id: "agents/codex".to_string(),
            path: format!("{root}/.codex/AGENTS.md"),
            note: "already there with other content; wsp did not write over it".to_string()
        }]
    );
}

fn ident_is_dev(ident: &str) -> bool {
    let Some(rest) = ident.strip_prefix("Dev <dev@acme.test> ") else { return false };
    let Some(rest) = rest.strip_suffix('\n') else { return false };
    let Some((secs, zone)) = rest.split_once(' ') else { return false };
    !secs.is_empty()
        && secs.bytes().all(|b| b.is_ascii_digit())
        && zone.len() == 5
        && matches!(zone.as_bytes()[0], b'+' | b'-')
        && zone[1..].bytes().all(|b| b.is_ascii_digit())
}

// TS: packages/engine/test/provision-files.test.ts:213
// writes over an empty file, which a workspace on that computer leaves as the mount point of its own copy, so the git
// identity lands
#[tokio::test(flavor = "multi_thread")]
async fn writes_over_an_empty_file_which_a_workspace_leaves_as_the_mount_point_so_the_git_identity_lands() {
    let g = box_();
    for rel in [".gitconfig", ".zshrc", ".zshenv"] {
        write(&g.root, rel, "");
    }
    let lands = vec![
        landing("configs/git", "git config", ".gitconfig"),
        landing("configs/shell", "shell config", ".zshrc"),
        landing("configs/shell", "shell config", ".zshenv"),
    ];
    let archive = tar(&[
        file(".gitconfig", "[user]\n\tname = Dev\n\temail = dev@acme.test\n"),
        file(".zshrc", "setopt autocd\n"),
        file(".zshenv", "export EDITOR=vi\n"),
    ]);
    let landed = land(&g, archive, &lands).await;
    let got: Vec<(String, Outcome, Option<String>)> = landed.rows.iter().map(|r| (r.id.clone(), r.outcome, r.note.clone())).collect();
    assert_eq!(
        got,
        vec![
            ("files/.gitconfig".to_string(), Outcome::Installed, None),
            ("files/.zshrc".to_string(), Outcome::Installed, None),
            ("files/.zshenv".to_string(), Outcome::Installed, None),
        ]
    );
    assert_eq!(landed.skipped, vec![]);
    let out = std::process::Command::new("git")
        .args(["var", "GIT_AUTHOR_IDENT"])
        .current_dir(&g.root)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string()))
        .env("HOME", &g.root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "git var failed: {}", String::from_utf8_lossy(&out.stderr));
    let ident = String::from_utf8(out.stdout).unwrap();
    assert!(ident_is_dev(&ident), "ident was {ident:?}");
}

// TS: packages/engine/test/provision-files.test.ts:233
// lands its own copy again when this computer's file changed, and leaves that file alone once the person has written it
// themselves
#[tokio::test(flavor = "multi_thread")]
async fn lands_its_own_copy_again_when_this_computers_file_changed_and_leaves_that_file_alone_once_the_person_wrote_it() {
    let g = box_();
    let at = place_provision_paths(&g.root_str());
    let lands = lands_all();
    land(&g, tar_all(Over::default()), &lands).await;
    close(&g, &[]).await;
    let mut listed: Vec<String> = lines_of(&at.landed).iter().map(|l| l.split('\t').next().unwrap().to_string()).collect();
    listed.sort();
    assert_eq!(
        listed,
        vec![
            ".claude-cfg/CLAUDE.md".to_string(),
            ODD.to_string(),
            ".claude-cfg/skills/why/SKILL.md".to_string(),
            ".codex/AGENTS.md".to_string()
        ]
    );
    assert!(!Path::new(&at.staging).exists());

    let again = land(&g, tar_all(Over::default()), &lands).await;
    assert_eq!(outcomes(&again.rows), vec![Outcome::Present, Outcome::Present, Outcome::Present]);
    close(&g, &[]).await;

    let changed = land(&g, tar_all(Over { skill: Some("the why skill, rewritten\n"), ..Over::default() }), &lands).await;
    assert_eq!(outcomes(&changed.rows), vec![Outcome::Installed, Outcome::Present, Outcome::Present]);
    assert_eq!(read(&g.root, ".claude-cfg/skills/why/SKILL.md"), "the why skill, rewritten\n");
    close(&g, &[]).await;

    write(&g.root, ".claude-cfg/skills/why/SKILL.md", "his own skill now\n");
    let theirs = land(&g, tar_all(Over { skill: Some("a third copy\n"), ..Over::default() }), &lands).await;
    assert_eq!(theirs.rows[0].outcome, Outcome::Present);
    assert!(theirs.rows[0].note.as_deref().unwrap().contains("already there with other content: .claude-cfg/skills/why/SKILL.md"));
    assert_eq!(read(&g.root, ".claude-cfg/skills/why/SKILL.md"), "his own skill now\n");
}

// TS: packages/engine/test/provision-files.test.ts:268
// leaves the list as it was after a round that landed nothing, so the paths it landed before stay its own
#[tokio::test(flavor = "multi_thread")]
async fn leaves_the_list_as_it_was_after_a_round_that_landed_nothing_so_the_paths_it_landed_before_stay_its_own() {
    let g = box_();
    let at = place_provision_paths(&g.root_str());
    let lands = lands_all();
    land(&g, tar_all(Over::default()), &lands).await;
    close(&g, &[]).await;
    let listed = fs::read_to_string(&at.landed).unwrap();

    for rel in [".claude-cfg/skills/why/SKILL.md", ODD, ".claude-cfg/CLAUDE.md", ".codex/AGENTS.md"] {
        write(&g.root, rel, &format!("his own {rel}\n"));
    }
    let none = land(&g, tar_all(Over::default()), &lands).await;
    assert!(none.rows.iter().all(|r| r.outcome == Outcome::Skipped));
    close(&g, &[]).await;
    assert_eq!(fs::read_to_string(&at.landed).unwrap(), listed);

    write(&g.root, ".claude-cfg/CLAUDE.md", "his standing rules\n");
    land(&g, tar_all(Over::default()), &lands).await;
    close(&g, &[]).await;
    let lines = lines_of(&at.landed);
    assert_eq!(lines.iter().filter(|l| l.starts_with(".claude-cfg/CLAUDE.md\t")).count(), 1);
}

// TS: packages/engine/test/provision-files.test.ts:291
// reads its own copy of a path holding a backslash off the list, and replaces it when this computer's copy changed
#[tokio::test(flavor = "multi_thread")]
async fn reads_its_own_copy_of_a_path_holding_a_backslash_off_the_list_and_replaces_it_when_the_copy_changed() {
    let g = box_();
    let lands = lands_all();
    land(&g, tar_all(Over::default()), &lands).await;
    close(&g, &[]).await;
    assert_eq!(read(&g.root, ODD), "a skill with a backslash in its folder\n");
    let changed = land(&g, tar_all(Over { odd: Some("the same skill, rewritten\n"), ..Over::default() }), &lands).await;
    assert_eq!(changed.rows[0].outcome, Outcome::Installed);
    assert!(!changed.rows[0].note.as_deref().unwrap().contains("other content"));
    assert_eq!(read(&g.root, ODD), "the same skill, rewritten\n");
}

// TS: packages/engine/test/provision-files.test.ts:303
// lands the file an agent keeps for itself only where it is missing, leaves what is in it after that, and keeps no line
// for it in the list
#[tokio::test(flavor = "multi_thread")]
async fn lands_the_file_an_agent_keeps_for_itself_only_where_it_is_missing_and_keeps_no_line_for_it_in_the_list() {
    let g = box_();
    let root = g.root_str();
    let at = place_provision_paths(&root);
    let mut lands = vec![landing_once("agents/claude", "Claude Code", ".claude-cfg/.claude.json")];
    lands.extend(lands_all());
    let tar_of_config =
        |config: &str| tar(&[file(".claude-cfg/.claude.json", config), file(".claude-cfg/CLAUDE.md", "his standing rules\n")]);
    fs::create_dir_all(&at.dir).unwrap();
    fs::write(&at.landed, ".claude-cfg/.claude.json\tdead\tdead\n").unwrap();

    let first = land(&g, tar_of_config("{}\n"), &lands).await;
    assert_eq!(first.rows[0].outcome, Outcome::Installed);
    assert_eq!(first.owned.get(&format!("{root}/.claude-cfg/.claude.json")), Some(&OwnedHow::Installed));
    close(&g, &once_paths_of(&lands)).await;
    assert!(!fs::read_to_string(&at.landed).unwrap().contains(".claude.json"));

    write(&g.root, ".claude-cfg/.claude.json", "{ \"numStartups\": 3 }\n");
    let again = land(&g, tar_of_config("{ \"mcpServers\": {} }\n"), &lands).await;
    assert_eq!(again.rows[0].outcome, Outcome::Present);
    assert_eq!(again.rows[0].note.as_deref(), Some("already there before this run, so it stands as it was"));
    let md = again.rows.iter().find(|r| r.id == "files/.claude-cfg/CLAUDE.md");
    assert_eq!(
        (md.map(|r| r.outcome), md.and_then(|r| r.note.clone())),
        (Some(Outcome::Present), None),
        "a file that lands whole says nothing of the kind"
    );
    assert_eq!(read(&g.root, ".claude-cfg/.claude.json"), "{ \"numStartups\": 3 }\n");
    close(&g, &once_paths_of(&lands)).await;
    assert!(!fs::read_to_string(&at.landed).unwrap().contains(".claude.json"));
}

fn said_into(said: &Arc<Mutex<Vec<String>>>) -> FilesSay {
    let said = said.clone();
    Arc::new(move |line: &str| said.lock().unwrap().push(line.to_string()))
}

// TS: packages/engine/test/provision-files.test.ts:332
// says what it packed, what went over and what the landing walked, each as a line of its own
#[tokio::test(flavor = "multi_thread")]
async fn says_what_it_packed_what_went_over_and_what_the_landing_walked_each_as_a_line_of_its_own() {
    let g = box_();
    let root = g.root_str();
    let said = Arc::new(Mutex::new(Vec::<String>::new()));
    let archive = tar_all(Over::default());
    let lands = lands_all();
    let landed = provision_files(
        &g.machine,
        ProvisionFilesAsk {
            home: root.clone(),
            lands: lands.clone(),
            pack: pack_of(archive.clone()),
            say: Some(said_into(&said)),
            stores: no_stores(),
        },
    )
    .await;
    assert_eq!(outcomes(&landed.rows), vec![Outcome::Installed, Outcome::Installed, Outcome::Installed]);
    let n = archive.len() as f64;
    assert_eq!(
        *said.lock().unwrap(),
        vec![
            provision_packed_line(lands.len() as f64, &Packed { bytes: n, ..Default::default() }),
            provision_shipped_line(&Shipped { part: 1.0, parts: 1.0, bytes: n, total: n, pieces: None }),
            provision_landed_line(4.0),
        ]
    );

    append_landing(&g.machine, &root, &[format!("{MCP_ID_PREFIX}claude/github\td1\td1")]).await;
    close(&g, &[]).await;
    said.lock().unwrap().clear();
    assert_eq!(landed_servers(&g.machine, &root, said_into(&said)).await.len(), 1);
    assert_eq!(*said.lock().unwrap(), vec![provision_list_read_line(1.0)]);
}

// TS: packages/engine/test/provision-files.test.ts:354
// closes a round that landed only keys, and carries a key's line through as the servers step wrote it
#[tokio::test(flavor = "multi_thread")]
async fn closes_a_round_that_landed_only_keys_and_carries_a_keys_line_through_as_the_servers_step_wrote_it() {
    let g = box_();
    let root = g.root_str();
    let at = place_provision_paths(&root);
    land(&g, tar_all(Over::default()), &lands_all()).await;
    let keys = vec![format!("{MCP_ID_PREFIX}claude/github\td1\td1"), format!("{MCP_ID_PREFIX}codex/context7\td2\td2")];
    append_landing(&g.machine, &root, &keys).await;
    close(&g, &[]).await;
    let listed = || lines_of(&at.landed);
    assert_eq!(listed().into_iter().filter(|l| l.starts_with(MCP_ID_PREFIX)).collect::<Vec<_>>(), keys);
    let md_line = listed().into_iter().find(|l| l.starts_with(".claude-cfg/CLAUDE.md\t")).unwrap();
    let md: Vec<&str> = md_line.split('\t').collect();
    assert!(is_hex64(md[1]));
    assert_eq!(md[2], md[1]);
    let expected: IndexMap<String, String> =
        [(format!("{MCP_ID_PREFIX}claude/github"), "d1".to_string()), (format!("{MCP_ID_PREFIX}codex/context7"), "d2".to_string())]
            .into_iter()
            .collect();
    assert_eq!(landed_servers(&g.machine, &root, quiet()).await, expected);

    append_landing(&g.machine, &root, &[format!("{MCP_ID_PREFIX}claude/gsc\td3\td3")]).await;
    close(&g, &[]).await;
    let mut ids: Vec<String> = landed_servers(&g.machine, &root, quiet()).await.keys().cloned().collect();
    ids.sort();
    assert_eq!(
        ids,
        vec![format!("{MCP_ID_PREFIX}claude/github"), format!("{MCP_ID_PREFIX}claude/gsc"), format!("{MCP_ID_PREFIX}codex/context7")]
    );
}

fn config_with(server: &str) -> String {
    let mut entry = serde_json::Map::new();
    entry.insert("command".to_string(), "npx".into());
    let mut servers = serde_json::Map::new();
    servers.insert(server.to_string(), entry.into());
    let mut top = serde_json::Map::new();
    top.insert("mcpServers".to_string(), servers.into());
    format!("{}\n", serde_json::to_string_pretty(&serde_json::Value::Object(top)).unwrap())
}

// TS: packages/engine/test/provision-files.test.ts:376
// reads a round whose files never left this computer as one that put nothing there, and leaves the servers in the agents'
// own files as they are
#[tokio::test(flavor = "multi_thread")]
async fn reads_a_round_whose_files_never_left_this_computer_as_one_that_put_nothing_there() {
    let g = box_();
    let root = g.root_str();
    let config = format!("{root}/.claude-cfg/.claude.json");
    let mut lands = lands_all();
    lands.push(landing_once("agents/claude", "Claude Code", ".claude-cfg/.claude.json"));
    let tar_of_server = |server: &str| tar(&[file(".claude-cfg/.claude.json", &config_with(server))]);

    let first = provision_files(
        &g.machine,
        ProvisionFilesAsk {
            home: root.clone(),
            lands: lands.clone(),
            pack: pack_of(tar_of_server("github")),
            say: None,
            stores: no_stores(),
        },
    )
    .await;
    assert_eq!(first.rows.last().unwrap().outcome, Outcome::Installed);
    let servers = provision_mcp(&g.machine, &mcp_plan_on(&root), mcp_options(&root, first.owned.clone())).await;
    assert_eq!(
        ids_and_outcomes(&servers),
        vec![(format!("{MCP_ID_PREFIX}claude/github"), Outcome::Installed), (format!("{MCP_ID_PREFIX}claude/gsc"), Outcome::Skipped)]
    );
    assert_eq!(servers[1].note.as_deref(), Some("not in the config that travelled"));
    close(&g, &once_paths_of(&lands)).await;
    let listed = fs::read_to_string(&place_provision_paths(&root).landed).unwrap();
    assert!(listed.contains(&format!("{MCP_ID_PREFIX}claude/github")));
    assert!(!listed.contains(".claude-cfg/.claude.json\t"));

    let failed = provision_files(
        &g.machine,
        ProvisionFilesAsk {
            home: root.clone(),
            lands: lands.clone(),
            pack: pack_that_fails("Keychain: user cancelled"),
            say: None,
            stores: no_stores(),
        },
    )
    .await;
    assert_eq!(outcomes(&failed.rows), lands.iter().map(|_| Outcome::Failed).collect::<Vec<_>>());
    assert!(failed.rows.iter().all(|r| r.note.as_deref() == Some("Keychain: user cancelled")));
    assert_eq!(failed.owned.len(), 0);

    close(&g, &once_paths_of(&lands)).await;
    assert_eq!(fs::read_to_string(&place_provision_paths(&root).landed).unwrap(), listed);
    let github = landed_servers(&g.machine, &root, quiet()).await;
    assert!(is_hex64(github.get(&format!("{MCP_ID_PREFIX}claude/github")).unwrap()));

    let again = provision_mcp(&g.machine, &mcp_plan_on(&root), mcp_options(&root, OwnedPaths::new())).await;
    assert_eq!(
        ids_and_outcomes(&again),
        vec![(format!("{MCP_ID_PREFIX}claude/github"), Outcome::Present), (format!("{MCP_ID_PREFIX}claude/gsc"), Outcome::Skipped)]
    );
    assert_eq!(again[1].note.as_deref(), Some(no_copy_line(&config).as_str()));
    assert!(!again.iter().any(|r| r.note.as_deref().unwrap_or("").contains("Claude Code's own")));
    assert!(fs::read_to_string(&config).unwrap().contains("github"));
}

// TS: packages/engine/test/provision-files.test.ts:425
// takes no key out of the list on a round whose copy of that agent's file never travelled
#[tokio::test(flavor = "multi_thread")]
async fn takes_no_key_out_of_the_list_on_a_round_whose_copy_of_that_agents_file_never_travelled() {
    let g = box_();
    let root = g.root_str();
    let at = place_provision_paths(&root);
    let lands = vec![
        landing("agents/claude", "Claude Code", ".claude-cfg/CLAUDE.md"),
        landing_once("agents/claude", "Claude Code", ".claude-cfg/.claude.json"),
    ];
    let both = tar(&[file(".claude-cfg/CLAUDE.md", "his standing rules\n"), file(".claude-cfg/.claude.json", &config_with("github"))]);
    let first = provision_files(
        &g.machine,
        ProvisionFilesAsk { home: root.clone(), lands: lands.clone(), pack: pack_of(both), say: None, stores: no_stores() },
    )
    .await;
    provision_mcp(&g.machine, &mcp_plan_on(&root), mcp_options(&root, first.owned.clone())).await;
    close(&g, &once_paths_of(&lands)).await;
    let listed = fs::read_to_string(&at.landed).unwrap();
    assert!(listed.contains(&format!("{MCP_ID_PREFIX}claude/github")));

    let only_md = tar(&[file(".claude-cfg/CLAUDE.md", "his standing rules\n")]);
    let again = provision_files(
        &g.machine,
        ProvisionFilesAsk { home: root.clone(), lands: lands.clone(), pack: pack_of(only_md), say: None, stores: no_stores() },
    )
    .await;
    let rows = provision_mcp(&g.machine, &mcp_plan_on(&root), mcp_options(&root, again.owned.clone())).await;
    assert_eq!(outcomes(&rows), vec![Outcome::Present, Outcome::Skipped]);
    close(&g, &once_paths_of(&lands)).await;
    assert_eq!(fs::read_to_string(&at.landed).unwrap(), listed);
}

// TS: packages/engine/test/provision-files.test.ts:445
// sweeps the markers the job's own log left in wsp's folder there, and keeps the files a person reads
#[tokio::test(flavor = "multi_thread")]
async fn sweeps_the_markers_the_jobs_own_log_left_in_wsps_folder_there_and_keeps_the_files_a_person_reads() {
    let g = box_();
    let at = place_provision_paths(&g.root_str());
    fs::create_dir_all(&at.dir).unwrap();
    for name in [at.log.clone(), format!("{}.appended", at.log), at.result.clone(), at.landed.clone()] {
        fs::write(name, "x\n").unwrap();
    }
    for mark in ["afff817b8c5d9", "afe821d3a8a49"] {
        fs::write(format!("{}.{mark}", at.log), "").unwrap();
    }
    land(&g, tar_all(Over::default()), &lands_all()).await;
    close(&g, &[]).await;
    let mut logs: Vec<String> =
        fs::read_dir(&at.dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).filter(|f| f.starts_with("log")).collect();
    logs.sort();
    assert_eq!(logs, vec!["log".to_string(), "log.appended".to_string()]);
    assert!(Path::new(&at.result).exists());
    assert!(Path::new(&at.landed).exists());
}

// TS: packages/engine/test/provision-files.test.ts:464
// puts what travels under wsp's own folder on that computer, never the folder every login there shares
#[tokio::test(flavor = "multi_thread")]
async fn puts_what_travels_under_wsps_own_folder_on_that_computer_never_the_folder_every_login_there_shares() {
    let g = box_();
    land(&g, tar_all(Over::default()), &lands_all()).await;
    let landed = g.landed();
    assert!(!landed.is_empty());
    let dir = place_provision_paths(&g.root_str()).dir;
    for path in landed {
        assert!(path.starts_with(&format!("{dir}/")), "{path}");
    }
}

fn stands() -> Vec<ProvisionLanding> {
    vec![
        landing("agents/claude", "Claude Code", ".claude-cfg/CLAUDE.md"),
        landing("agents/claude", "Claude Code", ".claude-cfg/skills/why/SKILL.md"),
        landing("agents/claude", "Claude Code", ".claude-cfg/skills/how/SKILL.md"),
        landing("agents/codex", "Codex", ".codex/AGENTS.md"),
        landing("agents/gemini", "Gemini CLI", ".gemini/GEMINI.md"),
        landing_once("agents/claude", "Claude Code", ".claude-cfg/.claude.json"),
    ]
}

const CONTENT: [(&str, &str); 6] = [
    (".claude-cfg/CLAUDE.md", "his standing rules\n"),
    (".claude-cfg/skills/why/SKILL.md", "the why skill\n"),
    (".claude-cfg/skills/how/SKILL.md", "the how skill\n"),
    (".codex/AGENTS.md", "the same rules for codex\n"),
    (".gemini/GEMINI.md", "the same rules again\n"),
    (".claude-cfg/.claude.json", "{}\n"),
];

fn content_paths() -> Vec<String> {
    CONTENT.iter().map(|(p, _)| p.to_string()).collect()
}

/// The lines of the list beside the job, path to the digest it says is standing there.
fn listed_in(root: &str) -> IndexMap<String, String> {
    lines_of(&place_provision_paths(root).landed)
        .iter()
        .map(|l| {
            let w: Vec<&str> = l.split('\t').collect();
            (w[0].to_string(), w[2].to_string())
        })
        .collect()
}

/// One round of the recipe's files and the close behind it, which is what leaves the list the next read uses.
async fn round(g: &BoxGuest, pack: PackFiles, say: FilesSay) -> LandFilesResult {
    let lands = stands();
    let landed = provision_files(
        &g.machine,
        ProvisionFilesAsk { home: g.root_str(), lands: lands.clone(), pack, say: Some(say), stores: no_stores() },
    )
    .await;
    close(g, &once_paths_of(&lands)).await;
    landed
}

// TS: packages/engine/test/provision-files.test.ts:512
// reads the digest of every path that stands there and the list's own line for it, off one xargs -0 sha256sum -z -- over
// the paths on the read's input (read on coreutils 9.4 and findutils 4.9.0)
#[tokio::test(flavor = "multi_thread")]
async fn reads_the_digest_of_every_path_that_stands_there_and_the_lists_own_line_for_it() {
    let g = box_();
    let root = g.root_str();
    let at = place_provision_paths(&root);
    let newline = ".claude-cfg/skills/two\nlines/SKILL.md";
    write(&g.root, ".claude-cfg/CLAUDE.md", "his standing rules\n");
    write(&g.root, ODD, "a skill with a backslash in its folder\n");
    write(&g.root, newline, "a skill under a folder with a newline in it\n");
    fs::create_dir_all(&at.dir).unwrap();
    fs::write(
        &at.landed,
        format!(
            ".claude-cfg/CLAUDE.md\twhat travelled\t{}\n{ODD}\twhat travelled\tan older digest\n{ODD}\twhat travelled\ta second line for the same path\n",
            digest("his standing rules\n")
        ),
    )
    .unwrap();

    let paths = [".claude-cfg/CLAUDE.md".to_string(), ODD.to_string(), newline.to_string(), ".codex/AGENTS.md".to_string()];
    let standing = standing_digests(&g.machine, &root, &paths).await.unwrap();
    assert_eq!(standing.at.get(".claude-cfg/CLAUDE.md"), Some(&digest("his standing rules\n")));
    assert_eq!(standing.at.get(ODD), Some(&digest("a skill with a backslash in its folder\n")));
    assert_eq!(standing.at.get(newline), Some(&digest("a skill under a folder with a newline in it\n")));
    assert!(!standing.at.contains_key(".codex/AGENTS.md"));
    assert_eq!(standing.listed.get(".claude-cfg/CLAUDE.md"), Some(&digest("his standing rules\n")));
    assert_eq!(standing.listed.get(ODD).map(String::as_str), Some("an older digest"));
    assert!(!standing.listed.contains_key(newline));
    let command = g.cmds().last().unwrap().clone();
    assert!(command.contains(STAND_MARK));
    assert!(!command.contains("CLAUDE.md"));
    assert!(!Path::new(&at.asked).exists());
}

fn packed_line_ok(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("packed on this computer: 6 paths, 3 files stand there already, 3 files packed, ") else {
        return false;
    };
    let Some(digits) = rest.strip_suffix(" B") else { return false };
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

// TS: packages/engine/test/provision-files.test.ts:543
// packs only what is not already there: three files stand at their digest with the list saying so and stay home, the one
// that differs and the one that is gone travel, and the file an agent keeps travels every run
#[tokio::test(flavor = "multi_thread")]
async fn packs_only_what_is_not_already_there() {
    let g = box_();
    let root = g.root_str();
    let first = packing(&CONTENT);
    round(&g, first.pack.clone(), quiet()).await;
    assert_eq!(first.sent(), vec![content_paths()]);

    write(&g.root, ".codex/AGENTS.md", "what he wrote on the box\n");
    fs::remove_file(g.root.join(".gemini/GEMINI.md")).unwrap();
    let at = place_provision_paths(&root);
    let line = format!(".claude-cfg/.claude.json\t{}\t{}\n", digest("{}\n"), digest("{}\n"));
    fs::write(&at.landed, format!("{}{line}", fs::read_to_string(&at.landed).unwrap())).unwrap();

    let said = Arc::new(Mutex::new(Vec::<String>::new()));
    let second = packing(&CONTENT);
    let landed = round(&g, second.pack.clone(), said_into(&said)).await;
    assert_eq!(
        second.sent()[0],
        vec![".codex/AGENTS.md".to_string(), ".gemini/GEMINI.md".to_string(), ".claude-cfg/.claude.json".to_string()]
    );
    let first_said = said.lock().unwrap()[0].clone();
    assert!(packed_line_ok(&first_said), "said {first_said:?}");
    assert_eq!(
        ids_and_outcomes(&landed.rows),
        vec![
            ("files/.claude-cfg/CLAUDE.md".to_string(), Outcome::Present),
            ("files/.claude-cfg/skills/why/SKILL.md".to_string(), Outcome::Present),
            ("files/.claude-cfg/skills/how/SKILL.md".to_string(), Outcome::Present),
            ("files/.codex/AGENTS.md".to_string(), Outcome::Skipped),
            ("files/.gemini/GEMINI.md".to_string(), Outcome::Installed),
            ("files/.claude-cfg/.claude.json".to_string(), Outcome::Present),
        ]
    );
    assert_eq!(landed.owned.get(&format!("{root}/.claude-cfg/CLAUDE.md")), Some(&OwnedHow::Present));
    assert_eq!(landed.skipped.iter().map(|s| s.path.clone()).collect::<Vec<_>>(), vec![format!("{root}/.codex/AGENTS.md")]);
    assert_eq!(read(&g.root, ".codex/AGENTS.md"), "what he wrote on the box\n");
    assert_eq!(read(&g.root, ".gemini/GEMINI.md"), "the same rules again\n");
    assert_eq!(listed_in(&root).get(".claude-cfg/CLAUDE.md"), Some(&digest("his standing rules\n")));
    let mut keys: Vec<String> = listed_in(&root).keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            ".claude-cfg/CLAUDE.md".to_string(),
            ".claude-cfg/skills/how/SKILL.md".to_string(),
            ".claude-cfg/skills/why/SKILL.md".to_string(),
            ".codex/AGENTS.md".to_string(),
            ".gemini/GEMINI.md".to_string(),
        ]
    );
}

// TS: packages/engine/test/provision-files.test.ts:584
// sends a file whose line in the list is older than the bytes standing there, and one the list does not name at all, so
// the close writes both lines again
#[tokio::test(flavor = "multi_thread")]
async fn sends_a_file_whose_line_in_the_list_is_older_than_the_bytes_standing_there_and_one_the_list_does_not_name() {
    let g = box_();
    let root = g.root_str();
    let at = place_provision_paths(&root);
    round(&g, packing(&CONTENT).pack, quiet()).await;

    let kept: Vec<String> = lines_of(&at.landed)
        .into_iter()
        .filter(|l| !l.starts_with(".claude-cfg/skills/how/SKILL.md\t"))
        .map(|l| {
            if l.starts_with(".claude-cfg/CLAUDE.md\t") {
                format!("{}\tan older digest", l.split('\t').take(2).collect::<Vec<_>>().join("\t"))
            } else {
                l
            }
        })
        .collect();
    fs::write(&at.landed, format!("{}\n", kept.join("\n"))).unwrap();

    let second = packing(&CONTENT);
    round(&g, second.pack.clone(), quiet()).await;
    assert_eq!(
        second.sent()[0],
        vec![".claude-cfg/CLAUDE.md".to_string(), ".claude-cfg/skills/how/SKILL.md".to_string(), ".claude-cfg/.claude.json".to_string()]
    );
    assert_eq!(listed_in(&root).get(".claude-cfg/CLAUDE.md"), Some(&digest("his standing rules\n")));
    assert_eq!(listed_in(&root).get(".claude-cfg/skills/how/SKILL.md"), Some(&digest("the how skill\n")));
}

// TS: packages/engine/test/provision-files.test.ts:607
// sends every file where the read itself did not happen: a link that threw, the daemon's own timer, no shell, and an
// answer with no mark of the read's
#[tokio::test(flavor = "multi_thread")]
async fn sends_every_file_where_the_read_itself_did_not_happen() {
    let g = box_();
    round(&g, packing(&CONTENT).pack, quiet()).await;
    let quiet_round = packing(&CONTENT);
    round(&g, quiet_round.pack.clone(), quiet()).await;
    assert_eq!(quiet_round.sent()[0], vec![".claude-cfg/.claude.json".to_string()]);

    let stands_line = format!(
        "{STAND_MARK}\t{d}\t.claude-cfg/CLAUDE.md\0{LEDGER_MARK}\t.claude-cfg/CLAUDE.md\t{d}\0",
        d = digest("his standing rules\n")
    );
    let answers: Vec<(&str, Result<ExecResult>)> = vec![
        ("the link threw", Err(other("spoo is not connected"))),
        ("the daemon's own timer cut it", Ok(res(EXEC_DEADLINE_EXIT, &stands_line, ""))),
        ("no shell ran it", Ok(res(127, &stands_line, "bash: not found\n"))),
        ("nothing of the read came back", Ok(res(0, "not a record of the read's\n", ""))),
    ];
    for (why, answer) in answers {
        let inner = g.machine.exec.clone();
        let broken = FakeMachine {
            exec: Arc::new(move |cmd, opts| {
                if cmd.contains(STAND_MARK) {
                    let answer = answer.clone();
                    Box::pin(async move { answer })
                } else {
                    inner(cmd, opts)
                }
            }),
            ..g.machine.clone()
        };
        let asked = packing(&CONTENT);
        provision_files(
            &broken,
            ProvisionFilesAsk { home: g.root_str(), lands: stands(), pack: asked.pack.clone(), say: Some(quiet()), stores: no_stores() },
        )
        .await;
        assert_eq!(asked.sent()[0], content_paths(), "{why}");
    }
}
