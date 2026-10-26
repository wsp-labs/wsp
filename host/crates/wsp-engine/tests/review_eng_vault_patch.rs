// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the vault's export and import (vault.rs, vault/export.rs, vault/import.rs) and of project state
//! (project_state/*): the thirteen tests that fail against the TS's answers. Each expected value is what a vitest probe
//! printed when it drove the TS on the same input; the comment above each test names the TS line and the probe line.
//! Nothing here reads a file from outside the crate.
mod common;

use std::process::Command;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use common::{bash, res, ExecFn, FakeMachine, RunFn};
use wsp_engine::errors::RoadError;
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::machine::{ExecOptions, RunOptions};
use wsp_engine::project_state::resolver::rewrite_cwd;
use wsp_engine::project_state::{
    move_project_state, parse_merge_output, parse_state_listing, project_state_resolvers, AgentOutcome, MergeOutput, ProjectStateMove,
};
use wsp_engine::vault::*;

// ---- deep nesting in a line read off disk (finding 1) ----

/// The scenario runs in a child copy of this test binary so a stack overflow (an abort, not a panic) is a failed
/// assertion in the parent and not the end of every other test in the file.
#[tokio::test]
async fn review_child_deep_line() {
    let Ok(spec) = std::env::var("REVIEW_DEEP") else { return };
    let (depth, cwd) = spec.split_once(':').unwrap();
    let depth: usize = depth.parse().unwrap();
    if cwd == "merge" {
        let line = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        let said = match parse_merge_output(&format!("noise\n{line}\n")) {
            Ok(o) => format!("ok {o:?}"),
            Err(e) => format!("error {}", e.to_string().chars().take(40).collect::<String>()),
        };
        println!("CHILD {said} | second line ");
        return;
    }
    let dir = tempfile::Builder::new().prefix("deep-").tempdir().unwrap();
    let file = dir.path().join("t.jsonl");
    std::fs::write(&file, format!("{{\"cwd\":\"{cwd}\",\"x\":{}{}}}\n{{\"cwd\":\"/work/p/ok\"}}\n", "[".repeat(depth), "]".repeat(depth))).unwrap();
    let said = match rewrite_cwd(file.to_str().unwrap(), "/work/p", "/work/q", None).await {
        Ok(n) => format!("changed {n}"),
        Err(e) => format!("error {e}"),
    };
    let second = std::fs::read_to_string(&file).unwrap().split('\n').nth(1).unwrap().to_string();
    println!("CHILD {said} | second line {second}");
}

fn deep_in_child(depth: usize, cwd: &str) -> (String, std::process::ExitStatus) {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "review_child_deep_line", "--nocapture", "--test-threads=1"])
        .env("REVIEW_DEEP", format!("{depth}:{cwd}"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).lines().find_map(|l| l.find("CHILD ").map(|at| l[at..].to_string())).unwrap_or_default();
    (format!("{text}\n{}", String::from_utf8_lossy(&out.stderr).lines().find(|l| l.contains("overflow") || l.contains("SIG")).unwrap_or("")), out.status)
}

// TS: project-state/resolver.ts:219 (JSON.parse does not recurse); probe deep.probe.ts printed `DEEP 100000 untouched line changed 1 ... second line now "{\"cwd\":\"/work/q/ok\"}"`.
#[test]
fn a_line_nested_a_hundred_thousand_deep_is_passed_over_and_the_next_line_still_rewritten() {
    let (said, status) = deep_in_child(100_000, "/other");
    assert!(status.success(), "the child died ({status:?}) reading one deeply nested line: {said}");
    assert!(said.contains("changed 1") && said.contains("{\"cwd\":\"/work/q/ok\"}"), "{said}");
}

// TS: project-state/resolver.ts:225 (JSON.stringify throws); probe deep.probe.ts printed `DEEP 100000 rewritten line threw RangeError: Maximum call stack size exceeded ... second line now "{\"cwd\":\"/work/p/ok\"}"`.
#[test]
fn a_line_nested_a_hundred_thousand_deep_that_must_be_rewritten_fails_the_rewrite_and_leaves_the_file() {
    let (said, status) = deep_in_child(100_000, "/work/p");
    assert!(status.success(), "the child died ({status:?}) rewriting one deeply nested line: {said}");
    assert!(said.contains("error") && said.contains("{\"cwd\":\"/work/p/ok\"}"), "{said}");
}

