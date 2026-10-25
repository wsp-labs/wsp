// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sub_life_reads.md: the pure half of sub-life.ts, against the TS's own answers
//! (tests/fixtures/machines_b/sub_life_reads.json, written by oracle/machines_b_reads.oracle.ts) and real git.
mod machines_b_support;

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use serde_json::Value;

use machines_b_support::*;
use wsp_engine::machine::MachineBackend;
use wsp_core::clock::Cancel;
use wsp_core::types::internal::{KindModule, SessionEntry, SubStartRecord, GONE_REASON, SUB_STARTS};
use wsp_core::types::wiring::{LiveWorkspace, StageReport, SubRecord, WorkspaceRecord};
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_life::*;
use wsp_words::exit::HostError;

fn fixture() -> Value {
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/machines_b/sub_life_reads.json");
    serde_json::from_str(&std::fs::read_to_string(at).unwrap()).unwrap()
}

const AT: &str = "2026-10-14T12:00:00.000Z";
const B: &str = "wsp/fix-the-flaky-upload-test";

#[test]
fn constants_hold_the_ts_values() {
    assert_eq!(SUB_SESSIONS, "sub-sessions");
    assert_eq!(SUB_SESSION_MAX_BYTES, 67_108_864);
    assert_eq!(SUB_STOP_AFTER_DELETE_MS, 600_000.0);
    assert_eq!(SUB_DELETE_RETRY_MS, 600_000.0);
    assert_eq!(SUB_LINE_WAIT_MS, 300_000.0);
    // A constant that holds without a body proves nothing alone: the command below must run too.
    let _ = sub_result_command("/root/web", B, "abc");
}

#[test]
fn result_command_is_byte_for_byte_the_ts_line() {
    for c in fixture()["commands"].as_array().unwrap() {
        let got = sub_result_command(c["folder"].as_str().unwrap(), c["branch"].as_str().unwrap(), c["baseSha"].as_str().unwrap());
        assert_eq!(got, c["command"].as_str().unwrap(), "folder {}", c["folder"]);
    }
}

#[test]
fn parses_every_answer_as_the_ts_does() {
    for p in fixture()["parsed"].as_array().unwrap() {
        let stdout = p["stdout"].as_str().unwrap();
        let got = sub_result_of(stdout, B, AT);
        let want = &p["result"];
        match got {
            None => assert!(want.is_null(), "{stdout:?} answered none, the TS answered {want}"),
            Some(r) => assert_eq!(text(&r), serde_json::to_string(want).unwrap(), "{stdout:?}"),
        }
    }
}

/// A remote holding main and a pull request branch five commits deep, and the empty folder a sub-thread opens in
/// (sub-result-read.test.ts:28).
struct Remote {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    url: String,
    folder: std::path::PathBuf,
    head: String,
}

