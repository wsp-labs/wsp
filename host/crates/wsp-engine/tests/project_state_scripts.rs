// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/engine/test/project-state.test.ts lines 692-948 (the listing the machine runs) and 1089-1385 (the
//! merge on the machine): the python3 scripts the engine emits run for real over fixture homes.
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

fn all_resolvers() -> Vec<Arc<dyn ProjectStateResolver>> {
    project_state_resolvers().values().cloned().collect()
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

use std::io::Write as _;
use std::process::{Command, Stdio};

use base64::Engine as _;
use indexmap::IndexMap;

use wsp_engine::project_state::merge::merge_script;
use wsp_engine::project_state::py::PY_PREAMBLE;
use wsp_engine::project_state::{
    guest_agent_homes, parse_merge_output, parse_state_listing, state_listing, store_copy, MergeOutput, UnreadStore,
};

fn catalog_ids() -> Vec<String> {
    wsp_catalog::catalog::catalog_agents().iter().map(|a| a.id.clone()).collect()
}

/// A gemini registry as the agent writes it: two-space JSON, keys in the order given, a trailing newline.
fn registry(pairs: &[(&str, &str)]) -> String {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), json!(v));
    }
    serde_json::to_string_pretty(&json!({ "projects": m })).unwrap() + "\n"
}

/// The registry in one line, as a test seeds a store the agent has not pretty-printed.
fn registry_compact(pairs: &[(&str, &str)]) -> String {
    let mut m = serde_json::Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), json!(v));
    }
    serde_json::to_string(&json!({ "projects": m })).unwrap() + "\n"
}

#[derive(Debug, Clone, PartialEq)]
struct Listing {
    exit: i32,
    paths: Vec<String>,
    unread: Vec<UnreadStore>,
    err: String,
}

fn ids_of(agent: &str) -> Vec<String> {
    vec![agent.to_string()]
}

/// Runs one agent's listing the way the machine does: the command state_listing builds, in a shell, with `into` the
/// directory on the machine the filtered copies are written under.
fn run_listing(agent: &str, home: &str, path: &str, into: &str) -> Listing {
    let command = state_listing(&homes_of(&[(agent, home)]), path, into, Some(&ids_of(agent))).unwrap();
    let out = Command::new("bash").arg("-c").arg(&command).output().unwrap();
    let parsed = parse_state_listing(&String::from_utf8_lossy(&out.stdout)).unwrap();
    Listing {
        exit: out.status.code().unwrap_or(-1),
        paths: parsed.paths,
        unread: parsed.unread,
        err: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// The python3 source of one agent's listing, off the command the machine would run.
fn listing_source(agent: &str, home: &str, path: &str, into: &str) -> String {
    let command = state_listing(&homes_of(&[(agent, home)]), path, into, Some(&ids_of(agent))).unwrap();
    let opens = "set -e\npython3 -c '";
    command[opens.len()..command.len() - 1].replace("'\\''", "'")
}

/// The peak resident memory of the python3 a listing runs, in MB, beside the paths it named: the script is exec'd
/// inside a wrapper that reads its own rusage once it ends.
fn listing_peak(agent: &str, home: &str, path: &str, into: &str) -> (f64, Vec<String>) {
    let wrapper = [
        "import resource, sys",
        "exec(compile(sys.stdin.read(), \"listing\", \"exec\"), {})",
        "sys.stderr.write(str(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / (1024 * 1024 if sys.platform == \"darwin\" else 1024)))",
    ]
    .join("\n");
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(&wrapper)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(listing_source(agent, home, path, into).as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "the listing failed (exit {:?}): {err}", out.status.code());
    let paths = String::from_utf8_lossy(&out.stdout).split('\n').filter(|l| !l.is_empty()).map(|l| l.to_string()).collect();
    (err.trim().parse().unwrap(), paths)
}

/// The store shape the cold review measured on: one Hermes store holding two projects with `per` messages of 2 KB
/// each, a session in a folder under the project, and the sibling sharing the project's prefix.
fn shared_hermes_store(root: &str, per: usize) -> String {
    let home = join(root, &["hermes-shared"]);
    let db = join(&home, &["state.db"]);
    fs::create_dir_all(&home).unwrap();
    let mut c = Connection::open(&db).unwrap();
    c.execute_batch("pragma page_size = 8192").unwrap();
    c.execute_batch(HERMES_SCHEMA).unwrap();
    let body = "m".repeat(2048);
    let tx = c.transaction().unwrap();
    for (id, cwd, n) in [("a1", FROM, per), ("a2", FROM_SUB, 1), ("b1", OTHER, per), ("o1", DECOY, 1)] {
        tx.execute("insert into sessions values (?, ?, ?, ?)", rusqlite::params![id, cwd, cwd, n as i64]).unwrap();
        let mut stmt = tx.prepare("insert into messages (session_id, content) values (?, ?)").unwrap();
        for _ in 0..n {
            stmt.execute(rusqlite::params![id, body]).unwrap();
        }
    }
    tx.commit().unwrap();
    home
}

/// Where a module's shared store is copied to for a trip: the store's own path under the scratch root.
fn copy_of(agent: &str, home: &str, into: &str) -> String {
    let shared = resolver_for(agent).shared().unwrap_or_else(|| panic!("{agent} keeps no store for every project"));
    store_copy(into, home, &shared.store)
}

fn count(c: &Connection, table: &str) -> i64 {
    c.query_row(&format!("select count(*) from {table}"), [], |r| r.get(0)).unwrap()
}

fn open_ro(db: &str) -> Connection {
    Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

fn carried(value: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(value).unwrap())
}

// --- the listing the machine runs ---

// TS: packages/engine/test/project-state.test.ts:693
#[test]
fn each_module_names_the_paths_under_its_home_it_reads_and_the_store_it_keeps_is_one_of_them() {
    let mut roots_by_agent: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut stores: BTreeMap<String, String> = BTreeMap::new();
    for r in all_resolvers().iter() {
        roots_by_agent.insert(r.agent().to_string(), r.roots().iter().map(|s| s.to_string()).collect());
        if let Some(s) = r.shared() {
            stores.insert(r.agent().to_string(), s.store);
        }
    }
    let want = |pairs: &[(&str, &[&str])]| -> BTreeMap<String, Vec<String>> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect())).collect()
    };
    assert_eq!(
        roots_by_agent,
        want(&[
            ("claude", &["projects"]),
            ("codex", &["state_5.sqlite", "sessions"]),
            ("gemini", &["projects.json", "tmp", "history"]),
            ("hermes", &["state.db"]),
            ("opencode", &["opencode.db"]),
            ("pi", &["sessions"]),
        ])
    );
    let want_stores: BTreeMap<String, String> =
        [("codex", "state_5.sqlite"), ("gemini", "projects.json"), ("hermes", "state.db"), ("opencode", "opencode.db")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
    assert_eq!(stores, want_stores);
    for r in all_resolvers().iter() {
        let roots = r.roots();
        for root in &roots {
            assert!(!root.starts_with('/') && !root.contains(".."), "{}: {root}", r.agent());
        }
        if let Some(s) = r.shared() {
            assert!(roots.contains(&s.store.as_str()), "{}", r.agent());
        }
    }
}

