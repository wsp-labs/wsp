// SPDX-License-Identifier: AGPL-3.0-only
//! Project state's M-rules (spec/project_state_core.md M1, project_state_files.md M2 to M5, project_state_agents.md M6
//! to M9): what a failed move leaves, how a rewritten JSONL line reads, which files the walk finds, the gemini
//! registry, its merge script and its slugs. Every expected value is the TS's run on the same tree
//! (oracle/strengthen.oracle.ts, tests/fixtures/oracle/strengthen_pstate.json, each home written `{home}`).
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use base64::Engine as _;
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_engine::project_state::gemini::GeminiResolver;
use wsp_engine::project_state::resolver::{files_under, rewrite_cwd, ProjectStateResolver};
use wsp_engine::project_state::{count_project_state, move_project_state, AgentMoveReport, AgentOutcome, ProjectStateMove};

fn fixture(key: &str, home: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle/strengthen_pstate.json");
    let all: Value = serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap();
    serde_json::from_str(&serde_json::to_string(&all[key]).unwrap().replace("{home}", home)).unwrap()
}

fn home() -> (tempfile::TempDir, String) {
    let d = tempfile::Builder::new().prefix("wsp-ps-").tempdir().unwrap();
    let h = fs::canonicalize(d.path()).unwrap().to_str().unwrap().to_string();
    (d, h)
}

