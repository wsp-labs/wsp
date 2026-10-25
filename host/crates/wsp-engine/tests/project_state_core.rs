// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/engine/test/project-state.test.ts lines 950-1087: sessions, entries, countProjectState and the
//! agentHomes and agentHome cases, against fixture homes on disk.
#![allow(dead_code, unused_imports)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use rusqlite::types::{Null, ToSql};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};

use wsp_engine::project_state::{project_state_resolvers, ProjectStateResolver};

const FROM: &str = "/private/tmp/wsp-r212/proj/b_2.x";
const TO: &str = "/root/work/b_2.x";
const OTHER: &str = "/Users/me/other";
const FROM_SUB: &str = "/private/tmp/wsp-r212/proj/b_2.x/sub";
const TO_SUB: &str = "/root/work/b_2.x/sub";
const DECOY: &str = "/private/tmp/wsp-r212/proj/b_2.x-old";

#[derive(Default)]
struct Roots(RefCell<Vec<tempfile::TempDir>>);
impl Roots {
    fn dir(&self) -> String {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().to_str().unwrap().to_string();
        self.0.borrow_mut().push(d);
        p
    }
}

fn join(base: &str, parts: &[&str]) -> String {
    let mut p = Path::new(base).to_path_buf();
    for part in parts {
        p.push(part);
    }
    p.to_str().unwrap().to_string()
}

fn write(file: &str, text: &str) {
    if let Some(parent) = Path::new(file).parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(file, text).unwrap();
}

fn lines(l: &[&str]) -> String {
    l.join("\n") + "\n"
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Text(String),
    Rows(BTreeMap<String, Vec<Value>>),
}

fn t(s: impl Into<String>) -> Node {
    Node::Text(s.into())
}

fn r(v: Value) -> Node {
    Node::Rows(serde_json::from_value(v).unwrap())
}

/// Every regular file under dir by relative path, text files as their bytes, sqlite files by their rows.
fn tree(dir: &str) -> BTreeMap<String, Node> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Node>) {
        for e in fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            let ft = e.file_type().unwrap();
            if ft.is_dir() {
                walk(base, &p, out);
            } else if ft.is_file() {
                let rel = p.strip_prefix(base).unwrap().to_str().unwrap().to_string();
                let name = e.file_name().to_str().unwrap().to_string();
                let node = if name.ends_with(".sqlite") || name.ends_with(".db") {
                    Node::Rows(rows(p.to_str().unwrap()))
                } else {
                    Node::Text(fs::read_to_string(&p).unwrap())
                };
                out.insert(rel, node);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(Path::new(dir), Path::new(dir), &mut out);
    out
}

fn rows(db: &str) -> BTreeMap<String, Vec<Value>> {
    let c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut out = BTreeMap::new();
    let names: Vec<String> = {
        let mut s = c.prepare("select name from sqlite_master where type = 'table' order by name").unwrap();
        let it = s.query_map([], |row| row.get::<_, String>(0)).unwrap();
        it.map(|n| n.unwrap()).collect()
    };
    for name in names {
        let mut s = c.prepare(&format!("select * from \"{name}\" order by 1, 2")).unwrap();
        let cols: Vec<String> = s.column_names().iter().map(|c| c.to_string()).collect();
        let it = s
            .query_map([], |row| {
                let mut m = serde_json::Map::new();
                for (i, col) in cols.iter().enumerate() {
                    let v = match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => Value::Null,
                        rusqlite::types::ValueRef::Integer(n) => json!(n),
                        rusqlite::types::ValueRef::Real(f) => json!(f),
                        rusqlite::types::ValueRef::Text(b) => Value::String(String::from_utf8_lossy(b).into_owned()),
                        rusqlite::types::ValueRef::Blob(b) => json!(b),
                    };
                    m.insert(col.clone(), v);
                }
                Ok(Value::Object(m))
            })
            .unwrap();
        out.insert(name, it.map(|v| v.unwrap()).collect());
    }
    out
}

struct Ins<'a>(&'a str, &'a [&'a dyn ToSql]);
fn ins<'a>(sql: &'a str, params: &'a [&'a dyn ToSql]) -> Ins<'a> {
    Ins(sql, params)
}

