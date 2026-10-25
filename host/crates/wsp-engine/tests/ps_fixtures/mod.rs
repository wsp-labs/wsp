// SPDX-License-Identifier: AGPL-3.0-only
//! The fixtures and helpers at the top of packages/engine/test/project-state.test.ts (lines 1-690): the paths, tree(),
//! rows(), seed(), resolverFor, each agent's headless-run fixture and the shared Hermes store. Test code only.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use indexmap::IndexMap;
use rusqlite::types::Value as SqlV;
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Map, Value};

use wsp_engine::project_state::{
    parse_state_listing, project_state_resolvers, state_listing, store_copy, MovedState, ProjectStateResolver, UnreadStore,
};

pub const FROM: &str = "/private/tmp/wsp-r212/proj/b_2.x";
pub const TO: &str = "/root/work/b_2.x";
pub const OTHER: &str = "/Users/me/other";
pub const FROM_SUB: &str = "/private/tmp/wsp-r212/proj/b_2.x/sub";
pub const TO_SUB: &str = "/root/work/b_2.x/sub";
pub const DECOY: &str = "/private/tmp/wsp-r212/proj/b_2.x-old";

/// A scratch root removed when dropped; `root` is its real path.
pub struct Scratch {
    _dir: tempfile::TempDir,
    pub root: String,
}

pub fn scratch() -> Scratch {
    let dir = tempfile::Builder::new().prefix("wsp-t221-").tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap().to_str().unwrap().to_string();
    Scratch { _dir: dir, root }
}

/// node's `join` for the parts these tests pass: each part appended with one `/`.
pub fn j(base: &str, parts: &[&str]) -> String {
    let mut p = PathBuf::from(base);
    for part in parts {
        p.push(part);
    }
    p.to_str().unwrap().to_string()
}

pub fn write(file: &str, text: &str) {
    if let Some(dir) = Path::new(file).parent() {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(file, text).unwrap();
}

pub fn lines(l: &[&str]) -> String {
    format!("{}\n", l.join("\n"))
}

pub fn txt(s: &str) -> Value {
    Value::String(s.to_string())
}

pub type Tree = BTreeMap<String, Value>;

/// Every regular file under dir by relative path, text files as their bytes, sqlite files by their rows.
pub fn tree(dir: &str) -> Tree {
    let mut out = Tree::new();
    walk(Path::new(dir), Path::new(dir), &mut out);
    out
}

fn walk(base: &Path, dir: &Path, out: &mut Tree) {
    for e in fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let ty = e.file_type().unwrap();
        let abs = e.path();
        if ty.is_dir() {
            walk(base, &abs, out);
        } else if ty.is_file() {
            let rel = abs.strip_prefix(base).unwrap().to_str().unwrap().to_string();
            let name = e.file_name().to_str().unwrap().to_string();
            let value = if name.ends_with(".sqlite") || name.ends_with(".db") {
                rows(abs.to_str().unwrap())
            } else {
                Value::String(String::from_utf8_lossy(&fs::read(&abs).unwrap()).into_owned())
            };
            out.insert(rel, value);
        }
    }
}

fn cell(v: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => json!(f),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::Array(b.iter().map(|x| Value::from(*x)).collect()),
    }
}

/// Every table of the database by name, its rows ordered by columns 1 and 2.
pub fn rows(db: &str) -> Value {
    let c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut names = c.prepare("select name from sqlite_master where type = 'table' order by name").unwrap();
    let tables: Vec<String> = names.query_map([], |r| r.get::<_, String>(0)).unwrap().map(|r| r.unwrap()).collect();
    let mut out = Map::new();
    for name in tables {
        let mut st = c.prepare(&format!("select * from \"{name}\" order by 1, 2")).unwrap();
        let cols: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
        let mut q = st.query([]).unwrap();
        let mut table = Vec::new();
        while let Some(r) = q.next().unwrap() {
            let mut row = Map::new();
            for (i, col) in cols.iter().enumerate() {
                row.insert(col.clone(), cell(r.get_ref(i).unwrap()));
            }
            table.push(Value::Object(row));
        }
        out.insert(name, Value::Array(table));
    }
    Value::Object(out)
}