fn put(h: &str, rel: &str, text: &[u8]) {
    let p = Path::new(h).join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// Every file under `h` by its relative path, a link as `<link>`, as the oracle's `tree`.
fn tree(h: &str) -> Value {
    fn walk(root: &Path, rel: &str, out: &mut serde_json::Map<String, Value>) {
        let mut names: Vec<String> = fs::read_dir(root.join(rel)).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        for name in names {
            let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            let meta = fs::symlink_metadata(root.join(&r)).unwrap();
            if meta.is_dir() {
                walk(root, &r, out);
            } else {
                let text = if meta.file_type().is_symlink() { "<link>".to_string() } else { fs::read_to_string(root.join(&r)).unwrap() };
                out.insert(r, Value::String(text));
            }
        }
    }
    let mut out = serde_json::Map::new();
    walk(Path::new(h), "", &mut out);
    Value::Object(out)
}

fn report_json(reports: &[AgentMoveReport]) -> Value {
    json!(reports
        .iter()
        .map(|r| match &r.outcome {
            AgentOutcome::Moved(moved) => json!({
                "agent": r.agent,
                "outcome": "moved",
                "moved": moved.iter().map(|m| {
                    let mut o = json!({ "state": m.state, "files": m.files, "changed": m.changed });
                    if let Some(s) = m.skipped { o["skipped"] = json!(s); }
                    o
                }).collect::<Vec<_>>(),
            }),
            AgentOutcome::Nothing => json!({ "agent": r.agent, "outcome": "nothing" }),
            AgentOutcome::TranscriptOnly => json!({ "agent": r.agent, "outcome": "transcript-only" }),
            AgentOutcome::Failed(e) => json!({ "agent": r.agent, "outcome": "failed", "error": e }),
        })
        .collect::<Vec<_>>())
}

async fn move_in(h: &str, agent: &str) -> Value {
    let mv = ProjectStateMove { from: "/root/p".into(), to: "/new/p".into(), homes: IndexMap::from([(agent.to_string(), h.to_string())]) };
    report_json(&move_project_state(&mv, Some(&[agent.to_string()])).await)
}

/// M1: a failed move is not rolled back: claude's first keyed folder is renamed and rewritten, the one that collides
/// is left, and the report is the failure alone with the exact `<path> already exists`.
#[tokio::test]
async fn a_failed_claude_move_leaves_what_it_already_moved() {
    let (_d, h) = home();
    put(&h, "projects/-root-p/S1.jsonl", b"{\"cwd\":\"/root/p\"}\n");
    put(&h, "projects/-root-p-sub/S2.jsonl", b"{\"cwd\":\"/root/p/sub\"}\n");
    put(&h, "projects/-new-p-sub/S9.jsonl", b"{\"cwd\":\"/new/p/sub\"}\n");
    let want = fixture("claudeHalfMoved", &h);
    assert_eq!(move_in(&h, "claude").await, want["report"]);
    assert_eq!(tree(&h), want["tree"]);
}

/// M2: a rewritten JSONL line is JS `JSON.parse` then `JSON.stringify`: invalid UTF-8 becomes U+FFFD on every line of
/// the file, integers past 2^53 round, an overflow is `null`, a lone surrogate escape survives, numbers respell, a
/// duplicate key keeps its first place and last value, only canonical array-index keys move to the front (nested too),
/// a rewritten line loses its `\r` and its outer spaces, an untouched line keeps its own, and a BOM line is not JSON.
#[tokio::test]
async fn a_rewritten_line_is_js_parse_then_js_stringify() {
    let cases = fixture("rewriteCwd", "");
    let b64 = base64::engine::general_purpose::STANDARD;
    for case in cases.as_array().unwrap() {
        let (_d, h) = home();
        let file = format!("{h}/f.jsonl");
        fs::write(&file, b64.decode(case["input"].as_str().unwrap()).unwrap()).unwrap();
        let n = rewrite_cwd(&file, "/old", "/new", None).await.unwrap();
        assert_eq!(n, case["n"].as_u64().unwrap(), "{}", case["name"]);
        assert_eq!(fs::read(&file).unwrap(), b64.decode(case["output"].as_str().unwrap()).unwrap(), "{}", case["name"]);
    }
}

/// M3: `files_under` follows a linked folder (the paths spelled through the link) and sorts by the whole path's UTF-16
/// units (`-` < `.` < `/` < `0` < `_`).
#[test]
fn files_under_follows_a_linked_folder_and_sorts_whole_paths() {
    let (_d, h) = home();
    put(&h, "real/a.jsonl", b"a\n");
    put(&h, "real/sub/b.jsonl", b"b\n");
    fs::create_dir(format!("{h}/t")).unwrap();
    symlink("../real", format!("{h}/t/linkdir")).unwrap();
    put(&h, "t/x.jsonl", b"x\n");
    for f in ["S1.jsonl", "S1/a.jsonl", "S1-b.jsonl", "S1_c.jsonl", "S10.jsonl"] {
        put(&h, &format!("u/{f}"), b"{}\n");
    }
    let want = fixture("filesUnder", &h);
    assert_eq!(json!(files_under(&format!("{h}/t"), "").unwrap()), want["linked"]);
    assert_eq!(json!(files_under(&format!("{h}/u"), ".jsonl").unwrap()), want["sorted"]);
}

/// M6: gemini rewrites the whole registry through `JSON.stringify(registry, null, 2)`: compact input comes out pretty,
/// `1.0` is `1`, a nested `"9"` goes ahead of `"10"`, other top-level keys stay where they were.
#[tokio::test]
async fn gemini_writes_the_whole_registry_back_as_js_stringify_does() {
    let (_d, h) = home();
    put(&h, "projects.json", br#"{"zz":1,"projects":{"/other":"o","/root/p":"s","/root/p/sub":"s2"},"a":{"10":1,"9":[1.0,{}]},"n":1.0}"#);
    let want = fixture("geminiRegistry", &h);
    assert_eq!(move_in(&h, "gemini").await, want["report"]);
    assert_eq!(tree(&h), want["tree"]);
}

/// M7: the registry is written first and the markers after, with no rollback: a marker with a second hard link fails the
/// move with the hard-link sentence while the registry and the markers before it are already rewritten.
#[tokio::test]
async fn a_gemini_marker_that_cannot_be_written_leaves_the_registry_moved() {
    let (_d, h) = home();
    put(&h, "projects.json", b"{\"projects\":{\"/root/p\":\"s\",\"/root/p/sub\":\"s2\"}}\n");
    put(&h, "tmp/s/.project_root", b"/root/p");
    put(&h, "tmp/s2/.project_root", b"/root/p/sub");
    fs::hard_link(format!("{h}/tmp/s2/.project_root"), format!("{h}/second-name")).unwrap();
    put(&h, "history/s/.project_root", b"/root/p");
    let want = fixture("geminiMarkerLinked", &h);
    assert_eq!(move_in(&h, "gemini").await, want["report"]);
    assert_eq!(tree(&h), want["tree"]);
}

/// M8: registry shapes: no `projects` fails the move with JS's words, a non-string slug fails with node's path words, a
/// target taken by `null` is taken, an unsafe slug is skipped before the check, `projects: []` is nothing.
#[tokio::test]
async fn gemini_registry_shapes_answer_as_the_ts_does() {
    let (_d, scratch) = home();
    for case in fixture("geminiRegistries", &scratch).as_array().unwrap() {
        let (_d, h) = home();
        let registry = case["registry"].as_str().unwrap();
        put(&h, "projects.json", registry.as_bytes());
        let want: Value = serde_json::from_str(&serde_json::to_string(&case["report"]).unwrap().replace(&scratch, &h)).unwrap();
        assert_eq!(move_in(&h, "gemini").await, want, "{registry}");
        assert_eq!(fs::read_to_string(format!("{h}/projects.json")).unwrap(), case["after"].as_str().unwrap(), "{registry}");
    }
}

/// M9: the gemini merge script is the TS's text byte for byte, KEYS carrying every registry key under `from`.
#[tokio::test]
async fn the_gemini_merge_script_is_the_ts_text() {
    let (_d, h) = home();
    put(&h, "projects.json", br#"{"projects":{"/root/p":"s","/root/p/sub":"s2","/other":"o"}}"#);
    let got = GeminiResolver.merge(&h, "/root/p", "/new/p", "/g/.gemini").await.unwrap();
    assert_eq!(json!(got), fixture("geminiMerge", &h));
}

/// M9: a slug registered twice is counted twice, its files and bytes too.
#[tokio::test]
async fn a_slug_registered_twice_is_counted_twice() {
    let (_d, h) = home();
    put(&h, "projects.json", br#"{"projects":{"/root/p":"s","/root/p/sub":"s"}}"#);
    put(&h, "tmp/s/chats/c1.jsonl", b"{}\n");
    put(&h, "tmp/s/.project_root", b"/root/p\n");
    let rows = count_project_state(
        "/root/p",
        &IndexMap::from([("gemini".to_string(), h.clone())]),
        Some(&[("gemini".to_string(), "Gemini".to_string())]),
    )
    .await;
    let want = &fixture("geminiDuplicateSlugs", &h)[0];
    assert_eq!((rows[0].sessions.0, rows[0].bytes.0), (want["sessions"].as_f64().unwrap(), want["bytes"].as_f64().unwrap()));
}

/// M10: pure readings: a rollout climbs out of `sessions/` and stays inside the home; `py_data` moves only canonical
/// array-index keys; the listing keeps a `\r` and needs the mark's space; an empty merge note is kept.
#[test]
fn rollouts_index_keys_listing_lines_and_an_empty_note() {
    use wsp_engine::project_state::codex::rollout_under;
    use wsp_engine::project_state::listing::parse_state_listing;
    use wsp_engine::project_state::merge::{parse_merge_output, MergeOutput};
    use wsp_engine::project_state::py::py_data;
    assert_eq!(rollout_under("/h", "/Users/me/.codex/sessions/../state_5.sqlite").as_deref(), Some("/h/state_5.sqlite"));
    assert_eq!(rollout_under("/h", "/x/sessions/").as_deref(), Some("/h/sessions"));
    let data = py_data(&serde_json::from_str(r#"{"b":1,"01":2,"-1":3,"4294967295":4,"4294967294":5,"0":6}"#).unwrap());
    let b64 = data.split('"').nth(1).unwrap();
    let text = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(b64).unwrap()).unwrap();
    assert_eq!(text, r#"{"0":6,"4294967294":5,"b":1,"01":2,"-1":3,"4294967295":4}"#);
    assert_eq!(parse_state_listing("/a\r\n/b\r\n").unwrap().paths, vec!["/a\r".to_string(), "/b\r".to_string()]);
    assert_eq!(parse_state_listing("!unread\n").unwrap().paths, vec!["!unread".to_string()]);
    assert_eq!(
        parse_merge_output(r#"{"merged":0,"kept":0,"note":""}"#).unwrap(),
        MergeOutput::Merged { merged: 0, kept: 0, note: Some(String::new()) }
    );
}

/// M11: sqlite opens with no busy wait (node:sqlite's timeout 0): a locked index fails at once with `database is
/// locked`; an integer past 2^53 is an error, as node:sqlite without big ints throws.
#[test]
fn sqlite_never_waits_on_a_lock_and_refuses_an_integer_past_2_53() {
    use wsp_engine::project_state::sqlite::{select_rows, update_rows, RowUpdate};
    let (_d, h) = home();
    let db = format!("{h}/state.sqlite");
    let holder = rusqlite::Connection::open(&db).unwrap();
    holder.execute_batch("create table t(s text, n integer); insert into t values ('a', 9007199254740993);").unwrap();
    let err = select_rows(&db, "select n from t", &[]).unwrap_err().to_string();
    assert_eq!(err, "Value is too large to be represented as a JavaScript number: 9007199254740993");
    holder.execute_batch("begin immediate").unwrap();
    let started = std::time::Instant::now();
    let err = update_rows(&db, &[RowUpdate { sql: "update t set s = 'x'".into(), params: vec![] }]).unwrap_err().to_string();
    assert_eq!(err, "database is locked");
    assert!(started.elapsed() < std::time::Duration::from_millis(500), "{:?}", started.elapsed());
}