fn git(at: &Path, args: &[&str]) -> String {
    let out = Command::new("git").arg("-C").arg(at).args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn sh(at: &Path, line: &str) -> String {
    let out = Command::new("sh").arg("-c").arg(line).current_dir(at).output().unwrap();
    String::from_utf8(out.stdout).unwrap()
}

fn remote() -> Remote {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let src = root.join("src");
    std::fs::create_dir(&src).unwrap();
    git(&src, &["init", "-q", "-b", "main"]);
    for i in 1..=5 {
        std::fs::write(src.join("f"), format!("{i}\n")).unwrap();
        git(&src, &["add", "f"]);
        git(&src, &["commit", "-qm", &format!("c{i}")]);
    }
    git(&src, &["switch", "-qc", "acme-pr"]);
    std::fs::write(src.join("g"), "pr\n").unwrap();
    git(&src, &["add", "g"]);
    git(&src, &["commit", "-qm", "pr"]);
    let bare = root.join("remote.git");
    assert!(Command::new("git").args(["clone", "-q", "--bare"]).arg(&src).arg(&bare).status().unwrap().success());
    let folder = root.join("web");
    std::fs::create_dir(&folder).unwrap();
    let head = git(&src, &["rev-parse", "HEAD"]);
    Remote { url: format!("file://{}", bare.display()), folder, head, root, _dir: dir }
}

/// One of the fixture's preambles, its placeholders filled.
fn preamble(which: &str, repo: &str, head: &str, start: &str) -> Vec<String> {
    fixture()["preambles"][which].as_array().unwrap().iter().map(|l| l.as_str().unwrap().replace("{REPO}", repo).replace("{HEAD}", head).replace("{START}", start)).collect()
}

fn read(folder: &Path, base: &str) -> wsp_wire::views::sub_row::SubResult {
    let stdout = sh(folder, &sub_result_command(folder.to_str().unwrap(), B, base));
    sub_result_of(&stdout, B, AT).expect("the read carries counts")
}

#[test]
fn counts_only_the_commits_made_on_the_machine_not_the_history_it_started_from() {
    let r = remote();
    // The brokered preamble's git lines, as the TS writes them (fixture `preambles`), run as the agent runs them.
    for line in preamble("brokered", &r.url, &r.head, "") {
        sh(&r.folder, &line);
    }
    sh(&r.folder, &format!("git fetch -q --deepen=4 origin {}", r.head));
    assert_eq!(sh(&r.folder, "git rev-list --count HEAD").trim(), "5");
    let first = read(&r.folder, &r.head);
    assert_eq!((first.unpushed.get(), first.changed.get(), first.pushed_sha.clone()), (0, 0, None));
    std::fs::write(r.folder.join("h"), "review\n").unwrap();
    git(&r.folder, &["add", "h"]);
    git(&r.folder, &["commit", "-qm", "review notes"]);
    let second = read(&r.folder, &r.head);
    assert_eq!((second.unpushed.get(), second.changed.get(), second.pushed_sha.clone()), (1, 0, None));
    git(&r.folder, &["push", "-q", "-u", "origin", B]);
    let third = read(&r.folder, &r.head);
    assert_eq!((third.unpushed.get(), third.changed.get(), third.commits.get()), (0, 0, 1));
    assert_eq!(third.pushed_sha.as_deref(), Some(git(&r.folder, &["rev-parse", "HEAD"]).as_str()));
}

#[test]
fn still_answers_on_a_new_machine_that_fetched_its_pushed_branch_and_not_the_commit() {
    let r = remote();
    let src = r.root.join("src");
    let start = git(&src, &["rev-parse", "HEAD~1"]);
    git(&src, &["push", "-q", r.root.join("remote.git").to_str().unwrap(), &format!("HEAD:refs/heads/{B}")]);
    // The resume preamble's git lines with the branch pushed, as the TS writes them.
    for line in preamble("resumed", &r.url, "", &start) {
        sh(&r.folder, &line);
    }
    let got = read(&r.folder, &start);
    assert_eq!((got.unpushed.get(), got.changed.get()), (0, 0));
    assert_eq!(got.pushed_sha.as_deref(), Some(r.head.as_str()));
}

#[test]
fn pack_and_land_commands_are_byte_for_byte_the_ts_lines() {
    let f = fixture();
    let file_of = |v: &Value| SessionFile { folder: v["folder"].as_str().unwrap().to_string(), name: v["name"].as_str().unwrap().to_string(), depth: v["depth"].as_u64().unwrap() as u32 };
    for p in f["packs"].as_array().unwrap() {
        assert_eq!(sub_session_pack_command(&file_of(&p["file"]), p["to"].as_str().unwrap()), p["command"].as_str().unwrap());
    }
    for l in f["lands"].as_array().unwrap() {
        assert_eq!(sub_session_land_command(&file_of(&l["file"]), l["path"].as_str().unwrap(), l["from"].as_str().unwrap()), l["command"].as_str().unwrap(), "path {}", l["path"]);
    }
}

#[test]
fn rows_read_as_the_ts_rows() {
    for r in fixture()["rows"].as_array().unwrap() {
        let record: WorkspaceRecord = serde_json::from_value(r["record"].clone()).unwrap();
        let got = sub_row_of(&record);
        match got {
            None => assert!(r["row"].is_null(), "no row for {}, the TS has {}", r["record"], r["row"]),
            Some(row) => assert_eq!(text(&row), serde_json::to_string(&r["row"]).unwrap(), "record {}", r["record"]),
        }
    }
}

/// A context whose `backend_for` answers a backend with the keep given, none, or fails.
struct Keeps(Option<Arc<dyn MachineBackend>>);

impl BackendFor for Keeps {
    fn backend_for(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.0.clone().ok_or_else(|| HostError::new("no kind"))
    }
}

/// A backend with no lifecycle at all.
struct NoLife;

#[async_trait::async_trait(?Send)]
impl MachineBackend for NoLife {
    fn capabilities(&self) -> wsp_engine::machine::Capabilities {
        boat_capabilities()
    }
    fn pricing(&self) -> Arc<dyn wsp_engine::machine::BackendPricing> {
        Arc::new(FakePricing)
    }
    async fn create(&self, _: wsp_engine::machine::MachineSpec, _: Option<wsp_engine::machine::Waiting>, _: Option<String>) -> Result<Arc<dyn wsp_engine::machine::Machine>, wsp_engine::errors::EngineError> {
        unreachable!()
    }
    async fn get(&self, _: &str) -> Result<Arc<dyn wsp_engine::machine::Machine>, wsp_engine::errors::EngineError> {
        unreachable!()
    }
    async fn list(&self, _: Option<&indexmap::IndexMap<String, String>>) -> Result<Vec<wsp_engine::machine::MachineListRow>, wsp_engine::errors::EngineError> {
        Ok(vec![])
    }
    async fn delete_snapshot(&self, _: &str) -> Result<(), wsp_engine::errors::EngineError> {
        Ok(())
    }
}

#[test]
fn disk_keep_reads_as_the_ts_reads() {
    for k in fixture()["keeps"].as_array().unwrap() {
        let mut record = serde_json::from_value::<Value>(sub_record("ws_1", "running")).unwrap();
        for (key, v) in k["record"].as_object().unwrap() {
            record[key] = v.clone();
        }
        let record: WorkspaceRecord = serde_json::from_value(record).unwrap();
        let ctx = match &k["diskKeptMs"] {
            Value::String(s) if s == "throws" => Keeps(None),
            Value::String(s) if s == "nolife" => Keeps(Some(Arc::new(NoLife))),
            ms => {
                let b = FakeBackend::new();
                *b.lifecycle.disk_kept_ms.lock().unwrap() = ms.as_u64();
                Keeps(Some(b))
            }
        };
        let stopped = k["stoppedAt"].as_f64();
        assert_eq!(disk_kept_ms(&ctx, &record), k["keptMs"].as_f64(), "{k}");
        assert_eq!(disk_kept_until(&ctx, &record, stopped), k["until"].as_f64(), "{k}");
    }
}