pub fn t(s: &str) -> SqlV {
    SqlV::Text(s.to_string())
}
pub fn n(i: i64) -> SqlV {
    SqlV::Integer(i)
}
pub fn nul() -> SqlV {
    SqlV::Null
}

pub fn seed(db: &str, schema: &str, inserts: Vec<(&str, Vec<SqlV>)>) {
    if let Some(dir) = Path::new(db).parent() {
        fs::create_dir_all(dir).unwrap();
    }
    let c = Connection::open(db).unwrap();
    c.execute_batch(schema).unwrap();
    for (sql, params) in inserts {
        c.execute(sql, rusqlite::params_from_iter(params)).unwrap();
    }
}

pub fn resolver_for(agent: &str) -> Arc<dyn ProjectStateResolver> {
    match project_state_resolvers().get(agent) {
        Some(r) => r.clone(),
        None => panic!("no resolver for {agent}"),
    }
}

pub fn moved(state: &str, files: Vec<String>, changed: u64) -> MovedState {
    MovedState { state: state.to_string(), files, changed, skipped: None }
}

// --- Claude Code ------------------------------------------------------------------------------------------------------

pub fn claude_user(cwd: &str) -> String {
    format!(r#"{{"type":"user","cwd":"{cwd}","sessionId":"S1","version":"2.1.257","message":{{"role":"user","content":"hi"}}}}"#)
}
pub fn claude_assistant(cwd: &str) -> String {
    format!(r#"{{"type":"assistant","cwd":"{cwd}","sessionId":"S1","message":{{"role":"assistant","content":[]}}}}"#)
}
pub const CLAUDE_SUMMARY: &str = r#"{"type":"summary","summary":"a scratch run","leafUuid":"u1"}"#;
pub fn claude_tail() -> String {
    format!(r#"{{"type":"user","cwd":"{FROM}","sessionId":"S1""#)
}
pub fn claude_sub(cwd: &str) -> String {
    format!(r#"{{"type":"user","cwd":"{cwd}","sessionId":"S1","isSidechain":true,"message":{{"role":"user","content":"look"}}}}"#)
}

pub fn claude_home(root: &str) -> String {
    let home = j(root, &["claude"]);
    let key = j(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x"]);
    write(&j(&key, &["S1.jsonl"]), &[claude_user(FROM), claude_assistant(FROM), CLAUDE_SUMMARY.to_string(), claude_tail()].join("\n"));
    write(&j(&key, &["S1", "subagents", "agent-a1.jsonl"]), &lines(&[&claude_sub(FROM)]));
    write(&j(&key, &["S1", "tool-results", "t1.txt"]), "ls output\n");
    write(&j(&key, &["memory", "MEMORY.md"]), "notes\n");
    write(&j(&home, &["projects", "-Users-me-other", "S2.jsonl"]), &lines(&[&claude_user(OTHER)]));
    write(&j(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x-sub", "S3.jsonl"]), &lines(&[&claude_user(FROM_SUB)]));
    write(&j(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x-old", "S4.jsonl"]), &lines(&[&claude_user(DECOY)]));
    write(&j(&home, &["sessions", "4242.json"]), &format!("{{\"pid\":4242,\"cwd\":\"{FROM}\"}}\n"));
    write(&j(&home, &["history.jsonl"]), &lines(&[&format!("{{\"display\":\"hi\",\"project\":\"{FROM}\"}}")]));
    home
}

pub fn claude_after(key: &str) -> Tree {
    let mut m = Tree::new();
    m.insert(
        format!("projects/{key}/S1.jsonl"),
        txt(&[claude_user(TO), claude_assistant(TO), CLAUDE_SUMMARY.to_string(), claude_tail()].join("\n")),
    );
    m.insert(format!("projects/{key}/S1/subagents/agent-a1.jsonl"), txt(&lines(&[&claude_sub(TO)])));
    m.insert(format!("projects/{key}/S1/tool-results/t1.txt"), txt("ls output\n"));
    m.insert(format!("projects/{key}/memory/MEMORY.md"), txt("notes\n"));
    m.insert("projects/-Users-me-other/S2.jsonl".into(), txt(&lines(&[&claude_user(OTHER)])));
    m.insert(format!("projects/{key}-sub/S3.jsonl"), txt(&lines(&[&claude_user(TO_SUB)])));
    m.insert("projects/-private-tmp-wsp-r212-proj-b-2-x-old/S4.jsonl".into(), txt(&lines(&[&claude_user(DECOY)])));
    m.insert("sessions/4242.json".into(), txt(&format!("{{\"pid\":4242,\"cwd\":\"{FROM}\"}}\n")));
    m.insert("history.jsonl".into(), txt(&lines(&[&format!("{{\"display\":\"hi\",\"project\":\"{FROM}\"}}")])));
    m
}

// --- Pi ---------------------------------------------------------------------------------------------------------------

pub fn pi_header(cwd: &str) -> String {
    format!(r#"{{"type":"session","version":3,"id":"s1","timestamp":"2026-09-05T21:56:00.000Z","cwd":"{cwd}"}}"#)
}
pub const PI_MESSAGE: &str = r#"{"type":"message","id":"m1","parentId":null,"timestamp":"2026-09-05T21:56:01.000Z","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}"#;

pub fn pi_home(root: &str) -> String {
    let home = j(root, &["pi"]);
    write(
        &j(&home, &["sessions", "--private-tmp-wsp-r212-proj-b_2.x--", "2026-09-05T21-56-00-000Z_s1.jsonl"]),
        &lines(&[&pi_header(FROM), PI_MESSAGE]),
    );
    write(&j(&home, &["sessions", "--Users-me-other--", "2026-09-05T21-50-00-000Z_s0.jsonl"]), &lines(&[&pi_header(OTHER), PI_MESSAGE]));
    write(
        &j(&home, &["sessions", "--private-tmp-wsp-r212-proj-b_2.x-sub--", "2026-09-05T21-57-00-000Z_s2.jsonl"]),
        &lines(&[&pi_header(FROM_SUB), PI_MESSAGE]),
    );
    write(
        &j(&home, &["sessions", "--private-tmp-wsp-r212-proj-b_2.x-old--", "2026-09-05T21-40-00-000Z_s3.jsonl"]),
        &lines(&[&pi_header(DECOY), PI_MESSAGE]),
    );
    write(&j(&home, &["trust.json"]), &format!("{{\"{FROM}\":true}}\n"));
    home
}

// --- Codex ------------------------------------------------------------------------------------------------------------

pub fn codex_meta(cwd: &str) -> String {
    format!(
        r#"{{"timestamp":"2026-09-05T21:58:00.000Z","type":"session_meta","payload":{{"id":"t1","timestamp":"2026-09-05T21:58:00.000Z","cwd":"{cwd}","originator":"codex_exec","cli_version":"0.153.0"}}}}"#
    )
}
pub const CODEX_ITEM: &str = r#"{"timestamp":"2026-09-05T21:58:00.100Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hi"}]}}"#;
pub fn codex_turn(cwd: &str) -> String {
    format!(
        r#"{{"timestamp":"2026-09-05T21:58:00.200Z","type":"turn_context","payload":{{"cwd":"{cwd}","approval_policy":"never","model":"gpt-5"}}}}"#
    )
}
pub const CODEX_EVENT: &str = r#"{"timestamp":"2026-09-05T21:58:01.000Z","type":"event_msg","payload":{"type":"error","message":"401"}}"#;
pub const CODEX_SCHEMA: &str = "create table threads (id text primary key, rollout_path text not null, cwd text not null, archived integer not null default 0, updated_at integer not null default 0)";

/// The four rollouts t1..t4 of codex_home, by absolute path.
pub fn codex_rollouts(home: &str) -> [String; 4] {
    [
        j(home, &["sessions", "2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]),
        j(home, &["sessions", "2026", "09", "04", "rollout-2026-09-04T10-00-00-t2.jsonl"]),
        j(home, &["sessions", "2026", "09", "05", "rollout-2026-09-05T22-00-00-t3.jsonl"]),
        j(home, &["sessions", "2026", "09", "05", "rollout-2026-09-05T22-01-00-t4.jsonl"]),
    ]
}

pub fn codex_home(root: &str) -> String {
    let home = j(root, &["codex"]);
    let [t1, t2, t3, t4] = codex_rollouts(&home);
    write(&t1, &lines(&[&codex_meta(FROM), CODEX_ITEM, &codex_turn(FROM), CODEX_EVENT]));
    write(&t2, &lines(&[&codex_meta(OTHER), CODEX_ITEM, &codex_turn(OTHER)]));
    write(&t3, &lines(&[&codex_meta(FROM_SUB), CODEX_ITEM, &codex_turn(FROM_SUB)]));
    write(&t4, &lines(&[&codex_meta(DECOY), CODEX_ITEM, &codex_turn(DECOY)]));
    seed(
        &j(&home, &["state_5.sqlite"]),
        CODEX_SCHEMA,
        vec![
            ("insert into threads values (?, ?, ?, 0, 1)", vec![t("t1"), t(&t1), t(FROM)]),
            ("insert into threads values (?, ?, ?, 0, 2)", vec![t("t2"), t(&t2), t(OTHER)]),
            ("insert into threads values (?, ?, ?, 0, 3)", vec![t("t3"), t(&t3), t(FROM_SUB)]),
            ("insert into threads values (?, ?, ?, 0, 4)", vec![t("t4"), t(&t4), t(DECOY)]),
        ],
    );
    write(&j(&home, &["config.toml"]), &format!("[projects.\"{FROM}\"]\ntrust_level = \"trusted\"\n"));
    home
}

// --- Hermes -----------------------------------------------------------------------------------------------------------

pub const HERMES_SCHEMA: &str = "create table sessions (id text primary key, cwd text, git_repo_root text, message_count integer); create table messages (id integer primary key, session_id text, content text)";

pub fn hermes_home(root: &str) -> String {
    let home = j(root, &["hermes"]);
    seed(
        &j(&home, &["state.db"]),
        HERMES_SCHEMA,
        vec![
            ("insert into sessions values (?, ?, ?, 2)", vec![t("20260906_033144_55e3e2"), t(FROM), t(FROM)]),
            ("insert into sessions values (?, ?, ?, 1)", vec![t("20260906_030000_aaaaaa"), t(OTHER), t(OTHER)]),
            ("insert into sessions values (?, ?, ?, 1)", vec![t("20260906_034000_bbbbbb"), t(FROM), nul()]),
            ("insert into sessions values (?, ?, ?, 1)", vec![t("20260906_035000_cccccc"), t(FROM_SUB), t(FROM)]),
            ("insert into sessions values (?, ?, ?, 1)", vec![t("20260906_036000_dddddd"), t(DECOY), t(DECOY)]),
            ("insert into messages values (1, ?, 'hi')", vec![t("20260906_033144_55e3e2")]),
            ("insert into messages values (2, ?, 'there')", vec![t("20260906_033144_55e3e2")]),
            ("insert into messages values (3, ?, 'other')", vec![t("20260906_030000_aaaaaa")]),
        ],
    );
    write(&j(&home, &["sessions", "request_dump_20260906_033144_55e3e2_1.json"]), "{}\n");
    home
}

// --- Gemini -----------------------------------------------------------------------------------------------------------

pub fn gemini_registry(path: &str) -> String {
    let mut projects = Map::new();
    projects.insert(path.to_string(), txt("b_2.x"));
    projects.insert(OTHER.to_string(), txt("other"));
    projects.insert(format!("{path}/sub"), txt("sub"));
    projects.insert(DECOY.to_string(), txt("b_2.x-old"));
    format!("{}\n", serde_json::to_string_pretty(&json!({ "projects": projects })).unwrap())
}
pub const GEMINI_CHAT: &str =
    "{\"sessionId\":\"a1b2c3d4-0000\",\"projectHash\":\"9d0b5e0f\",\"startTime\":\"2026-09-05T21:59:00.000Z\",\"messages\":[]}\n";

pub fn gemini_home(root: &str) -> String {
    let home = j(root, &["gemini"]);
    write(&j(&home, &["projects.json"]), &gemini_registry(FROM));
    write(&j(&home, &["tmp", "b_2.x", ".project_root"]), FROM);
    write(&j(&home, &["tmp", "b_2.x", "chats", "session-2026-09-05T21-59-a1b2c3d4.jsonl"]), GEMINI_CHAT);
    write(&j(&home, &["history", "b_2.x", ".project_root"]), &format!("{FROM}\n"));
    write(&j(&home, &["tmp", "other", ".project_root"]), OTHER);
    write(&j(&home, &["tmp", "sub", ".project_root"]), FROM_SUB);
    write(&j(&home, &["tmp", "b_2.x-old", ".project_root"]), DECOY);
    write(&j(&home, &["trustedFolders.json"]), &format!("{{\"{FROM}\":\"TRUST_FOLDER\"}}\n"));
    home
}

// --- OpenCode ---------------------------------------------------------------------------------------------------------

pub const HASH: &str = "9f1e2d3c4b5a69788796a5b4c3d2e1f0a1b2c3d4";
pub const OPENCODE_SCHEMA: &str = "create table project (id text primary key, worktree text not null, vcs text, name text, sandboxes text, time_created integer);create table project_directory (project_id text not null, directory text not null, primary key (project_id, directory));create table session (id text primary key, project_id text not null, directory text not null, title text);create table message (id text primary key, session_id text not null, role text);create table part (id text primary key, message_id text not null, text text)";

/// The `message` and `part` tables the opencode move leaves as they were, as (table, rows).
pub fn opencode_messages() -> [(&'static str, Value); 2] {
    [
        (
            "message",
            json!([{ "id": "msg_1", "session_id": "ses_1", "role": "user" }, { "id": "msg_2", "session_id": "ses_2", "role": "user" }]),
        ),
        (
            "part",
            json!([{ "id": "prt_1", "message_id": "msg_1", "text": "hi" }, { "id": "prt_2", "message_id": "msg_2", "text": "loose" }]),
        ),
    ]
}

pub fn opencode_home(root: &str) -> String {
    let home = j(root, &["opencode"]);
    seed(
        &j(&home, &["opencode.db"]),
        OPENCODE_SCHEMA,
        vec![
            ("insert into project values (?, ?, 'git', 'b_2.x', ?, 1)", vec![t(HASH), t(FROM), t(&format!("[\"{FROM}-copy\"]"))]),
            ("insert into project values ('global', '/', null, null, '[]', 0)", vec![]),
            ("insert into project values ('deadbeef', ?, 'git', 'b_2.x-old', '[]', 2)", vec![t(DECOY)]),
            ("insert into project_directory values (?, ?)", vec![t(HASH), t(FROM)]),
            ("insert into project_directory values (?, ?)", vec![t(HASH), t(FROM_SUB)]),
            ("insert into project_directory values ('deadbeef', ?)", vec![t(DECOY)]),
            ("insert into project_directory values ('global', ?)", vec![t("/Users/me/scratch")]),
            ("insert into session values ('ses_1', ?, ?, 'first')", vec![t(HASH), t(FROM)]),
            ("insert into session values ('ses_2', 'global', ?, 'loose')", vec![t("/Users/me/scratch")]),
            ("insert into session values ('ses_3', ?, ?, 'deeper')", vec![t(HASH), t(FROM_SUB)]),
            ("insert into session values ('ses_4', 'deadbeef', ?, 'sibling')", vec![t(DECOY)]),
            ("insert into message values ('msg_1', 'ses_1', 'user')", vec![]),
            ("insert into message values ('msg_2', 'ses_2', 'user')", vec![]),
            ("insert into part values ('prt_1', 'msg_1', 'hi')", vec![]),
            ("insert into part values ('prt_2', 'msg_2', 'loose')", vec![]),
        ],
    );
    home
}

// --- sessions per agent -----------------------------------------------------------------------------------------------

pub struct Listing {
    pub exit: i32,
    pub paths: Vec<String>,
    pub unread: Vec<UnreadStore>,
    pub err: String,
}

/// Runs one agent's listing the way the machine does: the command state_listing builds, in a shell, with `into` the
/// directory on the machine the filtered copies are written under.
pub fn run_listing(agent: &str, home: &str, path: &str, into: &str) -> Listing {
    let homes: IndexMap<String, String> = [(agent.to_string(), home.to_string())].into_iter().collect();
    let cmd = state_listing(&homes, path, into, Some(&[agent.to_string()])).unwrap();
    let r = Command::new("bash").arg("-c").arg(cmd).output().unwrap();
    let parsed = parse_state_listing(&String::from_utf8_lossy(&r.stdout)).unwrap();
    Listing {
        exit: r.status.code().unwrap_or(-1),
        paths: parsed.paths,
        unread: parsed.unread,
        err: String::from_utf8_lossy(&r.stderr).into_owned(),
    }
}

/// The python3 source of one agent's listing, off the command the machine would run.
pub fn listing_source(agent: &str, home: &str, path: &str, into: &str) -> String {
    let homes: IndexMap<String, String> = [(agent.to_string(), home.to_string())].into_iter().collect();
    let command = state_listing(&homes, path, into, Some(&[agent.to_string()])).unwrap();
    let opens = "set -e\npython3 -c '";
    command[opens.len()..command.len() - 1].replace("'\\''", "'")
}

/// The peak resident memory of the python3 a listing runs, in MB, beside the paths it named.
pub fn listing_peak(agent: &str, home: &str, path: &str, into: &str) -> (f64, Vec<String>) {
    let wrapper = [
        "import resource, sys",
        "exec(compile(sys.stdin.read(), \"listing\", \"exec\"), {})",
        "sys.stderr.write(str(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / (1024 * 1024 if sys.platform == \"darwin\" else 1024)))",
    ]
    .join("\n");
    let mut child =
        Command::new("python3").arg("-c").arg(wrapper).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(listing_source(agent, home, path, into).as_bytes()).unwrap();
    let r = child.wait_with_output().unwrap();
    if !r.status.success() {
        panic!("the listing failed (exit {:?}): {}", r.status.code(), String::from_utf8_lossy(&r.stderr));
    }
    let peak: f64 = String::from_utf8_lossy(&r.stderr).trim().parse().unwrap();
    (peak, String::from_utf8_lossy(&r.stdout).split('\n').filter(|l| !l.is_empty()).map(String::from).collect())
}

/// The store shape the cold review measured on: one Hermes store holding two projects with `per` messages of 2 KB
/// each, a session in a folder under the project, and the sibling sharing the project's prefix.
pub fn shared_hermes_store(root: &str, per: usize) -> String {
    let home = j(root, &["hermes-shared"]);
    let db = j(&home, &["state.db"]);
    fs::create_dir_all(&home).unwrap();
    let c = Connection::open(&db).unwrap();
    c.execute_batch("pragma page_size = 8192").unwrap();
    c.execute_batch(HERMES_SCHEMA).unwrap();
    let body = "m".repeat(2048);
    c.execute_batch("begin").unwrap();
    for (id, cwd, count) in [("a1", FROM, per), ("a2", FROM_SUB, 1), ("b1", OTHER, per), ("o1", DECOY, 1)] {
        c.execute("insert into sessions values (?, ?, ?, ?)", rusqlite::params![id, cwd, cwd, count as i64]).unwrap();
        for _ in 0..count {
            c.execute("insert into messages (session_id, content) values (?, ?)", rusqlite::params![id, body]).unwrap();
        }
    }
    c.execute_batch("commit").unwrap();
    home
}

/// Where a module's shared store is copied to for a trip: the store's own path under the scratch root.
pub fn copy_of(agent: &str, home: &str, into: &str) -> String {
    match resolver_for(agent).shared() {
        Some(shared) => store_copy(into, home, &shared.store),
        None => panic!("{agent} keeps no store for every project"),
    }
}