fn seed(db: &str, schema: &str, inserts: &[Ins]) {
    if let Some(parent) = Path::new(db).parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let c = Connection::open(db).unwrap();
    c.execute_batch(schema).unwrap();
    for Ins(sql, params) in inserts {
        c.execute(sql, rusqlite::params_from_iter(params.iter().copied())).unwrap();
    }
}

fn resolver_for(agent: &str) -> Arc<dyn ProjectStateResolver> {
    project_state_resolvers().get(agent).cloned().unwrap_or_else(|| panic!("no resolver for {agent}"))
}

fn homes_of(pairs: &[(&str, &str)]) -> indexmap::IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

// --- Claude Code ---

fn claude_user(cwd: &str) -> String {
    r#"{"type":"user","cwd":"@CWD@","sessionId":"S1","version":"2.1.257","message":{"role":"user","content":"hi"}}"#.replace("@CWD@", cwd)
}
fn claude_assistant(cwd: &str) -> String {
    r#"{"type":"assistant","cwd":"@CWD@","sessionId":"S1","message":{"role":"assistant","content":[]}}"#.replace("@CWD@", cwd)
}
const CLAUDE_SUMMARY: &str = r#"{"type":"summary","summary":"a scratch run","leafUuid":"u1"}"#;
fn claude_tail() -> String {
    r#"{"type":"user","cwd":"@CWD@","sessionId":"S1""#.replace("@CWD@", FROM)
}
fn claude_sub(cwd: &str) -> String {
    r#"{"type":"user","cwd":"@CWD@","sessionId":"S1","isSidechain":true,"message":{"role":"user","content":"look"}}"#.replace("@CWD@", cwd)
}