// TS: project-state/merge.ts:14-30 (parseMergeOutput); probe deepmerge.probe.ts printed `DEEPMERGE 100000 threw Error "the merge script ended with: [[[["`.
#[test]
fn a_merge_scripts_last_line_nested_a_hundred_thousand_deep_is_an_error_naming_it() {
    let (said, status) = deep_in_child(100_000, "merge");
    assert!(status.success(), "the child died ({status:?}) reading one deeply nested merge line: {said}");
    assert!(said.contains("error the merge script ended with: [[["), "{said}");
}

// ---- U+10F800 to U+10FFFF (finding 2) ----

async fn rewritten(line: &str) -> String {
    let dir = tempfile::Builder::new().prefix("puab-").tempdir().unwrap();
    let file = dir.path().join("t.jsonl");
    std::fs::write(&file, format!("{line}\n")).unwrap();
    let n = rewrite_cwd(file.to_str().unwrap(), "/work/p", "/work/q", None).await.unwrap();
    assert_eq!(n, 1);
    std::fs::read_to_string(&file).unwrap()
}

// TS: project-state/resolver.ts:225 (JSON.stringify writes the character raw); probe puab.probe.ts printed `PUAB raw U+10F800 changed 1 => "{\"cwd\":\"/work/q\",\"text\":\"a􏠀b\"}\n" hex 223a2261f48fa08062227d0a` (and the same raw character for U+10FFFF and for both escaped pairs).
#[tokio::test]
async fn a_private_use_character_from_plane_16_survives_a_rewrite_of_its_line() {
    let mut lost = vec![];
    for (name, line, want) in [
        ("raw U+10F800", "{\"cwd\":\"/work/p\",\"text\":\"a\u{10F800}b\"}", "{\"cwd\":\"/work/q\",\"text\":\"a\u{10F800}b\"}\n"),
        ("raw U+10FFFF", "{\"cwd\":\"/work/p\",\"text\":\"a\u{10FFFF}b\"}", "{\"cwd\":\"/work/q\",\"text\":\"a\u{10FFFF}b\"}\n"),
        ("the escaped pair for U+10FFFF", "{\"cwd\":\"/work/p\",\"text\":\"a\\udbff\\udfffb\"}", "{\"cwd\":\"/work/q\",\"text\":\"a\u{10FFFF}b\"}\n"),
        ("the escaped pair for U+10F800", "{\"cwd\":\"/work/p\",\"text\":\"\\udbfe\\udc00\"}", "{\"cwd\":\"/work/q\",\"text\":\"\u{10F800}\"}\n"),
    ] {
        let got = rewritten(line).await;
        if got != want {
            lost.push(format!("{name}: rust wrote {got:?}, the TS wrote {want:?}"));
        }
    }
    assert!(lost.is_empty(), "{}", lost.join("\n"));
}

// ---- the unread line's base64 (the guess) ----

/// `{"agent":"codex","store":"/root/.codex/state_5.sqlite","why":"locked ??>> ok!"}` as standard base64.
const STD: &str = "eyJhZ2VudCI6ImNvZGV4Iiwic3RvcmUiOiIvcm9vdC8uY29kZXgvc3RhdGVfNS5zcWxpdGUiLCJ3aHkiOiJsb2NrZWQgPz8+PiBvayEifQ==";

fn unread_after(line: &str) -> Result<Vec<String>, String> {
    parse_state_listing(&format!("/a\n!unread {line}\n/b\n")).map(|l| l.unread.into_iter().map(|u| u.why).collect()).map_err(|e| e.to_string())
}

