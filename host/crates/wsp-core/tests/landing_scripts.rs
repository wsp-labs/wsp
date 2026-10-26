// SPDX-License-Identifier: AGPL-3.0-only
//! Ports of packages/runtime/test/project-seed-script.test.ts, the brokered-clone cases of
//! packages/runtime/test/branch-of-record.test.ts (:158-363) and the free-name case of
//! packages/runtime/test/box-folder.test.ts (:603), and the TS's own answers for the script functions of
//! project-landing.ts, read from tests/fixtures/core_rest/landing_scripts.json (oracle/core_rest.oracle.ts,
//! the "project landing scripts" case).
mod rest;
use rest::*;

use std::cell::RefCell;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use base64::Engine as _;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Value};
use wsp_core::project_landing::{
    brokered_git_env, clone_lines, clone_script, free_folder_script, guest_memory_dir, last_line, on_record_lines, patch_cleanup_script,
    patch_script, read_unsaved, seed_into_script, seed_rest_script, under_login_home, unsaved_of, unsaved_script, BrokeredClone, CloneAsk,
    GitEnvRepo, PatchStep, RunHere, MEMORY_KEPT_MARK, STEP_MS,
};
use wsp_core::project_sources::project_source;
use wsp_wire::types::daemon_contract::BROKER_PORT;
use wsp_wire::views::workspace::BranchOfRecord;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

const SHA: &str = "1d2c3b4a5f6e7d8c9b0a1d2c3b4a5f6e7d8c9b0a";

fn fx(key: &str) -> Value {
    let all = fixture("landing_scripts");
    all.get(key).unwrap_or_else(|| panic!("landing_scripts.json has no {key}")).clone()
}

fn rows(key: &str) -> Vec<Vec<Value>> {
    fx(key).as_array().unwrap_or_else(|| panic!("{key} is not rows")).iter().map(|r| r.as_array().expect("a row").clone()).collect()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().expect("a list").iter().map(|s| s.as_str().expect("a string").to_string()).collect()
}

fn text(v: &Value) -> String {
    v.as_str().expect("a string").to_string()
}

fn names(rows: &[Vec<Value>]) -> Vec<String> {
    rows.iter().map(|r| text(&r[0])).collect()
}

