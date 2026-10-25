// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of project state: every script and listing against the TS's own text (tests/fixtures/oracle/
//! project-state-scripts.json, written by oracle/rest.oracle.ts), and the file rules the ported tests reach only in
//! passing. spec/project_state_{core,files,scripts,agents}.md name the rules.
#[path = "rest_common/mod.rs"]
mod common;

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use wsp_engine::errors::EngineError;
use wsp_engine::machine::ExecResult;
use wsp_engine::project_state::listing::{
    filter_step, keyed_step, list_script, parse_state_listing, store_copy, StateListing, UnreadStore, ROOTS_STEP,
};
use wsp_engine::project_state::merge::{jsonl_cwd_step, merge_script, parse_merge_output, MergeOutput};
use wsp_engine::project_state::pi::pi_project_key;
use wsp_engine::project_state::py::{py_data, PY_PREAMBLE};
use wsp_engine::project_state::resolver::{
    files_under, inside_folder, moved_path, resolve_project_path, rewrite_jsonl, under_home, SharedStore,
};
use wsp_engine::project_state::sqlite::{
    count_rows, moved_column, select_rows, sqlite_filter, sqlite_merge_step, under_path, update_rows, MergeMode, MergeTable, ProjectTable,
    Row, RowUpdate, SqlValue,
};
use wsp_engine::project_state::{
    agent_home, agent_homes, agents_on_machine, guest_agent_homes, project_state_key, project_state_resolvers, state_listing,
};

fn oracle() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle/project-state-scripts.json");
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

fn s(v: &Value) -> String {
    v.as_str().unwrap().to_string()
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(s).collect()
}

fn map(v: &Value) -> IndexMap<String, String> {
    v.as_object().unwrap().iter().map(|(k, v)| (k.clone(), s(v))).collect()
}

// spec/project_state_scripts.md Y1, Y2
#[test]
fn py_data_writes_the_ts_text_for_every_oracle_value() {
    let o = oracle();
    assert_eq!(PY_PREAMBLE, o["pyPreamble"].as_str().unwrap());
    assert_eq!(ROOTS_STEP, o["rootsStep"].as_str().unwrap());
    let cases = o["pyData"].as_array().unwrap();
    assert_eq!(cases.len(), 15);
    for c in cases {
        // serde_json keeps the text's own key order; py_data must still write JS's (integer-like keys first).
        let value: Value = serde_json::from_str(c["text"].as_str().unwrap()).unwrap();
        assert_eq!(py_data(&value), s(&c["py"]), "py_data of {}", c["text"]);
    }
}

// spec/project_state_scripts.md Y4, Y5
#[test]
fn list_and_filter_steps_match_the_ts_byte_for_byte() {
    let o = oracle();
    let steps = vec!["say(1)".to_string(), "for x in y:\n    say(x)".to_string()];
    assert_eq!(
        list_script("claude", "/root/.claude", "/Users/ada/b_2.x/", &strs(&json!(["projects", "a/../b"])), &steps),
        s(&o["listScript"])
    );
    assert_eq!(list_script("pi", "/h", "/p", &[], &[ROOTS_STEP.to_string()]), s(&o["listScriptNoSteps"]));
    assert_eq!(keyed_step("/root/.claude/projects", "lambda p: p"), s(&o["keyedStep"]));
    let shared = SharedStore { store: "state_5.sqlite".into(), filter: vec!["FILTERED = 1".into(), "if x:\n    y".into()] };
    assert_eq!(filter_step("/tmp/wsp-scratch", "/root/.codex", &shared, &["say(COPY)".to_string()]), s(&o["filterStep"]));
    assert_eq!(filter_step("/s", "/h", &SharedStore { store: "s.db".into(), filter: vec![] }, &[]), s(&o["filterStepNoSteps"]));
}

// spec/project_state_scripts.md Y6
#[test]
fn store_copy_joins_as_node_posix_join() {
    for c in oracle()["storeCopy"].as_array().unwrap() {
        assert_eq!(
            store_copy(c["scratch"].as_str().unwrap(), c["home"].as_str().unwrap(), c["store"].as_str().unwrap()),
            s(&c["copy"]),
            "{c}"
        );
    }
}