// TS: project-state/listing.ts:38 (Buffer.from(..., "base64") is lenient); probe b64.probe.ts printed `UNREADCASE codex/unpadded ok ["codex"]`, `codex/urlsafe`, `codex/trailing-cr`, `codex/leading-space` and `codex/inner-space` all `ok ["codex"]`.
#[test]
#[ignore = "ruling 7 of the fix round: the strict base64 read in listing.rs stays; node reads these leniently (listing.ts:38), so lifting it is one node_base64_decode"]
fn an_unread_line_in_unpadded_url_safe_or_spaced_base64_reads_as_node_reads_it() {
    let unpadded = STD.trim_end_matches('=').to_string();
    let url_safe = STD.replace('+', "-").replace('/', "_");
    let wrapped = format!("{}\r{}", &STD[..40], &STD[40..]);
    let spaced = format!("{} {}", &STD[..40], &STD[40..]);
    let cases: Vec<(&str, String)> = vec![
        ("standard", STD.to_string()),
        ("unpadded", unpadded),
        ("url-safe", url_safe),
        ("a carriage return inside the line", wrapped),
        ("a space inside the line", spaced),
        ("a trailing carriage return", format!("{STD}\r")),
        ("a leading space", format!(" {STD}")),
    ];
    let refused: Vec<String> = cases
        .iter()
        .filter_map(|(name, line)| match unread_after(line) {
            Ok(whys) if whys == ["locked ??>> ok!"] => None,
            other => Some(format!("{name}: {other:?}")),
        })
        .collect();
    assert!(refused.is_empty(), "node reads all seven; the Rust refuses {} of them:\n{}", refused.len(), refused.join("\n"));
}

// ---- sqlite: words and the smallest integer ----

// TS: project-state/sqlite.ts:43 (`new DatabaseSync(db)` in inTransaction); probe special.probe.ts, cases 10-codex-directory, 15-hermes-directory and 20-opencode-directory, wrote the report `"error": "unable to open database file"` into each ts.json.
#[tokio::test]
async fn a_store_that_will_not_open_for_the_move_fails_with_sqlites_words_alone() {
    let mut said = vec![];
    for (agent, file) in [("codex", "state_5.sqlite"), ("hermes", "state.db"), ("opencode", "opencode.db")] {
        let home = tempfile::Builder::new().prefix("dirdb-").tempdir().unwrap();
        std::fs::create_dir(home.path().join(file)).unwrap();
        let home_str = home.path().to_str().unwrap().to_string();
        let mv = ProjectStateMove { from: "/work/p".into(), to: "/work/q".into(), homes: indexmap::IndexMap::from([(agent.to_string(), home_str.clone())]) };
        let report = move_project_state(&mv, Some(&[agent.to_string()])).await;
        let AgentOutcome::Failed(why) = &report[0].outcome else { panic!("{:?}", report[0].outcome) };
        said.push(format!("{agent}: {}", why.replace(&home_str, "<home>")));
    }
    let want = "unable to open database file";
    assert!(said.iter().all(|s| s.ends_with(want)), "node says only `{want}`; here:\n{}", said.join("\n"));
}

fn hermes_home_with_n(n: &str) -> tempfile::TempDir {
    let home = tempfile::Builder::new().prefix("bi-").tempdir().unwrap();
    let conn = rusqlite::Connection::open(home.path().join("state.db")).unwrap();
    conn.execute_batch(&format!(
        "create table sessions (id text primary key, cwd, git_repo_root, n integer); create table messages (id integer primary key autoincrement, session_id text, content, raw); insert into sessions values ('s1', '/work/p', null, {n});"
    ))
    .unwrap();
    home
}

// TS: project-state/sqlite.ts:55 and :78 (selectRows, projectRows; node:sqlite 22.23.4 lets i64::MIN through); probe bigint.probe.ts printed `BIGINT merge -9223372036854775808 script 3009`.
#[tokio::test]
async fn the_smallest_i64_is_read_as_node_sqlite_reads_it() {
    let hermes = project_state_resolvers().get("hermes").unwrap();
    let home = hermes_home_with_n("-9223372036854775808");
    let got = hermes.merge(home.path().to_str().unwrap(), "/work/p", "/work/q", "/guest/home").await;
    let script = got.expect("node reads it").expect("the project has a row");
    assert_eq!(script.len(), 3009);
}

// ---- the merge script's last line ----