fn pairs_of(env: &IndexMap<String, String>) -> Vec<(String, String)> {
    env.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn fixture_pairs(v: &Value) -> Vec<(String, String)> {
    v.as_object().expect("an object").iter().map(|(k, v)| (k.clone(), text(v))).collect()
}

fn s(p: &Path) -> &str {
    p.to_str().expect("a utf-8 path")
}

fn github() -> Rc<dyn wsp_core::project_sources::ProjectSourceModule> {
    project_source("github").unwrap()
}

fn ask(kind: &str, remote: &str, checkout: &str, computer: &str) -> CloneAsk {
    CloneAsk {
        source: project_source(kind).unwrap(),
        remote: remote.into(),
        checkout: checkout.into(),
        computer: computer.into(),
        branch: None,
        setup_git: None,
        brokered: None,
    }
}

fn record(branch: &str, start_sha: Option<&str>, owner: &str, name: &str) -> BranchOfRecord {
    let mut v = json!({ "repoId": 7, "owner": owner, "name": name, "branch": branch });
    if let Some(sha) = start_sha {
        v["startSha"] = json!(sha);
    }
    wire(&v)
}

fn lab(branch: &str, start_sha: Option<&str>, repo: (&str, &str), held: bool) -> BrokeredClone {
    BrokeredClone { of_record: record(branch, start_sha, repo.0, repo.1), origin: "acme/lab".into(), held }
}

fn lab_own(branch: &str, start_sha: Option<&str>) -> BrokeredClone {
    lab(branch, start_sha, ("acme", "lab"), false)
}

fn github_ask(brokered: Option<BrokeredClone>) -> CloneAsk {
    CloneAsk {
        source: github(),
        branch: Some("main".into()),
        brokered,
        ..ask("github", "https://github.com/acme/lab", "/root/lab", "Boat")
    }
}

fn step() -> PatchStep {
    PatchStep { branch: "refactor/dashboard-polish".into(), base: "abc123".into(), back: "main".into() }
}

fn env_pairs(env: &[(&str, &str)]) -> Vec<(String, String)> {
    env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn git_with(at: &Path, args: &[&str], env: &[(String, String)]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(at)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(env.iter().map(|(k, v)| (k, v)))
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?} in {}: {}", at.display(), String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn git_as(at: &Path, args: &[&str]) -> String {
    let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@t"];
    all.extend_from_slice(args);
    git(at, &all)
}

fn exited(r: &ExecResult) -> i64 {
    r.exit_code.get()
}

fn ok(r: &ExecResult) {
    assert_eq!(exited(r), 0, "stderr: {}", r.stderr);
}

fn last_of(out: &str) -> String {
    out.trim().split('\n').next_back().unwrap().to_string()
}

// ---------------------------------------------------------------------------------------------------------------
// The TS's own answers.

fn clone_cases() -> Vec<(&'static str, CloneAsk)> {
    let o = |brokered: Option<BrokeredClone>| github_ask(brokered);
    let fork = ("ana", "lab-fork");
    vec![
        ("brokered", o(Some(lab_own("wsp/pricing-page", Some(SHA))))),
        ("brokered fork", o(Some(lab("main", Some(SHA), fork, false)))),
        ("brokered held own", o(Some(lab("fix-typo", Some(SHA), ("acme", "lab"), true)))),
        ("brokered held fork", o(Some(lab("main", Some(SHA), fork, true)))),
        ("brokered no sha", o(Some(lab_own("wsp/x", None)))),
        ("brokered held no sha", o(Some(lab("wsp/x", None, ("acme", "lab"), true)))),
        ("brokered no branch", CloneAsk { branch: None, ..o(Some(lab_own("wsp/x", Some(SHA)))) }),
        ("brokered case origin", o(Some(lab("b", Some(SHA), ("ACME", "Lab"), false)))),
        ("github", o(None)),
        ("github no setup", CloneAsk { setup_git: Some(false), ..o(None) }),
        ("github setup true", CloneAsk { setup_git: Some(true), ..o(None) }),
        ("gitlab", ask("gitlab", "https://gitlab.com/dev/thing.git", "/root/thing", "hetzner")),
        ("git", ask("git", "https://example.com/x.git", "/root/x", "Boat")),
        ("git branch", CloneAsk { branch: Some("feat/a b".into()), ..ask("git", "https://example.com/x.git", "/root/x y/it's", "Boat") }),
        ("folder", ask("folder", "git@github.com:a/b.git", "x", "Boat")),
        ("git root checkout", ask("git", "r", "/x", "c")),
    ]
}

// fixture: landing_scripts.json cloneLines
#[test]
fn clone_lines_answer_every_row_the_ts_did() {
    let want = rows("cloneLines");
    let cases = clone_cases();
    assert_eq!(names(&want), cases.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for (row, (name, ask)) in want.iter().zip(&cases) {
        assert_eq!(clone_lines(ask), strings(&row[1]), "cloneLines row {name}");
    }
}

// fixture: landing_scripts.json onRecord
#[test]
fn on_record_lines_answer_every_row_the_ts_did() {
    let want = rows("onRecord");
    let cases = [
        ("held clone", lab_own("wsp/pricing-page", None)),
        ("sha", lab_own("wsp/pricing-page", Some(SHA))),
        ("fork held", lab("main", Some(SHA), ("ana", "lab-fork"), true)),
    ];
    assert_eq!(names(&want), cases.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for (row, (name, b)) in want.iter().zip(&cases) {
        assert_eq!(on_record_lines("/srv/held", &b.of_record, &b.origin, b.held), strings(&row[1]), "onRecord row {name}");
    }
}

// fixture: landing_scripts.json gitEnv
#[test]
fn brokered_git_env_answers_every_row_the_ts_did_in_the_same_key_order() {
    let want = rows("gitEnv");
    let web = |public: bool| GitEnvRepo { owner: "acme".into(), name: "web".into(), public };
    let direct = |d: &[&str]| d.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let cases = [
        ("plain", brokered_git_env("wsp-op-token", None, &[])),
        ("public", brokered_git_env("wsp-op-token", Some(&web(true)), &[])),
        ("private", brokered_git_env("wsp-op-token", Some(&web(false)), &[])),
        ("direct", brokered_git_env("t", None, &direct(&["anthropics/claude-plugins", "acme/market"]))),
        ("public and direct overlap", brokered_git_env("t", Some(&web(true)), &direct(&["acme/web", "z/y"]))),
    ];
    assert_eq!(names(&want), cases.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for (row, (name, env)) in want.iter().zip(&cases) {
        assert_eq!(pairs_of(env), fixture_pairs(&row[1]), "gitEnv row {name}");
    }
}

// fixture: landing_scripts.json cloneScript
#[test]
fn clone_script_answers_every_row_the_ts_did() {
    let want = rows("cloneScript");
    let git_seed = clone_script(&ask("git", "/srv/origin.git", "/srv/checkout", "spoo"), Some("/tmp/seed it.tgz"));
    let github_plain =
        clone_script(&CloneAsk { branch: Some("main".into()), ..ask("github", "https://github.com/acme/lab", "/root/lab", "Boat") }, None);
    assert_eq!(names(&want), ["git with seed", "github no seed"]);
    assert_eq!(git_seed, text(&want[0][1]));
    assert_eq!(github_plain, text(&want[1][1]));
}

// fixture: landing_scripts.json patchScript
#[test]
fn patch_script_answers_as_the_ts_did() {
    assert_eq!(patch_script(&step(), "/srv/co"), text(&fx("patchScript")));
}

// fixture: landing_scripts.json patchCleanup
#[test]
fn patch_cleanup_script_answers_both_rows_as_the_ts_did() {
    let want = rows("patchCleanup");
    let same = PatchStep { branch: "main".into(), ..step() };
    assert_eq!(names(&want), ["own branch", "same branch"]);
    assert_eq!(patch_cleanup_script(&step(), "/srv/co"), text(&want[0][1]));
    assert_eq!(patch_cleanup_script(&same, "/srv/co"), text(&want[1][1]));
}

// fixture: landing_scripts.json unsavedScript
#[test]
fn unsaved_script_answers_as_the_ts_did_for_a_path_with_a_quote_and_a_space() {
    assert_eq!(unsaved_script("/srv/it's here"), text(&fx("unsavedScript")));
}

// fixture: landing_scripts.json seedRest
#[test]
fn seed_rest_script_answers_every_row_the_ts_did() {
    let want = rows("seedRest");
    let cases = [
        ("memory", seed_rest_script("/srv/co", "/root/.claude-cfg/projects/-srv-co/memory", "/tmp/seed.tgz", true)),
        ("no memory", seed_rest_script("/srv/co", "/m", "/tmp/seed.tgz", false)),
        ("memory no slash", seed_rest_script("co", "mem", "s.tgz", true)),
    ];
    assert_eq!(names(&want), cases.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>());
    for (row, (name, script)) in want.iter().zip(&cases) {
        assert_eq!(script, &text(&row[1]), "seedRest row {name}");
    }
}

// fixture: landing_scripts.json seedInto
#[test]
fn seed_into_script_answers_as_the_ts_did() {
    assert_eq!(seed_into_script("/srv/legacy", "/root/.wsp/put/seed-pr_1.tgz"), text(&fx("seedInto")));
}

// fixture: landing_scripts.json freeFolder
#[test]
fn free_folder_script_answers_both_rows_as_the_ts_did() {
    let want = rows("freeFolder");
    assert_eq!(names(&want), ["root", "slashes"]);
    assert_eq!(free_folder_script("/root", "spoo-ts"), text(&want[0][1]));
    assert_eq!(free_folder_script("/home/dev///", "it's"), text(&want[1][1]));
}

// fixture: landing_scripts.json underLoginHome
#[test]
fn under_login_home_answers_every_row_the_ts_did() {
    let want = rows("underLoginHome");
    assert_eq!(want.len(), 6);
    for row in &want {
        assert_eq!(under_login_home(&text(&row[0]), &text(&row[1])), text(&row[2]), "underLoginHome {} {}", row[0], row[1]);
    }
}

// fixture: landing_scripts.json guestMemoryDir
#[test]
fn guest_memory_dir_answers_both_rows_as_the_ts_did() {
    let want = rows("guestMemoryDir");
    assert_eq!(want.len(), 2);
    for row in &want {
        assert_eq!(guest_memory_dir(&text(&row[0])), text(&row[1]), "guestMemoryDir {}", row[0]);
    }
    assert_eq!(guest_memory_dir("-srv-web"), "/root/.claude-cfg/projects/-srv-web/memory");
}

// fixture: landing_scripts.json memoryKeptMark
#[test]
fn memory_kept_mark_is_the_ts_word() {
    assert_eq!(MEMORY_KEPT_MARK, text(&fx("memoryKeptMark")));
    assert_eq!(MEMORY_KEPT_MARK, "wsp-memory-kept");
}

// fixture: landing_scripts.json unsavedOf
#[test]
fn unsaved_of_answers_every_row_the_ts_did() {
    let want = rows("unsavedOf");
    assert_eq!(want.len(), 18);
    for row in &want {
        let ran: ExecResult = wire(&row[0]);
        let expect = row[1].as_str().map(String::from);
        assert_eq!(unsaved_of("web", &ran), expect, "unsavedOf {}", row[0]);
    }
}

// fixture: landing_scripts.json readUnsaved
#[test]
fn read_unsaved_answers_a_failed_run_a_plain_failure_and_a_clean_read_as_the_ts_did() {
    let want = rows("readUnsaved");
    assert_eq!(names(&want), ["throws", "throws string", "clean"]);
    run_local(async {
        let seen: Rc<RefCell<Vec<(String, u64)>>> = Rc::new(RefCell::new(vec![]));
        let failing = |message: &'static str| -> RunHere {
            let seen = seen.clone();
            Rc::new(move |cmd: String, ms: u64| -> LocalBoxFuture<'static, Result<ExecResult, HostError>> {
                seen.borrow_mut().push((cmd, ms));
                Box::pin(async move { Err(HostError::new(message)) })
            })
        };
        let clean: RunHere = {
            let seen = seen.clone();
            Rc::new(move |cmd: String, ms: u64| -> LocalBoxFuture<'static, Result<ExecResult, HostError>> {
                seen.borrow_mut().push((cmd, ms));
                Box::pin(async move { Ok(result(0, "0 0 0\n", "")) })
            })
        };
        assert_eq!(read_unsaved("x", "/srv/x", failing("the fork did not wake")).await, Some(text(&want[0][1])));
        assert_eq!(read_unsaved("x", "/srv/x", failing("plain")).await, Some(text(&want[1][1])));
        assert_eq!(read_unsaved("x", "/srv/x", clean).await, None);
        assert!(want[2][1].is_null());
        let asked = (unsaved_script("/srv/x"), STEP_MS);
        assert_eq!(*seen.borrow(), vec![asked.clone(), asked.clone(), asked]);
    });
}

// TS: project-landing.ts:636 lastLine
#[test]
fn last_line_trims_the_output_and_answers_nothing_for_an_empty_one() {
    assert_eq!(last_line("a\nb\n"), Some("b".into()));
    assert_eq!(last_line("  x  "), Some("x".into()));
    assert_eq!(last_line("a\n  b  \n\n"), Some("b".into()));
    assert_eq!(last_line("a\r\nb\r\n"), Some("b".into()));
    assert_eq!(last_line(""), None);
    assert_eq!(last_line("\n  \n"), None);
}

// ---------------------------------------------------------------------------------------------------------------
// project-seed-script.test.ts: the scripts against real git.

fn run(script: &str, root: &Path) -> ExecResult {
    sh(script, &no_identity(root))
}

struct Fx {
    _dir: tempfile::TempDir,
    root: PathBuf,
    origin: PathBuf,
    folder: PathBuf,
}

fn origin_and_clone() -> Fx {
    let dir = tempfile::Builder::new().prefix("wsp-seed-script-").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let origin = root.join("origin.git");
    git(&root, &["init", "-q", "--bare", "-b", "main", s(&origin)]);
    let folder = root.join("spoo-landing");
    git(&root, &["clone", "-q", s(&origin), s(&folder)]);
    fs::write(folder.join("README.md"), "one\n").unwrap();
    git(&folder, &["add", "-A"]);
    git(&folder, &["commit", "-qm", "first"]);
    git(&folder, &["push", "-q", "origin", "main"]);
    Fx { _dir: dir, root, origin, folder }
}

fn commit(at: &Path, name: &str) {
    fs::write(at.join(format!("{name}.md")), format!("{name}\n")).unwrap();
    git(at, &["add", "-A"]);
    git(at, &["commit", "-qm", name]);
}

fn merge_base(folder: &Path) -> String {
    git(folder, &["merge-base", "HEAD", "origin/main"]).trim().to_string()
}

fn patch_of(folder: &Path, base: &str) -> String {
    git(folder, &["format-patch", "--stdout", &format!("{base}..HEAD")])
}

fn seed_tar(root: &Path, patch: &str, memory: &str) -> String {
    let stage = tempfile::Builder::new().prefix("stage-").tempdir_in(root).unwrap();
    fs::create_dir_all(stage.path().join(".wsp-seed").join("memory")).unwrap();
    fs::write(stage.path().join(".env.local"), "TOKEN=abc\n").unwrap();
    fs::write(stage.path().join(".wsp-seed").join("commits.patch"), patch).unwrap();
    fs::write(stage.path().join(".wsp-seed").join("memory").join("MEMORY.md"), memory).unwrap();
    let tar = root.join("seed.tgz");
    let made = Command::new("tar").args(["-czf", s(&tar), "-C", s(stage.path()), ".env.local", ".wsp-seed"]).status().unwrap();
    assert!(made.success());
    s(&tar).to_string()
}

struct Plan {
    branch: &'static str,
    base: String,
    unpushed: bool,
}

#[derive(Clone, Copy)]
struct Choice {
    memory: bool,
    commits: bool,
}

const CHOICE: Choice = Choice { memory: true, commits: true };
const NO_COMMITS: Choice = Choice { memory: true, commits: false };

struct Landing {
    root: PathBuf,
    checkout: PathBuf,
    memory_dir: PathBuf,
    seed_tar: String,
    memory: bool,
    clone: ExecResult,
    patch: ExecResult,
    step: PatchStep,
}

impl Landing {
    fn rest(&self) -> ExecResult {
        run(&seed_rest_script(s(&self.checkout), s(&self.memory_dir), &self.seed_tar, self.memory), &self.root)
    }
}

fn landing(root: &Path, remote: &Path, checkout: &Path, memory_dir: &Path, plan: &Plan, seed_tar: String, choice: Choice) -> Landing {
    let clone_ask = ask("git", s(remote), s(checkout), "spoo");
    let clone = run(&clone_script(&clone_ask, Some(&seed_tar)), root);
    let step =
        PatchStep { branch: plan.branch.into(), base: if plan.unpushed { plan.base.clone() } else { String::new() }, back: "main".into() };
    let patch = if choice.commits && plan.unpushed { run(&patch_script(&step, s(checkout)), root) } else { result(0, "", "") };
    Landing {
        root: root.to_path_buf(),
        checkout: checkout.to_path_buf(),
        memory_dir: memory_dir.to_path_buf(),
        seed_tar,
        memory: choice.memory,
        clone,
        patch,
        step,
    }
}

fn plan(branch: &'static str, base: &str) -> Plan {
    Plan { branch, base: base.into(), unpushed: true }
}

// TS: project-seed-script.test.ts:120
#[test]
fn lands_the_commits_of_a_branch_the_remote_has_never_seen_as_a_branch_of_its_own_on_top_of_where_they_started() {
    let f = origin_and_clone();
    git(&f.folder, &["checkout", "-qb", "refactor/dashboard-polish"]);
    for n in ["two", "three"] {
        commit(&f.folder, n);
    }
    let base = merge_base(&f.folder);
    let patch = patch_of(&f.folder, &base);
    let checkout = f.root.join("checkout");
    let memory_dir = f.root.join("memory");
    let ran = landing(
        &f.root,
        &f.origin,
        &checkout,
        &memory_dir,
        &plan("refactor/dashboard-polish", &base),
        seed_tar(&f.root, &patch, "- one thing\n"),
        CHOICE,
    );
    ok(&ran.clone);
    ok(&ran.patch);
    assert_eq!(last_of(&ran.patch.stdout), "2");
    ok(&ran.rest());
    assert_eq!(git(&checkout, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "main");
    assert_eq!(
        git(&checkout, &["log", "--format=%s", "refactor/dashboard-polish", "-3"]).trim().split('\n').collect::<Vec<_>>(),
        ["three", "two", "first"]
    );
    assert_eq!(fs::read_to_string(checkout.join(".env.local")).unwrap(), "TOKEN=abc\n");
    assert_eq!(fs::read_to_string(memory_dir.join("MEMORY.md")).unwrap(), "- one thing\n");
    assert!(!checkout.join(".wsp-seed").exists());
}

// TS: project-seed-script.test.ts:149
#[test]
fn signs_them_as_wsp_and_leaves_the_person_as_their_author_since_the_computer_has_no_identity_to_commit_with() {
    let f = origin_and_clone();
    fs::write(f.folder.join("two.md"), "two\n").unwrap();
    git(&f.folder, &["add", "-A"]);
    let dev = env_pairs(&[
        ("GIT_AUTHOR_NAME", "Dev"),
        ("GIT_AUTHOR_EMAIL", "dev@example.com"),
        ("GIT_COMMITTER_NAME", "Dev"),
        ("GIT_COMMITTER_EMAIL", "dev@example.com"),
    ]);
    git_with(&f.folder, &["commit", "-qm", "two"], &dev);
    let base = merge_base(&f.folder);
    let patch = patch_of(&f.folder, &base);
    let checkout = f.root.join("checkout");
    let ran = landing(
        &f.root,
        &f.origin,
        &checkout,
        &f.root.join("memory"),
        &plan("main", &base),
        seed_tar(&f.root, &patch, "- one thing\n"),
        CHOICE,
    );
    ok(&ran.patch);
    assert_eq!(last_of(&ran.patch.stdout), "1");
    assert_eq!(
        git(&checkout, &["log", "--format=%an <%ae> committed by %cn <%ce>", "-1"]).trim(),
        "Dev <dev@example.com> committed by wsp <wsp@localhost>"
    );
    assert_eq!(git(&checkout, &["log", "--format=%s", "-2"]).trim().split('\n').collect::<Vec<_>>(), ["two", "first"]);
}

// TS: project-seed-script.test.ts:165
#[test]
fn lands_the_commits_of_the_remotes_own_default_branch_on_top_of_it_where_the_clone_already_made_that_branch() {
    let f = origin_and_clone();
    for n in ["two", "three"] {
        commit(&f.folder, n);
    }
    let base = merge_base(&f.folder);
    let patch = patch_of(&f.folder, &base);
    let checkout = f.root.join("checkout");
    let memory_dir = f.root.join("memory");
    let ran = landing(&f.root, &f.origin, &checkout, &memory_dir, &plan("main", &base), seed_tar(&f.root, &patch, "- one thing\n"), CHOICE);
    ok(&ran.patch);
    ok(&ran.rest());
    assert_eq!(git(&checkout, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "main");
    assert_eq!(git(&checkout, &["log", "--format=%s", "-3"]).trim().split('\n').collect::<Vec<_>>(), ["three", "two", "first"]);
    assert!(!checkout.join(".wsp-seed").exists());
}

// TS: project-seed-script.test.ts:185
#[test]
fn leaves_the_memory_already_standing_at_the_agents_path_alone_and_says_so_on_its_own_output() {
    let f = origin_and_clone();
    let base = merge_base(&f.folder);
    let checkout = f.root.join("checkout");
    let memory_dir = f.root.join("state").join("projects").join("-srv-spoo-landing").join("memory");
    fs::create_dir_all(&memory_dir).unwrap();
    fs::write(memory_dir.join("MEMORY.md"), "- what the agent learned here\n").unwrap();
    let ran = landing(
        &f.root,
        &f.origin,
        &checkout,
        &memory_dir,
        &plan("main", &base),
        seed_tar(&f.root, "", "- what the folder carried\n"),
        NO_COMMITS,
    );
    ok(&ran.clone);
    let rest = ran.rest();
    ok(&rest);
    assert_eq!(fs::read_to_string(memory_dir.join("MEMORY.md")).unwrap(), "- what the agent learned here\n");
    assert!(rest.stdout.contains(MEMORY_KEPT_MARK), "stdout: {}", rest.stdout);
    assert_eq!(fs::read_to_string(checkout.join(".env.local")).unwrap(), "TOKEN=abc\n");
    assert!(!checkout.join(".wsp-seed").exists());
}

// TS: project-seed-script.test.ts:214
#[test]
fn takes_an_empty_folder_at_the_agents_path_as_nothing_kept_so_the_seeds_memory_lands_in_its_place() {
    let f = origin_and_clone();
    let base = merge_base(&f.folder);
    let checkout = f.root.join("checkout");
    let memory_dir = f.root.join("state").join("projects").join("-srv-spoo-landing").join("memory");
    fs::create_dir_all(&memory_dir).unwrap();
    let ran = landing(
        &f.root,
        &f.origin,
        &checkout,
        &memory_dir,
        &plan("main", &base),
        seed_tar(&f.root, "", "- what the folder carried\n"),
        NO_COMMITS,
    );
    ok(&ran.clone);
    let rest = ran.rest();
    ok(&rest);
    assert_eq!(fs::read_to_string(memory_dir.join("MEMORY.md")).unwrap(), "- what the folder carried\n");
    assert!(!rest.stdout.contains(MEMORY_KEPT_MARK), "stdout: {}", rest.stdout);
}

// TS: project-seed-script.test.ts:239
#[test]
fn is_one_command_per_line_so_a_step_that_cannot_make_the_folder_ends_before_the_seed_is_swept() {
    let f = origin_and_clone();
    let base = merge_base(&f.folder);
    let checkout = f.root.join("checkout");
    let parent = f.root.join("state");
    fs::write(&parent, "not a folder\n").unwrap();
    let memory_dir = parent.join("projects").join("-srv-spoo-landing").join("memory");
    let ran = landing(
        &f.root,
        &f.origin,
        &checkout,
        &memory_dir,
        &plan("main", &base),
        seed_tar(&f.root, "", "- what the folder carried\n"),
        NO_COMMITS,
    );
    ok(&ran.clone);
    let rest = ran.rest();
    assert_ne!(exited(&rest), 0);
    assert!(!rest.stdout.contains(MEMORY_KEPT_MARK), "stdout: {}", rest.stdout);
    assert!(checkout.join(".wsp-seed").join("memory").join("MEMORY.md").exists());
}

// TS: project-seed-script.test.ts:265
#[test]
fn lands_the_seeds_memory_where_nothing_stands_at_all_making_the_folder_above_it() {
    let f = origin_and_clone();
    let base = merge_base(&f.folder);
    let checkout = f.root.join("checkout");
    let memory_dir = f.root.join("state").join("projects").join("-srv-spoo-landing").join("memory");
    let ran = landing(
        &f.root,
        &f.origin,
        &checkout,
        &memory_dir,
        &plan("main", &base),
        seed_tar(&f.root, "", "- what the folder carried\n"),
        NO_COMMITS,
    );
    ok(&ran.clone);
    let rest = ran.rest();
    ok(&rest);
    assert_eq!(fs::read_to_string(memory_dir.join("MEMORY.md")).unwrap(), "- what the folder carried\n");
    assert!(!rest.stdout.contains(MEMORY_KEPT_MARK), "stdout: {}", rest.stdout);
}

// TS: project-seed-script.test.ts:286
#[test]
fn clones_and_seeds_with_no_patch_at_all_where_the_person_kept_none() {
    let f = origin_and_clone();
    let checkout = f.root.join("checkout");
    let memory_dir = f.root.join("memory");
    let no_patch = Plan { branch: "main", base: "x".into(), unpushed: false };
    let ran = landing(&f.root, &f.origin, &checkout, &memory_dir, &no_patch, seed_tar(&f.root, "", "- one thing\n"), NO_COMMITS);
    ok(&ran.clone);
    ok(&ran.rest());
    assert_eq!(git(&checkout, &["log", "--format=%s", "-1"]).trim(), "first");
    assert_eq!(fs::read_to_string(checkout.join(".env.local")).unwrap(), "TOKEN=abc\n");
}

const UNAPPLIABLE: &str = "From 1111111111111111111111111111111111111111 Mon Sep 17 00:00:00 2001
From: Dev <dev@example.com>
Date: Thu, 18 Sep 2026 00:00:00 +0000
Subject: [PATCH] two

---
 README.md | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)

diff --git a/README.md b/README.md
index 1111111aaaaaaa2222222bbbbbbb3333333ccccccc..4444444ddddddd5555555eeeeeee6666666fffffff 100644
--- a/README.md
+++ b/README.md
@@ -1 +1 @@
-nothing like what is there
+two
--
2.39.0

";

// TS: project-seed-script.test.ts:330
#[test]
fn fails_its_own_step_and_leaves_the_remotes_default_branch_back_at_the_remotes_tip_with_nothing_half_applied() {
    let f = origin_and_clone();
    let tip = git(&f.folder, &["rev-parse", "origin/main"]).trim().to_string();
    let checkout = f.root.join("checkout");
    let memory_dir = f.root.join("memory");
    let ran =
        landing(&f.root, &f.origin, &checkout, &memory_dir, &plan("main", &tip), seed_tar(&f.root, UNAPPLIABLE, "- one thing\n"), CHOICE);
    ok(&ran.clone);
    assert_ne!(exited(&ran.patch), 0);
    run(&patch_cleanup_script(&ran.step, s(&checkout)), &f.root);
    assert_eq!(git(&checkout, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(), "main");
    assert_eq!(git(&checkout, &["rev-parse", "HEAD"]).trim(), tip);
    assert!(!checkout.join(".git").join("rebase-apply").exists());
    ok(&ran.rest());
    assert_eq!(fs::read_to_string(memory_dir.join("MEMORY.md")).unwrap(), "- one thing\n");
    assert!(!checkout.join(".wsp-seed").exists());
}

// TS: project-seed-script.test.ts:353
#[test]
fn drops_the_branch_of_the_persons_own_that_the_step_made_so_the_checkout_carries_no_half_branch() {
    let f = origin_and_clone();
    let tip = git(&f.folder, &["rev-parse", "origin/main"]).trim().to_string();
    let checkout = f.root.join("checkout");
    let ran = landing(
        &f.root,
        &f.origin,
        &checkout,
        &f.root.join("memory"),
        &plan("refactor/dashboard-polish", &tip),
        seed_tar(&f.root, UNAPPLIABLE, "- one thing\n"),
        CHOICE,
    );
    assert_ne!(exited(&ran.patch), 0);
    run(&patch_cleanup_script(&ran.step, s(&checkout)), &f.root);
    assert_eq!(git(&checkout, &["branch", "--format=%(refname:short)"]).trim().split('\n').collect::<Vec<_>>(), ["main"]);
    assert_eq!(git(&checkout, &["rev-parse", "HEAD"]).trim(), tip);
}

// TS: project-seed-script.test.ts:364
#[test]
fn a_base_the_remote_no_longer_has_fails_the_step_at_its_first_line_and_the_cleanup_after_it_fails_nothing() {
    let f = origin_and_clone();
    let tip = git(&f.folder, &["rev-parse", "origin/main"]).trim().to_string();
    let checkout = f.root.join("checkout");
    let gone = "1111111111111111111111111111111111111111";
    let ran = landing(
        &f.root,
        &f.origin,
        &checkout,
        &f.root.join("memory"),
        &plan("main", gone),
        seed_tar(&f.root, UNAPPLIABLE, "- one thing\n"),
        CHOICE,
    );
    assert_ne!(exited(&ran.patch), 0);
    run(&patch_cleanup_script(&ran.step, s(&checkout)), &f.root);
    assert_eq!(git(&checkout, &["rev-parse", "HEAD"]).trim(), tip);
    assert!(!checkout.join(".git").join("rebase-apply").exists());
}

fn counted(checkout: &Path, root: &Path) -> String {
    run(&unsaved_script(s(checkout)), root).stdout.trim().to_string()
}

fn unsaved_here(folder: &Path, root: &Path) -> Option<String> {
    unsaved_of("x", &run(&unsaved_script(s(folder)), root))
}

fn could_not_read() -> Regex {
    Regex::new(r"^x: could not read what is not pushed: \S").unwrap()
}

// TS: project-seed-script.test.ts:387
#[test]
fn reads_a_clone_with_nothing_of_its_own_as_clean() {
    let f = origin_and_clone();
    assert_eq!(counted(&f.folder, &f.root), "0 0 0");
    assert_eq!(unsaved_here(&f.folder, &f.root), None);
}

// TS: project-seed-script.test.ts:393
#[test]
fn counts_a_commit_on_a_branch_that_is_not_checked_out_which_the_checked_out_branchs_own_count_never_sees() {
    let f = origin_and_clone();
    git(&f.folder, &["checkout", "-qb", "feature"]);
    commit(&f.folder, "two");
    git(&f.folder, &["checkout", "-q", "main"]);
    assert_eq!(counted(&f.folder, &f.root), "1 0 0");
    assert_eq!(unsaved_here(&f.folder, &f.root), Some("x holds 1 commit not pushed".into()));
}

// TS: project-seed-script.test.ts:402
#[test]
fn does_not_count_the_store_wsps_own_install_put_at_the_checkouts_root_and_counts_one_the_project_keeps_deeper() {
    let f = origin_and_clone();
    fs::create_dir_all(f.folder.join(".pnpm-store").join("v10")).unwrap();
    fs::write(f.folder.join(".pnpm-store").join("v10").join("index.json"), "{}\n").unwrap();
    assert_eq!(counted(&f.folder, &f.root), "0 0 0");
    fs::create_dir_all(f.folder.join("apps").join(".pnpm-store")).unwrap();
    fs::write(f.folder.join("apps").join(".pnpm-store").join("index.json"), "{}\n").unwrap();
    assert_eq!(counted(&f.folder, &f.root), "0 1 0");
}

// TS: project-seed-script.test.ts:412
#[test]
fn counts_a_commit_at_a_detached_head_which_no_branch_holds() {
    let f = origin_and_clone();
    git(&f.folder, &["checkout", "-q", "--detach"]);
    commit(&f.folder, "two");
    assert_eq!(counted(&f.folder, &f.root), "1 0 0");
}

// TS: project-seed-script.test.ts:419
#[test]
fn counts_what_a_linked_worktree_holds_its_uncommitted_files_and_the_commit_at_its_detached_head() {
    let f = origin_and_clone();
    let tree = f.root.join("tree");
    git(&f.folder, &["worktree", "add", "-q", "--detach", s(&tree)]);
    fs::write(tree.join("draft.md"), "draft\n").unwrap();
    assert_eq!(counted(&f.folder, &f.root), "0 1 0");
    commit(&tree, "three");
    assert_eq!(counted(&f.folder, &f.root), "1 0 0");
}

// TS: project-seed-script.test.ts:429
#[test]
fn counts_every_commit_where_there_is_no_remote_at_all_and_the_stashes() {
    let dir = tempfile::Builder::new().prefix("wsp-unsaved-").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let folder = root.join("alone");
    git(&root, &["init", "-q", "-b", "main", s(&folder)]);
    commit(&folder, "one");
    fs::write(folder.join("one.md"), "changed\n").unwrap();
    git(&folder, &["stash", "-q"]);
    fs::write(folder.join("new.md"), "new\n").unwrap();
    assert_eq!(counted(&folder, &root), "1 1 1");
    assert_eq!(unsaved_here(&folder, &root), Some("x holds 1 commit not pushed, 1 uncommitted file and 1 stash".into()));
}

// TS: project-seed-script.test.ts:442
#[test]
fn does_not_count_the_commits_a_seed_carried_over_from_the_persons_own_folder_and_counts_one_made_after_them() {
    let f = origin_and_clone();
    git(&f.folder, &["checkout", "-qb", "mine"]);
    commit(&f.folder, "two");
    commit(&f.folder, "three");
    let base = merge_base(&f.folder);
    let patch = patch_of(&f.folder, &base);
    let checkout = f.root.join("checkout");
    let ran = landing(
        &f.root,
        &f.origin,
        &checkout,
        &f.root.join("memory"),
        &plan("mine", &base),
        seed_tar(&f.root, &patch, "- one thing\n"),
        CHOICE,
    );
    ok(&ran.patch);
    ok(&ran.rest());
    assert_eq!(counted(&checkout, &f.root), "0 1 0");
    git(&checkout, &["checkout", "-q", "mine"]);
    commit(&checkout, "four");
    assert_eq!(counted(&checkout, &f.root), "1 0 0");
}

// TS: project-seed-script.test.ts:460
#[test]
fn reads_a_checkout_whose_head_git_cannot_open_as_one_it_could_not_read_never_as_a_folder_with_no_checkout() {
    let said = could_not_read();
    for variant in 0..2 {
        let f = origin_and_clone();
        commit(&f.folder, "two");
        let head = f.folder.join(".git").join("HEAD");
        if variant == 0 {
            fs::write(&head, "").unwrap();
        } else {
            fs::remove_file(&head).unwrap();
        }
        let got = unsaved_here(&f.folder, &f.root).expect("a read that failed is a line");
        assert!(said.is_match(&got), "variant {variant}: {got}");
    }
}

// TS: project-seed-script.test.ts:469
#[test]
fn reads_a_stash_list_that_fails_as_a_read_that_failed_not_as_no_stashes() {
    let f = origin_and_clone();
    fs::write(f.folder.join(".git").join("refs").join("stash"), format!("{}\n", "1".repeat(40))).unwrap();
    let got = unsaved_here(&f.folder, &f.root).expect("a read that failed is a line");
    assert!(could_not_read().is_match(&got), "{got}");
}

// TS: project-seed-script.test.ts:475
#[test]
fn reads_a_fresh_clone_of_a_remote_holding_a_tag_on_a_branch_deleted_since_as_clean() {
    let f = origin_and_clone();
    git(&f.folder, &["checkout", "-qb", "release"]);
    commit(&f.folder, "released");
    git(&f.folder, &["tag", "-a", "-m", "v1", "v1"]);
    git(&f.folder, &["push", "-q", "origin", "release", "v1"]);
    git(&f.folder, &["push", "-q", "origin", "--delete", "release"]);
    let fresh = f.root.join("fresh");
    git(&f.root, &["clone", "-q", s(&f.origin), s(&fresh)]);
    assert_eq!(git(&fresh, &["tag"]).trim(), "v1");
    assert_eq!(counted(&fresh, &f.root), "0 0 0");
    assert_eq!(unsaved_here(&fresh, &f.root), None);
}

// TS: project-seed-script.test.ts:489
#[test]
fn reads_a_dot_git_that_is_a_dangling_link_as_a_checkout_it_could_not_read() {
    let f = origin_and_clone();
    fs::remove_dir_all(f.folder.join(".git")).unwrap();
    symlink(f.root.join("gone"), f.folder.join(".git")).unwrap();
    let got = unsaved_here(&f.folder, &f.root).expect("a dangling link is a line");
    assert!(could_not_read().is_match(&got), "{got}");
}

// TS: project-seed-script.test.ts:496
#[test]
fn reads_a_dangling_dot_git_link_in_a_folder_inside_another_repository_as_could_not_read_never_as_that_repository() {
    let f = origin_and_clone();
    fs::write(f.folder.join("README.md"), "two\n").unwrap();
    git(&f.folder, &["commit", "-qam", "never pushed"]);
    let inner = f.folder.join("inner");
    fs::create_dir(&inner).unwrap();
    symlink(f.root.join("gone"), inner.join(".git")).unwrap();
    let got = unsaved_here(&inner, &f.root).expect("a dangling link is a line");
    assert!(could_not_read().is_match(&got), "{got}");
}

// TS: project-seed-script.test.ts:506
#[test]
fn says_why_a_read_that_did_not_run_could_not_read_off_what_stopped_it() {
    run_local(async {
        let run: RunHere = Rc::new(|_cmd: String, _ms: u64| -> LocalBoxFuture<'static, Result<ExecResult, HostError>> {
            Box::pin(async { Err(HostError::new("the fork did not wake")) })
        });
        assert_eq!(read_unsaved("x", "/srv/x", run).await, Some("x: could not read what is not pushed: the fork did not wake".into()));
    });
}

// TS: project-seed-script.test.ts:510
#[test]
fn reads_a_folder_git_does_not_track_as_nothing_to_lose_and_a_read_that_failed_as_one_it_could_not_read() {
    let dir = tempfile::Builder::new().prefix("wsp-unsaved-").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    assert_eq!(unsaved_here(&root, &root), None);
    assert_eq!(unsaved_here(&root.join("gone"), &root), None);
    assert_eq!(unsaved_of("x", &result(1, "", "")), Some("x: could not read what is not pushed: the read exited 1".into()));
    assert_eq!(unsaved_of("x", &result(0, "half\n", "")), Some("x: could not read what is not pushed: the read printed \"half\"".into()));
}

// ---------------------------------------------------------------------------------------------------------------
// branch-of-record.test.ts: a clone where the host keeps the machine's credentials.

fn run_lines(lines: &[String], env: &[(String, String)]) -> String {
    let out = Command::new("sh")
        .arg("-ec")
        .arg(lines.join("\n"))
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(env.iter().map(|(k, v)| (k, v)))
        .output()
        .expect("sh runs");
    assert!(out.status.success(), "{lines:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

struct Lab {
    _dir: tempfile::TempDir,
    root: PathBuf,
    bare: PathBuf,
    seed: PathBuf,
}

fn commit_file(at: &Path, file: &str) -> String {
    fs::write(at.join(file), file).unwrap();
    git(at, &["add", file]);
    git(at, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", file]);
    git(at, &["rev-parse", "HEAD"]).trim().to_string()
}

fn lab_repo() -> Lab {
    let dir = tempfile::Builder::new().prefix("wsp-branch-of-record-").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let bare = root.join("lab.git");
    let seed = root.join("seed");
    git(&root, &["init", "-q", "--bare", "-b", "main", s(&bare)]);
    git(&root, &["init", "-q", "-b", "main", s(&seed)]);
    for file in ["one", "two"] {
        commit_file(&seed, file);
    }
    git(&seed, &["push", "-q", s(&bare), "main"]);
    Lab { _dir: dir, root, bare, seed }
}

fn rev(at: &Path, name: &str) -> String {
    git(at, &["rev-parse", name]).trim().to_string()
}

// TS: branch-of-record.test.ts:164
#[test]
fn clones_one_commit_deep_at_the_base_adds_one_refspec_for_the_branch_of_record_and_checks_it_out_at_its_start_commit_with_no_broker_todays_lines(
) {
    let o = github_ask(None);
    let brokered = CloneAsk { brokered: Some(lab_own("wsp/pricing-page", Some(SHA))), ..o.clone() };
    assert_eq!(
        clone_lines(&brokered),
        vec![
            "set -e".to_string(),
            "mkdir -p '/root'".to_string(),
            "git clone --depth 1 --branch main -- https://github.com/acme/lab /root/lab".to_string(),
            "git -C /root/lab config --add remote.origin.fetch '+refs/heads/wsp/pricing-page*:refs/remotes/origin/wsp/pricing-page*'"
                .to_string(),
            format!("git -C /root/lab cat-file -e '{SHA}^{{commit}}' 2>/dev/null || git -C /root/lab fetch -q --depth 1 origin {SHA}"),
            format!("git -C /root/lab checkout -q -B wsp/pricing-page {SHA}"),
            "git -C /root/lab rev-parse HEAD".to_string(),
        ]
    );
    let fork = CloneAsk { brokered: Some(lab("main", Some(SHA), ("ana", "lab-fork"), false)), ..o.clone() };
    assert_eq!(
        clone_lines(&fork)[3..].to_vec(),
        vec![
            "git -C /root/lab remote add -t 'main*' ana https://github.com/ana/lab-fork.git".to_string(),
            format!("git -C /root/lab cat-file -e '{SHA}^{{commit}}' 2>/dev/null || git -C /root/lab fetch -q --depth 1 ana {SHA}"),
            format!("git -C /root/lab checkout -q -B main {SHA}"),
            "git -C /root/lab config branch.main.remote ana".to_string(),
            "git -C /root/lab config branch.main.merge refs/heads/main".to_string(),
            "git -C /root/lab rev-parse HEAD".to_string(),
        ]
    );
    let today = clone_lines(&o);
    assert!(today.contains(&"gh auth setup-git".to_string()), "{today:?}");
    assert_eq!(today.last().unwrap(), "gh repo clone acme/lab /root/lab");
}

// TS: branch-of-record.test.ts:208
#[test]
fn leaves_a_real_checkout_one_commit_deep_on_the_branch_of_record_whose_lease_holds_after_a_push_and_a_rebase_on_the_base() {
    let l = lab_repo();
    let checkout = l.root.join("work").join("lab");
    let tip = rev(&l.bare, "main");
    let lines = clone_lines(&CloneAsk {
        source: github(),
        remote: format!("file://{}", s(&l.bare)),
        checkout: s(&checkout).into(),
        computer: "Boat".into(),
        branch: Some("main".into()),
        setup_git: None,
        brokered: Some(lab_own("wsp/pricing-page", Some(&tip))),
    });
    assert_eq!(run_lines(&lines, &[]), tip);
    assert_eq!(git(&checkout, &["rev-list", "--count", "HEAD"]).trim(), "1");
    assert_eq!(git(&checkout, &["branch", "--show-current"]).trim(), "wsp/pricing-page");
    let tracking = Command::new("git")
        .args(["-C", s(&checkout), "rev-parse", "--verify", "-q", "refs/remotes/origin/wsp/pricing-page"])
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(!tracking.success());
    assert_eq!(
        git(&checkout, &["config", "--get-all", "remote.origin.fetch"]).trim().split('\n').collect::<Vec<_>>(),
        ["+refs/heads/main:refs/remotes/origin/main", "+refs/heads/wsp/pricing-page*:refs/remotes/origin/wsp/pricing-page*"]
    );
    git(&checkout, &["fetch", "-q", "origin"]);
    fs::write(checkout.join("work"), "one").unwrap();
    git(&checkout, &["add", "work"]);
    git_as(&checkout, &["commit", "-q", "-m", "work"]);
    git(&checkout, &["push", "-q", "-u", "origin", "wsp/pricing-page"]);
    git_as(&checkout, &["commit", "-q", "--amend", "-m", "work again"]);
    git(&checkout, &["push", "-q", "--force-with-lease", "origin", "wsp/pricing-page"]);
    commit_file(&l.seed, "three");
    git(&l.seed, &["push", "-q", s(&l.bare), "main"]);
    git(&checkout, &["fetch", "-q", "--depth", "1", "origin", "main"]);
    git_as(&checkout, &["rebase", "-q", "origin/main"]);
    git(&checkout, &["push", "-q", "--force-with-lease", "origin", "wsp/pricing-page"]);
    assert_eq!(rev(&l.bare, "wsp/pricing-page^"), rev(&l.bare, "main"));
}

// TS: branch-of-record.test.ts:236
#[test]
fn puts_a_pull_request_thread_on_the_head_itself_its_own_repos_or_a_forks_tracking_it_so_a_plain_push_and_a_first_lease_land_where_the_head_lives(
) {
    let l = lab_repo();
    git(&l.seed, &["checkout", "-q", "-b", "fix-typo"]);
    let head = commit_file(&l.seed, "typo");
    git(&l.seed, &["push", "-q", s(&l.bare), "fix-typo"]);
    git(&l.seed, &["checkout", "-q", "main"]);
    commit_file(&l.seed, "three");
    git(&l.seed, &["push", "-q", s(&l.bare), "main"]);
    let pushes = |checkout: &Path, env: &[(String, String)]| {
        fs::write(checkout.join("more"), "more").unwrap();
        git(checkout, &["add", "more"]);
        git_as(checkout, &["commit", "-q", "-m", "more"]);
        git_with(checkout, &["push", "-q"], env);
        git_as(checkout, &["commit", "-q", "--amend", "-m", "more again"]);
        git_with(checkout, &["push", "-q", "--force-with-lease"], env);
    };
    let remote = format!("file://{}", s(&l.bare));
    let own = l.root.join("own").join("lab");
    let own_lines = clone_lines(&CloneAsk {
        source: github(),
        remote: remote.clone(),
        checkout: s(&own).into(),
        computer: "Boat".into(),
        branch: Some("main".into()),
        setup_git: None,
        brokered: Some(lab("fix-typo", Some(&head), ("acme", "lab"), true)),
    });
    assert_eq!(run_lines(&own_lines, &[]), head);
    assert_eq!(git(&own, &["status", "-sb"]).lines().next().unwrap(), "## fix-typo...origin/fix-typo");
    pushes(&own, &[]);
    assert_eq!(rev(&l.bare, "fix-typo"), rev(&own, "HEAD"));

    let fork = l.root.join("fork.git");
    git(&l.root, &["init", "-q", "--bare", "-b", "main", s(&fork)]);
    git(&l.seed, &["push", "-q", s(&fork), &format!("{head}:refs/heads/main")]);
    let env = vec![
        ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
        ("GIT_CONFIG_KEY_0".to_string(), format!("url.file://{}.insteadOf", s(&fork))),
        ("GIT_CONFIG_VALUE_0".to_string(), "https://github.com/ana/lab-fork.git".to_string()),
    ];
    let theirs = l.root.join("theirs").join("lab");
    let theirs_lines = clone_lines(&CloneAsk {
        source: github(),
        remote,
        checkout: s(&theirs).into(),
        computer: "Boat".into(),
        branch: Some("main".into()),
        setup_git: None,
        brokered: Some(lab("main", Some(&head), ("ana", "lab-fork"), true)),
    });
    assert_eq!(run_lines(&theirs_lines, &env), head);
    assert_eq!(git(&theirs, &["status", "-sb"]).lines().next().unwrap(), "## main...ana/main");
    pushes(&theirs, &env);
    assert_eq!(rev(&fork, "main"), rev(&theirs, "HEAD"));
    assert_ne!(rev(&l.bare, "main"), rev(&theirs, "HEAD"));
}

// TS: branch-of-record.test.ts:273
#[test]
fn puts_a_checkout_an_image_already_holds_on_its_branch_of_record_where_it_stands_and_says_the_commit() {
    let l = lab_repo();
    let held = l.root.join("held");
    git(&l.root, &["clone", "-q", s(&l.bare), s(&held)]);
    let b = lab_own("wsp/pricing-page", None);
    let mut lines = vec!["set -e".to_string()];
    lines.extend(on_record_lines(s(&held), &b.of_record, &b.origin, b.held));
    assert_eq!(run_lines(&lines, &[]), rev(&held, "HEAD"));
    assert_eq!(git(&held, &["branch", "--show-current"]).trim(), "wsp/pricing-page");
}

struct Door {
    port: u16,
    heard: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Door {
    fn start(prefixes: &'static [&'static str], answer_403: bool) -> Door {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let heard = Arc::new(Mutex::new(vec![]));
        let stop = Arc::new(AtomicBool::new(false));
        let (heard_in, stop_in) = (heard.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            for conn in listener.incoming() {
                if stop_in.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut conn) = conn else { continue };
                conn.set_read_timeout(Some(Duration::from_secs(5))).ok();
                let mut got = Vec::new();
                let mut buf = [0u8; 4096];
                while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                    match conn.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => got.extend_from_slice(&buf[..n]),
                    }
                }
                let said: String = got.iter().map(|b| *b as char).collect();
                let lines: Vec<&str> = said.split("\r\n").filter(|l| prefixes.iter().any(|p| l.starts_with(p))).collect();
                heard_in.lock().unwrap().push(lines.join(" | "));
                if answer_403 {
                    conn.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").ok();
                }
            }
        });
        Door { port, heard, stop, thread: Some(thread) }
    }

    fn heard(&self) -> Vec<String> {
        self.heard.lock().unwrap().clone()
    }
}

impl Drop for Door {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        TcpStream::connect(("127.0.0.1", self.port)).ok();
        if let Some(t) = self.thread.take() {
            t.join().ok();
        }
    }
}