// spec/project_state_scripts.md Y7
#[test]
fn parse_state_listing_matches_the_ts() {
    let unread = format!("!unread {}", base64_of(br#"{"agent":"codex","store":"/root/.codex/state_5.sqlite","why":"database is locked"}"#));
    let got = parse_state_listing(&format!("/a\n\n{unread}\n/b c\n")).unwrap();
    let want = oracle()["parseStateListing"].clone();
    assert_eq!(
        got,
        StateListing {
            paths: strs(&want["paths"]),
            unread: vec![UnreadStore {
                agent: "codex".into(),
                store: "/root/.codex/state_5.sqlite".into(),
                why: "database is locked".into()
            }],
        }
    );
    assert_eq!(got.paths, vec!["/a".to_string(), "/b c".to_string()]);
    assert!(parse_state_listing("!unread not-base64-json\n").is_err(), "a marked line that does not decode is a failure");
}

fn base64_of(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// spec/project_state_scripts.md Y8, Y9
#[test]
fn merge_script_and_cwd_step_match_the_ts_byte_for_byte() {
    let o = oracle();
    assert_eq!(
        merge_script("/Users/ada/b_2.x", "/root/work/b_2.x", &["merged += 1".to_string(), "kept += 2".to_string()]),
        s(&o["mergeScript"])
    );
    assert_eq!(merge_script("/a", "/b", &[]), s(&o["mergeScriptNoSteps"]));
    assert_eq!(jsonl_cwd_step(&strs(&json!(["/root/.codex/sessions/a.jsonl", "/root/x y.jsonl"])), Some("payload")), s(&o["jsonlCwdStep"]));
    assert_eq!(jsonl_cwd_step(&[], None), s(&o["jsonlCwdStepNoHolder"]));
}

// spec/project_state_scripts.md Y10
#[test]
fn parse_merge_output_answers_as_the_ts_for_every_oracle_line() {
    for c in oracle()["parseMergeOutput"].as_array().unwrap() {
        let got = parse_merge_output(c["stdout"].as_str().unwrap());
        match (&c["out"]["ok"], &c["out"]["error"]) {
            (Value::Object(ok), _) => {
                let want = match ok.get("waiting") {
                    Some(w) => MergeOutput::Waiting { waiting: s(w) },
                    None => MergeOutput::Merged {
                        merged: ok["merged"].as_u64().unwrap(),
                        kept: ok["kept"].as_u64().unwrap(),
                        note: ok.get("note").map(s),
                    },
                };
                assert_eq!(got.unwrap(), want, "{}", c["stdout"]);
            }
            (_, Value::String(error)) => assert_eq!(got.unwrap_err().to_string(), *error, "{}", c["stdout"]),
            _ => unreachable!(),
        }
    }
}

fn typed_rows(v: &Value) -> Vec<Row> {
    use base64::Engine;
    v.as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .map(|cell| {
                    let c = cell.as_array().unwrap();
                    let value = match c[1].as_str().unwrap() {
                        "null" => SqlValue::Null,
                        "int" => SqlValue::Integer(c[2].as_i64().unwrap()),
                        "real" => SqlValue::Real(c[2].as_f64().unwrap()),
                        "text" => SqlValue::Text(s(&c[2])),
                        "blob" => SqlValue::Blob(base64::engine::general_purpose::STANDARD.decode(c[2].as_str().unwrap()).unwrap()),
                        other => panic!("{other}"),
                    };
                    (s(&c[0]), value)
                })
                .collect()
        })
        .collect()
}

// spec/project_state_scripts.md Y11 to Y13, spec/project_state_files.md Q1
#[test]
fn sqlite_filter_and_merge_step_match_the_ts_byte_for_byte() {
    let o = oracle();
    assert_eq!(under_path("cwd"), s(&o["underPath"]));
    assert_eq!(moved_column("git_repo_root"), s(&o["movedColumn"]));
    let tables = vec![
        ProjectTable { table: "threads".into(), r#where: under_path("cwd") },
        ProjectTable { table: "m".into(), r#where: "session_id in (select id from s)".into() },
    ];
    assert_eq!(sqlite_filter(&tables), strs(&o["sqliteFilter"]));
    assert_eq!(sqlite_filter(&[]), strs(&o["sqliteFilterNone"]));
    let rows = o["sqliteMergeStepRows"].as_array().unwrap();
    let merge = vec![
        MergeTable {
            table: "threads".into(),
            mode: MergeMode::Keyed { key: vec!["id".into()], set: vec!["cwd".into(), "rollout_path".into()] },
            rows: typed_rows(&rows[0]["rows"]),
        },
        MergeTable {
            table: "messages".into(),
            mode: MergeMode::By { by: "session_id".into(), drop: vec!["id".into()] },
            rows: typed_rows(&rows[1]["rows"]),
        },
    ];
    assert_eq!(sqlite_merge_step("/root/.codex/state_5.sqlite", &merge), s(&o["sqliteMergeStep"]));
}

// spec/project_state_core.md P1, spec/project_state_agents.md
#[test]
fn the_registry_matches_the_ts_modules() {
    let o = oracle();
    let want = o["resolvers"].as_array().unwrap();
    let got = project_state_resolvers();
    assert_eq!(got.keys().copied().collect::<Vec<_>>(), want.iter().map(|r| r["agent"].as_str().unwrap()).collect::<Vec<_>>());
    for r in want {
        let m = &got[r["agent"].as_str().unwrap()];
        assert_eq!(m.states(), strs(&r["states"]).iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(m.roots(), strs(&r["roots"]).iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(serde_json::to_value(m.carry()).unwrap(), r["carry"]);
        assert_eq!(m.has_merge(), r["hasMerge"].as_bool().unwrap(), "{}", r["agent"]);
    }
}

// spec/project_state_agents.md, each module's listing, filter and key
#[test]
fn each_modules_listing_and_filter_match_the_ts() {
    for r in oracle()["resolvers"].as_array().unwrap() {
        let m = &project_state_resolvers()[r["agent"].as_str().unwrap()];
        let listing = m.listing("/root/.agent home", "/Users/ada/b_2.x");
        assert_eq!(listing, if r["listing"].is_null() { None } else { Some(strs(&r["listing"])) }, "{} listing", r["agent"]);
        let shared = m.shared();
        let want = if r["shared"].is_null() {
            None
        } else {
            Some(SharedStore { store: s(&r["shared"]["store"]), filter: strs(&r["shared"]["filter"]) })
        };
        assert_eq!(shared, want, "{} shared", r["agent"]);
        assert_eq!(m.key("/Users/ada/b_2.x"), r["key"].as_str().map(String::from), "{} key", r["agent"]);
    }
}

// spec/project_state_core.md P2
#[test]
fn project_state_key_is_claudes_alone() {
    let dir = tempfile::tempdir().unwrap();
    let real = fs::canonicalize(dir.path()).unwrap();
    let link = real.join("link");
    fs::create_dir(real.join("proj")).unwrap();
    symlink(real.join("proj"), &link).unwrap();
    // The key is built from the resolved path: a link to the folder keys as the folder.
    let got = project_state_key("claude", link.to_str().unwrap());
    assert_eq!(got, Some(wsp_words::project_path::claude_project_key(real.join("proj").to_str().unwrap())));
    assert_eq!(project_state_key("codex", "/a"), None);
    assert_eq!(project_state_key("pi", "/a"), None);
    assert_eq!(project_state_key("nope", "/a"), None);
}

// spec/project_state_core.md P4
#[test]
fn agent_homes_match_the_ts_oracle() {
    let o = &oracle()["agentHomes"];
    assert_eq!(agent_homes("/Users/ada", None), map(&o["plain"]));
    let vars: IndexMap<String, String> =
        [("CLAUDE_CONFIG_DIR", "/opt/claude"), ("CODEX_HOME", "")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    assert_eq!(agent_homes("/Users/ada", Some(&vars)), map(&o["env"]));
    assert_eq!(guest_agent_homes(), map(&o["guest"]));
    assert_eq!(agent_home("/Users/ada", "nope", None), s(&o["one"]));
    assert_eq!(agent_homes("/Users/ada", None).keys().cloned().collect::<Vec<_>>(), strs(&o["catalog"]));
}

// spec/project_state_core.md P5
#[test]
fn state_listing_matches_the_ts_byte_for_byte() {
    let o = oracle();
    for c in o["stateListings"].as_array().unwrap() {
        let homes = map(&c["homes"]);
        let ids = if c["ids"].is_null() { None } else { Some(strs(&c["ids"])) };
        let (path, scratch) = if homes.len() == 1 { ("/p", "/s") } else { ("/Users/ada/b_2.x", "/tmp/wsp-scratch") };
        assert_eq!(state_listing(&homes, path, scratch, ids.as_deref()).unwrap(), s(&c["script"]), "ids {}", c["ids"]);
    }
    let homes = map(&o["stateListings"][0]["homes"]);
    let err = state_listing(&homes, "/p", "/s", Some(&["claude".to_string(), "nope".to_string()])).unwrap_err();
    assert_eq!(err.to_string(), s(&o["unknownAgent"]["error"]));
}

// spec/project_state_agents.md A4
#[test]
fn pi_keys_match_the_ts_oracle() {
    for c in oracle()["piKeys"].as_array().unwrap() {
        assert_eq!(pi_project_key(c["path"].as_str().unwrap()), s(&c["key"]), "{}", c["path"]);
    }
}

// spec/project_state_files.md F2 to F4
#[test]
fn paths_match_the_ts_oracle() {
    let o = &oracle()["paths"];
    for c in o["insideFolder"].as_array().unwrap() {
        assert_eq!(inside_folder(c["rel"].as_str().unwrap()), c["inside"].as_bool().unwrap(), "{}", c["rel"]);
    }
    for c in o["underHome"].as_array().unwrap() {
        let parts = strs(&c["parts"]);
        let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
        assert_eq!(under_home(c["home"].as_str().unwrap(), &parts), c["at"].as_str().map(String::from), "{c}");
    }
    for c in o["movedPath"].as_array().unwrap() {
        assert_eq!(
            moved_path(c["path"].as_str().unwrap(), c["from"].as_str().unwrap(), c["to"].as_str().unwrap()),
            c["moved"].as_str().map(String::from),
            "{c}"
        );
    }
}

// spec/project_state_files.md F1
#[test]
fn resolve_project_path_resolves_dots_and_links() {
    let dir = tempfile::tempdir().unwrap();
    let real = fs::canonicalize(dir.path()).unwrap();
    fs::create_dir_all(real.join("a/b")).unwrap();
    symlink(real.join("a"), real.join("l")).unwrap();
    let r = real.to_str().unwrap();
    assert_eq!(resolve_project_path(&format!("{r}/l/b/")), format!("{r}/a/b"));
    assert_eq!(resolve_project_path(&format!("{r}/a/./b/../b")), format!("{r}/a/b"));
    // A path that is not there, or not a directory, keeps its resolved spelling and never follows a link.
    assert_eq!(resolve_project_path(&format!("{r}/l/missing/../x/")), format!("{r}/l/x"));
    fs::write(real.join("a/f"), "x").unwrap();
    symlink(real.join("a/f"), real.join("lf")).unwrap();
    assert_eq!(resolve_project_path(&format!("{r}/lf")), format!("{r}/lf"));
    assert_eq!(resolve_project_path("/"), "/");
    assert_eq!(resolve_project_path("//x//y/"), "/x/y");
}

// spec/project_state_files.md F10
#[test]
fn files_under_skips_links_and_sorts_by_utf16() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    fs::create_dir_all(d.join("b/c")).unwrap();
    fs::write(d.join("b/c/z.jsonl"), "").unwrap();
    fs::write(d.join("a.jsonl"), "").unwrap();
    fs::write(d.join("\u{ff61}.jsonl"), "").unwrap();
    fs::write(d.join("\u{1f600}.jsonl"), "").unwrap();
    fs::write(d.join("b/x.txt"), "").unwrap();
    symlink(d.join("a.jsonl"), d.join("link.jsonl")).unwrap();
    let root = d.to_str().unwrap();
    // UTF-16 order puts the astral 😀 (a surrogate, 0xD83D) before U+FF61; UTF-8 byte order would not.
    assert_eq!(
        files_under(root, ".jsonl").unwrap(),
        vec![format!("{root}/a.jsonl"), format!("{root}/b/c/z.jsonl"), format!("{root}/\u{1f600}.jsonl"), format!("{root}/\u{ff61}.jsonl")]
    );
    assert_eq!(files_under(&format!("{root}/b"), "").unwrap(), vec![format!("{root}/b/c/z.jsonl"), format!("{root}/b/x.txt")]);
    assert_eq!(files_under(&format!("{root}/nope"), "").unwrap(), Vec::<String>::new());
}

// spec/project_state_files.md F11
#[tokio::test]
async fn rewrite_jsonl_reorders_integer_keys_on_a_rewritten_line_only() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("s.jsonl");
    let text =
        "{\"b\":1,\"2\":2,\"cwd\":\"/old\",\"n\":1.0}\n{\"b\":1,\"2\":2,\"cwd\":\"/keep\",\"n\":1.0}\nnot json\n[1]\n{\"cwd\":\"/old\"";
    fs::write(&f, text).unwrap();
    fs::set_permissions(&f, fs::Permissions::from_mode(0o640)).unwrap();
    let n = rewrite_jsonl(f.to_str().unwrap(), &mut |line: &mut Map<String, Value>| {
        if line.get("cwd") == Some(&json!("/old")) {
            line.insert("cwd".into(), json!("/new"));
            return Ok(true);
        }
        Ok(false)
    })
    .await
    .unwrap();
    // The truncated last line parses as nothing, so it passes through untouched; 1.0 is written as JS writes it.
    assert_eq!(n, 1);
    assert_eq!(
        fs::read_to_string(&f).unwrap(),
        "{\"2\":2,\"b\":1,\"cwd\":\"/new\",\"n\":1}\n{\"b\":1,\"2\":2,\"cwd\":\"/keep\",\"n\":1.0}\nnot json\n[1]\n{\"cwd\":\"/old\""
    );
    assert_eq!(fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o640);
    assert!(!dir.path().join("s.jsonl.wsp-move").exists());
}

// spec/project_state_files.md F12
#[tokio::test]
async fn a_file_with_nothing_to_rewrite_is_not_touched() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("s.jsonl");
    fs::write(&f, "{\"cwd\":\"/keep\"}\n").unwrap();
    let before = fs::metadata(&f).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let n = rewrite_jsonl(f.to_str().unwrap(), &mut |_| Ok(false)).await.unwrap();
    assert_eq!(n, 0);
    assert_eq!(fs::read_to_string(&f).unwrap(), "{\"cwd\":\"/keep\"}\n");
    assert_eq!(fs::metadata(&f).unwrap().modified().unwrap(), before);
    assert!(!dir.path().join("s.jsonl.wsp-move").exists());
    let empty = dir.path().join("e.jsonl");
    fs::write(&empty, "").unwrap();
    assert_eq!(rewrite_jsonl(empty.to_str().unwrap(), &mut |_| Ok(true)).await.unwrap(), 0);
}

fn seed(db: &Path, sql: &str) {
    rusqlite::Connection::open(db).unwrap().execute_batch(sql).unwrap();
}

// spec/project_state_files.md Q2
#[test]
fn a_missing_database_is_never_created() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("none.db");
    let d = db.to_str().unwrap();
    assert_eq!(count_rows(d, "select 1 as n", &[]).unwrap(), 0);
    assert_eq!(select_rows(d, "select 1", &[]).unwrap(), None);
    assert_eq!(update_rows(d, &[RowUpdate { sql: "create table t (x)".into(), params: vec![] }]).unwrap(), None);
    assert!(!db.exists());
}

// spec/project_state_files.md Q4
#[test]
fn update_rows_rolls_back_every_update_when_one_fails() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("s.db");
    seed(&db, "create table s (cwd text); insert into s values ('/a'), ('/a/x'), ('/b');");
    let d = db.to_str().unwrap();
    let params = vec![("$from".to_string(), SqlValue::Text("/a".into())), ("$to".to_string(), SqlValue::Text("/z".into()))];
    let ok = RowUpdate { sql: format!("update s set cwd = {} where {}", moved_column("cwd"), under_path("cwd")), params: params.clone() };
    assert!(update_rows(d, &[ok.clone(), RowUpdate { sql: "update nope set x = 1".into(), params: vec![] }]).is_err());
    let rows = select_rows(d, "select cwd from s order by 1", &[]).unwrap().unwrap();
    assert_eq!(
        rows.iter().map(|r| r["cwd"].clone()).collect::<Vec<_>>(),
        vec![SqlValue::Text("/a".into()), SqlValue::Text("/a/x".into()), SqlValue::Text("/b".into())]
    );
    assert_eq!(update_rows(d, &[ok]).unwrap(), Some(vec![2]));
    let rows = select_rows(d, "select cwd from s order by 1", &[]).unwrap().unwrap();
    assert_eq!(
        rows.iter().map(|r| r["cwd"].clone()).collect::<Vec<_>>(),
        vec![SqlValue::Text("/b".into()), SqlValue::Text("/z".into()), SqlValue::Text("/z/x".into())]
    );
}

// spec/project_state_core.md P7
#[tokio::test]
async fn agents_on_machine_reads_the_missing_lines() {
    let none = common::FakeMachine::new("m1");
    assert!(agents_on_machine(&none, &[]).await.unwrap().is_empty());
    assert!(none.ran().is_empty(), "no named agent asks the machine nothing");
    let m = common::FakeMachine::exec("m1", |_, _| {
        Ok(ExecResult { exit_code: 0, stdout: "missing codex\nother\n".into(), stderr: String::new() })
    });
    let ids = vec!["codex".to_string(), "claude".to_string(), "nope".to_string()];
    let got = agents_on_machine(&m, &ids).await.unwrap();
    assert_eq!(got.into_iter().collect::<std::collections::BTreeSet<_>>(), ["claude".to_string()].into_iter().collect());
    let cmd = &m.cmds()[0];
    // Catalog order, not the order asked: claude before codex.
    assert!(cmd.contains("for b in 'claude' 'codex'; do"), "{cmd}");
    let failing = common::FakeMachine::exec("m1", |_, _| Ok(ExecResult { exit_code: 1, stdout: String::new(), stderr: "boom".into() }));
    let err = agents_on_machine(&failing, &ids).await.unwrap_err();
    assert!(
        matches!(&err, EngineError::Other { message, .. } if message.starts_with("could not see which agents are on the machine: ")),
        "{err}"
    );
}

// spec/project_state_scripts.md Y12
#[test]
fn portable_writes_blobs_as_base64_and_whole_reals_as_js_numbers() {
    use wsp_engine::project_state::sqlite::portable;
    let row: Row = [
        ("r".to_string(), SqlValue::Real(1.0)),
        ("h".to_string(), SqlValue::Real(1.5)),
        ("i".to_string(), SqlValue::Integer(7)),
        ("b".to_string(), SqlValue::Blob(vec![0, 1, 254, 255])),
        ("n".to_string(), SqlValue::Null),
        ("t".to_string(), SqlValue::Text("x".into())),
    ]
    .into_iter()
    .collect();
    let v = portable(&row);
    assert_eq!(v.as_object().unwrap().keys().cloned().collect::<Vec<_>>(), vec!["r", "h", "i", "b", "n", "t"]);
    assert_eq!(v["b"], json!({ "$blob": "AAH+/w==" }));
    assert_eq!(py_data(&v), format!("data(\"{}\")", base64_of(br#"{"r":1,"h":1.5,"i":7,"b":{"$blob":"AAH+/w=="},"n":null,"t":"x"}"#)));
}