// TS: project-state/merge.ts:25 (typeof o["merged"] === "number"); probe mergeout.probe.ts printed `MERGEOUT {"merged":-1,"kept":0} => {"merged":-1,"kept":0}`, likewise 1.5 and `{"merged":1e+30,"kept":0}` for 1e30.
#[test]
fn a_merge_line_with_a_negative_fractional_or_huge_count_is_read_as_the_ts_reads_it() {
    let read = |line: &str| parse_merge_output(line).map_err(|e| e.to_string());
    assert!(matches!(read(r#"{"merged":-1,"kept":0}"#), Ok(MergeOutput::Merged { .. })), "{:?}", read(r#"{"merged":-1,"kept":0}"#));
    assert!(matches!(read(r#"{"merged":1.5,"kept":0}"#), Ok(MergeOutput::Merged { .. })), "{:?}", read(r#"{"merged":1.5,"kept":0}"#));
    match read(r#"{"merged":1e30,"kept":0}"#) {
        Ok(MergeOutput::Merged { merged, .. }) => assert_eq!(merged as f64, 1e30),
        other => panic!("{other:?}"),
    }
}

// ---- the provider's refusal body, nested past serde_json's depth limit ----

struct Refuses {
    status: u16,
    body: String,
    calls: Mutex<usize>,
}

#[async_trait(?Send)]
impl Fetch for Refuses {
    async fn fetch(&self, _req: FetchRequest) -> Result<FetchResponse, RoadError> {
        *self.calls.lock().unwrap() += 1;
        Ok(FetchResponse { status: self.status, headers: vec![], body: self.body.clone().into_bytes() })
    }
}

async fn refused_with(status: u16, body: String) -> (String, usize) {
    let fetch = Arc::new(Refuses { status, body, calls: Mutex::new(0) });
    let exec: ExecFn = Arc::new(|_cmd: String, _o: ExecOptions| Box::pin(async { Ok(res(0, "", "")) }));
    let machine = FakeMachine::new(exec);
    let o = VaultOptions { fetch: Some(fetch.clone()), ..Default::default() };
    let e = import_into(&machine, b"x", "/d", &o).await.unwrap_err();
    let calls = *fetch.calls.lock().unwrap();
    (e.to_string().chars().take(150).collect(), calls)
}

// TS: vault.ts:295-310 (parseBody, JSON.parse at any depth); probe body.probe.ts printed `BODY error beside an array 1000 deep calls 1 "vault import upload failed: HTTP 413 Payload Too Large; the upload takes at most 33554432 bytes per PUT"` (127 and 129 deep the same).
#[tokio::test(start_paused = true)]
async fn a_refusal_body_nested_deeper_than_128_is_still_read_for_its_error_and_limit() {
    let want = "vault import upload failed: HTTP 413 Payload Too Large; the upload takes at most 33554432 bytes per PUT";
    let mut wrong = vec![];
    for depth in [100usize, 126, 127, 129, 1000] {
        let body = format!("{{\"error\":\"Payload Too Large\",\"limit\":33554432,\"x\":{}{}}}", "[".repeat(depth), "]".repeat(depth));
        let (said, calls) = refused_with(413, body).await;
        if said != want || calls != 1 {
            wrong.push(format!("depth {depth}: {calls} call(s), said {said:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

// TS: vault.ts:326-329 (parseBody then classify, shouldRetry); probe body.probe.ts printed `BODY 502 snapshot refusal beside 200 deep calls 1 "vault import upload failed: HTTP 502 Failed to snapshot sandbox"`.
#[tokio::test(start_paused = true)]
async fn a_502_naming_the_snapshot_beside_a_deep_array_is_final_at_once() {
    let body = format!("{{\"error\":\"Failed to snapshot sandbox\",\"x\":{}{}}}", "[".repeat(200), "]".repeat(200));
    let (said, calls) = refused_with(502, body).await;
    assert_eq!((said.as_str(), calls), ("vault import upload failed: HTTP 502 Failed to snapshot sandbox", 1));
}

// ---- exporting into a path this computer cannot open ----

// TS: vault.ts:115-128 (downloadFile, createWriteStream); probe into.probe.ts printed `INTO missing parent folder => threw Error: ENOENT: no such file or directory, open '/tmp/into-83xOKu/no/such/out.tgz'`.
#[tokio::test]
async fn an_export_into_a_folder_that_is_not_there_fails_with_nodes_words() {
    let dir = tempfile::Builder::new().prefix("into-").tempdir().unwrap();
    let into = dir.path().join("no/such/out.tgz");
    let exec: ExecFn = Arc::new(|_cmd: String, _o: ExecOptions| Box::pin(async { Ok(res(0, "12\n", "")) }));
    let machine = FakeMachine::new(exec);
    let fetch = Arc::new(Refuses { status: 200, body: "hello world!".into(), calls: Mutex::new(0) });
    let o = VaultOptions { fetch: Some(fetch), ..Default::default() };
    let groups = vec![ArchiveGroup { root: "/".into(), paths: vec!["/x".into()] }];
    let e = export_paths_into(&machine, &groups, &into, &o).await.unwrap_err();
    assert_eq!(e.to_string(), format!("ENOENT: no such file or directory, open '{}'", into.display()));
    assert!(!into.exists());
}

// ---- a caller that stops waiting ----

/// A signed-URL upload that lands the part at the path the URL names.
struct PutFetch;

#[async_trait(?Send)]
impl Fetch for PutFetch {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let path = common::url_decode(req.url.split_once("path=").unwrap().1.split('&').next().unwrap());
        std::fs::write(&path, req.body.unwrap()).unwrap();
        Ok(FetchResponse { status: 200, headers: vec![], body: b"{}".to_vec() })
    }
}

/// A gzipped tar of six top-level entries (`dir`, `top.txt`, `old.txt`, `sub`, `empty`, `ln`), as this computer's tar writes it.
fn six_entry_tar() -> Vec<u8> {
    let src = tempfile::Builder::new().prefix("src-").tempdir().unwrap();
    let at = src.path().to_str().unwrap();
    let made = Command::new("bash")
        .arg("-c")
        .arg(format!("cd {at} && mkdir -p dir sub/deep empty && echo -n new > dir/new.txt && echo -n top > top.txt && echo -n replaced > old.txt && echo -n x > sub/deep/x && ln -s top.txt ln"))
        .status()
        .unwrap();
    assert!(made.success());
    let out = Command::new("tar").args(["-czf", "-", "-C", at, "dir", "top.txt", "old.txt", "sub", "empty", "ln"]).output().unwrap();
    assert!(out.status.success());
    out.stdout
}

// TS: vault.ts:424-474 (importInto carries on after its caller stops waiting); probe drop.probe.ts printed `DROP 1.5 s later: tmp [] dest ["dir","empty","ln","old.txt","sub","top.txt"]`.
#[tokio::test]
#[ignore = "needs a ruling: import_into borrows its machine, which is not Send, so nothing can run on once a caller drops the future; it takes an owned machine and a LocalSet to spawn on (vault.ts:424-474)"]
async fn a_caller_that_stops_waiting_on_an_import_still_gets_it_finished_and_the_part_removed() {
    let root = tempfile::Builder::new().prefix("drop-").tempdir().unwrap();
    std::fs::create_dir(root.path().join("tmp")).unwrap();
    std::fs::create_dir(root.path().join("dest")).unwrap();
    let tar = six_entry_tar();
    let exec: ExecFn = Arc::new(|cmd: String, o: ExecOptions| Box::pin(async move { Ok(bash(&cmd, o.stdin, None).await) }));
    let run: RunFn = Arc::new(|cmd: String, _o: RunOptions| {
        Box::pin(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(bash(&cmd, None, None).await)
        })
    });
    let mut machine = FakeMachine::new(exec);
    machine.run = Some(run);
    let o = VaultOptions { fetch: Some(Arc::new(PutFetch)), tmp_dir: Some(format!("{}/tmp", root.path().display())), ..Default::default() };
    let dest = format!("{}/dest", root.path().display());
    let waited = tokio::time::timeout(std::time::Duration::from_millis(50), import_into(&machine, &tar, &dest, &o)).await;
    assert!(waited.is_err(), "the extract was meant to still be pending");
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    let names = |p: &str| std::fs::read_dir(format!("{}/{p}", root.path().display())).unwrap().count();
    assert_eq!((names("tmp"), names("dest")), (0, 6), "the part is still in tmp, nothing was extracted");
}

// ---- the resolver registry ----

// TS: project-state/index.ts:27 (PROJECT_STATE_RESOLVERS is one module-level const Map, one per process); no probe printed it, there is nothing to run.
#[test]
fn the_resolver_registry_is_one_map_for_the_whole_process() {
    let here = project_state_resolvers() as *const _ as usize;
    let there = std::thread::spawn(|| project_state_resolvers() as *const _ as usize).join().unwrap();
    assert_eq!(here, there);
}