fn claude_home(root: &str) -> String {
    let home = join(root, &["claude"]);
    let key = join(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x"]);
    write(&join(&key, &["S1.jsonl"]), &[claude_user(FROM), claude_assistant(FROM), CLAUDE_SUMMARY.to_string(), claude_tail()].join("\n"));
    write(&join(&key, &["S1", "subagents", "agent-a1.jsonl"]), &lines(&[&claude_sub(FROM)]));
    write(&join(&key, &["S1", "tool-results", "t1.txt"]), "ls output\n");
    write(&join(&key, &["memory", "MEMORY.md"]), "notes\n");
    write(&join(&home, &["projects", "-Users-me-other", "S2.jsonl"]), &lines(&[&claude_user(OTHER)]));
    write(&join(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x-sub", "S3.jsonl"]), &lines(&[&claude_user(FROM_SUB)]));
    write(&join(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x-old", "S4.jsonl"]), &lines(&[&claude_user(DECOY)]));
    write(&join(&home, &["sessions", "4242.json"]), &format!("{{\"pid\":4242,\"cwd\":\"{FROM}\"}}\n"));
    write(&join(&home, &["history.jsonl"]), &lines(&[&format!("{{\"display\":\"hi\",\"project\":\"{FROM}\"}}")]));
    home
}

// --- Pi ---

fn pi_header(cwd: &str) -> String {
    r#"{"type":"session","version":3,"id":"s1","timestamp":"2026-09-05T21:56:00.000Z","cwd":"@CWD@"}"#.replace("@CWD@", cwd)
}
const PI_MESSAGE: &str = r#"{"type":"message","id":"m1","parentId":null,"timestamp":"2026-09-05T21:56:01.000Z","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}"#;

fn pi_home(root: &str) -> String {
    let home = join(root, &["pi"]);
    write(
        &join(&home, &["sessions", "--private-tmp-wsp-r212-proj-b_2.x--", "2026-09-05T21-56-00-000Z_s1.jsonl"]),
        &lines(&[&pi_header(FROM), PI_MESSAGE]),
    );
    write(&join(&home, &["sessions", "--Users-me-other--", "2026-09-05T21-50-00-000Z_s0.jsonl"]), &lines(&[&pi_header(OTHER), PI_MESSAGE]));
    write(
        &join(&home, &["sessions", "--private-tmp-wsp-r212-proj-b_2.x-sub--", "2026-09-05T21-57-00-000Z_s2.jsonl"]),
        &lines(&[&pi_header(FROM_SUB), PI_MESSAGE]),
    );
    write(
        &join(&home, &["sessions", "--private-tmp-wsp-r212-proj-b_2.x-old--", "2026-09-05T21-40-00-000Z_s3.jsonl"]),
        &lines(&[&pi_header(DECOY), PI_MESSAGE]),
    );
    write(&join(&home, &["trust.json"]), &format!("{{\"{FROM}\":true}}\n"));
    home
}

// --- Codex ---

fn codex_meta(cwd: &str) -> String {
    r#"{"timestamp":"2026-09-05T21:58:00.000Z","type":"session_meta","payload":{"id":"t1","timestamp":"2026-09-05T21:58:00.000Z","cwd":"@CWD@","originator":"codex_exec","cli_version":"0.153.0"}}"#.replace("@CWD@", cwd)
}
const CODEX_ITEM: &str = r#"{"timestamp":"2026-09-05T21:58:00.100Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hi"}]}}"#;
fn codex_turn(cwd: &str) -> String {
    r#"{"timestamp":"2026-09-05T21:58:00.200Z","type":"turn_context","payload":{"cwd":"@CWD@","approval_policy":"never","model":"gpt-5"}}"#
        .replace("@CWD@", cwd)
}
const CODEX_EVENT: &str = r#"{"timestamp":"2026-09-05T21:58:01.000Z","type":"event_msg","payload":{"type":"error","message":"401"}}"#;
const CODEX_SCHEMA: &str = "create table threads (id text primary key, rollout_path text not null, cwd text not null, archived integer not null default 0, updated_at integer not null default 0)";

fn codex_home(root: &str) -> String {
    let home = join(root, &["codex"]);
    let t1 = join(&home, &["sessions", "2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]);
    let t2 = join(&home, &["sessions", "2026", "09", "04", "rollout-2026-09-04T10-00-00-t2.jsonl"]);
    let t3 = join(&home, &["sessions", "2026", "09", "05", "rollout-2026-09-05T22-00-00-t3.jsonl"]);
    let t4 = join(&home, &["sessions", "2026", "09", "05", "rollout-2026-09-05T22-01-00-t4.jsonl"]);
    write(&t1, &lines(&[&codex_meta(FROM), CODEX_ITEM, &codex_turn(FROM), CODEX_EVENT]));
    write(&t2, &lines(&[&codex_meta(OTHER), CODEX_ITEM, &codex_turn(OTHER)]));
    write(&t3, &lines(&[&codex_meta(FROM_SUB), CODEX_ITEM, &codex_turn(FROM_SUB)]));
    write(&t4, &lines(&[&codex_meta(DECOY), CODEX_ITEM, &codex_turn(DECOY)]));
    seed(
        &join(&home, &["state_5.sqlite"]),
        CODEX_SCHEMA,
        &[
            ins("insert into threads values (?, ?, ?, 0, 1)", &[&"t1", &t1, &FROM]),
            ins("insert into threads values (?, ?, ?, 0, 2)", &[&"t2", &t2, &OTHER]),
            ins("insert into threads values (?, ?, ?, 0, 3)", &[&"t3", &t3, &FROM_SUB]),
            ins("insert into threads values (?, ?, ?, 0, 4)", &[&"t4", &t4, &DECOY]),
        ],
    );
    write(&join(&home, &["config.toml"]), &format!("[projects.\"{FROM}\"]\ntrust_level = \"trusted\"\n"));
    home
}

// --- Hermes ---

const HERMES_SCHEMA: &str = "create table sessions (id text primary key, cwd text, git_repo_root text, message_count integer); create table messages (id integer primary key, session_id text, content text)";

fn hermes_home(root: &str) -> String {
    let home = join(root, &["hermes"]);
    seed(
        &join(&home, &["state.db"]),
        HERMES_SCHEMA,
        &[
            ins("insert into sessions values (?, ?, ?, 2)", &[&"20260906_033144_55e3e2", &FROM, &FROM]),
            ins("insert into sessions values (?, ?, ?, 1)", &[&"20260906_030000_aaaaaa", &OTHER, &OTHER]),
            ins("insert into sessions values (?, ?, ?, 1)", &[&"20260906_034000_bbbbbb", &FROM, &Null]),
            ins("insert into sessions values (?, ?, ?, 1)", &[&"20260906_035000_cccccc", &FROM_SUB, &FROM]),
            ins("insert into sessions values (?, ?, ?, 1)", &[&"20260906_036000_dddddd", &DECOY, &DECOY]),
            ins("insert into messages values (1, ?, 'hi')", &[&"20260906_033144_55e3e2"]),
            ins("insert into messages values (2, ?, 'there')", &[&"20260906_033144_55e3e2"]),
            ins("insert into messages values (3, ?, 'other')", &[&"20260906_030000_aaaaaa"]),
        ],
    );
    write(&join(&home, &["sessions", "request_dump_20260906_033144_55e3e2_1.json"]), "{}\n");
    home
}

// --- Gemini ---

fn gemini_registry(path: &str) -> String {
    let sub = format!("{path}/sub");
    let mut m = serde_json::Map::new();
    m.insert(path.to_string(), json!("b_2.x"));
    m.insert(OTHER.to_string(), json!("other"));
    m.insert(sub, json!("sub"));
    m.insert(DECOY.to_string(), json!("b_2.x-old"));
    serde_json::to_string_pretty(&json!({ "projects": m })).unwrap() + "\n"
}
const GEMINI_CHAT: &str =
    "{\"sessionId\":\"a1b2c3d4-0000\",\"projectHash\":\"9d0b5e0f\",\"startTime\":\"2026-09-05T21:59:00.000Z\",\"messages\":[]}\n";

fn gemini_home(root: &str) -> String {
    let home = join(root, &["gemini"]);
    write(&join(&home, &["projects.json"]), &gemini_registry(FROM));
    write(&join(&home, &["tmp", "b_2.x", ".project_root"]), FROM);
    write(&join(&home, &["tmp", "b_2.x", "chats", "session-2026-09-05T21-59-a1b2c3d4.jsonl"]), GEMINI_CHAT);
    write(&join(&home, &["history", "b_2.x", ".project_root"]), &format!("{FROM}\n"));
    write(&join(&home, &["tmp", "other", ".project_root"]), OTHER);
    write(&join(&home, &["tmp", "sub", ".project_root"]), FROM_SUB);
    write(&join(&home, &["tmp", "b_2.x-old", ".project_root"]), DECOY);
    write(&join(&home, &["trustedFolders.json"]), &format!("{{\"{FROM}\":\"TRUST_FOLDER\"}}\n"));
    home
}

// --- OpenCode ---

const HASH: &str = "9f1e2d3c4b5a69788796a5b4c3d2e1f0a1b2c3d4";
const OPENCODE_SCHEMA: &str = "create table project (id text primary key, worktree text not null, vcs text, name text, sandboxes text, time_created integer);create table project_directory (project_id text not null, directory text not null, primary key (project_id, directory));create table session (id text primary key, project_id text not null, directory text not null, title text);create table message (id text primary key, session_id text not null, role text);create table part (id text primary key, message_id text not null, text text)";

fn opencode_home(root: &str) -> String {
    let home = join(root, &["opencode"]);
    let sandboxes = format!("[\"{FROM}-copy\"]");
    seed(
        &join(&home, &["opencode.db"]),
        OPENCODE_SCHEMA,
        &[
            ins("insert into project values (?, ?, 'git', 'b_2.x', ?, 1)", &[&HASH, &FROM, &sandboxes]),
            ins("insert into project values ('global', '/', null, null, '[]', 0)", &[]),
            ins("insert into project values ('deadbeef', ?, 'git', 'b_2.x-old', '[]', 2)", &[&DECOY]),
            ins("insert into project_directory values (?, ?)", &[&HASH, &FROM]),
            ins("insert into project_directory values (?, ?)", &[&HASH, &FROM_SUB]),
            ins("insert into project_directory values ('deadbeef', ?)", &[&DECOY]),
            ins("insert into project_directory values ('global', ?)", &[&"/Users/me/scratch"]),
            ins("insert into session values ('ses_1', ?, ?, 'first')", &[&HASH, &FROM]),
            ins("insert into session values ('ses_2', 'global', ?, 'loose')", &[&"/Users/me/scratch"]),
            ins("insert into session values ('ses_3', ?, ?, 'deeper')", &[&HASH, &FROM_SUB]),
            ins("insert into session values ('ses_4', 'deadbeef', ?, 'sibling')", &[&DECOY]),
            ins("insert into message values ('msg_1', 'ses_1', 'user')", &[]),
            ins("insert into message values ('msg_2', 'ses_2', 'user')", &[]),
            ins("insert into part values ('prt_1', 'msg_1', 'hi')", &[]),
            ins("insert into part values ('prt_2', 'msg_2', 'loose')", &[]),
        ],
    );
    home
}

use std::os::unix::fs::symlink;

use indexmap::IndexMap;

use wsp_engine::project_state::{agent_home, agent_homes, count_project_state, guest_agent_homes};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::project_bundle::{ProjectAgent, ProjectCarry};

fn catalog_ids() -> Vec<String> {
    wsp_catalog::catalog::catalog_agents().iter().map(|a| a.id.clone()).collect()
}

fn catalog_pairs() -> Vec<(String, String)> {
    wsp_catalog::catalog::catalog_agents().iter().map(|a| (a.id.clone(), a.name.clone())).collect()
}

fn bytes_of(files: &[String]) -> f64 {
    files.iter().map(|f| fs::metadata(f).unwrap().len() as f64).sum()
}

fn row(agent: &str, name: &str, sessions: f64, bytes: f64, carry: ProjectCarry, error: Option<String>) -> ProjectAgent {
    ProjectAgent { agent: agent.into(), name: name.into(), sessions: JsNumber(sessions), bytes: JsNumber(bytes), carry, error }
}

// --- sessions ---

// TS: packages/engine/test/project-state.test.ts:951
#[tokio::test]
async fn counts_every_agents_sessions_at_the_path_or_under_it_never_a_sibling_another_project_or_a_subagent() {
    let roots = Roots::default();
    let root = roots.dir();
    assert_eq!(resolver_for("claude").sessions(&claude_home(&root), FROM).await.unwrap(), 2);
    assert_eq!(resolver_for("pi").sessions(&pi_home(&root), FROM).await.unwrap(), 2);
    assert_eq!(resolver_for("codex").sessions(&codex_home(&root), FROM).await.unwrap(), 2);
    assert_eq!(resolver_for("hermes").sessions(&hermes_home(&root), FROM).await.unwrap(), 3);
    assert_eq!(resolver_for("gemini").sessions(&gemini_home(&root), FROM).await.unwrap(), 1);
    assert_eq!(resolver_for("opencode").sessions(&opencode_home(&root), FROM).await.unwrap(), 2);
}

// TS: packages/engine/test/project-state.test.ts:961
#[tokio::test]
async fn counts_one_for_a_sub_folder_alone_and_nothing_for_an_unknown_path_or_an_empty_home() {
    let roots = Roots::default();
    let root = roots.dir();
    let homes = [
        ("claude", claude_home(&root)),
        ("pi", pi_home(&root)),
        ("codex", codex_home(&root)),
        ("hermes", hermes_home(&root)),
        ("gemini", gemini_home(&root)),
        ("opencode", opencode_home(&root)),
    ];
    assert_eq!(resolver_for("claude").sessions(&homes[0].1, FROM_SUB).await.unwrap(), 1);
    assert_eq!(resolver_for("codex").sessions(&homes[2].1, FROM_SUB).await.unwrap(), 1);
    for (agent, home) in &homes {
        assert_eq!(resolver_for(agent).sessions(home, "/Users/me/never").await.unwrap(), 0, "{agent}");
        let empty = join(&root, &[&format!("{agent}-empty")]);
        fs::create_dir(&empty).unwrap();
        assert_eq!(resolver_for(agent).sessions(&empty, FROM).await.unwrap(), 0, "{agent}");
    }
}

// TS: packages/engine/test/project-state.test.ts:974
#[tokio::test]
async fn a_claude_code_directory_holding_memory_alone_is_zero_sessions() {
    let roots = Roots::default();
    let home = join(&roots.dir(), &["claude"]);
    write(&join(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x", "memory", "MEMORY.md"]), "notes\n");
    assert_eq!(resolver_for("claude").sessions(&home, FROM).await.unwrap(), 0);
}

// --- entries ---

// TS: packages/engine/test/project-state.test.ts:982
#[tokio::test]
async fn names_the_files_holding_the_projects_state_alone_never_a_shared_index_or_another_projects() {
    let roots = Roots::default();
    let root = roots.dir();
    let claude = claude_home(&root);
    let key = join(&claude, &["projects", "-private-tmp-wsp-r212-proj-b-2-x"]);
    assert_eq!(
        resolver_for("claude").entries(&claude, FROM).await.unwrap(),
        vec![
            join(&key, &["S1.jsonl"]),
            join(&key, &["S1", "subagents", "agent-a1.jsonl"]),
            join(&key, &["S1", "tool-results", "t1.txt"]),
            join(&key, &["memory", "MEMORY.md"]),
            join(&claude, &["projects", "-private-tmp-wsp-r212-proj-b-2-x-sub", "S3.jsonl"]),
        ]
    );
    let pi = pi_home(&root);
    assert_eq!(
        resolver_for("pi").entries(&pi, FROM).await.unwrap(),
        vec![
            join(&pi, &["sessions", "--private-tmp-wsp-r212-proj-b_2.x--", "2026-09-05T21-56-00-000Z_s1.jsonl"]),
            join(&pi, &["sessions", "--private-tmp-wsp-r212-proj-b_2.x-sub--", "2026-09-05T21-57-00-000Z_s2.jsonl"]),
        ]
    );
    let codex = codex_home(&root);
    assert_eq!(
        resolver_for("codex").entries(&codex, FROM).await.unwrap(),
        vec![
            join(&codex, &["sessions", "2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]),
            join(&codex, &["sessions", "2026", "09", "05", "rollout-2026-09-05T22-00-00-t3.jsonl"]),
        ]
    );
    let gemini = gemini_home(&root);
    assert_eq!(
        resolver_for("gemini").entries(&gemini, FROM).await.unwrap(),
        vec![
            join(&gemini, &["history", "b_2.x", ".project_root"]),
            join(&gemini, &["tmp", "b_2.x", ".project_root"]),
            join(&gemini, &["tmp", "b_2.x", "chats", "session-2026-09-05T21-59-a1b2c3d4.jsonl"]),
            join(&gemini, &["tmp", "sub", ".project_root"]),
        ]
    );
    assert_eq!(resolver_for("hermes").entries(&hermes_home(&root), FROM).await.unwrap(), Vec::<String>::new());
    assert_eq!(resolver_for("opencode").entries(&opencode_home(&root), FROM).await.unwrap(), Vec::<String>::new());
    for (agent, home) in [("claude", &claude), ("pi", &pi), ("codex", &codex), ("gemini", &gemini)] {
        assert_eq!(resolver_for(agent).entries(home, "/Users/me/never").await.unwrap(), Vec::<String>::new(), "{agent}");
    }
}

// --- countProjectState ---

// TS: packages/engine/test/project-state.test.ts:1019
#[tokio::test]
async fn lists_each_agent_whose_home_holds_sessions_with_its_catalog_name_in_catalog_order() {
    let roots = Roots::default();
    let root = roots.dir();
    let claude = claude_home(&root);
    let pi = pi_home(&root);
    let gemini = join(&root, &["gemini-empty"]);
    fs::create_dir(&gemini).unwrap();
    let no_such = join(&root, &["no-such-home"]);
    let rows =
        count_project_state(FROM, &homes_of(&[("claude", &claude), ("pi", &pi), ("gemini", &gemini), ("codex", &no_such)]), None).await;
    let claude_bytes = bytes_of(&resolver_for("claude").entries(&claude, FROM).await.unwrap());
    let pi_bytes = bytes_of(&resolver_for("pi").entries(&pi, FROM).await.unwrap());
    assert_eq!(
        rows,
        vec![
            row("claude", "Claude Code", 2.0, claude_bytes, ProjectCarry::Moves, None),
            row("pi", "Pi", 2.0, pi_bytes, ProjectCarry::Moves, None),
        ]
    );
    assert!(rows[0].bytes.0 > rows[1].bytes.0);
    let rows2 = count_project_state(
        FROM,
        &homes_of(&[
            ("codex", &codex_home(&root)),
            ("gemini", &gemini_home(&root)),
            ("hermes", &hermes_home(&root)),
            ("opencode", &opencode_home(&root)),
        ]),
        None,
    )
    .await;
    let seen: Vec<(String, f64, ProjectCarry, bool)> =
        rows2.iter().map(|r| (r.agent.clone(), r.sessions.0, r.carry, r.bytes.0 > 0.0)).collect();
    assert_eq!(
        seen,
        vec![
            ("codex".to_string(), 2.0, ProjectCarry::TranscriptOnly, true),
            ("gemini".to_string(), 1.0, ProjectCarry::TranscriptOnly, true),
            ("opencode".to_string(), 2.0, ProjectCarry::TranscriptOnly, false),
            ("hermes".to_string(), 3.0, ProjectCarry::TranscriptOnly, false),
        ]
    );
}

// TS: packages/engine/test/project-state.test.ts:1040
#[tokio::test]
async fn keys_on_the_real_path_and_skips_a_catalog_agent_with_no_module_whose_home_it_never_opens() {
    let roots = Roots::default();
    let root = roots.dir();
    let claude = claude_home(&root);
    fs::create_dir_all(join(&root, &["real", "proj"])).unwrap();
    symlink(join(&root, &["real"]), join(&root, &["link"])).unwrap();
    let real = fs::canonicalize(join(&root, &["real", "proj"])).unwrap().to_str().unwrap().to_string();
    let key: String = real.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    write(&join(&claude, &["projects", &key, "S7.jsonl"]), &lines(&[&claude_user(&real)]));
    let hermes = hermes_home(&root);
    let before = tree(&hermes);
    let mut agents = catalog_pairs();
    agents.push(("newagent".to_string(), "New".to_string()));
    let rows = count_project_state(
        &(join(&root, &["link", "proj"]) + "/"),
        &homes_of(&[("claude", &claude), ("newagent", &hermes)]),
        Some(&agents),
    )
    .await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows, vec![row("claude", "Claude Code", 1.0, rows[0].bytes.0, ProjectCarry::Moves, None)]);
    assert_eq!(tree(&hermes), before);
}

// TS: packages/engine/test/project-state.test.ts:1054
#[tokio::test]
async fn an_agent_whose_store_cannot_be_read_keeps_its_row_with_the_reason_and_the_others_are_counted() {
    let roots = Roots::default();
    let root = roots.dir();
    let claude = claude_home(&root);
    let opencode = join(&root, &["opencode"]);
    write(&join(&opencode, &["opencode.db"]), "not a database\n");
    let pi = pi_home(&root);
    let rows = count_project_state(FROM, &homes_of(&[("claude", &claude), ("opencode", &opencode), ("pi", &pi)]), None).await;
    assert_eq!(rows.len(), 3);
    let error = rows[1].error.clone().expect("the opencode row carries the reason");
    assert!(error.contains("not a database"), "{error}");
    assert_eq!(
        rows,
        vec![
            row("claude", "Claude Code", 2.0, rows[0].bytes.0, ProjectCarry::Moves, None),
            row("opencode", "OpenCode", 0.0, 0.0, ProjectCarry::TranscriptOnly, Some(error)),
            row("pi", "Pi", 2.0, rows[2].bytes.0, ProjectCarry::Moves, None),
        ]
    );
}

// TS: packages/engine/test/project-state.test.ts:1067
#[test]
fn agent_homes_places_every_registered_agents_home_under_the_given_home_directory() {
    let homes = agent_homes("/Users/me", None);
    assert_eq!(homes.keys().cloned().collect::<Vec<_>>(), catalog_ids());
    assert_eq!(homes["claude"], "/Users/me/.claude");
    assert_eq!(homes["opencode"], "/Users/me/.local/share/opencode");
    assert_eq!(homes["pi"], "/Users/me/.pi/agent");
    let guest = guest_agent_homes();
    assert_eq!(guest.keys().cloned().collect::<Vec<_>>(), catalog_ids());
    assert_eq!(guest["claude"], "/root/.claude-cfg");
    assert_eq!(guest["codex"], "/root/.codex");
}

// TS: packages/engine/test/project-state.test.ts:1079
#[test]
fn one_harness_home_under_a_home_directory_is_one_rule_the_persons_variable_wins_else_the_catalogs() {
    assert_eq!(agent_home("/Users/me", "claude", None), "/Users/me/.claude");
    let vars: IndexMap<String, String> = [("CLAUDE_CONFIG_DIR".to_string(), "/Users/me/.claude-cfg".to_string())].into_iter().collect();
    assert_eq!(agent_home("/Users/me", "claude", Some(&vars)), "/Users/me/.claude-cfg");
    assert_eq!(agent_home("/root", "codex", None), "/root/.codex");
    assert_eq!(agent_home("/root", "nobody", None), "/root/.nobody");
}