fn through_port(env: &IndexMap<String, String>, port: u16) -> Vec<(String, String)> {
    env.iter().map(|(k, v)| (k.clone(), v.replace(&format!(":{BROKER_PORT}"), &format!(":{port}")))).collect()
}

fn git_exit(args: &[&str], cwd: &Path, env: &[(String, String)], without: &[String]) -> i32 {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    for k in without {
        cmd.env_remove(k);
    }
    cmd.envs(env.iter().map(|(k, v)| (k, v))).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.status().unwrap().code().unwrap_or(-1)
}

fn proxy_auth() -> String {
    format!("Proxy-Authorization: Basic {}", base64::engine::general_purpose::STANDARD.encode("wsp:wsp-op-token"))
}

const REMOTE_FORMS: [&str; 4] = [
    "https://github.com/acme/web.git",
    "git@github.com:acme/web.git",
    "ssh://git@github.com/acme/web.git",
    "ssh://git@ssh.github.com:443/acme/web.git",
];

// TS: branch-of-record.test.ts:281
#[test]
fn sends_every_github_remote_form_to_the_daemons_door_as_gits_proxy_the_token_its_password_with_no_prompt_and_no_helper() {
    let env = brokered_git_env("wsp-op-token", None, &[]);
    assert_eq!(env["GIT_CONFIG_COUNT"], "7");
    assert_eq!(env["GIT_CONFIG_VALUE_4"], format!("http://wsp:wsp-op-token@127.0.0.1:{BROKER_PORT}"));
    assert_eq!(env["GIT_TERMINAL_PROMPT"], "0");
    let door = Door::start(&["GET", "Proxy-Authorization"], true);
    let mut local = through_port(&env, door.port);
    local.push(("GIT_CONFIG_NOSYSTEM".into(), "1".into()));
    local.push(("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()));
    let dir = tempfile::tempdir().unwrap();
    for remote in REMOTE_FORMS {
        assert_ne!(git_exit(&["ls-remote", remote], dir.path(), &local, &[]), 0, "{remote}");
    }
    let heard = door.heard();
    drop(door);
    let want = format!("GET http://github.com/acme/web.git/info/refs?service=git-upload-pack HTTP/1.1 | {}", proxy_auth());
    assert_eq!(heard, vec![want; 4]);
}

// TS: branch-of-record.test.ts:313
#[test]
fn fetches_a_public_repo_of_records_dot_git_remote_from_github_itself_with_no_credential_and_still_pushes_it_through_the_door() {
    let repo = |public: bool| GitEnvRepo { owner: "acme".into(), name: "web".into(), public };
    let env = brokered_git_env("wsp-op-token", Some(&repo(true)), &[]);
    assert_eq!(
        pairs_of(&brokered_git_env("wsp-op-token", Some(&repo(false)), &[])),
        pairs_of(&brokered_git_env("wsp-op-token", None, &[]))
    );
    let prefixes: &'static [&'static str] = &["GET", "CONNECT", "Proxy-Authorization", "Authorization"];
    let door = Door::start(prefixes, false);
    let github = Door::start(prefixes, false);
    let mut run_env = through_port(&env, door.port);
    run_env.push(("GIT_CONFIG_NOSYSTEM".into(), "1".into()));
    run_env.push(("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()));
    let via = format!("http://127.0.0.1:{}", github.port);
    run_env.push(("HTTPS_PROXY".into(), via.clone()));
    run_env.push(("https_proxy".into(), via));
    let outer: Vec<String> =
        std::env::vars().map(|(k, _)| k).filter(|k| matches!(k.to_lowercase().as_str(), "no_proxy" | "all_proxy")).collect();
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "one"]);
    for remote in REMOTE_FORMS {
        assert_ne!(git_exit(&["ls-remote", remote], dir.path(), &run_env, &outer), 0, "{remote}");
    }
    assert_eq!(door.heard(), Vec::<String>::new());
    assert_eq!(github.heard(), vec!["CONNECT github.com:443 HTTP/1.1".to_string(); 4]);
    for remote in REMOTE_FORMS {
        assert_ne!(git_exit(&["push", remote, "HEAD:refs/heads/wsp/x"], dir.path(), &run_env, &outer), 0, "{remote}");
    }
    assert_ne!(git_exit(&["ls-remote", "https://github.com/acme/web-private.git"], dir.path(), &run_env, &outer), 0);
    assert_ne!(git_exit(&["ls-remote", "https://github.com/acme/web"], dir.path(), &run_env, &outer), 0);
    let at_door = door.heard();
    let direct = github.heard();
    drop(door);
    drop(github);
    let auth = proxy_auth();
    assert_eq!(direct.len(), 4);
    let mut want = vec![format!("GET http://github.com/acme/web.git/info/refs?service=git-receive-pack HTTP/1.1 | {auth}"); 4];
    want.push(format!("GET http://github.com/acme/web-private.git/info/refs?service=git-upload-pack HTTP/1.1 | {auth}"));
    want.push(format!("GET http://github.com/acme/web/info/refs?service=git-upload-pack HTTP/1.1 | {auth}"));
    assert_eq!(at_door, want);
}

// ---------------------------------------------------------------------------------------------------------------
// box-folder.test.ts: the first free name in a home.

// TS: box-folder.test.ts:603
#[test]
fn claims_the_first_free_name_in_the_home_in_one_command() {
    let script = free_folder_script("/root", "spoo-ts");
    assert!(script.contains("mkdir '/root/spoo-ts'\"$n\""), "{script}");
    assert!(script.contains("-2"), "{script}");
}

fn free_home() -> (tempfile::TempDir, tempfile::TempDir) {
    (tempfile::Builder::new().prefix("wsp-home-").tempdir().unwrap(), tempfile::Builder::new().prefix("wsp-env-").tempdir().unwrap())
}

// TS: box-folder.test.ts:603
#[test]
fn free_folder_script_run_under_sh_makes_the_name_then_the_next_one() {
    let (home, env_root) = free_home();
    let h = s(home.path()).to_string();
    let script = free_folder_script(&h, "spoo-ts");
    let first = sh(&script, &no_identity(env_root.path()));
    ok(&first);
    assert_eq!(first.stdout, format!("{h}/spoo-ts\n"));
    assert!(home.path().join("spoo-ts").is_dir());
    let second = sh(&script, &no_identity(env_root.path()));
    ok(&second);
    assert_eq!(second.stdout, format!("{h}/spoo-ts-2\n"));
    assert!(home.path().join("spoo-ts-2").is_dir());
    let third = sh(&script, &no_identity(env_root.path()));
    assert_eq!(third.stdout, format!("{h}/spoo-ts-3\n"));
}

// TS: box-folder.test.ts:603
#[test]
fn free_folder_script_run_under_sh_in_a_home_where_every_name_stands_exits_one_with_the_sentence_on_stderr() {
    let (home, env_root) = free_home();
    let h = s(home.path()).to_string();
    fs::create_dir(home.path().join("spoo-ts")).unwrap();
    for n in 2..=99 {
        fs::create_dir(home.path().join(format!("spoo-ts-{n}"))).unwrap();
    }
    let ran = sh(&free_folder_script(&h, "spoo-ts"), &no_identity(env_root.path()));
    assert_eq!(exited(&ran), 1);
    assert_eq!(ran.stdout, "");
    assert_eq!(ran.stderr, format!("every name from spoo-ts to spoo-ts-99 is taken in {h}\n"));
    assert!(!home.path().join("spoo-ts-100").exists());
}