// TS: packages/engine/test/project-state.test.ts:714
#[test]
fn brings_down_the_projects_own_entries_and_a_filtered_copy_of_each_shared_store_never_another_projects() {
    let roots = Roots::default();
    let root = roots.dir();
    let into = roots.dir();
    let claude = claude_home(&root);
    assert_eq!(
        run_listing("claude", &claude, FROM, &into),
        Listing {
            exit: 0,
            err: String::new(),
            unread: vec![],
            paths: vec![
                join(&claude, &["projects", "-private-tmp-wsp-r212-proj-b-2-x"]),
                join(&claude, &["projects", "-private-tmp-wsp-r212-proj-b-2-x-sub"])
            ],
        }
    );
    let pi = pi_home(&root);
    assert_eq!(
        run_listing("pi", &pi, FROM, &into).paths,
        vec![
            join(&pi, &["sessions", "--private-tmp-wsp-r212-proj-b_2.x--"]),
            join(&pi, &["sessions", "--private-tmp-wsp-r212-proj-b_2.x-sub--"])
        ]
    );
    let codex = codex_home(&root);
    assert_eq!(
        run_listing("codex", &codex, FROM, &into).paths,
        vec![
            copy_of("codex", &codex, &into),
            join(&codex, &["sessions", "2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]),
            join(&codex, &["sessions", "2026", "09", "05", "rollout-2026-09-05T22-00-00-t3.jsonl"]),
        ]
    );
    let gemini = gemini_home(&root);
    assert_eq!(
        run_listing("gemini", &gemini, FROM, &into).paths,
        vec![
            copy_of("gemini", &gemini, &into),
            join(&gemini, &["tmp", "b_2.x"]),
            join(&gemini, &["history", "b_2.x"]),
            join(&gemini, &["tmp", "sub"])
        ]
    );
    let hermes = hermes_home(&root);
    assert_eq!(run_listing("hermes", &hermes, FROM, &into).paths, vec![copy_of("hermes", &hermes, &into)]);
    let opencode = opencode_home(&root);
    assert_eq!(run_listing("opencode", &opencode, FROM, &into).paths, vec![copy_of("opencode", &opencode, &into)]);
}

// TS: packages/engine/test/project-state.test.ts:748
#[test]
fn codex_the_copy_of_the_index_holds_the_projects_threads_alone_and_the_store_on_the_machine_is_not_touched() {
    let roots = Roots::default();
    let root = roots.dir();
    let into = roots.dir();
    let home = codex_home(&root);
    let before = tree(&home);
    assert_eq!(run_listing("codex", &home, FROM, &into).exit, 0);
    let t1 = join(&home, &["sessions", "2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]);
    let t3 = join(&home, &["sessions", "2026", "09", "05", "rollout-2026-09-05T22-00-00-t3.jsonl"]);
    let expected: BTreeMap<String, Vec<Value>> = serde_json::from_value(json!({
        "threads": [
            { "id": "t1", "rollout_path": t1, "cwd": FROM, "archived": 0, "updated_at": 1 },
            { "id": "t3", "rollout_path": t3, "cwd": FROM_SUB, "archived": 0, "updated_at": 3 },
        ]
    }))
    .unwrap();
    assert_eq!(rows(&copy_of("codex", &home, &into)), expected);
    assert_eq!(tree(&home), before);
}

// TS: packages/engine/test/project-state.test.ts:763
#[test]
fn hermes_the_copy_of_the_store_holds_the_projects_sessions_and_their_messages_alone() {
    let roots = Roots::default();
    let root = roots.dir();
    let into = roots.dir();
    let home = hermes_home(&root);
    let before = tree(&home);
    assert_eq!(run_listing("hermes", &home, FROM, &into).exit, 0);
    let expected: BTreeMap<String, Vec<Value>> = serde_json::from_value(json!({
        "sessions": [
            { "id": "20260906_033144_55e3e2", "cwd": FROM, "git_repo_root": FROM, "message_count": 2 },
            { "id": "20260906_034000_bbbbbb", "cwd": FROM, "git_repo_root": null, "message_count": 1 },
            { "id": "20260906_035000_cccccc", "cwd": FROM_SUB, "git_repo_root": FROM, "message_count": 1 },
        ],
        "messages": [
            { "id": 1, "session_id": "20260906_033144_55e3e2", "content": "hi" },
            { "id": 2, "session_id": "20260906_033144_55e3e2", "content": "there" },
        ]
    }))
    .unwrap();
    assert_eq!(rows(&copy_of("hermes", &home, &into)), expected);
    assert_eq!(tree(&home), before);
}

// TS: packages/engine/test/project-state.test.ts:783
#[test]
fn opencode_the_copy_of_the_store_holds_the_projects_rows_in_every_table_its_messages_and_parts_alone() {
    let roots = Roots::default();
    let root = roots.dir();
    let into = roots.dir();
    let home = opencode_home(&root);
    let before = tree(&home);
    assert_eq!(run_listing("opencode", &home, FROM, &into).exit, 0);
    let expected: BTreeMap<String, Vec<Value>> = serde_json::from_value(json!({
        "project": [{ "id": HASH, "worktree": FROM, "vcs": "git", "name": "b_2.x", "sandboxes": format!("[\"{FROM}-copy\"]"), "time_created": 1 }],
        "project_directory": [{ "project_id": HASH, "directory": FROM }, { "project_id": HASH, "directory": FROM_SUB }],
        "session": [{ "id": "ses_1", "project_id": HASH, "directory": FROM, "title": "first" }, { "id": "ses_3", "project_id": HASH, "directory": FROM_SUB, "title": "deeper" }],
        "message": [{ "id": "msg_1", "session_id": "ses_1", "role": "user" }],
        "part": [{ "id": "prt_1", "message_id": "msg_1", "text": "hi" }],
    }))
    .unwrap();
    assert_eq!(rows(&copy_of("opencode", &home, &into)), expected);
    assert_eq!(tree(&home), before);
}

// TS: packages/engine/test/project-state.test.ts:799
#[test]
fn gemini_the_copy_of_the_registry_holds_the_projects_keys_alone_in_the_shape_the_agent_writes() {
    let roots = Roots::default();
    let root = roots.dir();
    let into = roots.dir();
    let home = gemini_home(&root);
    let before = tree(&home);
    assert_eq!(run_listing("gemini", &home, FROM, &into).exit, 0);
    assert_eq!(fs::read_to_string(copy_of("gemini", &home, &into)).unwrap(), registry(&[(FROM, "b_2.x"), (FROM_SUB, "sub")]));
    assert_eq!(tree(&home), before);
}

// TS: packages/engine/test/project-state.test.ts:809
#[test]
fn a_second_trip_through_the_same_scratch_root_writes_the_copy_again_rather_than_into_the_one_there() {
    let roots = Roots::default();
    let root = roots.dir();
    let into = roots.dir();
    let home = hermes_home(&root);
    let first = run_listing("hermes", &home, FROM, &into);
    let copied = rows(&copy_of("hermes", &home, &into));
    assert_eq!(run_listing("hermes", &home, FROM, &into), first);
    assert_eq!(rows(&copy_of("hermes", &home, &into)), copied);
}

// TS: packages/engine/test/project-state.test.ts:819
#[test]
fn a_store_whose_schema_holds_a_virtual_table_is_filtered_all_the_same() {
    let roots = Roots::default();
    let root = roots.dir();
    let into = roots.dir();
    let home = join(&root, &["hermes"]);
    seed(
        &join(&home, &["state.db"]),
        "create table sessions (id text primary key, cwd text, git_repo_root text, message_count integer); create virtual table notes using fts5(body); create table messages (id integer primary key, session_id text, content text)",
        &[
            ins("insert into sessions values (?, ?, ?, 1)", &[&"s1", &FROM, &FROM]),
            ins("insert into sessions values (?, ?, ?, 1)", &[&"s2", &OTHER, &OTHER]),
            ins("insert into notes values ('a note')", &[]),
        ],
    );
    let run = run_listing("hermes", &home, FROM, &into);
    assert_eq!(run.exit, 0);
    assert_eq!(run.paths, vec![copy_of("hermes", &home, &into)]);
    assert_eq!(run.unread, vec![]);
    let d = open_ro(&copy_of("hermes", &home, &into));
    let sessions: Vec<Value> = {
        let mut s = d.prepare("select id, cwd, git_repo_root, message_count from sessions order by id").unwrap();
        let it = s
            .query_map([], |row| Ok(json!({ "id": row.get::<_, String>(0)?, "cwd": row.get::<_, String>(1)?, "git_repo_root": row.get::<_, String>(2)?, "message_count": row.get::<_, i64>(3)? })))
            .unwrap();
        it.map(|v| v.unwrap()).collect()
    };
    assert_eq!(sessions, vec![json!({ "id": "s1", "cwd": FROM, "git_repo_root": FROM, "message_count": 1 })]);
    assert_eq!(count(&d, "notes"), 0);
}

// TS: packages/engine/test/project-state.test.ts:838
#[test]
fn the_filter_streams_the_projects_slice_into_the_copy_peak_memory_stays_a_fraction_of_the_rows_it_writes() {
    let roots = Roots::default();
    let into = roots.dir();
    let home = shared_hermes_store(&roots.dir(), 120_000);
    let (peak_mb, paths) = listing_peak("hermes", &home, FROM, &into);
    assert_eq!(paths, vec![copy_of("hermes", &home, &into)]);
    let copied = open_ro(&copy_of("hermes", &home, &into));
    assert_eq!(count(&copied, "sessions"), 2);
    assert_eq!(count(&copied, "messages"), 120_001);
    // 120001 rows of 2 KB is a 246 MB slice: fetchall of it peaked at 277.75 MB on this box, the fetchmany loop at 20.75 MB.
    assert!(peak_mb < 64.0, "peak {peak_mb} MB");
}

// TS: packages/engine/test/project-state.test.ts:855
#[test]
fn names_nothing_for_a_path_no_agent_ran_in_a_store_not_made_and_a_home_that_is_not_there() {
    let roots = Roots::default();
    let root = roots.dir();
    let into = roots.dir();
    let claude = claude_home(&root);
    let never = "/Users/me/never";
    let none = Vec::<String>::new();
    assert_eq!(run_listing("claude", &claude, never, &into).paths, none);
    assert_eq!(run_listing("pi", &pi_home(&root), never, &into).paths, none);
    assert_eq!(run_listing("codex", &codex_home(&root), never, &into).paths, none);
    assert_eq!(run_listing("gemini", &gemini_home(&root), never, &into).paths, none);
    assert_eq!(run_listing("hermes", &hermes_home(&root), never, &into).paths, none);
    assert_eq!(run_listing("opencode", &opencode_home(&root), never, &into).paths, none);
    // A home the agent made but never wrote its store in: its sessions tree is not the answer for the project.
    let bare = join(&root, &["codex-bare"]);
    write(&join(&bare, &["sessions", "2026", "09", "05", "rollout-other.jsonl"]), &lines(&[&codex_meta(OTHER)]));
    assert_eq!(run_listing("codex", &bare, FROM, &into).paths, none);
    for agent in ["claude", "pi", "codex", "gemini", "hermes", "opencode"] {
        assert_eq!(run_listing(agent, &join(&root, &[&format!("{agent}-gone")]), FROM, &into).paths, none, "{agent}");
    }
    // The sibling sharing the project's prefix is its own project, named only when it is the one asked for.
    assert_eq!(
        run_listing("claude", &claude, DECOY, &into).paths,
        vec![join(&claude, &["projects", "-private-tmp-wsp-r212-proj-b-2-x-old"])]
    );
}

// TS: packages/engine/test/project-state.test.ts:875
#[test]
fn a_source_with_a_trailing_slash_or_a_dot_segment_as_the_wire_carries_it_names_the_same_paths() {
    let roots = Roots::default();
    let claude = claude_home(&roots.dir());
    let into = roots.dir();
    let plain = run_listing("claude", &claude, FROM, &into).paths;
    assert_eq!(plain.len(), 2);
    assert_eq!(run_listing("claude", &claude, &format!("{FROM}/"), &into).paths, plain);
    assert_eq!(run_listing("claude", &claude, &format!("{FROM}/./"), &into).paths, plain);
}

// TS: packages/engine/test/project-state.test.ts:884
#[test]
fn a_store_on_the_machine_it_cannot_read_brings_nothing_from_that_home_and_names_the_store_and_why() {
    let roots = Roots::default();
    let root = roots.dir();
    let into = roots.dir();
    let mut homes: Vec<(&str, String)> = Vec::new();
    let codex = codex_home(&root);
    homes.push(("codex", codex.clone()));
    write(&join(&codex, &["state_5.sqlite"]), "not a database\n");
    // Never the index and never the sessions/ tree beside it, which holds every other project's rollouts.
    assert_eq!(
        run_listing("codex", &codex, FROM, &into),
        Listing {
            exit: 0,
            err: String::new(),
            paths: vec![],
            unread: vec![UnreadStore {
                agent: "codex".into(),
                store: join(&codex, &["state_5.sqlite"]),
                why: "file is not a database".into()
            }],
        }
    );
    let gemini = gemini_home(&root);
    homes.push(("gemini", gemini.clone()));
    write(&join(&gemini, &["projects.json"]), "{ not json\n");
    let registry_run = run_listing("gemini", &gemini, FROM, &into);
    // Never the registry and never the whole of tmp/ and history/, which hold every other project's chats.
    assert_eq!(registry_run.paths, Vec::<String>::new());
    assert_eq!(registry_run.unread.len(), 1);
    assert_eq!(registry_run.unread[0].agent, "gemini");
    assert_eq!(registry_run.unread[0].store, join(&gemini, &["projects.json"]));
    assert!(registry_run.unread[0].why.contains("Expecting property name"), "{}", registry_run.unread[0].why);
    let hermes = hermes_home(&root);
    homes.push(("hermes", hermes.clone()));
    write(&join(&hermes, &["state.db"]), "not a database\n");
    assert_eq!(
        run_listing("hermes", &hermes, FROM, &into),
        Listing {
            exit: 0,
            err: String::new(),
            paths: vec![],
            unread: vec![UnreadStore { agent: "hermes".into(), store: join(&hermes, &["state.db"]), why: "file is not a database".into() }],
        }
    );
    let opencode = opencode_home(&root);
    homes.push(("opencode", opencode.clone()));
    write(&join(&opencode, &["opencode.db"]), "not a database\n");
    assert_eq!(
        run_listing("opencode", &opencode, FROM, &into).unread,
        vec![UnreadStore { agent: "opencode".into(), store: join(&opencode, &["opencode.db"]), why: "file is not a database".into() }]
    );
    // Nor a half written copy under the scratch root, which the trip would have pulled at the store's own path.
    for (agent, home) in &homes {
        assert!(!Path::new(&copy_of(agent, home, &into)).exists(), "{agent}");
    }
}

// TS: packages/engine/test/project-state.test.ts:919
#[test]
fn every_listing_and_the_merge_open_with_the_one_preamble_so_the_under_rule_is_defined_once() {
    let definitions = |script: &str| script.matches("def under(").count();
    let roots = Roots::default();
    let into = roots.dir();
    for r in all_resolvers().iter() {
        let agent = r.agent();
        let script = listing_source(agent, &join("/root", &[&format!(".{agent}")]), FROM, &into);
        assert!(script.starts_with(PY_PREAMBLE), "{agent}");
        assert_eq!(definitions(&script), 1, "{agent}");
    }
    let merge = merge_script(FROM, TO, &[]);
    assert!(merge.starts_with(PY_PREAMBLE));
    assert_eq!(definitions(&merge), 1);
}

// TS: packages/engine/test/project-state.test.ts:932
#[test]
fn state_listing_runs_one_python3_per_agent_with_a_home_and_a_module_narrows_and_refuses_an_unknown_id() {
    let homes = guest_agent_homes();
    let roots = Roots::default();
    let into = roots.dir();
    let runs = |command: &str| command.split("\npython3 -c '").count() - 1;
    let command = state_listing(&homes, FROM, &into, None).unwrap();
    assert!(command.starts_with("set -e\npython3 -c '"));
    assert_eq!(runs(&command), all_resolvers().len());
    for (id, home) in &homes {
        assert_eq!(command.contains(&carried(home)), all_resolvers().iter().any(|r| r.agent() == id.as_str()), "{id}");
    }
    assert!(command.contains(&carried(FROM)));
    for r in all_resolvers().iter() {
        if let Some(s) = r.shared() {
            assert!(command.contains(&carried(&store_copy(&into, &homes[r.agent()], &s.store))), "{}", r.agent());
        }
    }
    let two = ["pi".to_string(), "claude".to_string()];
    assert_eq!(runs(&state_listing(&homes, FROM, &into, Some(&two)).unwrap()), 2);
    assert_eq!(runs(&state_listing(&homes_of(&[("claude", "/x/.claude")]), FROM, &into, None).unwrap()), 1);
    assert_eq!(state_listing(&IndexMap::new(), FROM, &into, None).unwrap(), "");
    let bad = ["claude".to_string(), "codx".to_string()];
    let err = state_listing(&homes, FROM, &into, Some(&bad)).unwrap_err();
    assert!(err.to_string().contains(&format!("no agent called codx; the catalog knows {}", catalog_ids().join(", "))), "{err}");
}

// --- the merge on the machine ---

#[derive(Debug, PartialEq)]
struct MergeRun {
    exit: i32,
    out: Option<MergeOutput>,
    err: String,
}

/// Runs the script as the runtime does on the machine: python3 over a file, the last stdout line as the result.
fn run_merge(script: &str) -> MergeRun {
    let dir = tempfile::tempdir().unwrap();
    let file = join(dir.path().to_str().unwrap(), &["merge.py"]);
    fs::write(&file, script).unwrap();
    let out = Command::new("python3").arg(&file).output().unwrap();
    MergeRun {
        exit: out.status.code().unwrap_or(-1),
        out: parse_merge_output(&String::from_utf8_lossy(&out.stdout)).ok(),
        err: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn counts(merged: u64, kept: u64) -> Option<MergeOutput> {
    Some(MergeOutput::Merged { merged: merged as f64, kept: kept as f64, note: None })
}

async fn merge_for(agent: &str, home: &str, from: &str, to: &str, guest: &str) -> Option<String> {
    let resolver = resolver_for(agent);
    assert!(resolver.has_merge(), "{agent} has no merge");
    resolver.merge(home, from, to, guest).await.unwrap()
}

/// A machine's Codex home after the overlay landed: its own thread for another project, and the project's rollouts
/// as they left this computer, cwd unrewritten because the skeleton had no index to name them.
fn codex_guest(root: &str) -> String {
    let guest = join(root, &["guest-codex"]);
    let t2 = join(&guest, &["sessions", "2026", "09", "04", "rollout-2026-09-04T10-00-00-t2.jsonl"]);
    write(&t2, &lines(&[&codex_meta(OTHER), CODEX_ITEM, &codex_turn(OTHER)]));
    write(
        &join(&guest, &["sessions", "2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]),
        &lines(&[&codex_meta(FROM), CODEX_ITEM, &codex_turn(FROM), CODEX_EVENT]),
    );
    write(
        &join(&guest, &["sessions", "2026", "09", "05", "rollout-2026-09-05T22-00-00-t3.jsonl"]),
        &lines(&[&codex_meta(FROM_SUB), CODEX_ITEM, &codex_turn(FROM_SUB)]),
    );
    seed(&join(&guest, &["state_5.sqlite"]), CODEX_SCHEMA, &[ins("insert into threads values (?, ?, ?, 0, 7)", &[&"t2", &t2, &OTHER])]);
    guest
}

// TS: packages/engine/test/project-state.test.ts:1126
#[tokio::test]
async fn codex_merge_lands_thread_rows_at_machine_paths_fixes_cwd_keeps_machine_rows_and_a_second_run_changes_nothing() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = codex_home(&root);
    let guest = codex_guest(&root);
    seed(
        &join(&mac, &["state_5.sqlite"]),
        "",
        &[ins(
            "insert into threads values ('t5', ?, ?, 1, 5)",
            &[&join(&mac, &["archived_sessions", "rollout-2026-09-01T09-00-00-t5.jsonl"]), &FROM],
        )],
    );
    let before = tree(&mac);
    let script = merge_for("codex", &mac, FROM, TO, &guest).await.expect("no script");
    assert_eq!(run_merge(&script), MergeRun { exit: 0, out: counts(2, 0), err: String::new() });
    let t1 = join(&guest, &["sessions", "2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]);
    let t2 = join(&guest, &["sessions", "2026", "09", "04", "rollout-2026-09-04T10-00-00-t2.jsonl"]);
    let t3 = join(&guest, &["sessions", "2026", "09", "05", "rollout-2026-09-05T22-00-00-t3.jsonl"]);
    let after: BTreeMap<String, Node> = BTreeMap::from([
        (
            "sessions/2026/09/04/rollout-2026-09-04T10-00-00-t2.jsonl".to_string(),
            t(lines(&[&codex_meta(OTHER), CODEX_ITEM, &codex_turn(OTHER)])),
        ),
        (
            "sessions/2026/09/05/rollout-2026-09-05T21-58-00-t1.jsonl".to_string(),
            t(lines(&[&codex_meta(TO), CODEX_ITEM, &codex_turn(TO), CODEX_EVENT])),
        ),
        (
            "sessions/2026/09/05/rollout-2026-09-05T22-00-00-t3.jsonl".to_string(),
            t(lines(&[&codex_meta(TO_SUB), CODEX_ITEM, &codex_turn(TO_SUB)])),
        ),
        (
            "state_5.sqlite".to_string(),
            r(json!({
                "threads": [
                    { "id": "t1", "rollout_path": t1, "cwd": TO, "archived": 0, "updated_at": 1 },
                    { "id": "t2", "rollout_path": t2, "cwd": OTHER, "archived": 0, "updated_at": 7 },
                    { "id": "t3", "rollout_path": t3, "cwd": TO_SUB, "archived": 0, "updated_at": 3 },
                ]
            })),
        ),
    ]);
    assert_eq!(tree(&guest), after);
    assert_eq!(run_merge(&script), MergeRun { exit: 0, out: counts(0, 2), err: String::new() });
    assert_eq!(tree(&guest), after);
    assert_eq!(tree(&mac), before);
}

// TS: packages/engine/test/project-state.test.ts:1156
#[tokio::test]
async fn codex_a_rollout_line_that_is_not_utf8_passes_through_byte_for_byte_while_the_cwd_lines_are_rewritten() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = codex_home(&root);
    let guest = codex_guest(&root);
    let t1 = join(&guest, &["sessions", "2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]);
    let mut raw: Vec<u8> = b"{\"type\":\"raw\",\"data\":\"".to_vec();
    raw.extend_from_slice(&[0xff, 0xfe]);
    raw.extend_from_slice(b"\"}\n");
    let mut f = fs::OpenOptions::new().append(true).open(&t1).unwrap();
    f.write_all(&raw).unwrap();
    drop(f);
    let script = merge_for("codex", &mac, FROM, TO, &guest).await;
    assert_eq!(run_merge(&script.unwrap()), MergeRun { exit: 0, out: counts(2, 0), err: String::new() });
    let mut want = lines(&[&codex_meta(TO), CODEX_ITEM, &codex_turn(TO), CODEX_EVENT]).into_bytes();
    want.extend_from_slice(&raw);
    assert_eq!(fs::read(&t1).unwrap(), want);
}

// TS: packages/engine/test/project-state.test.ts:1168
#[tokio::test]
async fn codex_a_thread_the_machine_already_lists_keeps_its_own_columns_and_gets_only_cwd_and_rollout_path() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = codex_home(&root);
    let guest = codex_guest(&root);
    let t1 = join(&guest, &["sessions", "2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]);
    seed(
        &join(&guest, &["state_5.sqlite"]),
        "",
        &[ins(
            "insert into threads values ('t1', ?, ?, 1, 9)",
            &[&"/Users/me/.codex/sessions/2026/09/05/rollout-2026-09-05T21-58-00-t1.jsonl", &FROM],
        )],
    );
    let script = merge_for("codex", &mac, FROM, TO, &guest).await;
    assert_eq!(run_merge(&script.unwrap()).out, counts(2, 0));
    let Node::Rows(db) = tree(&guest).remove("state_5.sqlite").unwrap() else { panic!("not a database") };
    let threads = &db["threads"];
    assert_eq!(threads.len(), 3);
    assert_eq!(threads[0], json!({ "id": "t1", "rollout_path": t1, "cwd": TO, "archived": 1, "updated_at": 9 }));
    assert_eq!(threads[1]["id"], "t2");
    assert_eq!(threads[2]["id"], "t3");
}

// TS: packages/engine/test/project-state.test.ts:1183
#[tokio::test]
async fn a_machine_without_the_store_waits_and_gets_none_a_store_missing_a_column_fails_and_is_left_alone() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = codex_home(&root);
    let empty = join(&root, &["guest-empty"]);
    fs::create_dir(&empty).unwrap();
    let script = merge_for("codex", &mac, FROM, TO, &empty).await;
    assert_eq!(
        run_merge(&script.unwrap()),
        MergeRun {
            exit: 0,
            out: Some(MergeOutput::Waiting { waiting: format!("no {} on the machine", join(&empty, &["state_5.sqlite"])) }),
            err: String::new()
        }
    );
    assert_eq!(tree(&empty), BTreeMap::new());
    let old = join(&root, &["guest-old"]);
    seed(
        &join(&old, &["state_5.sqlite"]),
        "create table threads (id text primary key, rollout_path text not null, cwd text not null)",
        &[ins("insert into threads values ('t2', 'x', ?)", &[&OTHER])],
    );
    let before = tree(&old);
    let run = run_merge(&merge_for("codex", &mac, FROM, TO, &old).await.unwrap());
    assert_eq!(run.exit, 1);
    assert_eq!(run.out, None);
    assert!(run.err.contains("no column named archived"), "{}", run.err);
    assert_eq!(tree(&old), before);
}

// TS: packages/engine/test/project-state.test.ts:1201
#[tokio::test]
async fn hermes_sessions_land_with_cwd_and_root_moved_messages_under_the_machines_ids_and_its_rows_stay() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = hermes_home(&root);
    let guest = join(&root, &["guest-hermes"]);
    seed(
        &join(&guest, &["state.db"]),
        HERMES_SCHEMA,
        &[
            ins("insert into sessions values (?, ?, ?, 1)", &[&"20260906_030000_aaaaaa", &"/root/other", &"/root/other"]),
            ins("insert into messages values (1, ?, 'theirs')", &[&"20260906_030000_aaaaaa"]),
        ],
    );
    let script = merge_for("hermes", &mac, FROM, TO, &guest).await.unwrap();
    assert_eq!(run_merge(&script), MergeRun { exit: 0, out: counts(5, 0), err: String::new() });
    let after: BTreeMap<String, Node> = BTreeMap::from([(
        "state.db".to_string(),
        r(json!({
            "sessions": [
                { "id": "20260906_030000_aaaaaa", "cwd": "/root/other", "git_repo_root": "/root/other", "message_count": 1 },
                { "id": "20260906_033144_55e3e2", "cwd": TO, "git_repo_root": TO, "message_count": 2 },
                { "id": "20260906_034000_bbbbbb", "cwd": TO, "git_repo_root": null, "message_count": 1 },
                { "id": "20260906_035000_cccccc", "cwd": TO_SUB, "git_repo_root": TO, "message_count": 1 },
            ],
            "messages": [
                { "id": 1, "session_id": "20260906_030000_aaaaaa", "content": "theirs" },
                { "id": 2, "session_id": "20260906_033144_55e3e2", "content": "hi" },
                { "id": 3, "session_id": "20260906_033144_55e3e2", "content": "there" },
            ]
        })),
    )]);
    assert_eq!(tree(&guest), after);
    assert_eq!(run_merge(&script).out, counts(0, 5));
    assert_eq!(tree(&guest), after);
}

// TS: packages/engine/test/project-state.test.ts:1231
#[tokio::test]
async fn hermes_a_session_whose_messages_the_machine_holds_keeps_them_even_when_this_computer_has_more() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = hermes_home(&root);
    let guest = join(&root, &["guest-hermes"]);
    seed(
        &join(&guest, &["state.db"]),
        HERMES_SCHEMA,
        &[
            ins("insert into sessions values (?, ?, ?, 1)", &[&"20260906_033144_55e3e2", &TO, &TO]),
            ins("insert into messages values (7, ?, 'edited here')", &[&"20260906_033144_55e3e2"]),
        ],
    );
    let script = merge_for("hermes", &mac, FROM, TO, &guest).await.unwrap();
    assert_eq!(run_merge(&script).out, counts(2, 3));
    let Node::Rows(db) = tree(&guest).remove("state.db").unwrap() else { panic!("not a database") };
    assert_eq!(db["messages"], vec![json!({ "id": 7, "session_id": "20260906_033144_55e3e2", "content": "edited here" })]);
}

// TS: packages/engine/test/project-state.test.ts:1243
#[tokio::test]
async fn opencode_project_keeps_its_id_takes_the_machine_path_sandboxes_cleared_rows_land_global_stays() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = opencode_home(&root);
    let guest = join(&root, &["guest-opencode"]);
    let elsewhere_copy = r#"["/root/elsewhere/b_2.x-copy"]"#;
    seed(
        &join(&guest, &["opencode.db"]),
        OPENCODE_SCHEMA,
        &[
            ins("insert into project values (?, '/root/elsewhere/b_2.x', 'git', 'b_2.x', ?, 5)", &[&HASH, &elsewhere_copy]),
            ins("insert into project values ('global', '/', null, null, '[]', 0)", &[]),
            ins("insert into project_directory values ('global', '/root/scratch')", &[]),
            ins("insert into session values ('ses_9', 'global', '/root/scratch', 'theirs')", &[]),
            ins("insert into message values ('msg_9', 'ses_9', 'user')", &[]),
            ins("insert into part values ('prt_9', 'msg_9', 'theirs')", &[]),
        ],
    );
    let script = merge_for("opencode", &mac, FROM, TO, &guest).await.unwrap();
    assert_eq!(run_merge(&script), MergeRun { exit: 0, out: counts(7, 0), err: String::new() });
    let after: BTreeMap<String, Node> = BTreeMap::from([(
        "opencode.db".to_string(),
        r(json!({
            "project": [
                { "id": HASH, "worktree": TO, "vcs": "git", "name": "b_2.x", "sandboxes": "[]", "time_created": 5 },
                { "id": "global", "worktree": "/", "vcs": null, "name": null, "sandboxes": "[]", "time_created": 0 },
            ],
            "project_directory": [
                { "project_id": HASH, "directory": TO },
                { "project_id": HASH, "directory": TO_SUB },
                { "project_id": "global", "directory": "/root/scratch" },
            ],
            "session": [
                { "id": "ses_1", "project_id": HASH, "directory": TO, "title": "first" },
                { "id": "ses_3", "project_id": HASH, "directory": TO_SUB, "title": "deeper" },
                { "id": "ses_9", "project_id": "global", "directory": "/root/scratch", "title": "theirs" },
            ],
            "message": [
                { "id": "msg_1", "session_id": "ses_1", "role": "user" },
                { "id": "msg_9", "session_id": "ses_9", "role": "user" },
            ],
            "part": [
                { "id": "prt_1", "message_id": "msg_1", "text": "hi" },
                { "id": "prt_9", "message_id": "msg_9", "text": "theirs" },
            ]
        })),
    )]);
    assert_eq!(tree(&guest), after);
    assert_eq!(run_merge(&script).out, counts(0, 7));
    assert_eq!(tree(&guest), after);
}

// TS: packages/engine/test/project-state.test.ts:1288
#[tokio::test]
async fn gemini_registry_keys_land_with_their_slugs_markers_get_the_machine_path_a_machine_without_a_registry_gets_one() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = gemini_home(&root);
    let guest = join(&root, &["guest-gemini"]);
    write(&join(&guest, &["projects.json"]), &registry(&[("/root/other", "other")]));
    write(&join(&guest, &["tmp", "other", ".project_root"]), "/root/other");
    write(&join(&guest, &["tmp", "b_2.x", ".project_root"]), FROM);
    write(&join(&guest, &["tmp", "b_2.x", "chats", "session-2026-09-05T21-59-a1b2c3d4.jsonl"]), GEMINI_CHAT);
    write(&join(&guest, &["history", "b_2.x", ".project_root"]), &format!("{FROM}\n"));
    write(&join(&guest, &["tmp", "sub", ".project_root"]), FROM_SUB);
    let script = merge_for("gemini", &mac, FROM, TO, &guest).await.unwrap();
    assert_eq!(run_merge(&script), MergeRun { exit: 0, out: counts(2, 0), err: String::new() });
    let after: BTreeMap<String, Node> = BTreeMap::from([
        ("projects.json".to_string(), t(registry(&[("/root/other", "other"), (TO, "b_2.x"), (TO_SUB, "sub")]))),
        ("tmp/other/.project_root".to_string(), t("/root/other")),
        ("tmp/b_2.x/.project_root".to_string(), t(TO)),
        ("tmp/b_2.x/chats/session-2026-09-05T21-59-a1b2c3d4.jsonl".to_string(), t(GEMINI_CHAT)),
        ("history/b_2.x/.project_root".to_string(), t(format!("{TO}\n"))),
        ("tmp/sub/.project_root".to_string(), t(TO_SUB)),
    ]);
    assert_eq!(tree(&guest), after);
    assert_eq!(run_merge(&script).out, counts(0, 2));
    assert_eq!(tree(&guest), after);
    let bare = join(&root, &["guest-bare"]);
    assert_eq!(run_merge(&merge_for("gemini", &mac, FROM, TO, &bare).await.unwrap()).out, counts(2, 0));
    assert_eq!(tree(&bare), BTreeMap::from([("projects.json".to_string(), t(registry(&[(TO, "b_2.x"), (TO_SUB, "sub")])))]));
}

// TS: packages/engine/test/project-state.test.ts:1316
#[tokio::test]
async fn gemini_a_slug_another_path_on_the_machine_owns_fails_naming_it_and_the_registry_is_left_alone() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = gemini_home(&root);
    let guest = join(&root, &["guest-gemini"]);
    let reg = registry_compact(&[("/root/other/b_2.x", "b_2.x")]);
    write(&join(&guest, &["projects.json"]), &reg);
    let run = run_merge(&merge_for("gemini", &mac, FROM, TO, &guest).await.unwrap());
    assert_eq!(
        run,
        MergeRun {
            exit: 1,
            out: None,
            err: format!("slug b_2.x already belongs to /root/other/b_2.x in {}\n", join(&guest, &["projects.json"]))
        }
    );
    assert_eq!(tree(&guest), BTreeMap::from([("projects.json".to_string(), t(reg))]));
}

// TS: packages/engine/test/project-state.test.ts:1327
#[tokio::test]
async fn gemini_a_destination_the_machine_maps_to_its_own_slug_keeps_that_slug_and_counts_kept_with_a_note() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = gemini_home(&root);
    let guest = join(&root, &["guest-gemini"]);
    write(&join(&guest, &["projects.json"]), &registry(&[(TO, "proj")]));
    write(&join(&guest, &["tmp", "proj", ".project_root"]), TO);
    write(&join(&guest, &["tmp", "proj", "chats", "session-2026-09-06T01-00-ffffffff.jsonl"]), GEMINI_CHAT);
    write(&join(&guest, &["tmp", "b_2.x", ".project_root"]), FROM);
    write(&join(&guest, &["tmp", "b_2.x", "chats", "session-2026-09-05T21-59-a1b2c3d4.jsonl"]), GEMINI_CHAT);
    write(&join(&guest, &["tmp", "sub", ".project_root"]), FROM_SUB);
    let script = merge_for("gemini", &mac, FROM, TO, &guest).await.unwrap();
    let note =
        format!("the machine already lists {TO} as proj, so that slug stays and the carried chats under tmp/b_2.x are not listed there");
    assert_eq!(
        run_merge(&script),
        MergeRun { exit: 0, out: Some(MergeOutput::Merged { merged: 1.0, kept: 1.0, note: Some(note.clone()) }), err: String::new() }
    );
    let after: BTreeMap<String, Node> = BTreeMap::from([
        ("projects.json".to_string(), t(registry(&[(TO, "proj"), (TO_SUB, "sub")]))),
        ("tmp/proj/.project_root".to_string(), t(TO)),
        ("tmp/proj/chats/session-2026-09-06T01-00-ffffffff.jsonl".to_string(), t(GEMINI_CHAT)),
        ("tmp/b_2.x/.project_root".to_string(), t(TO)),
        ("tmp/b_2.x/chats/session-2026-09-05T21-59-a1b2c3d4.jsonl".to_string(), t(GEMINI_CHAT)),
        ("tmp/sub/.project_root".to_string(), t(TO_SUB)),
    ]);
    assert_eq!(tree(&guest), after);
    assert_eq!(run_merge(&script).out, Some(MergeOutput::Merged { merged: 0.0, kept: 2.0, note: Some(note) }));
    assert_eq!(tree(&guest), after);
}

// TS: packages/engine/test/project-state.test.ts:1353
#[tokio::test]
async fn gemini_a_later_keys_slug_conflict_fails_before_any_marker_of_an_earlier_key_is_rewritten() {
    let roots = Roots::default();
    let root = roots.dir();
    let mac = gemini_home(&root);
    let guest = join(&root, &["guest-gemini"]);
    let reg = registry_compact(&[("/root/other/sub", "sub")]);
    write(&join(&guest, &["projects.json"]), &reg);
    write(&join(&guest, &["tmp", "b_2.x", ".project_root"]), FROM);
    let run = run_merge(&merge_for("gemini", &mac, FROM, TO, &guest).await.unwrap());
    assert_eq!(
        run,
        MergeRun {
            exit: 1,
            out: None,
            err: format!("slug sub already belongs to /root/other/sub in {}\n", join(&guest, &["projects.json"]))
        }
    );
    assert_eq!(tree(&guest), BTreeMap::from([("projects.json".to_string(), t(reg)), ("tmp/b_2.x/.project_root".to_string(), t(FROM))]));
}

// TS: packages/engine/test/project-state.test.ts:1365
#[tokio::test]
async fn a_home_with_no_row_for_the_path_emits_no_script_and_the_agents_whose_files_carry_every_key_have_no_merge() {
    let roots = Roots::default();
    let root = roots.dir();
    for (agent, home) in
        [("codex", codex_home(&root)), ("hermes", hermes_home(&root)), ("opencode", opencode_home(&root)), ("gemini", gemini_home(&root))]
    {
        assert_eq!(merge_for(agent, &home, "/Users/me/never", TO, "/root/x").await, None, "{agent}");
        let empty = join(&root, &[&format!("{agent}-empty")]);
        fs::create_dir(&empty).unwrap();
        assert_eq!(merge_for(agent, &empty, FROM, TO, "/root/x").await, None, "{agent}");
    }
    assert!(!resolver_for("claude").has_merge());
    assert!(!resolver_for("pi").has_merge());
}

// TS: packages/engine/test/project-state.test.ts:1377
#[test]
fn the_scripts_result_is_its_last_line_anything_else_is_an_error_naming_what_came_back() {
    assert_eq!(
        parse_merge_output("noise\n{\"merged\": 2, \"kept\": 1}\n").unwrap(),
        MergeOutput::Merged { merged: 2.0, kept: 1.0, note: None }
    );
    assert_eq!(parse_merge_output("{\"waiting\": \"no store\"}").unwrap(), MergeOutput::Waiting { waiting: "no store".into() });
    assert_eq!(
        parse_merge_output("{\"merged\": 0, \"kept\": 1, \"note\": \"kept as it was\"}").unwrap(),
        MergeOutput::Merged { merged: 0.0, kept: 1.0, note: Some("kept as it was".into()) }
    );
    assert!(parse_merge_output("Traceback\n").unwrap_err().to_string().contains("Traceback"));
    assert!(parse_merge_output("").unwrap_err().to_string().contains("nothing"));
    assert!(parse_merge_output("{\"merged\": \"x\"}").unwrap_err().to_string().contains("merged"));
}
